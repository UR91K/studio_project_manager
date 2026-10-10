//! Schema recreation and plugin-scan persistence.
//!
//! These cover the three behaviours that only show up at the boundaries: discarding a
//! database written by an older schema, refusing one written by a newer one, and the
//! difference between a full scan (which may declare plugins missing) and a partial one
//! (which may not).

use std::path::PathBuf;

use rusqlite::Connection;
use seula::database::plugin_scan::PLUGIN_SCAN_COMPLETED_KEY;
use seula::database::SCHEMA_VERSION;
use seula::database::{ProjectDatabase, ProjectScope};
use seula::models::PluginKey;
use seula::scan::plugins::{PluginScanResult, ScanReport};
use tempfile::TempDir;
use vst_meta::meta::{FormatExtra, PluginFormat, PluginMeta, Vst3Class};
use vst_meta::protocol::{ErrorType, Outcome};

use crate::common::setup;

// ---------------------------------------------------------------- fixtures

/// A VST3 uid in the form the scanner reports: 32 hex digits, uppercase.
fn vst3_uid(seed: u8) -> String {
    format!("{:02X}", seed).repeat(16)
}

fn scanned_vst3(uid: &str, name: &str, classes: Vec<Vst3Class>) -> PluginMeta {
    PluginMeta {
        path: format!("C:\\Plugins\\{}.vst3", name),
        format: PluginFormat::Vst3,
        name: name.to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        uid: uid.to_string(),
        category: "Fx".to_string(),
        is_instrument: false,
        audio_in_channels: 2,
        audio_out_channels: 2,
        audio_in_buses: Some(1),
        audio_out_buses: Some(1),
        has_midi_input: false,
        has_midi_output: false,
        presets: None,
        parameters: None,
        latency_samples: None,
        has_gui: Some(true),
        vendor_url: None,
        vendor_email: None,
        is_shell: false,
        shell_parent_uid: None,
        extra: FormatExtra::Vst3 {
            factory_flags: 16,
            classes,
            buses: Vec::new(),
        },
    }
}

fn class(name: &str, class_id: &str) -> Vst3Class {
    Vst3Class {
        name: name.to_string(),
        category: "Audio Module Class".to_string(),
        class_id: class_id.to_string(),
        cardinality: 0x7FFF_FFFF,
        version: "1.0.0".to_string(),
    }
}

fn report_of(plugins: Vec<PluginMeta>) -> ScanReport {
    ScanReport {
        results: plugins
            .into_iter()
            .map(|meta| PluginScanResult {
                path: PathBuf::from(&meta.path),
                outcome: Outcome::Success {
                    plugins: vec![meta],
                },
            })
            .collect(),
        restarts: 0,
        budget_exhausted: false,
    }
}

fn temp_db() -> (TempDir, ProjectDatabase) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seula.db");
    let db = ProjectDatabase::new(path).expect("fresh database");
    (dir, db)
}

fn installed_state(db: &ProjectDatabase, uid: &str) -> Option<bool> {
    db.conn
        .query_row(
            "SELECT installed FROM plugins WHERE uid = ?",
            [uid],
            |row| row.get(0),
        )
        .unwrap()
}

// ---------------------------------------------------------------- recreate path

#[test]
fn a_fresh_database_is_stamped_with_the_current_schema() {
    setup("error");
    let (_dir, db) = temp_db();

    let version: i32 = db
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn a_database_from_before_versioning_is_discarded() {
    setup("error");
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seula.db");

    // Stand in for a pre-versioning database: tables present, user_version still 0.
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE plugins (id TEXT PRIMARY KEY, dev_identifier TEXT);
             INSERT INTO plugins (id, dev_identifier) VALUES ('old', 'device:vst3:audiofx:x');",
        )
        .unwrap();
    }

    let db = ProjectDatabase::new(path.clone()).expect("stale database should be rebuilt");

    let survivors: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(survivors, 0, "the old contents should be gone");

    // And the rebuilt file really is the current schema, not the old one.
    let version: i32 = db
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    db.conn
        .query_row("SELECT COUNT(*) FROM plugin_refs", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("plugin_refs should exist after the rebuild");
}

/// The files beside `dir/seula.db` that are backups, by name.
fn backups_in(dir: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".backup-v")
        })
        .collect()
}

#[test]
fn an_old_database_is_backed_up_before_it_is_discarded() {
    setup("error");
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seula.db");
    let old_version = SCHEMA_VERSION - 1;

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE tags (name TEXT);
             INSERT INTO tags VALUES ('my hand-made tag');",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", old_version)
            .unwrap();
    }

    let db = ProjectDatabase::new(path.clone()).expect("the stale database should be rebuilt");

    // The live database is the fresh one.
    let live: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
        .unwrap();
    assert_eq!(live, 0, "the rebuilt database starts empty");

    // And what was in it is in exactly one backup, named for the schema it came from.
    let backups = backups_in(dir.path());
    assert_eq!(backups.len(), 1, "expected one backup, found {backups:?}");
    let name = backups[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        name.starts_with(&format!("seula.backup-v{old_version}-")) && name.ends_with(".db"),
        "unexpected backup name {name}"
    );
    let backup = Connection::open(&backups[0]).unwrap();
    let kept: String = backup
        .query_row("SELECT name FROM tags", [], |row| row.get(0))
        .unwrap();
    assert_eq!(kept, "my hand-made tag");
    let kept_version: i32 = backup
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        kept_version, old_version,
        "the backup is the old database as it was"
    );
}

#[test]
fn a_new_or_current_database_is_not_backed_up() {
    setup("error");
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seula.db");

    // A brand-new file has nothing to lose, and neither does reopening a current one.
    drop(ProjectDatabase::new(path.clone()).unwrap());
    drop(ProjectDatabase::new(path).unwrap());

    assert!(backups_in(dir.path()).is_empty());
}

#[test]
fn a_database_from_a_newer_build_is_refused_not_destroyed() {
    setup("error");
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("seula.db");

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE keepsake (note TEXT); INSERT INTO keepsake VALUES ('x');")
            .unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
    }

    assert!(
        ProjectDatabase::new(path.clone()).is_err(),
        "a newer schema must not be opened"
    );

    // The point of refusing is that the data is still there afterwards.
    let conn = Connection::open(&path).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM keepsake", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);
}

// ---------------------------------------------------------------- persisting

#[test]
fn a_scan_inserts_plugins_and_is_idempotent() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let uid = vst3_uid(0x11);
    let report = report_of(vec![scanned_vst3(&uid, "Test Reverb", vec![])]);

    let first = db.persist_plugin_scan(&report, true).unwrap();
    assert_eq!(first.inserted, 1);
    assert_eq!(first.updated, 0);

    let second = db.persist_plugin_scan(&report, true).unwrap();
    assert_eq!(second.inserted, 0, "rescanning must not duplicate the row");
    assert_eq!(second.updated, 1);

    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);

    let key = PluginKey::from_uid_hex(&uid).unwrap();
    assert_eq!(installed_state(&db, &key.uid_hex()), Some(true));
}

#[test]
fn scanned_uids_are_stored_normalised() {
    setup("error");
    let (_dir, mut db) = temp_db();

    // The scanner reports uppercase; Ableton writes lowercase. If the stored form
    // followed the scanner, every reference lookup would miss.
    let uid = vst3_uid(0xAB);
    db.persist_plugin_scan(
        &report_of(vec![scanned_vst3(&uid, "Case Test", vec![])]),
        true,
    )
    .unwrap();

    let stored: String = db
        .conn
        .query_row("SELECT uid FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, uid.to_lowercase());
}

#[test]
fn a_full_scan_marks_what_it_did_not_find_as_missing() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let gone = vst3_uid(0x22);
    let kept = vst3_uid(0x33);

    db.persist_plugin_scan(
        &report_of(vec![
            scanned_vst3(&gone, "Uninstalled Later", vec![]),
            scanned_vst3(&kept, "Still Here", vec![]),
        ]),
        true,
    )
    .unwrap();

    // The second scan no longer sees the first plugin.
    let result = db
        .persist_plugin_scan(
            &report_of(vec![scanned_vst3(&kept, "Still Here", vec![])]),
            true,
        )
        .unwrap();

    assert_eq!(result.marked_missing, 1);
    assert_eq!(installed_state(&db, &gone.to_lowercase()), Some(false));
    assert_eq!(installed_state(&db, &kept.to_lowercase()), Some(true));
}

#[test]
fn a_partial_scan_leaves_plugins_it_did_not_reach_alone() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let elsewhere = vst3_uid(0x44);
    let scanned = vst3_uid(0x55);

    db.persist_plugin_scan(
        &report_of(vec![scanned_vst3(&elsewhere, "In Another Folder", vec![])]),
        true,
    )
    .unwrap();

    // A scan narrowed to one directory has no grounds to call anything missing.
    let result = db
        .persist_plugin_scan(
            &report_of(vec![scanned_vst3(&scanned, "In This Folder", vec![])]),
            false,
        )
        .unwrap();

    assert_eq!(result.marked_missing, 0);
    assert_eq!(
        installed_state(&db, &elsewhere.to_lowercase()),
        Some(true),
        "a partial scan must not contradict what a full one established"
    );
}

#[test]
fn a_failed_scan_result_does_not_become_a_plugin() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let report = ScanReport {
        results: vec![PluginScanResult {
            path: PathBuf::from("C:\\Plugins\\Broken.vst3"),
            outcome: Outcome::Error {
                error_type: ErrorType::LoadFailed,
                error: "nope".to_string(),
            },
        }],
        restarts: 0,
        budget_exhausted: false,
    };

    let result = db.persist_plugin_scan(&report, true).unwrap();

    assert_eq!(result.inserted, 0);
    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn vst3_classes_are_stored_for_the_matching_fallback() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let bundle_uid = vst3_uid(0x66);
    let other_class = vst3_uid(0x77);

    db.persist_plugin_scan(
        &report_of(vec![scanned_vst3(
            &bundle_uid,
            "Multi Class Bundle",
            vec![
                class("Processor", &bundle_uid),
                class("Controller", &other_class),
            ],
        )]),
        true,
    )
    .unwrap();

    let stored: Vec<String> = {
        let mut stmt = db
            .conn
            .prepare("SELECT class_id FROM plugin_classes ORDER BY class_id")
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows
    };

    let mut expected = vec![bundle_uid.to_lowercase(), other_class.to_lowercase()];
    expected.sort();
    assert_eq!(stored, expected, "class ids must be normalised like uids");
}

#[test]
fn a_phantom_row_is_merged_into_the_bundle_that_owns_its_class() {
    setup("error");
    let (_dir, mut db) = temp_db();

    let bundle_uid = vst3_uid(0x88);
    let class_uid = vst3_uid(0x99);

    // A project referenced the bundle's non-primary class before anything was
    // scanned, so that reference became a row of its own.
    let phantom_id = uuid::Uuid::new_v4().to_string();
    let project_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute_batch(&format!(
            "INSERT INTO plugins (id, plugin_kind, uid, name, format)
             VALUES ('{phantom}', 'VST3', '{class_uid}', 'Phantom', 'VST3 Effect');
             INSERT INTO plugin_refs (dev_identifier, plugin_id, resolved_via, first_seen_at)
             VALUES ('device:vst3:audiofx:{class_uid}', '{phantom}', 'created', 0);
             INSERT INTO projects (
                 id, path, name, hash, created_at, modified_at, last_parsed_at,
                 tempo, time_signature_numerator, time_signature_denominator,
                 daw_type, daw_version_display
             ) VALUES ('{project}', 'C:\\\\p.als', 'p', 'h', 0, 0, 0, 120.0, 4, 4, 'Ableton Live', '11.0.0');
             INSERT INTO project_plugins (project_id, plugin_id)
             VALUES ('{project}', '{phantom}');",
            phantom = phantom_id,
            class_uid = class_uid.to_lowercase(),
            project = project_id,
        ))
        .unwrap();

    // Now the bundle itself is scanned, and it exports that class.
    let result = db
        .persist_plugin_scan(
            &report_of(vec![scanned_vst3(
                &bundle_uid,
                "Real Bundle",
                vec![class("Processor", &bundle_uid), class("Extra", &class_uid)],
            )]),
            true,
        )
        .unwrap();

    assert_eq!(result.reconciled, 1);

    // The phantom is gone...
    let phantoms: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM plugins WHERE id = ?",
            [&phantom_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(phantoms, 0);

    // ...and the project still points at a plugin — the real one.
    let linked_name: String = db
        .conn
        .query_row(
            "SELECT p.name FROM plugins p
             JOIN project_plugins pp ON pp.plugin_id = p.id
             WHERE pp.project_id = ?",
            [&project_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(linked_name, "Real Bundle");

    // The reference survives too, now resolved by class.
    let via: String = db
        .conn
        .query_row(
            "SELECT resolved_via FROM plugin_refs WHERE dev_identifier = ?",
            [format!("device:vst3:audiofx:{}", class_uid.to_lowercase())],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(via, "class_id");
}

#[test]
fn scanning_fills_in_a_plugin_a_project_referenced_but_did_not_have() {
    setup("error");
    let (_dir, mut db) = temp_db();

    // The gap-filling claim, end to end: a reference exists with nothing behind it,
    // then the plugin turns up in a scan.
    let uid = vst3_uid(0xAA);
    let plugin_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO plugins (id, plugin_kind, uid, name, format)
             VALUES (?, 'VST3', ?, 'Wanted', 'VST3 Effect')",
            rusqlite::params![plugin_id, uid.to_lowercase()],
        )
        .unwrap();

    assert_eq!(
        installed_state(&db, &uid.to_lowercase()),
        None,
        "nothing has looked for it yet"
    );

    db.persist_plugin_scan(&report_of(vec![scanned_vst3(&uid, "Wanted", vec![])]), true)
        .unwrap();

    assert_eq!(installed_state(&db, &uid.to_lowercase()), Some(true));

    // Same row, not a second one.
    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);

    let id: String = db
        .conn
        .query_row("SELECT id FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(id, plugin_id, "the existing row must keep its identity");
}

// ---------------------------------------------------------------- first-run trigger

#[test]
fn a_new_database_has_never_scanned_plugins() {
    setup("error");
    let (_dir, db) = temp_db();

    assert!(!db.has_scanned_plugins().unwrap());
}

#[test]
fn a_full_scan_records_that_we_have_looked() {
    setup("error");
    let (_dir, mut db) = temp_db();

    db.persist_plugin_scan(
        &report_of(vec![scanned_vst3(&vst3_uid(0xBB), "Anything", vec![])]),
        true,
    )
    .unwrap();

    assert!(db.has_scanned_plugins().unwrap());
    assert!(db
        .get_app_state(PLUGIN_SCAN_COMPLETED_KEY)
        .unwrap()
        .is_some());
}

#[test]
fn a_scan_that_finds_nothing_still_counts_as_having_looked() {
    setup("error");
    let (_dir, mut db) = temp_db();

    // The case an "is the plugins table empty" trigger gets wrong: a machine whose
    // plugins all fail to load leaves the table empty, and would be rescanned on every
    // single project scan, forever.
    db.persist_plugin_scan(&report_of(vec![]), true).unwrap();

    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 0, "nothing was found");
    assert!(
        db.has_scanned_plugins().unwrap(),
        "but we have still looked, so the first-run scan must not repeat"
    );
}

#[test]
fn a_partial_scan_does_not_count_as_having_looked() {
    setup("error");
    let (_dir, mut db) = temp_db();

    // A scan narrowed with --paths, or cut short by the restart budget, leaves the
    // question open.
    db.persist_plugin_scan(
        &report_of(vec![scanned_vst3(&vst3_uid(0xCC), "Partial", vec![])]),
        false,
    )
    .unwrap();

    assert!(!db.has_scanned_plugins().unwrap());
}

#[test]
fn app_state_round_trips_and_overwrites() {
    setup("error");
    let (_dir, db) = temp_db();

    assert_eq!(db.get_app_state("some_key").unwrap(), None);

    db.set_app_state("some_key", "first").unwrap();
    assert_eq!(
        db.get_app_state("some_key").unwrap().as_deref(),
        Some("first")
    );

    db.set_app_state("some_key", "second").unwrap();
    assert_eq!(
        db.get_app_state("some_key").unwrap().as_deref(),
        Some("second")
    );
}

// ---------------------------------------------------------------- aggregates

/// Insert a plugin row directly, so the test can set all three install states —
/// including `NULL`, which no write path produces on demand. Returns the generated id
/// so callers can link it into `project_plugins`.
fn insert_plugin(
    db: &ProjectDatabase,
    name: &str,
    vendor: &str,
    format: &str,
    installed: Option<bool>,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO plugins (id, plugin_kind, uid, name, format, vendor, installed)
             VALUES (?, 'VST3', ?, ?, ?, ?, ?)",
            rusqlite::params![
                id,
                format!(
                    "{:032x}",
                    name.len() as u128 + vendor.len() as u128 * 997 + format.len() as u128 * 31
                ),
                name,
                format,
                vendor,
                installed,
            ],
        )
        .unwrap();
    id
}

/// Insert a bare project row, so the test can link it to plugins via `project_plugins`.
fn insert_project(db: &ProjectDatabase, path: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO projects (
                 id, path, name, hash, created_at, modified_at, last_parsed_at,
                 tempo, time_signature_numerator, time_signature_denominator,
                 daw_type, daw_version_display
             ) VALUES (?, ?, 'p', 'h', 0, 0, 0, 120.0, 4, 4, 'Ableton Live', '11.0.0')",
            rusqlite::params![id, path],
        )
        .unwrap();
    id
}

/// Link a plugin into a project's `project_plugins`.
fn link_plugin_to_project(db: &ProjectDatabase, project_id: &str, plugin_id: &str) {
    db.conn
        .execute(
            "INSERT INTO project_plugins (project_id, plugin_id) VALUES (?, ?)",
            rusqlite::params![project_id, plugin_id],
        )
        .unwrap();
}

/// A plugin used in several projects must still count once toward `plugin_count`. The
/// usage subquery groups by `(plugin_id, project_id)` so it has one row per project;
/// joining that straight onto `plugins` fans a single plugin row out into one per
/// project, and the outer `COUNT(*)` counts the fanned-out rows instead of plugins.
/// Found 2026-09-16 (docs/status.md), alongside the unscanned-count aggregates.
#[test]
fn vendor_and_format_plugin_count_does_not_inflate_with_project_usage() {
    setup("error");
    let (_dir, db) = temp_db();

    let widely_used = insert_plugin(&db, "Everywhere", "Acme", "VST3 AudioFx", Some(true));
    let _rarely_used = insert_plugin(&db, "OnceOnly", "Acme", "VST3 AudioFx", Some(true));

    let project_a = insert_project(&db, "C:\\a.als");
    let project_b = insert_project(&db, "C:\\b.als");
    let project_c = insert_project(&db, "C:\\c.als");

    // The same plugin used in three separate projects...
    link_plugin_to_project(&db, &project_a, &widely_used);
    link_plugin_to_project(&db, &project_b, &widely_used);
    link_plugin_to_project(&db, &project_c, &widely_used);
    // ...plus a second, unrelated plugin, used once, from the same vendor and format.
    link_plugin_to_project(&db, &project_a, &_rarely_used);

    let (vendors, _) = db
        .get_plugin_vendors(None, None, None, None, ProjectScope::All)
        .unwrap();
    let acme = vendors.iter().find(|v| v.vendor == "Acme").unwrap();
    // Two distinct plugins exist for this vendor, regardless of how many projects use
    // either of them.
    assert_eq!(acme.plugin_count, 2);
    assert_eq!(acme.unique_projects_using, 3);
    assert_eq!(acme.total_usage_count, 4);

    let (formats, _) = db
        .get_plugin_formats(None, None, None, None, ProjectScope::All)
        .unwrap();
    let audiofx = formats.iter().find(|f| f.format == "VST3 AudioFx").unwrap();
    assert_eq!(audiofx.plugin_count, 2);
    assert_eq!(audiofx.unique_projects_using, 3);
    assert_eq!(audiofx.total_usage_count, 4);
}

/// The vendor and format aggregates must account for every row, not just the ones a
/// scan has ruled on. Reporting only installed and missing leaves the never-scanned
/// rows (ADR-0012) out of a total they are counted in, so the columns do not sum.
#[test]
fn vendor_and_format_aggregates_account_for_unscanned_plugins() {
    setup("error");
    let (_dir, db) = temp_db();

    // One vendor and one format carrying all three states at once.
    insert_plugin(&db, "Found", "Acme", "VST3 AudioFx", Some(true));
    insert_plugin(&db, "Absent", "Acme", "VST3 AudioFx", Some(false));
    insert_plugin(&db, "NeverLookedFor", "Acme", "VST3 AudioFx", None);
    insert_plugin(&db, "OtherVendorUnscanned", "Bolt", "VST2 Instrument", None);

    let (vendors, _) = db
        .get_plugin_vendors(None, None, None, None, ProjectScope::All)
        .unwrap();
    assert_eq!(vendors.len(), 2);

    for vendor in &vendors {
        assert_eq!(
            vendor.installed_plugins + vendor.missing_plugins + vendor.unknown_plugins,
            vendor.plugin_count,
            "vendor {} does not reconcile",
            vendor.vendor
        );
    }

    let acme = vendors.iter().find(|v| v.vendor == "Acme").unwrap();
    assert_eq!(acme.plugin_count, 3);
    assert_eq!(acme.installed_plugins, 1);
    assert_eq!(acme.missing_plugins, 1);
    assert_eq!(acme.unknown_plugins, 1);

    let bolt = vendors.iter().find(|v| v.vendor == "Bolt").unwrap();
    assert_eq!(bolt.unknown_plugins, 1);

    let (formats, _) = db
        .get_plugin_formats(None, None, None, None, ProjectScope::All)
        .unwrap();
    assert_eq!(formats.len(), 2);

    for format in &formats {
        assert_eq!(
            format.installed_plugins + format.missing_plugins + format.unknown_plugins,
            format.plugin_count,
            "format {} does not reconcile",
            format.format
        );
    }

    let audiofx = formats.iter().find(|f| f.format == "VST3 AudioFx").unwrap();
    assert_eq!(audiofx.plugin_count, 3);
    assert_eq!(audiofx.installed_plugins, 1);
    assert_eq!(audiofx.missing_plugins, 1);
    assert_eq!(audiofx.unknown_plugins, 1);

    // And the aggregates agree with the top-level stats, which was the point: a user
    // who sees a nonzero unknown total can now find where those plugins are.
    let stats = db.get_plugin_stats().unwrap();
    assert_eq!(stats.unknown_plugins, 2);
    assert_eq!(
        vendors.iter().map(|v| v.unknown_plugins).sum::<i32>(),
        stats.unknown_plugins
    );
    assert_eq!(
        formats.iter().map(|f| f.unknown_plugins).sum::<i32>(),
        stats.unknown_plugins
    );
}

// ---------------------------------------------------------------- per-file scans (ADR-0067)

use seula::scan::plugins::PluginFile;

/// A file as discovery would report it; `stamp` stands for its size and mtime.
fn file(name: &str, stamp: i64) -> PluginFile {
    PluginFile {
        path: PathBuf::from(format!(r"C:\Plugins\{}.vst3", name)),
        size: stamp as u64,
        modified: Some(stamp),
    }
}

fn failed(name: &str, error_type: ErrorType) -> PluginScanResult {
    PluginScanResult {
        path: PathBuf::from(format!(r"C:\Plugins\{}.vst3", name)),
        outcome: Outcome::Error {
            error_type,
            error: format!("{} went wrong", name),
        },
    }
}

fn report_with(results: Vec<PluginScanResult>) -> ScanReport {
    ScanReport {
        results,
        restarts: 0,
        budget_exhausted: false,
    }
}

#[test]
fn only_new_and_changed_files_need_loading() {
    setup("error");
    let (_dir, mut db) = temp_db();
    let a = vst3_uid(0x41);
    let files = vec![file("A", 100)];
    db.persist_plugin_scan_of(
        &report_of(vec![scanned_vst3(&a, "A", vec![])]),
        &files,
        true,
    )
    .unwrap();

    let unchanged =
        |db: &ProjectDatabase, f: Vec<PluginFile>| db.unchanged_plugin_files(&f).unwrap();
    assert!(unchanged(&db, vec![file("A", 100)]).contains(&file("A", 100).path));
    assert!(
        unchanged(&db, vec![file("A", 101)]).is_empty(),
        "a changed file is loaded again"
    );
    assert!(
        unchanged(&db, vec![file("New", 100)]).is_empty(),
        "a new file is loaded"
    );
    let no_mtime = PluginFile {
        modified: None,
        ..file("A", 100)
    };
    assert!(
        unchanged(&db, vec![no_mtime]).is_empty(),
        "no mtime never counts as unchanged"
    );
}

#[test]
fn a_scan_that_loads_nothing_keeps_unchanged_plugins_installed() {
    setup("error");
    let (_dir, mut db) = temp_db();
    let a = vst3_uid(0x42);
    let b = vst3_uid(0x43);
    db.persist_plugin_scan_of(
        &report_of(vec![
            scanned_vst3(&a, "A", vec![]),
            scanned_vst3(&b, "B", vec![]),
        ]),
        &[file("A", 1), file("B", 1)],
        true,
    )
    .unwrap();

    // Both unchanged, so nothing was loaded; B's file has gone since.
    let result = db
        .persist_plugin_scan_of(&report_with(vec![]), &[file("A", 1)], true)
        .unwrap();

    assert_eq!(installed_state(&db, &a.to_lowercase()), Some(true));
    assert_eq!(installed_state(&db, &b.to_lowercase()), Some(false));
    assert_eq!(result.marked_missing, 1);
    let rows: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM plugin_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "the vanished file is forgotten");
}

#[test]
fn a_plugin_whose_file_starts_failing_is_failed_not_missing() {
    setup("error");
    let (_dir, mut db) = temp_db();
    let a = vst3_uid(0x44);
    db.persist_plugin_scan_of(
        &report_of(vec![scanned_vst3(&a, "A", vec![])]),
        &[file("A", 1)],
        true,
    )
    .unwrap();

    // An update, and now it crashes the worker.
    db.persist_plugin_scan_of(
        &report_with(vec![failed("A", ErrorType::Crashed)]),
        &[file("A", 2)],
        true,
    )
    .unwrap();

    assert_eq!(installed_state(&db, &a.to_lowercase()), Some(true));
    let errors = db.plugin_scan_errors().unwrap();
    assert_eq!(errors.values().collect::<Vec<_>>(), vec!["crashed"]);
    assert!(
        db.failed_plugin_files().unwrap().is_empty(),
        "it is known, so it is no file row"
    );

    // Fixed by the next update: it loads again.
    db.persist_plugin_scan_of(
        &report_of(vec![scanned_vst3(&a, "A", vec![])]),
        &[file("A", 3)],
        true,
    )
    .unwrap();
    assert!(db.plugin_scan_errors().unwrap().is_empty());
}

#[test]
fn files_that_never_loaded_are_listed_unless_they_are_not_plugins() {
    setup("error");
    let (_dir, mut db) = temp_db();
    db.persist_plugin_scan_of(
        &report_with(vec![
            failed("Hangs", ErrorType::Timeout),
            failed("Helper", ErrorType::InvalidFormat),
            failed("Refuses", ErrorType::LoadFailed),
        ]),
        &[file("Hangs", 1), file("Helper", 1), file("Refuses", 1)],
        true,
    )
    .unwrap();

    let listed: Vec<(String, String)> = db
        .failed_plugin_files()
        .unwrap()
        .into_iter()
        .map(|f| (f.path, f.error_type))
        .collect();
    assert_eq!(
        listed,
        vec![
            (r"C:\Plugins\Hangs.vst3".to_string(), "timeout".to_string()),
            (
                r"C:\Plugins\Refuses.vst3".to_string(),
                "load_failed".to_string()
            ),
        ]
    );
    // All three are remembered, so a scan for changes skips them.
    assert_eq!(
        db.unchanged_plugin_files(&[file("Hangs", 1), file("Helper", 1), file("Refuses", 1)])
            .unwrap()
            .len(),
        3
    );
}
