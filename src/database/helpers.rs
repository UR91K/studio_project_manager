use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use chrono::{Local, TimeZone};
use rusqlite::{params, Row, Transaction};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

/// Insert a plugin into the database
pub fn insert_plugin(tx: &Transaction, plugin: &Plugin) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT OR REPLACE INTO plugins (
            id, ableton_plugin_id, ableton_module_id, dev_identifier, name, format,
            installed, vendor, version, sdk_version, flags, scanstate, enabled
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            plugin.id.to_string(),
            plugin.plugin_id,
            plugin.module_id,
            plugin.dev_identifier,
            plugin.name,
            plugin.plugin_format.to_string(),
            plugin.installed,
            plugin.vendor,
            plugin.version,
            plugin.sdk_version,
            plugin.flags,
            plugin.scanstate,
            plugin.enabled,
        ],
    )?;
    Ok(())
}

/// Insert a sample into the database
pub fn insert_sample(tx: &Transaction, sample: &Sample) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT OR REPLACE INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
        params![
            sample.id.to_string(),
            sample.name,
            sample.path.to_string_lossy().to_string(),
            sample.is_present,
        ],
    )?;
    Ok(())
}

/// Link a project to a plugin
pub fn link_project_plugin(
    tx: &Transaction,
    project_id: &str,
    plugin_id: &str,
) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT OR REPLACE INTO project_plugins (project_id, plugin_id) VALUES (?, ?)",
        params![project_id, plugin_id],
    )?;
    Ok(())
}

/// Link a project to a sample
pub fn link_project_sample(
    tx: &Transaction,
    project_id: &str,
    sample_id: &str,
) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT OR REPLACE INTO project_samples (project_id, sample_id) VALUES (?, ?)",
        params![project_id, sample_id],
    )?;
    Ok(())
}

/// Convert a database row to a LiveSet object
pub fn row_to_live_set(row: &Row) -> rusqlite::Result<LiveSet> {
    let id: String = row.get("id")?;
    let created_timestamp: i64 = row.get("created_at")?;
    let modified_timestamp: i64 = row.get("modified_at")?;
    let parsed_timestamp: i64 = row.get("last_parsed_at")?;
    let duration_secs: Option<i64> = row.get("duration_seconds")?;

    Ok(LiveSet {
        is_active: row.get("is_active")?,
        id: Uuid::parse_str(&id).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        file_path: PathBuf::from(row.get::<_, String>("path")?),
        name: row.get("name")?,
        file_hash: row.get("hash")?,
        created_time: Local
            .timestamp_opt(created_timestamp, 0)
            .single()
            .ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp",
                    )),
                )
            })?,
        modified_time: Local
            .timestamp_opt(modified_timestamp, 0)
            .single()
            .ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp",
                    )),
                )
            })?,
        last_parsed_timestamp: Local
            .timestamp_opt(parsed_timestamp, 0)
            .single()
            .ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Integer,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp",
                    )),
                )
            })?,

        tempo: row.get("tempo")?,
        time_signature: TimeSignature {
            numerator: row.get("time_signature_numerator")?,
            denominator: row.get("time_signature_denominator")?,
        },
        key_signature: match (
            row.get::<_, Option<String>>("key_signature_tonic")?,
            row.get::<_, Option<String>>("key_signature_scale")?,
        ) {
            (Some(tonic), Some(scale)) => Some(KeySignature {
                tonic: tonic.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?,
                scale: scale.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?,
            }),
            _ => None,
        },
        furthest_bar: row.get("furthest_bar")?,

        ableton_version: AbletonVersion {
            major: row.get("ableton_version_major")?,
            minor: row.get("ableton_version_minor")?,
            patch: row.get("ableton_version_patch")?,
            beta: row.get("ableton_version_beta")?,
        },

        estimated_duration: duration_secs.map(chrono::Duration::seconds),
        plugins: HashSet::new(), // These will be loaded separately when needed
        samples: HashSet::new(), // These will be loaded separately when needed
        tags: HashSet::new(),    // These will be loaded separately when needed
    })
}
