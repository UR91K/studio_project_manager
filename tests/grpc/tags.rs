//! Tags-related gRPC tests

use studio_project_manager::grpc::tags::tag_service_server::TagService;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_get_tags_empty() {
    setup("error");

    let server = create_test_server().await;

    let request = GetTagsRequest {};
    let response = server.get_tags(Request::new(request)).await.unwrap();
    let tags = response.into_inner().tags;

    assert_eq!(tags.len(), 0);
}

#[tokio::test]
async fn test_create_tag() {
    setup("error");

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
    setup("error");

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
    setup("error");

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
    let tag_project_resp = server
        .tag_project(Request::new(tag_project_req))
        .await
        .unwrap();
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
    setup("error");

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
    server
        .tag_project(Request::new(tag_project_req1))
        .await
        .unwrap();

    let tag_project_req2 = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag2.id.clone(),
    };
    server
        .tag_project(Request::new(tag_project_req2))
        .await
        .unwrap();

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
    let untag_project_resp = server
        .untag_project(Request::new(untag_project_req))
        .await
        .unwrap();
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
    setup("error");

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
    setup("error");

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
    setup("error");

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
    setup("error");

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
    let result1 = server
        .tag_project(Request::new(tag_project_req.clone()))
        .await
        .unwrap();
    let result2 = server
        .tag_project(Request::new(tag_project_req.clone()))
        .await
        .unwrap();
    let result3 = server
        .tag_project(Request::new(tag_project_req))
        .await
        .unwrap();

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

#[tokio::test]
async fn test_update_tag() {
    setup("error");

    let server = create_test_server().await;

    // Create a tag
    let create_req = CreateTagRequest {
        name: "Original Tag".to_string(),
    };
    let create_resp = server.create_tag(Request::new(create_req)).await.unwrap();
    let tag = create_resp.into_inner().tag.unwrap();

    // Update the tag
    let update_req = UpdateTagRequest {
        tag_id: tag.id.clone(),
        name: "Updated Tag".to_string(),
    };
    let update_resp = server.update_tag(Request::new(update_req)).await.unwrap();
    let updated_tag = update_resp.into_inner().tag.unwrap();

    assert_eq!(updated_tag.name, "Updated Tag");
    assert_eq!(updated_tag.id, tag.id);

    // Verify the tag was updated by getting all tags
    let get_req = GetTagsRequest {};
    let get_resp = server.get_tags(Request::new(get_req)).await.unwrap();
    let tags = get_resp.into_inner().tags;

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Updated Tag");
    assert_eq!(tags[0].id, tag.id);
}

#[tokio::test]
async fn test_update_tag_nonexistent() {
    setup("error");

    let server = create_test_server().await;

    // Try to update a non-existent tag
    let update_req = UpdateTagRequest {
        tag_id: "non-existent-tag-id".to_string(),
        name: "New Name".to_string(),
    };
    let update_resp = server.update_tag(Request::new(update_req)).await;

    // Should return an error for non-existent tag
    assert!(update_resp.is_err());
}

#[tokio::test]
async fn test_update_tag_empty_name() {
    setup("error");

    let server = create_test_server().await;

    // Create a tag
    let create_req = CreateTagRequest {
        name: "Test Tag".to_string(),
    };
    let create_resp = server.create_tag(Request::new(create_req)).await.unwrap();
    let tag = create_resp.into_inner().tag.unwrap();

    // Update with empty name (should be allowed)
    let update_req = UpdateTagRequest {
        tag_id: tag.id.clone(),
        name: "".to_string(),
    };
    let update_resp = server.update_tag(Request::new(update_req)).await.unwrap();
    let updated_tag = update_resp.into_inner().tag.unwrap();

    assert_eq!(updated_tag.name, "");
    assert_eq!(updated_tag.id, tag.id);

    // Verify the tag was updated to empty name
    let get_req = GetTagsRequest {};
    let get_resp = server.get_tags(Request::new(get_req)).await.unwrap();
    let tags = get_resp.into_inner().tags;

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "");
}

#[tokio::test]
async fn test_delete_tag() {
    setup("error");

    let server = create_test_server().await;

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

    // Verify both tags exist
    let get_req = GetTagsRequest {};
    let get_resp = server.get_tags(Request::new(get_req)).await.unwrap();
    let tags = get_resp.into_inner().tags;
    assert_eq!(tags.len(), 2);

    // Delete one tag
    let delete_req = DeleteTagRequest {
        tag_id: tag1.id.clone(),
    };
    let delete_resp = server.delete_tag(Request::new(delete_req)).await.unwrap();
    let result = delete_resp.into_inner();

    assert!(result.success);

    // Verify only one tag remains
    let get_req = GetTagsRequest {};
    let get_resp = server.get_tags(Request::new(get_req)).await.unwrap();
    let tags = get_resp.into_inner().tags;

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Tag 2");
    assert_eq!(tags[0].id, tag2.id);
}

#[tokio::test]
async fn test_delete_tag_nonexistent() {
    setup("error");

    let server = create_test_server().await;

    // Try to delete a non-existent tag
    let delete_req = DeleteTagRequest {
        tag_id: "non-existent-tag-id".to_string(),
    };
    let delete_resp = server.delete_tag(Request::new(delete_req)).await.unwrap();
    let result = delete_resp.into_inner();

    // Should succeed but have no effect (graceful handling)
    assert!(result.success);
}

#[tokio::test]
async fn test_delete_tag_with_project_associations() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Associated Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();

    // Tag the project
    let tag_project_req = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag.id.clone(),
    };
    server
        .tag_project(Request::new(tag_project_req))
        .await
        .unwrap();

    // Verify the association exists
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 1);
    }

    // Delete the tag
    let delete_req = DeleteTagRequest {
        tag_id: tag.id.clone(),
    };
    let delete_resp = server.delete_tag(Request::new(delete_req)).await.unwrap();
    let result = delete_resp.into_inner();

    assert!(result.success);

    // Verify the tag is deleted
    let get_req = GetTagsRequest {};
    let get_resp = server.get_tags(Request::new(get_req)).await.unwrap();
    let tags = get_resp.into_inner().tags;
    assert_eq!(tags.len(), 0);

    // Verify the project association is also removed (cascading delete)
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 0);
    }
}

#[tokio::test]
async fn test_update_tag_with_project_associations() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Create a tag
    let tag_req = CreateTagRequest {
        name: "Original Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();

    // Tag the project
    let tag_project_req = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag.id.clone(),
    };
    server
        .tag_project(Request::new(tag_project_req))
        .await
        .unwrap();

    // Update the tag name
    let update_req = UpdateTagRequest {
        tag_id: tag.id.clone(),
        name: "Updated Tag".to_string(),
    };
    let update_resp = server.update_tag(Request::new(update_req)).await.unwrap();
    let updated_tag = update_resp.into_inner().tag.unwrap();

    assert_eq!(updated_tag.name, "Updated Tag");
    assert_eq!(updated_tag.id, tag.id);

    // Verify the tag name was updated but association remains
    {
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 1);
        assert!(project_tags.contains("Updated Tag"));
        assert!(!project_tags.contains("Original Tag"));
    }
}
