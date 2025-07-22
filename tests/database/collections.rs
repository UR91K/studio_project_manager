//! Database collections functionality tests

use super::*;
use crate::{
    common::{setup, LiveSetBuilder},
    database::core::create_test_live_set_from_parse,
};

// TODO: Create database-level collection tests

#[test]
fn test_collections() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create three test projects with different characteristics
    let edm_project = create_test_live_set_from_parse(
        "EDM Project.als",
        LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_tempo(140.0)
            .build(),
    );

    let rock_project = create_test_live_set_from_parse(
        "Rock Band.als",
        LiveSetBuilder::new()
            .with_plugin("Guitar Rig 6")
            .with_tempo(120.0)
            .build(),
    );

    let ambient_project = create_test_live_set_from_parse(
        "Ambient Soundscape.als",
        LiveSetBuilder::new()
            .with_plugin("Omnisphere")
            .with_tempo(80.0)
            .build(),
    );

    // Insert all projects
    db.insert_project(&edm_project)
        .expect("Failed to insert EDM project");
    db.insert_project(&rock_project)
        .expect("Failed to insert rock project");
    db.insert_project(&ambient_project)
        .expect("Failed to insert ambient project");

    // Test creating a collection
    let collection_id = db
        .create_collection(
            "Electronic Music",
            Some("Collection of electronic music projects"),
            None,
        )
        .expect("Failed to create collection");

    // Test listing collections
    let collections = db.list_collections().expect("Failed to list collections");
    assert_eq!(collections.len(), 1);
    let (id, name, description) = &collections[0];
    assert_eq!(id, &collection_id);
    assert_eq!(name, "Electronic Music");
    assert_eq!(
        description.as_ref(),
        Some(&"Collection of electronic music projects".to_string())
    );

    // Test adding projects to collection
    db.add_project_to_collection(&collection_id, &edm_project.id.to_string())
        .expect("Failed to add EDM project");
    db.add_project_to_collection(&collection_id, &ambient_project.id.to_string())
        .expect("Failed to add ambient project");
    db.add_project_to_collection(&collection_id, &rock_project.id.to_string())
        .expect("Failed to add rock project");

    // Test retrieving projects in order
    let projects = db
        .get_collection_projects(&collection_id)
        .expect("Failed to get collection projects");
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].name, "EDM Project.als");
    assert_eq!(projects[1].name, "Ambient Soundscape.als");
    assert_eq!(projects[2].name, "Rock Band.als");

    // Test reordering projects
    db.reorder_project_in_collection(&collection_id, &rock_project.id.to_string(), 0)
        .expect("Failed to reorder project");

    let projects = db
        .get_collection_projects(&collection_id)
        .expect("Failed to get collection projects after reorder");
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].name, "Rock Band.als");
    assert_eq!(projects[1].name, "EDM Project.als");
    assert_eq!(projects[2].name, "Ambient Soundscape.als");

    // Test removing a project
    db.remove_project_from_collection(&collection_id, &ambient_project.id.to_string())
        .expect("Failed to remove project");

    let projects = db
        .get_collection_projects(&collection_id)
        .expect("Failed to get collection projects after removal");
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].name, "Rock Band.als");
    assert_eq!(projects[1].name, "EDM Project.als");

    // Test deleting collection
    db.delete_collection(&collection_id)
        .expect("Failed to delete collection");
    let collections = db
        .list_collections()
        .expect("Failed to list collections after deletion");
    assert_eq!(collections.len(), 0);
}
