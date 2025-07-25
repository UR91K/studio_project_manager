//! Core database functionality tests

use std::collections::HashSet;

use chrono::Local;
use studio_project_manager::{
    AbletonVersion, KeySignature, Plugin, PluginFormat, Sample, Scale,
    TimeSignature, Tonic,
};
use uuid::Uuid;

use super::*;
use crate::common::{create_test_live_set_from_parse, setup, LiveSetBuilder};

pub fn create_test_live_set() -> LiveSet {
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
        is_active: true,
        id: Uuid::new_v4(),
        file_path: PathBuf::from("C:/test/test_project.als"),
        name: "test_project.als".to_string(),
        file_hash: "test_hash".to_string(),
        created_time: now,
        modified_time: now,
        last_parsed_timestamp: now,

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

#[test]
pub fn test_database_initialization() {
    setup("error");
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
pub fn test_insert_and_retrieve_project() {
    setup("error");
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
    assert_eq!(retrieved_live_set.name, original_live_set.name);
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
pub fn test_multiple_projects() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create three different projects with distinct characteristics
    let edm_project = create_test_live_set_from_parse(
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

    let rock_project = create_test_live_set_from_parse(
        "Rock Band.als",
        LiveSetBuilder::new()
            .with_plugin("Guitar Rig 6")
            .with_installed_plugin("Pro-R", Some("FabFilter".to_string()))
            .with_sample("guitar_riff.wav")
            .with_sample("drums.wav")
            .with_tempo(120.0)
            .build(),
    );

    let ambient_project = create_test_live_set_from_parse(
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
    let fab_filter_results = db.search_simple("FabFilter").expect("Search failed");
    assert_eq!(fab_filter_results.len(), 3); // All projects have a FabFilter plugin

    let edm_results = db.search_simple("kick.wav").expect("Search failed");
    assert_eq!(edm_results.len(), 1); // Only EDM project has kick.wav

    let serum_results = db.search_simple("Serum").expect("Search failed");
    assert_eq!(serum_results.len(), 1); // Only EDM project has Serum
}
#[test]
#[allow(unused_variables)]
fn test_notes_and_tasks() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create a test project
    let project = create_test_live_set();
    let project_id = project.id.to_string();
    db.insert_project(&project)
        .expect("Failed to insert project");

    // Create a test collection
    let collection_id = db
        .create_collection(
            "Test Collection",
            Some("A collection for testing notes and tasks"),
            None,
        )
        .expect("Failed to create collection");

    // Add project to collection
    db.add_project_to_collection(&collection_id, &project_id)
        .expect("Failed to add project to collection");

    // Test project notes
    db.set_project_notes(&project_id, "Project note: needs mixing")
        .expect("Failed to set project notes");
    let project_notes = db
        .get_project_notes(&project_id)
        .expect("Failed to get project notes");
    assert_eq!(
        project_notes,
        Some("Project note: needs mixing".to_string())
    );

    // Test collection notes
    db.set_collection_notes(&collection_id, "Collection note: work in progress")
        .expect("Failed to set collection notes");
    let collection_notes = db
        .get_collection_notes(&collection_id)
        .expect("Failed to get collection notes");
    assert_eq!(
        collection_notes,
        Some("Collection note: work in progress".to_string())
    );

    // Test adding tasks to project
    let task1_id = db
        .add_task(&project_id, "Fix the bass mix")
        .expect("Failed to add task 1");
    let task2_id = db
        .add_task(&project_id, "Add more reverb")
        .expect("Failed to add task 2");
    let task3_id = db
        .add_task(&project_id, "Export final version")
        .expect("Failed to add task 3");

    // Test getting project tasks
    let project_tasks = db
        .get_project_tasks(&project_id)
        .expect("Failed to get project tasks");
    assert_eq!(project_tasks.len(), 3);
    assert!(project_tasks
        .iter()
        .any(|(_, desc, _, _)| desc == "Fix the bass mix"));
    assert!(project_tasks
        .iter()
        .any(|(_, desc, _, _)| desc == "Add more reverb"));
    assert!(project_tasks
        .iter()
        .any(|(_, desc, _, _)| desc == "Export final version"));

    // Test completing a task
    db.complete_task(&task1_id, true)
        .expect("Failed to complete task");
    let project_tasks = db
        .get_project_tasks(&project_id)
        .expect("Failed to get project tasks after completion");
    let completed_task = project_tasks
        .iter()
        .find(|(id, _, _, _)| id == &task1_id)
        .expect("Couldn't find completed task");
    assert!(completed_task.2); // Check completion status

    // Test removing a task
    db.remove_task(&task2_id).expect("Failed to remove task");
    let project_tasks = db
        .get_project_tasks(&project_id)
        .expect("Failed to get project tasks after removal");
    assert_eq!(project_tasks.len(), 2);
    assert!(!project_tasks.iter().any(|(id, _, _, _)| id == &task2_id));

    // Test getting collection tasks
    let collection_tasks = db
        .get_collection_tasks(&collection_id)
        .expect("Failed to get collection tasks");
    assert_eq!(collection_tasks.len(), 2);

    // Verify collection tasks contain project name and correct completion status
    let completed_collection_task = collection_tasks
        .iter()
        .find(|(id, _, desc, _, _)| desc == "Fix the bass mix")
        .expect("Couldn't find completed task in collection");
    assert!(completed_collection_task.3); // Check completion status
    assert_eq!(completed_collection_task.1, "test_project.als"); // Check project name

    // Create a second project with tasks
    let project2 =
        create_test_live_set_from_parse("Second Project.als", LiveSetBuilder::new().build());
    let project2_id = project2.id.to_string();
    db.insert_project(&project2)
        .expect("Failed to insert second project");
    db.add_project_to_collection(&collection_id, &project2_id)
        .expect("Failed to add second project to collection");

    // Add tasks to second project
    let task4_id = db
        .add_task(&project2_id, "Record vocals")
        .expect("Failed to add task to second project");

    // Verify collection tasks show tasks from both projects in correct order
    let collection_tasks = db
        .get_collection_tasks(&collection_id)
        .expect("Failed to get collection tasks after adding second project");
    assert_eq!(collection_tasks.len(), 3);

    // Tasks should be ordered by project position in collection
    assert_eq!(collection_tasks[0].1, "test_project.als");
    assert_eq!(collection_tasks[2].1, "Second Project.als");
}

#[test]
fn test_mark_project_deleted() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create and insert a test project
    let live_set = create_test_live_set();
    let project_id = live_set.id;
    db.insert_project(&live_set)
        .expect("Failed to insert project");

    // Mark project as deleted
    db.mark_project_deleted(&project_id)
        .expect("Failed to mark project as deleted");

    // Verify project is marked as deleted
    let deleted_projects = db
        .get_all_projects_with_status(Some(false))
        .expect("Failed to get deleted projects");
    assert_eq!(deleted_projects.len(), 1);
    assert_eq!(deleted_projects[0].id, project_id);
    assert!(!deleted_projects[0].is_active);

    // Verify active projects list is empty
    let active_projects = db
        .get_all_projects_with_status(Some(true))
        .expect("Failed to get active projects");
    assert!(active_projects.is_empty());
}

#[test]
fn test_find_deleted_by_hash() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create and insert a test project
    let live_set = create_test_live_set();
    let project_id = live_set.id;
    let file_hash = live_set.file_hash.clone();
    db.insert_project(&live_set)
        .expect("Failed to insert project");

    // Mark project as deleted
    db.mark_project_deleted(&project_id)
        .expect("Failed to mark project as deleted");

    // Find deleted project by hash
    let deleted_projects = db
        .get_all_projects_with_status(Some(false))
        .expect("Failed to get deleted projects");
    let found_project = deleted_projects
        .iter()
        .find(|p| p.file_hash == file_hash)
        .expect("Could not find deleted project by hash");

    assert_eq!(found_project.id, project_id);
    assert_eq!(found_project.file_hash, file_hash);
    assert!(!found_project.is_active);
}

#[test]
fn test_reactivate_project() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create and insert a test project
    let live_set = create_test_live_set();
    let project_id = live_set.id;
    db.insert_project(&live_set)
        .expect("Failed to insert project");

    // Mark project as deleted
    db.mark_project_deleted(&project_id)
        .expect("Failed to mark project as deleted");

    // Reactivate project with new path
    let new_path = PathBuf::from("C:/test/restored_project.als");
    db.reactivate_project(&project_id, &new_path)
        .expect("Failed to reactivate project");

    // Verify project is reactivated
    let active_projects = db
        .get_all_projects_with_status(Some(true))
        .expect("Failed to get active projects");
    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].id, project_id);
    assert!(active_projects[0].is_active);
    assert_eq!(active_projects[0].file_path, new_path);

    // Verify deleted projects list is empty
    let deleted_projects = db
        .get_all_projects_with_status(Some(false))
        .expect("Failed to get deleted projects");
    assert!(deleted_projects.is_empty());
}

#[test]
fn test_permanent_deletion() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create and insert a test project
    let live_set = create_test_live_set();
    let project_id = live_set.id;
    db.insert_project(&live_set)
        .expect("Failed to insert project");

    // Attempt to permanently delete active project (should fail)
    db.permanently_delete_project(&project_id)
        .expect_err("Should not be able to permanently delete active project");

    // Verify project still exists and is active
    let active_projects = db
        .get_all_projects_with_status(Some(true))
        .expect("Failed to get active projects");
    assert_eq!(
        active_projects.len(),
        1,
        "Active project should still exist"
    );
    assert_eq!(active_projects[0].id, project_id);

    // Mark project as deleted
    db.mark_project_deleted(&project_id)
        .expect("Failed to mark project as deleted");

    // Now permanent deletion should succeed
    db.permanently_delete_project(&project_id)
        .expect("Failed to permanently delete project");

    // Verify project is completely gone
    let all_projects = db
        .get_all_projects_with_status(None)
        .expect("Failed to get all projects");
    assert!(all_projects.is_empty());
}
