//! Persisting the results of a system plugin scan.
//!
//! Mirrors [`batch`](super::batch)'s role: one module owning one write path, in one
//! transaction. The scan is the only thing that writes `plugins.installed` — parsing a
//! project never does, because a project file cannot know what is installed.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::{params, OptionalExtension, Transaction};
use tracing::{debug, info, warn};
use uuid::Uuid;

use vst_meta::meta::{FormatExtra, PluginMeta};
use vst_meta::protocol::{ErrorType, Outcome};

use super::models::SqlDateTime;
use crate::database::ProjectDatabase;
use crate::error::DatabaseError;
use crate::models::PluginKey;
use crate::scan::plugins::{PluginFile, ScanReport};

/// Records when a full plugin scan last completed.
///
/// Deliberately not "is the plugins table empty". A machine whose plugins all fail to
/// load would leave that table empty forever, so an empty-table trigger would rescan on
/// every project scan and never stop. What matters is whether we have *looked*.
pub const PLUGIN_SCAN_COMPLETED_KEY: &str = "plugins_last_scanned_at";

/// What a persist run changed.
#[derive(Debug, Default, Clone)]
pub struct PluginScanPersist {
    pub inserted: usize,
    pub updated: usize,
    /// Rows a full scan looked for and did not find.
    pub marked_missing: usize,
    /// Phantom rows merged into the bundle that actually owns them.
    pub reconciled: usize,
    /// Scan results whose uid could not be parsed into an identity.
    pub skipped: usize,
}

impl ProjectDatabase {
    /// Write a scan report into the database.
    ///
    /// `full_scan` must be false when the scan was narrowed (`--paths`) or cut short
    /// (`budget_exhausted`). A partial scan may only report what it saw; it must not
    /// declare everything it did not reach to be missing.
    pub fn persist_plugin_scan(
        &mut self,
        report: &ScanReport,
        full_scan: bool,
    ) -> Result<PluginScanPersist, DatabaseError> {
        // Every file the report covers, and nothing else: a scan of exactly these.
        // Unknown size and mtime never match, so a later scan loads them again.
        let files: Vec<PluginFile> = report
            .results
            .iter()
            .map(|r| PluginFile {
                path: r.path.clone(),
                size: 0,
                modified: None,
            })
            .collect();
        self.persist_plugin_scan_of(report, &files, full_scan)
    }

    /// Write a scan of `files`, every plugin file discovery found, of which `report`
    /// covers the ones the scan loaded (ADR-0067). The files it did not load were
    /// unchanged, and keep what they gave last time.
    ///
    /// A full scan then marks a plugin not installed only when no file found on disk
    /// yields it. A file that fails keeps the plugins it yielded when it last loaded, so
    /// a plugin that has started to fail stays installed and reads as failed rather
    /// than missing.
    pub fn persist_plugin_scan_of(
        &mut self,
        report: &ScanReport,
        files: &[PluginFile],
        full_scan: bool,
    ) -> Result<PluginScanPersist, DatabaseError> {
        let tx = self.conn.transaction()?;
        let mut result = PluginScanPersist::default();
        let now = SqlDateTime::from(Local::now());
        let stats: HashMap<&Path, &PluginFile> =
            files.iter().map(|f| (f.path.as_path(), f)).collect();

        for scanned in report.results.iter() {
            let path = path_text(&scanned.path);
            let (size, modified) = stats
                .get(scanned.path.as_path())
                .map(|f| (f.size as i64, f.modified))
                .unwrap_or((0, None));

            match &scanned.outcome {
                Outcome::Success { plugins } => {
                    let mut ids = Vec::new();
                    for meta in plugins {
                        let Some(key) = PluginKey::from_uid_hex(&meta.uid) else {
                            warn!(
                                "Skipping scanned plugin with unparseable uid {:?} at {}",
                                meta.uid, meta.path
                            );
                            result.skipped += 1;
                            continue;
                        };

                        let plugin_id = upsert_scanned_plugin(&tx, &key, meta, &now, &mut result)?;
                        replace_child_rows(&tx, &plugin_id, meta)?;
                        ids.push(plugin_id);
                    }
                    record_file(&tx, &path, size, modified, &now, None)?;
                    tx.execute("DELETE FROM plugin_file_plugins WHERE path = ?", [&path])?;
                    for id in &ids {
                        tx.execute(
                            "INSERT OR IGNORE INTO plugin_file_plugins (path, plugin_id) VALUES (?, ?)",
                            params![path, id],
                        )?;
                    }
                }
                // Gone between discovery and loading: nothing to remember.
                Outcome::Error {
                    error_type: ErrorType::FileNotFound,
                    ..
                } => {
                    tx.execute("DELETE FROM plugin_files WHERE path = ?", [&path])?;
                }
                // Its links, if any, stay: they say which plugin this file was.
                Outcome::Error { error_type, error } => {
                    record_file(
                        &tx,
                        &path,
                        size,
                        modified,
                        &now,
                        Some((error_code(*error_type), error.as_str())),
                    )?;
                }
            }
        }

        if full_scan {
            forget_vanished_files(&tx, files)?;
            result.marked_missing = sweep_unseen(&tx, &now)?;
        } else {
            debug!("Partial scan: leaving plugins it did not reach untouched");
        }

        result.reconciled = reconcile_phantoms(&tx)?;

        tx.commit()?;

        // Only a full scan counts as having looked. A narrowed one leaves the question
        // open, so it must not suppress the first-run scan.
        if full_scan {
            self.set_app_state(PLUGIN_SCAN_COMPLETED_KEY, &Local::now().to_rfc3339())?;
        }

        info!(
            "Persisted plugin scan: {} inserted, {} updated, {} marked missing, {} reconciled, {} skipped",
            result.inserted, result.updated, result.marked_missing, result.reconciled, result.skipped
        );

        Ok(result)
    }

    /// Whether a full plugin scan has ever completed against this database.
    ///
    /// Drives the first-run scan. False means the `installed` column is entirely
    /// unknown rather than merely negative.
    pub fn has_scanned_plugins(&self) -> Result<bool, DatabaseError> {
        Ok(self.get_app_state(PLUGIN_SCAN_COMPLETED_KEY)?.is_some())
    }
}

/// Insert or update the row for one scanned plugin, and return its id.
///
/// Scanner data wins over anything a project reference wrote: it came from the binary.
fn upsert_scanned_plugin(
    tx: &Transaction,
    key: &PluginKey,
    meta: &PluginMeta,
    now: &SqlDateTime,
    result: &mut PluginScanPersist,
) -> Result<String, DatabaseError> {
    // `.optional()?` rather than `.ok()`: the latter would fold a real database error
    // into "no such row", which is exactly the confusion that made
    // `refresh_plugin_installation_status` mark every plugin installed (ADR-0006).
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM plugins WHERE plugin_kind = ? AND uid = ?",
            params![key.kind(), key.uid_hex()],
            |row| row.get(0),
        )
        .optional()?;

    let plugin_id = existing
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if existing.is_some() {
        result.updated += 1;
    } else {
        result.inserted += 1;
    }

    // The four-variant format is display information. Derived from the plugin's own
    // category rather than Ableton's opinion of it -- ADR-0005 keeps that opinion out
    // of identity, and here we simply have a better source.
    let format = match (key, meta.is_instrument) {
        (PluginKey::Vst2(_), true) => "VST2 Instrument",
        (PluginKey::Vst2(_), false) => "VST2 Effect",
        (PluginKey::Vst3(_), true) => "VST3 Instrument",
        (PluginKey::Vst3(_), false) => "VST3 Effect",
    };

    let (
        fourcc,
        preset_chunks,
        f64_precision,
        silent_when_stopped,
        midi_in,
        midi_out,
        factory_flags,
    ) = match &meta.extra {
        FormatExtra::Vst2 {
            fourcc,
            preset_chunks,
            f64_precision,
            silent_when_stopped,
            midi_in_channels,
            midi_out_channels,
        } => (
            Some(fourcc.clone()),
            Some(*preset_chunks),
            Some(*f64_precision),
            Some(*silent_when_stopped),
            Some(*midi_in_channels),
            Some(*midi_out_channels),
            None,
        ),
        FormatExtra::Vst3 { factory_flags, .. } => {
            (None, None, None, None, None, None, Some(*factory_flags))
        }
    };

    tx.execute(
        "INSERT INTO plugins (
            id, plugin_kind, uid, name, format, vendor, version,
            installed, last_scanned_at,
            path, category, is_instrument,
            audio_in_channels, audio_out_channels, audio_in_buses, audio_out_buses,
            has_midi_input, has_midi_output, presets, parameters, latency_samples,
            has_gui, vendor_url, vendor_email, is_shell, shell_parent_uid,
            fourcc, preset_chunks, f64_precision, silent_when_stopped,
            midi_in_channels, midi_out_channels, factory_flags
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?,
            1, ?,
            ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?
        )
        ON CONFLICT(plugin_kind, uid) DO UPDATE SET
            name = EXCLUDED.name,
            format = EXCLUDED.format,
            vendor = EXCLUDED.vendor,
            version = EXCLUDED.version,
            installed = 1,
            last_scanned_at = EXCLUDED.last_scanned_at,
            path = EXCLUDED.path,
            category = EXCLUDED.category,
            is_instrument = EXCLUDED.is_instrument,
            audio_in_channels = EXCLUDED.audio_in_channels,
            audio_out_channels = EXCLUDED.audio_out_channels,
            audio_in_buses = EXCLUDED.audio_in_buses,
            audio_out_buses = EXCLUDED.audio_out_buses,
            has_midi_input = EXCLUDED.has_midi_input,
            has_midi_output = EXCLUDED.has_midi_output,
            presets = EXCLUDED.presets,
            parameters = EXCLUDED.parameters,
            latency_samples = EXCLUDED.latency_samples,
            has_gui = EXCLUDED.has_gui,
            vendor_url = EXCLUDED.vendor_url,
            vendor_email = EXCLUDED.vendor_email,
            is_shell = EXCLUDED.is_shell,
            shell_parent_uid = EXCLUDED.shell_parent_uid,
            fourcc = EXCLUDED.fourcc,
            preset_chunks = EXCLUDED.preset_chunks,
            f64_precision = EXCLUDED.f64_precision,
            silent_when_stopped = EXCLUDED.silent_when_stopped,
            midi_in_channels = EXCLUDED.midi_in_channels,
            midi_out_channels = EXCLUDED.midi_out_channels,
            factory_flags = EXCLUDED.factory_flags
        ",
        params![
            plugin_id,
            key.kind(),
            key.uid_hex(),
            meta.name,
            format,
            non_empty(&meta.vendor),
            non_empty(&meta.version),
            now,
            meta.path,
            non_empty(&meta.category),
            meta.is_instrument,
            meta.audio_in_channels,
            meta.audio_out_channels,
            meta.audio_in_buses,
            meta.audio_out_buses,
            meta.has_midi_input,
            meta.has_midi_output,
            meta.presets,
            meta.parameters,
            meta.latency_samples,
            meta.has_gui,
            meta.vendor_url,
            meta.vendor_email,
            meta.is_shell,
            // Normalise so it joins against uid the same way everything else does.
            meta.shell_parent_uid
                .as_deref()
                .and_then(PluginKey::from_uid_hex)
                .map(|k| k.uid_hex()),
            fourcc,
            preset_chunks,
            f64_precision,
            silent_when_stopped,
            midi_in,
            midi_out,
            factory_flags,
        ],
    )?;

    Ok(plugin_id)
}

/// Replace a plugin's class and bus rows wholesale.
///
/// Diffing them would buy nothing: they are small, and a rescan is the only thing that
/// writes them.
fn replace_child_rows(
    tx: &Transaction,
    plugin_id: &str,
    meta: &PluginMeta,
) -> Result<(), DatabaseError> {
    tx.execute(
        "DELETE FROM plugin_classes WHERE plugin_id = ?",
        params![plugin_id],
    )?;
    tx.execute(
        "DELETE FROM plugin_buses WHERE plugin_id = ?",
        params![plugin_id],
    )?;

    let FormatExtra::Vst3 { classes, buses, .. } = &meta.extra else {
        return Ok(());
    };

    for class in classes {
        // Store the class ID in the same normalised form as plugins.uid, or the
        // matching fallback silently fails to join.
        let Some(class_key) = PluginKey::from_uid_hex(&class.class_id) else {
            warn!(
                "Skipping VST3 class with unparseable class_id {:?} on {}",
                class.class_id, meta.name
            );
            continue;
        };

        tx.execute(
            "INSERT INTO plugin_classes (
                plugin_id, name, category, class_id, cardinality, version
            ) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                plugin_id,
                class.name,
                class.category,
                class_key.uid_hex(),
                class.cardinality,
                class.version,
            ],
        )?;
    }

    for bus in buses {
        tx.execute(
            "INSERT INTO plugin_buses (
                plugin_id, direction, media, name, channel_count, bus_type, flags
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                plugin_id,
                bus.direction,
                bus.media,
                bus.name,
                bus.channel_count,
                bus.bus_type,
                bus.flags,
            ],
        )?;
    }

    Ok(())
}

/// Mark every plugin no file on disk yields as not installed.
///
/// Only valid after a scan that covered every configured search path, once the files it
/// did not find are forgotten: a narrowed or truncated scan has no grounds to call
/// anything missing. A failing file's plugins count as yielded (ADR-0067).
fn sweep_unseen(tx: &Transaction, now: &SqlDateTime) -> Result<usize, DatabaseError> {
    Ok(tx.execute(
        "UPDATE plugins
         SET installed = 0, last_scanned_at = ?
         WHERE id NOT IN (SELECT plugin_id FROM plugin_file_plugins)
           AND (installed IS NULL OR installed = 1)",
        [now],
    )?)
}

/// Record what loading one file gave: `error` is the failure's code and message.
fn record_file(
    tx: &Transaction,
    path: &str,
    size: i64,
    modified: Option<i64>,
    now: &SqlDateTime,
    error: Option<(&str, &str)>,
) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT INTO plugin_files (path, size_bytes, modified_at, scanned_at, error_type, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(path) DO UPDATE SET
             size_bytes = ?2, modified_at = ?3, scanned_at = ?4, error_type = ?5, error_message = ?6",
        params![path, size, modified, now, error.map(|e| e.0), error.map(|e| e.1)],
    )?;
    Ok(())
}

/// Forget the files a full scan no longer found, and with them the plugins they yielded.
fn forget_vanished_files(tx: &Transaction, files: &[PluginFile]) -> Result<(), DatabaseError> {
    let present: HashSet<String> = files.iter().map(|f| path_text(&f.path)).collect();
    let known: Vec<String> = tx
        .prepare("SELECT path FROM plugin_files")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for path in known.iter().filter(|p| !present.contains(*p)) {
        tx.execute("DELETE FROM plugin_files WHERE path = ?", [path])?;
    }
    Ok(())
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// How a failure is stored and sent: the protocol's own serde names.
fn error_code(error_type: ErrorType) -> &'static str {
    match error_type {
        ErrorType::FileNotFound => "file_not_found",
        ErrorType::InvalidFormat => "invalid_format",
        ErrorType::LoadFailed => "load_failed",
        ErrorType::Timeout => "timeout",
        ErrorType::Crashed => "crashed",
        ErrorType::ResultLost => "result_lost",
    }
}

/// A plugin file that has never loaded (ADR-0067), so no plugin row stands for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedPluginFile {
    pub path: String,
    /// `crashed`, `timeout`, `load_failed` or `result_lost`. Files that are not plugins at all
    /// (`invalid_format`) are not listed.
    pub error_type: String,
    pub error_message: Option<String>,
    /// Epoch seconds.
    pub scanned_at: i64,
}

impl ProjectDatabase {
    /// Of `files`, the ones whose size and mtime match their last scan (ADR-0067). A
    /// file with no mtime never matches, and neither does one whose result was lost
    /// (ADR-0083): it may have loaded fine, so it is worth asking again.
    pub fn unchanged_plugin_files(
        &self,
        files: &[PluginFile],
    ) -> Result<HashSet<PathBuf>, DatabaseError> {
        let recorded: HashMap<String, (i64, Option<i64>)> = self
            .conn
            .prepare(
                "SELECT path, size_bytes, modified_at FROM plugin_files
                 WHERE error_type IS NULL OR error_type != 'result_lost'",
            )?
            .query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
            .collect::<Result<_, _>>()?;
        Ok(files
            .iter()
            .filter(|f| {
                f.modified.is_some()
                    && recorded.get(&path_text(&f.path)) == Some(&(f.size as i64, f.modified))
            })
            .map(|f| f.path.clone())
            .collect())
    }

    /// Plugins whose every file failed its last scan, by id, with how the first one
    /// failed. They loaded once, so they are known, but no file of theirs loads now.
    pub fn plugin_scan_errors(&self) -> Result<HashMap<String, String>, DatabaseError> {
        Ok(self
            .conn
            .prepare(
                "SELECT pfp.plugin_id, MIN(pf.error_type)
                 FROM plugin_file_plugins pfp JOIN plugin_files pf ON pf.path = pfp.path
                 GROUP BY pfp.plugin_id
                 HAVING SUM(pf.error_type IS NULL) = 0",
            )?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }

    /// Plugin files that failed and have never loaded, by path.
    pub fn failed_plugin_files(&self) -> Result<Vec<FailedPluginFile>, DatabaseError> {
        Ok(self
            .conn
            .prepare(
                "SELECT path, error_type, error_message, scanned_at FROM plugin_files pf
                 WHERE error_type IS NOT NULL AND error_type != 'invalid_format'
                   AND NOT EXISTS (SELECT 1 FROM plugin_file_plugins pfp WHERE pfp.path = pf.path)
                 ORDER BY path",
            )?
            .query_map([], |row| {
                Ok(FailedPluginFile {
                    path: row.get(0)?,
                    error_type: row.get(1)?,
                    error_message: row.get(2)?,
                    scanned_at: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }
}

/// Merge rows that a project reference created from a class ID into the bundle that
/// actually exports that class.
///
/// A project can reference a bundle's non-primary processor class before that bundle
/// has ever been scanned. The reference is a real identity, so it gets its own row —
/// but once the bundle is scanned, that row is a duplicate of a plugin we own. This is
/// what makes "install the missing plugin, run refresh" work without reparsing
/// anything.
fn reconcile_phantoms(tx: &Transaction) -> Result<usize, DatabaseError> {
    let pairs: Vec<(String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT phantom.id, cls.plugin_id
             FROM plugins phantom
             JOIN plugin_classes cls ON cls.class_id = phantom.uid
             WHERE phantom.id != cls.plugin_id
               AND phantom.installed IS NOT 1",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        rows.collect::<Result<Vec<_>, _>>()?
    };

    for (phantom_id, real_id) in &pairs {
        debug!("Reconciling phantom plugin {} into {}", phantom_id, real_id);

        tx.execute(
            "UPDATE plugin_refs SET plugin_id = ?, resolved_via = 'class_id' WHERE plugin_id = ?",
            params![real_id, phantom_id],
        )?;

        // The junction is a primary key pair, so a project already linked to the real
        // plugin must not gain a duplicate row.
        tx.execute(
            "INSERT OR IGNORE INTO project_plugins (project_id, plugin_id)
             SELECT project_id, ? FROM project_plugins WHERE plugin_id = ?",
            params![real_id, phantom_id],
        )?;
        tx.execute(
            "DELETE FROM project_plugins WHERE plugin_id = ?",
            params![phantom_id],
        )?;

        tx.execute("DELETE FROM plugins WHERE id = ?", params![phantom_id])?;
    }

    Ok(pairs.len())
}

/// The scanner reports absent strings as empty; the database prefers NULL.
fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
