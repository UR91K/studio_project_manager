//! Project-related gRPC tests

use studio_project_manager::grpc::proto::studio_project_manager_server::StudioProjectManager;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_update_project_name() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Get the project to verify initial name
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    let project = get_project_resp.into_inner().project.unwrap();
    let original_name = project.name;

    // Update the project name
    let new_name = "My Custom Project Alias";
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: new_name.to_string(),
    };
    let update_resp = server
        .update_project_name(Request::new(update_req))
        .await
        .unwrap();
    let result = update_resp.into_inner();

    assert!(result.success);

    // Verify the name was updated
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();

    assert_eq!(updated_project.name, new_name);
    assert_ne!(updated_project.name, original_name);
}

#[tokio::test]
async fn test_update_project_name_nonexistent_project() {
    setup("error");

    let server = create_test_server().await;

    // Try to update a non-existent project
    let update_req = UpdateProjectNameRequest {
        project_id: "non-existent-project-id".to_string(),
        name: "New Name".to_string(),
    };
    let update_resp = server
        .update_project_name(Request::new(update_req))
        .await
        .unwrap();
    let result = update_resp.into_inner();

    // Should succeed but have no effect (graceful handling of non-existent projects)
    assert!(result.success);
}

#[tokio::test]
async fn test_update_project_name_empty_string() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Update with empty string (should be allowed)
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: "".to_string(),
    };
    let update_resp = server
        .update_project_name(Request::new(update_req))
        .await
        .unwrap();
    let result = update_resp.into_inner();

    assert!(result.success);

    // Verify the name was updated to empty string
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();

    assert_eq!(updated_project.name, "");
}

#[tokio::test]
async fn test_update_project_name_special_characters() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Update with special characters and unicode
    let new_name = "🎵 My Project (remix) [2024] - Draft v2.1 🎶";
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: new_name.to_string(),
    };
    let update_resp = server
        .update_project_name(Request::new(update_req))
        .await
        .unwrap();
    let result = update_resp.into_inner();

    assert!(result.success);

    // Verify the name was updated correctly with special characters
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();

    assert_eq!(updated_project.name, new_name);
}

#[tokio::test]
async fn test_update_project_name_persistence() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Update the project name
    let new_name = "Persistent Test Name";
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: new_name.to_string(),
    };
    let update_resp = server
        .update_project_name(Request::new(update_req))
        .await
        .unwrap();
    assert!(update_resp.into_inner().success);

    // Note: Since we're using in-memory databases, we'll verify persistence
    // by querying the same server multiple times rather than creating a new instance

    // Verify the name persists through multiple get operations
    for _ in 0..3 {
        let get_project_req = GetProjectRequest {
            project_id: project_id.clone(),
        };
        let get_project_resp = server
            .get_project(Request::new(get_project_req))
            .await
            .unwrap();
        let project = get_project_resp.into_inner().project.unwrap();

        assert_eq!(project.name, new_name);
    }
}

#[tokio::test]
async fn test_mark_project_deleted() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Mark the project as deleted
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: project_id.clone(),
    };
    let mark_deleted_resp = server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await
        .unwrap();
    let result = mark_deleted_resp.into_inner();

    assert!(result.success);

    // Verify the project is marked as deleted by checking deleted projects list
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;

    assert_eq!(deleted_projects.len(), 1);
    assert_eq!(deleted_projects[0].id, project_id);
}

#[tokio::test]
async fn test_reactivate_project() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Mark the project as deleted first
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: project_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await
        .unwrap();

    // Reactivate the project
    let reactivate_req = ReactivateProjectRequest {
        project_id: project_id.clone(),
    };
    let reactivate_resp = server
        .reactivate_project(Request::new(reactivate_req))
        .await
        .unwrap();
    let result = reactivate_resp.into_inner();

    assert!(result.success);

    // Verify the project is back in active list
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;

    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].id, project_id);
}

#[tokio::test]
async fn test_get_projects_by_deletion_status() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create multiple test projects
    let project1_id = create_test_project_in_db(db).await;
    let project2_id = create_test_project_in_db(db).await;
    let project3_id = create_test_project_in_db(db).await;

    // Mark two projects as deleted
    let mark_deleted_req1 = MarkProjectDeletedRequest {
        project_id: project1_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req1))
        .await
        .unwrap();

    let mark_deleted_req2 = MarkProjectDeletedRequest {
        project_id: project2_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req2))
        .await
        .unwrap();

    // Get active projects
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;

    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].id, project3_id);

    // Get deleted projects
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;

    assert_eq!(deleted_projects.len(), 2);

    // Verify project IDs are correct
    let deleted_ids: Vec<&str> = deleted_projects.iter().map(|p| p.id.as_str()).collect();
    assert!(deleted_ids.contains(&project1_id.as_str()));
    assert!(deleted_ids.contains(&project2_id.as_str()));
}

#[tokio::test]
async fn test_permanently_delete_project() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Mark the project as deleted first
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: project_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await
        .unwrap();

    // Permanently delete the project
    let permanently_delete_req = PermanentlyDeleteProjectRequest {
        project_id: project_id.clone(),
    };
    let permanently_delete_resp = server
        .permanently_delete_project(Request::new(permanently_delete_req))
        .await
        .unwrap();
    let result = permanently_delete_resp.into_inner();

    assert!(result.success);

    // Verify the project is completely gone
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server.get_project(Request::new(get_project_req)).await;

    // Should return None since project no longer exists
    assert!(get_project_resp.is_ok());
    let response = get_project_resp.unwrap().into_inner();
    assert!(response.project.is_none());
}

#[tokio::test]
async fn test_permanently_delete_project_not_marked_deleted() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project (but don't mark it as deleted)
    let project_id = create_test_project_in_db(db).await;

    // Try to permanently delete a project that's not marked as deleted
    let permanently_delete_req = PermanentlyDeleteProjectRequest {
        project_id: project_id.clone(),
    };
    let permanently_delete_resp = server
        .permanently_delete_project(Request::new(permanently_delete_req))
        .await
        .unwrap();
    let result = permanently_delete_resp.into_inner();

    // Should fail (graceful handling)
    assert!(!result.success);

    // Verify the project still exists
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    assert!(get_project_resp.into_inner().project.is_some());
}

#[tokio::test]
async fn test_mark_project_deleted_nonexistent() {
    setup("error");

    let server = create_test_server().await;

    // Try to mark a non-existent project as deleted (with invalid UUID format)
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: "non-existent-project-id".to_string(),
    };
    let mark_deleted_resp = server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await;

    // Should return an error for invalid UUID format
    assert!(mark_deleted_resp.is_err());
}

#[tokio::test]
async fn test_reactivate_project_nonexistent() {
    setup("error");

    let server = create_test_server().await;

    // Try to reactivate a non-existent project (with invalid UUID format)
    let reactivate_req = ReactivateProjectRequest {
        project_id: "non-existent-project-id".to_string(),
    };
    let reactivate_resp = server
        .reactivate_project(Request::new(reactivate_req))
        .await;

    // Should return an error for invalid UUID format
    assert!(reactivate_resp.is_err());
}

#[tokio::test]
async fn test_get_projects_by_deletion_status_pagination() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create multiple test projects
    let mut project_ids = Vec::new();
    for _ in 0..5 {
        let project_id = create_test_project_in_db(db).await;
        project_ids.push(project_id);
    }

    // Mark all projects as deleted
    for project_id in &project_ids {
        let mark_deleted_req = MarkProjectDeletedRequest {
            project_id: project_id.clone(),
        };
        server
            .mark_project_deleted(Request::new(mark_deleted_req))
            .await
            .unwrap();
    }

    // Test pagination - get first 3 projects
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(3),
        offset: Some(0),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;

    assert_eq!(deleted_projects.len(), 3);

    // Test pagination - get next 2 projects
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(3),
        offset: Some(3),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;

    assert_eq!(deleted_projects.len(), 2);

    // Test pagination - get beyond available data
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(3),
        offset: Some(6),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;

    assert_eq!(deleted_projects.len(), 0);
}

#[tokio::test]
async fn test_project_deletion_workflow() {
    setup("trace");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Verify project is initially active
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;
    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].id, project_id);

    // Mark as deleted
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: project_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await
        .unwrap();

    // Verify it's in deleted list
    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;
    assert_eq!(deleted_projects.len(), 1);
    assert_eq!(deleted_projects[0].id, project_id);

    // Verify it's not in active list
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;
    assert_eq!(active_projects.len(), 0);

    // Reactivate
    let reactivate_req = ReactivateProjectRequest {
        project_id: project_id.clone(),
    };
    server
        .reactivate_project(Request::new(reactivate_req))
        .await
        .unwrap();

    // Verify it's back in active list
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;
    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].id, project_id);

    // Mark as deleted again
    let mark_deleted_req = MarkProjectDeletedRequest {
        project_id: project_id.clone(),
    };
    server
        .mark_project_deleted(Request::new(mark_deleted_req))
        .await
        .unwrap();

    // Permanently delete
    let permanently_delete_req = PermanentlyDeleteProjectRequest {
        project_id: project_id.clone(),
    };
    server
        .permanently_delete_project(Request::new(permanently_delete_req))
        .await
        .unwrap();

    // Verify it's completely gone from both lists
    let get_active_req = GetProjectsByDeletionStatusRequest {
        is_deleted: false,
        limit: Some(10),
        offset: Some(0),
    };
    let get_active_resp = server
        .get_projects_by_deletion_status(Request::new(get_active_req))
        .await
        .unwrap();
    let active_projects = get_active_resp.into_inner().projects;
    assert_eq!(active_projects.len(), 0);

    let get_deleted_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_deleted_resp = server
        .get_projects_by_deletion_status(Request::new(get_deleted_req))
        .await
        .unwrap();
    let deleted_projects = get_deleted_resp.into_inner().projects;
    assert_eq!(deleted_projects.len(), 0);
}
