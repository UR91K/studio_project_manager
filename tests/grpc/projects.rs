//! Project-related gRPC tests

use studio_project_manager::grpc::proto::studio_project_manager_server::StudioProjectManager;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_update_project_name() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Get the project to verify initial name
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server.get_project(Request::new(get_project_req)).await.unwrap();
    let project = get_project_resp.into_inner().project.unwrap();
    let original_name = project.name;
    
    // Update the project name
    let new_name = "My Custom Project Alias";
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: new_name.to_string(),
    };
    let update_resp = server.update_project_name(Request::new(update_req)).await.unwrap();
    let result = update_resp.into_inner();
    
    assert!(result.success);
    
    // Verify the name was updated
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server.get_project(Request::new(get_project_req)).await.unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();
    
    assert_eq!(updated_project.name, new_name);
    assert_ne!(updated_project.name, original_name);
}

#[tokio::test]
async fn test_update_project_name_nonexistent_project() {
    setup("debug");
    
    let server = create_test_server().await;
    
    // Try to update a non-existent project
    let update_req = UpdateProjectNameRequest {
        project_id: "non-existent-project-id".to_string(),
        name: "New Name".to_string(),
    };
    let update_resp = server.update_project_name(Request::new(update_req)).await.unwrap();
    let result = update_resp.into_inner();
    
    // Should succeed but have no effect (graceful handling of non-existent projects)
    assert!(result.success);
}

#[tokio::test]
async fn test_update_project_name_empty_string() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Update with empty string (should be allowed)
    let update_req = UpdateProjectNameRequest {
        project_id: project_id.clone(),
        name: "".to_string(),
    };
    let update_resp = server.update_project_name(Request::new(update_req)).await.unwrap();
    let result = update_resp.into_inner();
    
    assert!(result.success);
    
    // Verify the name was updated to empty string
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server.get_project(Request::new(get_project_req)).await.unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();
    
    assert_eq!(updated_project.name, "");
}

#[tokio::test]
async fn test_update_project_name_special_characters() {
    setup("debug");
    
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
    let update_resp = server.update_project_name(Request::new(update_req)).await.unwrap();
    let result = update_resp.into_inner();
    
    assert!(result.success);
    
    // Verify the name was updated correctly with special characters
    let get_project_req = GetProjectRequest {
        project_id: project_id.clone(),
    };
    let get_project_resp = server.get_project(Request::new(get_project_req)).await.unwrap();
    let updated_project = get_project_resp.into_inner().project.unwrap();
    
    assert_eq!(updated_project.name, new_name);
}

#[tokio::test]
async fn test_update_project_name_persistence() {
    setup("debug");
    
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
    let update_resp = server.update_project_name(Request::new(update_req)).await.unwrap();
    assert!(update_resp.into_inner().success);
    
    // Note: Since we're using in-memory databases, we'll verify persistence 
    // by querying the same server multiple times rather than creating a new instance
    
    // Verify the name persists through multiple get operations
    for _ in 0..3 {
        let get_project_req = GetProjectRequest {
            project_id: project_id.clone(),
        };
        let get_project_resp = server.get_project(Request::new(get_project_req)).await.unwrap();
        let project = get_project_resp.into_inner().project.unwrap();
        
        assert_eq!(project.name, new_name);
    }
}