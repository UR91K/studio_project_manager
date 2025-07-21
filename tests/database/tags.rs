//! Database tags functionality tests

use super::*;
use crate::{common::setup, database::core::create_test_live_set};

// TODO: Add database-level tag tests

#[test]
fn test_tags() {
    setup("debug");
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
    assert!(tags.iter().any(|(_, name, _)| name == "work-in-progress"));
    assert!(tags.iter().any(|(_, name, _)| name == "favorite"));

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
    assert!(tags.iter().any(|(_, name, _)| name == "work-in-progress"));
}
