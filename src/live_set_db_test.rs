#![allow(dead_code, unused_imports)]
use crate::live_set_db::LiveSetDatabase;
use crate::scan::ScanResult;
use crate::test_utils::LiveSetBuilder;
use chrono::Local;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[allow(unused_imports)]
use log::{debug, info, warn};

use crate::live_set::LiveSet;
use crate::models::{
    AbletonVersion, KeySignature, Plugin, PluginFormat, Sample, Scale, TimeSignature, Tonic,
};
use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    let _ = INIT.call_once(|| {
        let _ = std::env::set_var("RUST_LOG", "debug");
        if let Err(_) = env_logger::try_init() {
            // Logger already initialized, that's fine
        }
    });
}

fn create_test_live_set() -> LiveSet {
    let now = Local::now();
    let mut plugins = HashSet::new();
    let mut samples = HashSet::new();

    // Add a test plugin
    plugins.insert(Plugin {
        id: Uuid::new_v4(),
        plugin_id: Some(1),
        module_id: Some(2),
        dev_identifier: "device:vst3:audiofx:test-plugin".to_string(),
        name: "Test Plugin".to_string(),
        vendor: Some("Test Vendor".to_string()),
        version: Some("1.0.0".to_string()),
        sdk_version: Some("1.0".to_string()),
        flags: Some(0),
        scanstate: Some(1),
        enabled: Some(1),
        plugin_format: PluginFormat::VST3AudioFx,
        installed: true,
    });

    // Add a test sample
    samples.insert(Sample {
        id: Uuid::new_v4(),
        name: "test_sample.wav".to_string(),
        path: PathBuf::from("C:/test/test_sample.wav"),
        is_present: true,
    });

    LiveSet {
        id: Uuid::new_v4(),
        file_path: PathBuf::from("C:/test/test_project.als"),
        file_name: "test_project.als".to_string(),
        file_hash: "test_hash".to_string(),
        created_time: now,
        modified_time: now,
        last_scan_timestamp: now,
        xml_data: Vec::new(),

        ableton_version: AbletonVersion {
            major: 11,
            minor: 1,
            patch: 0,
            beta: false,
        },

        key_signature: Some(KeySignature {
            tonic: Tonic::C,
            scale: Scale::Major,
        }),
        tempo: 120.0,
        time_signature: TimeSignature {
            numerator: 4,
            denominator: 4,
        },
        furthest_bar: Some(16.0),
        plugins,
        samples,
        estimated_duration: Some(chrono::Duration::seconds(60)),
        tags: HashSet::new(),
    }
}

fn create_test_live_set_from_scan(name: &str, scan_result: ScanResult) -> LiveSet {
    let now = Local::now();
    LiveSet {
        id: Uuid::new_v4(),
        file_path: PathBuf::from(format!("C:/test/{}", name)),
        file_name: name.to_string(),
        file_hash: format!("test_hash_{}", name),
        created_time: now,
        modified_time: now,
        last_scan_timestamp: now,
        xml_data: Vec::new(),

        ableton_version: scan_result.version,
        key_signature: scan_result.key_signature,
        tempo: scan_result.tempo,
        time_signature: scan_result.time_signature,
        furthest_bar: scan_result.furthest_bar,
        plugins: scan_result.plugins,
        samples: scan_result.samples,

        estimated_duration: Some(chrono::Duration::seconds(60)),
        tags: HashSet::new(),
    }
}

#[test]
fn test_database_initialization() {
    setup();
    let db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Verify tables exist
    let tables = db
        .conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(tables.contains(&"projects".to_string()));
    assert!(tables.contains(&"plugins".to_string()));
    assert!(tables.contains(&"samples".to_string()));
    assert!(tables.contains(&"project_plugins".to_string()));
    assert!(tables.contains(&"project_samples".to_string()));
}

#[test]
fn test_insert_and_retrieve_project() {
    setup();
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create and insert a test project
    let original_live_set = create_test_live_set();
    db.insert_project(&original_live_set)
        .expect("Failed to insert project");

    // Retrieve the project by path
    let path = original_live_set.file_path.to_string_lossy().to_string();
    let retrieved_live_set = db
        .get_project_by_path(&path)
        .expect("Failed to retrieve project")
        .expect("Project not found");

    // Compare relevant fields
    assert_eq!(retrieved_live_set.file_name, original_live_set.file_name);
    assert_eq!(retrieved_live_set.file_hash, original_live_set.file_hash);
    assert_eq!(retrieved_live_set.tempo, original_live_set.tempo);
    assert_eq!(
        retrieved_live_set.time_signature.numerator,
        original_live_set.time_signature.numerator
    );
    assert_eq!(
        retrieved_live_set.time_signature.denominator,
        original_live_set.time_signature.denominator
    );
    assert_eq!(
        retrieved_live_set.key_signature,
        original_live_set.key_signature
    );
    assert_eq!(
        retrieved_live_set.furthest_bar,
        original_live_set.furthest_bar
    );
    assert_eq!(
        retrieved_live_set.ableton_version,
        original_live_set.ableton_version
    );

    // Compare collections
    assert_eq!(
        retrieved_live_set.plugins.len(),
        original_live_set.plugins.len()
    );
    assert_eq!(
        retrieved_live_set.samples.len(),
        original_live_set.samples.len()
    );

    // Compare first plugin
    let original_plugin = original_live_set.plugins.iter().next().unwrap();
    let retrieved_plugin = retrieved_live_set.plugins.iter().next().unwrap();
    assert_eq!(retrieved_plugin.name, original_plugin.name);
    assert_eq!(
        retrieved_plugin.plugin_format,
        original_plugin.plugin_format
    );
    assert_eq!(retrieved_plugin.installed, original_plugin.installed);

    // Compare first sample
    let original_sample = original_live_set.samples.iter().next().unwrap();
    let retrieved_sample = retrieved_live_set.samples.iter().next().unwrap();
    assert_eq!(retrieved_sample.name, original_sample.name);
    assert_eq!(retrieved_sample.path, original_sample.path);
    assert_eq!(retrieved_sample.is_present, original_sample.is_present);
}

#[test]
fn test_multiple_projects() {
    setup();
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create three different projects with distinct characteristics
    let edm_project = create_test_live_set_from_scan(
        "EDM Project.als",
        LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_plugin("Massive")
            .with_installed_plugin("Pro-Q 3", Some("FabFilter".to_string()))
            .with_sample("kick.wav")
            .with_sample("snare.wav")
            .with_tempo(140.0)
            .build(),
    );

    let rock_project = create_test_live_set_from_scan(
        "Rock Band.als",
        LiveSetBuilder::new()
            .with_plugin("Guitar Rig 6")
            .with_installed_plugin("Pro-R", Some("FabFilter".to_string()))
            .with_sample("guitar_riff.wav")
            .with_sample("drums.wav")
            .with_tempo(120.0)
            .build(),
    );

    let ambient_project = create_test_live_set_from_scan(
        "Ambient Soundscape.als",
        LiveSetBuilder::new()
            .with_plugin("Omnisphere")
            .with_installed_plugin("Pro-L 2", Some("FabFilter".to_string()))
            .with_sample("pad.wav")
            .with_sample("atmosphere.wav")
            .with_tempo(80.0)
            .build(),
    );

    // Store the IDs before inserting
    let edm_id = edm_project.id;
    // let rock_id = rock_project.id;
    // let ambient_id = ambient_project.id;

    // Insert all projects
    db.insert_project(&edm_project)
        .expect("Failed to insert EDM project");
    db.insert_project(&rock_project)
        .expect("Failed to insert rock project");
    db.insert_project(&ambient_project)
        .expect("Failed to insert ambient project");

    // Test retrieval by path
    let retrieved_edm = db
        .get_project_by_path(&edm_project.file_path.to_string_lossy())
        .expect("Failed to retrieve EDM project")
        .expect("EDM project not found");

    // Verify project details
    assert_eq!(retrieved_edm.id, edm_id);
    assert_eq!(retrieved_edm.tempo, 140.0);
    assert_eq!(retrieved_edm.plugins.len(), 3);
    assert_eq!(retrieved_edm.samples.len(), 2);

    // Verify specific plugin exists
    assert!(retrieved_edm.plugins.iter().any(|p| p.name == "Serum"));
    assert!(retrieved_edm
        .plugins
        .iter()
        .any(|p| p.name == "Pro-Q 3" && p.vendor == Some("FabFilter".to_string())));

    // Test basic search functionality
    let fab_filter_results = db.search("FabFilter").expect("Search failed");
    assert_eq!(fab_filter_results.len(), 3); // All projects have a FabFilter plugin

    let edm_results = db.search("kick.wav").expect("Search failed");
    assert_eq!(edm_results.len(), 1); // Only EDM project has kick.wav

    let serum_results = db.search("Serum").expect("Search failed");
    assert_eq!(serum_results.len(), 1); // Only EDM project has Serum
}

#[test]
fn test_tags() {
    setup();
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test project
    let live_set = create_test_live_set();
    db.insert_project(&live_set)
        .expect("Failed to insert project");

    // Test adding tags
    let tag1_id = db.add_tag("work-in-progress").expect("Failed to add tag");
    let tag2_id = db.add_tag("favorite").expect("Failed to add tag");

    // Test listing tags
    let tags = db.list_tags().expect("Failed to list tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|(_, name)| name == "work-in-progress"));
    assert!(tags.iter().any(|(_, name)| name == "favorite"));

    // Test tagging project
    db.tag_project(&live_set.id.to_string(), &tag1_id)
        .expect("Failed to tag project");
    db.tag_project(&live_set.id.to_string(), &tag2_id)
        .expect("Failed to tag project");

    // Test getting project tags
    let project_tags = db
        .get_project_tags(&live_set.id.to_string())
        .expect("Failed to get project tags");
    assert_eq!(project_tags.len(), 2);
    assert!(project_tags.contains("work-in-progress"));
    assert!(project_tags.contains("favorite"));

    // Test getting projects by tag
    let tagged_projects = db
        .get_projects_by_tag(&tag1_id)
        .expect("Failed to get projects by tag");
    assert_eq!(tagged_projects.len(), 1);
    assert_eq!(tagged_projects[0].id, live_set.id);

    // Test untagging project
    db.untag_project(&live_set.id.to_string(), &tag1_id)
        .expect("Failed to untag project");
    let project_tags = db
        .get_project_tags(&live_set.id.to_string())
        .expect("Failed to get project tags");
    assert_eq!(project_tags.len(), 1);
    assert!(project_tags.contains("favorite"));

    // Test removing tag
    db.remove_tag(&tag2_id).expect("Failed to remove tag");
    let tags = db.list_tags().expect("Failed to list tags");
    assert_eq!(tags.len(), 1);
    assert!(tags.iter().any(|(_, name)| name == "work-in-progress"));
}
