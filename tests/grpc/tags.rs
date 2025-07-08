//! Tags-related gRPC tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following tests from src/grpc/server.rs:
//! - test_get_tags_empty() (line ~1204)
//! - test_create_tag() (line ~1225)
//! - test_get_tags_with_data() (line ~1269)
//! - test_tag_project() (line ~1303)
//! - test_untag_project() (line ~1517)
//! - test_tag_project_nonexistent_project() (line ~1535)
//! - test_tag_project_nonexistent_tag() (line ~1565)
//! - test_create_duplicate_tag() (line ~1596)
//! - test_tag_project_idempotent() (line ~1596) 

use studio_project_manager::grpc::proto::studio_project_manager_server::StudioProjectManager;

use super::*;
use crate::common::setup;

// TODO: Move all tag-related gRPC tests from src/grpc/server.rs
// This includes all GetTags, CreateTag, TagProject, and UntagProject tests
// Total: ~9 tests to move
// These tests are in the "TAG TESTS" section starting around line 1204 


#[tokio::test]
async fn test_get_tags_empty() {
    setup("debug");
    
    let server = create_test_server().await;
    
    let request = GetTagsRequest {};
    let response = server.get_tags(Request::new(request)).await.unwrap();
    let tags = response.into_inner().tags;
    
    assert_eq!(tags.len(), 0);
}

#[tokio::test]
async fn test_create_tag() {
    setup("debug");
    
    let server = create_test_server().await;
    
    let request = CreateTagRequest {
        name: "Electronic".to_string(),
    };
    let response = server.create_tag(Request::new(request)).await.unwrap();
    let tag = response.into_inner().tag.unwrap();
    
    assert_eq!(tag.name, "Electronic");
    assert!(!tag.id.is_empty());
    
    // Verify timestamp is recent
    let now = chrono::Utc::now().timestamp();
    assert!((tag.created_at - now).abs() < 5);
}

#[allow(unused)]
#[tokio::test]
async fn test_get_tags_with_data() {
    setup("debug");
    
    let server = create_test_server().await;
    
    // Create multiple tags
    let tag1_req = CreateTagRequest {
        name: "Rock".to_string(),
    };
    let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
    // let tag1 = tag1_resp.into_inner().tag.unwrap();
    
    let tag2_req = CreateTagRequest {
        name: "Electronic".to_string(),
    };
    let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
    // let tag2 = tag2_resp.into_inner().tag.unwrap();
    
    let tag3_req = CreateTagRequest {
        name: "Ambient".to_string(),
    };
    let tag3_resp = server.create_tag(Request::new(tag3_req)).await.unwrap();
    // let tag3 = tag3_resp.into_inner().tag.unwrap();
    
    // Get all tags
    let request = GetTagsRequest {};
    let response = server.get_tags(Request::new(request)).await.unwrap();
    let tags = response.into_inner().tags;
    
    assert_eq!(tags.len(), 3);
    
    // Verify tags are sorted by name (as per database query)
    let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tag_names, vec!["Ambient", "Electronic", "Rock"]);
    
    // Verify all tags have valid IDs and timestamps
    for tag in tags {
        assert!(!tag.id.is_empty());
        assert!(!tag.name.is_empty());
        assert!(tag.created_at > 0);
    }
}

#[tokio::test]
async fn test_tag_project() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Work In Progress".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();
    
    // Tag the project
    let tag_project_req = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag.id.clone(),
    };
    let tag_project_resp = server.tag_project(Request::new(tag_project_req)).await.unwrap();
    let result = tag_project_resp.into_inner();
    
    assert!(result.success);
    
    // Verify the project is tagged by checking database directly
    let mut db_guard = db.lock().await;
    let project_tags = db_guard.get_project_tags(&project_id).unwrap();
    assert_eq!(project_tags.len(), 1);
    assert!(project_tags.contains("Work In Progress"));
}

#[tokio::test]
async fn test_untag_project() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Create multiple tags
    let tag1_req = CreateTagRequest {
        name: "Tag 1".to_string(),
    };
    let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
    let tag1 = tag1_resp.into_inner().tag.unwrap();
    
    let tag2_req = CreateTagRequest {
        name: "Tag 2".to_string(),
    };
    let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
    let tag2 = tag2_resp.into_inner().tag.unwrap();
    
    // Tag the project with both tags
    let tag_project_req1 = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag1.id.clone(),
    };
    server.tag_project(Request::new(tag_project_req1)).await.unwrap();
    
    let tag_project_req2 = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag2.id.clone(),
    };
    server.tag_project(Request::new(tag_project_req2)).await.unwrap();
    
    // Verify both tags are applied
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 2);
        assert!(project_tags.contains("Tag 1"));
        assert!(project_tags.contains("Tag 2"));
    }
    
    // Untag one tag
    let untag_project_req = UntagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag1.id.clone(),
    };
    let untag_project_resp = server.untag_project(Request::new(untag_project_req)).await.unwrap();
    let result = untag_project_resp.into_inner();
    
    assert!(result.success);
    
    // Verify only one tag remains
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 1);
        assert!(project_tags.contains("Tag 2"));
        assert!(!project_tags.contains("Tag 1"));
    }
}

#[tokio::test]
async fn test_tag_project_nonexistent_project() {
    setup("debug");
    
    let server = create_test_server().await;
    
    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Test Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();
    
    // Try to tag a non-existent project
    let tag_project_req = TagProjectRequest {
        project_id: "non-existent-project-id".to_string(),
        tag_id: tag.id,
    };
    let result = server.tag_project(Request::new(tag_project_req)).await;
    
    // Should fail due to foreign key constraint (project doesn't exist)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_tag_project_nonexistent_tag() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Try to tag with a non-existent tag
    let tag_project_req = TagProjectRequest {
        project_id,
        tag_id: "non-existent-tag-id".to_string(),
    };
    let result = server.tag_project(Request::new(tag_project_req)).await;
    
    // Should fail due to foreign key constraint
    assert!(result.is_err());
}

    
#[allow(unused)]
#[tokio::test]
async fn test_create_duplicate_tag() {
    setup("debug");
    
    let server = create_test_server().await;
    
    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Duplicate Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();
    
    // Try to create another tag with the same name
    let duplicate_req = CreateTagRequest {
        name: "Duplicate Tag".to_string(),
    };
    let result = server.create_tag(Request::new(duplicate_req)).await;
    
    // Should fail due to unique constraint
    assert!(result.is_err());
}

#[tokio::test]
async fn test_tag_project_idempotent() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Idempotent Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();
    
    // Tag the project multiple times
    let tag_project_req = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag.id.clone(),
    };
    let result1 = server.tag_project(Request::new(tag_project_req.clone())).await.unwrap();
    let result2 = server.tag_project(Request::new(tag_project_req.clone())).await.unwrap();
    let result3 = server.tag_project(Request::new(tag_project_req)).await.unwrap();
    
    // All should succeed
    assert!(result1.into_inner().success);
    assert!(result2.into_inner().success);
    assert!(result3.into_inner().success);
    
    // Verify only one tag association exists
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 1);
        assert!(project_tags.contains("Idempotent Tag"));
    }
}
