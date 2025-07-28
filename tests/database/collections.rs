//! Database collections functionality tests

use super::*;
use crate::common::{create_test_live_set_from_parse, setup, LiveSetBuilder};

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
    let (collections, total_count) = db.list_collections(None, None, None, None).expect("Failed to list collections");
    assert_eq!(collections.len(), 1);
    assert_eq!(total_count, 1);
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
    let (collections, total_count) = db
        .list_collections(None, None, None, None)
        .expect("Failed to list collections after deletion");
    assert_eq!(collections.len(), 0);
    assert_eq!(total_count, 0);
}

#[test]
fn test_duplicate_collection() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test projects
    let project1 = create_test_live_set_from_parse(
        "Project 1.als",
        LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_tempo(140.0)
            .build(),
    );

    let project2 = create_test_live_set_from_parse(
        "Project 2.als",
        LiveSetBuilder::new()
            .with_plugin("Guitar Rig 6")
            .with_tempo(120.0)
            .build(),
    );

    let project3 = create_test_live_set_from_parse(
        "Project 3.als",
        LiveSetBuilder::new()
            .with_plugin("Omnisphere")
            .with_tempo(80.0)
            .build(),
    );

    // Insert projects
    db.insert_project(&project1).expect("Failed to insert project 1");
    db.insert_project(&project2).expect("Failed to insert project 2");
    db.insert_project(&project3).expect("Failed to insert project 3");

    // Create original collection
    let original_collection_id = db
        .create_collection(
            "Original Collection",
            Some("Original description"),
            Some("Original notes"),
        )
        .expect("Failed to create original collection");

    // Add projects to original collection
    db.add_project_to_collection(&original_collection_id, &project1.id.to_string())
        .expect("Failed to add project 1");
    db.add_project_to_collection(&original_collection_id, &project2.id.to_string())
        .expect("Failed to add project 2");
    db.add_project_to_collection(&original_collection_id, &project3.id.to_string())
        .expect("Failed to add project 3");

    // Reorder projects in original collection
    db.reorder_project_in_collection(&original_collection_id, &project3.id.to_string(), 0)
        .expect("Failed to reorder project");

    // Duplicate the collection
    let duplicated_collection_id = db
        .duplicate_collection(
            &original_collection_id,
            "Duplicated Collection",
            Some("New description"),
            None, // Use original notes
        )
        .expect("Failed to duplicate collection");

    // Verify the duplicated collection exists
    let (collections, total_count) = db
        .list_collections(None, None, None, None)
        .expect("Failed to list collections");
    assert_eq!(collections.len(), 2);
    assert_eq!(total_count, 2);

    // Get both collections
    let original_collection = db
        .get_collection_by_id(&original_collection_id)
        .expect("Failed to get original collection")
        .expect("Original collection not found");

    let duplicated_collection = db
        .get_collection_by_id(&duplicated_collection_id)
        .expect("Failed to get duplicated collection")
        .expect("Duplicated collection not found");

    // Verify collection metadata
    assert_eq!(original_collection.1, "Original Collection");
    assert_eq!(duplicated_collection.1, "Duplicated Collection");
    assert_eq!(original_collection.2, Some("Original description".to_string()));
    assert_eq!(duplicated_collection.2, Some("New description".to_string()));
    assert_eq!(original_collection.3, Some("Original notes".to_string()));
    assert_eq!(duplicated_collection.3, Some("Original notes".to_string())); // Should inherit original notes

    // Verify both collections have the same projects in the same order
    assert_eq!(original_collection.6.len(), 3);
    assert_eq!(duplicated_collection.6.len(), 3);
    assert_eq!(original_collection.6, duplicated_collection.6);

    // Verify project order is preserved
    let original_projects = db
        .get_collection_projects(&original_collection_id)
        .expect("Failed to get original collection projects");
    let duplicated_projects = db
        .get_collection_projects(&duplicated_collection_id)
        .expect("Failed to get duplicated collection projects");

    assert_eq!(original_projects.len(), 3);
    assert_eq!(duplicated_projects.len(), 3);
    assert_eq!(original_projects[0].name, "Project 3.als");
    assert_eq!(duplicated_projects[0].name, "Project 3.als");
    assert_eq!(original_projects[1].name, "Project 1.als");
    assert_eq!(duplicated_projects[1].name, "Project 1.als");
    assert_eq!(original_projects[2].name, "Project 2.als");
    assert_eq!(duplicated_projects[2].name, "Project 2.als");

    // Verify collection IDs are different
    assert_ne!(original_collection_id, duplicated_collection_id);

    // Test duplicating with all new metadata
    let fully_new_collection_id = db
        .duplicate_collection(
            &original_collection_id,
            "Fully New Collection",
            Some("Completely new description"),
            Some("Completely new notes"),
        )
        .expect("Failed to duplicate collection with new metadata");

    let fully_new_collection = db
        .get_collection_by_id(&fully_new_collection_id)
        .expect("Failed to get fully new collection")
        .expect("Fully new collection not found");

    assert_eq!(fully_new_collection.1, "Fully New Collection");
    assert_eq!(fully_new_collection.2, Some("Completely new description".to_string()));
    assert_eq!(fully_new_collection.3, Some("Completely new notes".to_string()));

    // Test duplicating non-existent collection
    let result = db.duplicate_collection(
        "non-existent-id",
        "Should Fail",
        None,
        None,
    );
    assert!(result.is_err());
}
