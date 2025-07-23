//! Scanning service tests

use crate::grpc::*;

#[tokio::test]
async fn test_add_multiple_projects() {
    let server = create_test_server().await;
    
    // Test with empty list
    let request = Request::new(AddMultipleProjectsRequest {
        file_paths: vec![],
    });
    let response = server.add_multiple_projects(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return success (no projects to import, so no failures)
    assert!(response.success);
    assert_eq!(response.total_requested, 0);
    assert_eq!(response.successful_imports, 0);
    assert_eq!(response.failed_imports, 0);
    assert!(response.projects.is_empty());
    assert!(response.failed_paths.is_empty());
    assert!(response.error_messages.is_empty());
}

#[tokio::test]
async fn test_add_multiple_projects_with_invalid_paths() {
    let server = create_test_server().await;
    
    // Create a temporary file with wrong extension to test extension validation
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_file.txt");
    std::fs::write(&temp_file, b"dummy content").expect("Failed to create temp file");
    
    // Test with invalid paths
    let request = Request::new(AddMultipleProjectsRequest {
        file_paths: vec![
            "nonexistent_file.als".to_string(),
            temp_file.to_string_lossy().to_string(),
        ],
    });
    let response = server.add_multiple_projects(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return failure (all imports failed)
    assert!(!response.success);
    assert_eq!(response.total_requested, 2);
    assert_eq!(response.successful_imports, 0);
    assert_eq!(response.failed_imports, 2);
    assert!(response.projects.is_empty());
    assert_eq!(response.failed_paths.len(), 2);
    assert_eq!(response.error_messages.len(), 2);
    
    // Check error messages
    assert!(response.error_messages.contains(&"File does not exist".to_string()));
    assert!(response.error_messages.contains(&"File must have .als extension".to_string()));
    
    // Clean up temp file
    let _ = std::fs::remove_file(temp_file);
}

#[tokio::test]
async fn test_add_multiple_projects_mixed_success_failure() {
    let server = create_test_server().await;
    
    // Test with mixed valid and invalid paths
    let request = Request::new(AddMultipleProjectsRequest {
        file_paths: vec![
            "nonexistent_file.als".to_string(),
            "another_nonexistent_file.als".to_string(),
        ],
    });
    let response = server.add_multiple_projects(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return failure (all imports failed)
    assert!(!response.success);
    assert_eq!(response.total_requested, 2);
    assert_eq!(response.successful_imports, 0);
    assert_eq!(response.failed_imports, 2);
    assert_eq!(response.projects.len(), 0);
    assert_eq!(response.failed_paths.len(), 2);
    assert_eq!(response.error_messages.len(), 2);
    
    // Check error messages
    assert!(response.error_messages.contains(&"File does not exist".to_string()));
} 