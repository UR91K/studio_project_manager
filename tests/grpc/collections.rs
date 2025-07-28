//! Collections-related gRPC tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following tests from src/grpc/server.rs:
//! - test_get_collections_empty() (line ~831)
//! - test_create_collection() (line ~842)
//! - test_get_collections_with_data() (line ~857)
//! - test_update_collection() (line ~888)
//! - test_update_collection_partial() (line ~918)
//! - test_update_nonexistent_collection() (line ~934)
//! - test_add_project_to_collection() (line ~971)
//! - test_add_multiple_projects_to_collection() (line ~1020)
//! - test_remove_project_from_collection() (line ~1063)
//! - test_remove_project_maintains_order() (line ~1111)
//! - test_add_project_to_nonexistent_collection() (line ~1128)
//! - test_remove_project_from_nonexistent_collection() (line ~1144)
//! - test_collection_timestamps() (line ~1191)

use crate::common::setup;

use super::*;
// use crate::common::setup;
use studio_project_manager::grpc::collections::collection_service_server::CollectionService;

#[tokio::test]
async fn test_get_collections_empty() {
    setup("error");
    let server = create_test_server().await;
    let request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });

    let response = server.get_collections(request).await.unwrap();
    let collections = response.into_inner().collections;

    assert_eq!(collections.len(), 0);
}

#[tokio::test]
async fn test_create_collection() {
    setup("error");
    let server = create_test_server().await;
    let request = Request::new(CreateCollectionRequest {
        name: "My Test Collection".to_string(),
        description: Some("A collection for testing".to_string()),
        notes: Some("Test notes".to_string()),
    });

    let response = server.create_collection(request).await.unwrap();
    let collection = response
        .into_inner()
        .collection
        .expect("Collection should be present");

    assert_eq!(collection.name, "My Test Collection");
    assert_eq!(
        collection.description,
        Some("A collection for testing".to_string())
    );
    assert_eq!(collection.notes, Some("Test notes".to_string()));
    assert!(!collection.id.is_empty());
    assert!(collection.created_at > 0);
    assert!(collection.modified_at > 0);
    assert_eq!(collection.project_ids.len(), 0);
}

#[tokio::test]
async fn test_get_collections_with_data() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: Some("Test Description".to_string()),
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let created_collection = create_response.into_inner().collection.unwrap();

    // Get all collections
    let get_request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });
    let get_response = server.get_collections(get_request).await.unwrap();
    let collections = get_response.into_inner().collections;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].id, created_collection.id);
    assert_eq!(collections[0].name, "Test Collection");
    assert_eq!(
        collections[0].description,
        Some("Test Description".to_string())
    );
    assert_eq!(collections[0].notes, None);
}

#[tokio::test]
async fn test_update_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Original Name".to_string(),
        description: Some("Original Description".to_string()),
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Update the collection
    let update_request = Request::new(UpdateCollectionRequest {
        collection_id: collection_id.clone(),
        name: Some("Updated Name".to_string()),
        description: Some("Updated Description".to_string()),
        notes: Some("Updated Notes".to_string()),
    });

    let update_response = server.update_collection(update_request).await.unwrap();
    let updated_collection = update_response.into_inner().collection.unwrap();

    assert_eq!(updated_collection.id, collection_id);
    assert_eq!(updated_collection.name, "Updated Name");
    assert_eq!(
        updated_collection.description,
        Some("Updated Description".to_string())
    );
    assert_eq!(updated_collection.notes, Some("Updated Notes".to_string()));
}

#[tokio::test]
async fn test_update_collection_partial() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Original Name".to_string(),
        description: Some("Original Description".to_string()),
        notes: Some("Original Notes".to_string()),
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Update only the name
    let update_request = Request::new(UpdateCollectionRequest {
        collection_id: collection_id.clone(),
        name: Some("Updated Name Only".to_string()),
        description: None,
        notes: None,
    });

    let update_response = server.update_collection(update_request).await.unwrap();
    let updated_collection = update_response.into_inner().collection.unwrap();

    assert_eq!(updated_collection.name, "Updated Name Only");
    assert_eq!(
        updated_collection.description,
        Some("Original Description".to_string())
    );
    assert_eq!(updated_collection.notes, Some("Original Notes".to_string()));
}

#[tokio::test]
async fn test_update_nonexistent_collection() {
    setup("error");
    let server = create_test_server().await;

    let update_request = Request::new(UpdateCollectionRequest {
        collection_id: Uuid::new_v4().to_string(),
        name: Some("Should Fail".to_string()),
        description: None,
        notes: None,
    });

    let result = server.update_collection(update_request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), Code::NotFound);
}

#[tokio::test]
async fn test_add_project_to_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Create a test project
    let project_id = create_test_project_in_db(server.db()).await;

    // Add project to collection
    let add_request = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id.clone(),
        position: None,
    });

    let add_response = server.add_project_to_collection(add_request).await.unwrap();
    assert!(add_response.into_inner().success);

    // Verify the project was added
    let get_request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });
    let get_response = server.get_collections(get_request).await.unwrap();
    let collections = get_response.into_inner().collections;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].project_ids.len(), 1);
    assert_eq!(collections[0].project_ids[0], project_id);
}

#[tokio::test]
async fn test_add_multiple_projects_to_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Multi-Project Collection".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Create multiple test projects
    let project_id1 = create_test_project_in_db(server.db()).await;
    let project_id2 = create_test_project_in_db(server.db()).await;

    // Add first project
    let add_request1 = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id1.clone(),
        position: None,
    });

    let add_response1 = server
        .add_project_to_collection(add_request1)
        .await
        .unwrap();
    assert!(add_response1.into_inner().success);

    // Add second project
    let add_request2 = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id2.clone(),
        position: None,
    });

    let add_response2 = server
        .add_project_to_collection(add_request2)
        .await
        .unwrap();
    assert!(add_response2.into_inner().success);

    // Verify both projects were added in order
    let get_request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });
    let get_response = server.get_collections(get_request).await.unwrap();
    let collections = get_response.into_inner().collections;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].project_ids.len(), 2);
    assert_eq!(collections[0].project_ids[0], project_id1);
    assert_eq!(collections[0].project_ids[1], project_id2);
}

#[tokio::test]
async fn test_remove_project_from_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Create and add a test project
    let project_id = create_test_project_in_db(server.db()).await;

    let add_request = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id.clone(),
        position: None,
    });

    server.add_project_to_collection(add_request).await.unwrap();

    // Remove the project
    let remove_request = Request::new(RemoveProjectFromCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id.clone(),
    });

    let remove_response = server
        .remove_project_from_collection(remove_request)
        .await
        .unwrap();
    assert!(remove_response.into_inner().success);

    // Verify the project was removed
    let get_request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });
    let get_response = server.get_collections(get_request).await.unwrap();
    let collections = get_response.into_inner().collections;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].project_ids.len(), 0);
}

#[tokio::test]
async fn test_remove_project_maintains_order() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Order Test Collection".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Create three test projects
    let project_id1 = create_test_project_in_db(server.db()).await;
    let project_id2 = create_test_project_in_db(server.db()).await;
    let project_id3 = create_test_project_in_db(server.db()).await;

    // Add all projects
    for project_id in [&project_id1, &project_id2, &project_id3] {
        let add_request = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id.clone(),
            position: None,
        });
        server.add_project_to_collection(add_request).await.unwrap();
    }

    // Remove the middle project
    let remove_request = Request::new(RemoveProjectFromCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project_id2.clone(),
    });

    server
        .remove_project_from_collection(remove_request)
        .await
        .unwrap();

    // Verify the remaining projects maintain their relative order
    let get_request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: None,
        sort_desc: None,
    });
    let get_response = server.get_collections(get_request).await.unwrap();
    let collections = get_response.into_inner().collections;

    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].project_ids.len(), 2);
    assert_eq!(collections[0].project_ids[0], project_id1);
    assert_eq!(collections[0].project_ids[1], project_id3);
}

#[tokio::test]
async fn test_add_project_to_nonexistent_collection() {
    setup("error");
    let server = create_test_server().await;

    let project_id = create_test_project_in_db(server.db()).await;

    let add_request = Request::new(AddProjectToCollectionRequest {
        collection_id: Uuid::new_v4().to_string(),
        project_id,
        position: None,
    });

    let result = server.add_project_to_collection(add_request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), Code::Internal);
}

#[tokio::test]
async fn test_remove_project_from_nonexistent_collection() {
    setup("error");
    let server = create_test_server().await;

    let project_id = create_test_project_in_db(server.db()).await;

    let remove_request = Request::new(RemoveProjectFromCollectionRequest {
        collection_id: Uuid::new_v4().to_string(),
        project_id,
    });

    let result = server.remove_project_from_collection(remove_request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), Code::Internal);
}

#[tokio::test]
async fn test_collection_timestamps() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection and record creation time
    let before_create = chrono::Utc::now().timestamp();

    let create_request = Request::new(CreateCollectionRequest {
        name: "Timestamp Test".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection = create_response.into_inner().collection.unwrap();
    let after_create = chrono::Utc::now().timestamp();

    // Verify creation timestamps
    assert!(collection.created_at >= before_create);
    assert!(collection.created_at <= after_create);
    assert!(collection.modified_at >= before_create);
    assert!(collection.modified_at <= after_create);

    // Wait a bit then update (ensure timestamp difference)
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
    let before_update = chrono::Utc::now().timestamp();

    let update_request = Request::new(UpdateCollectionRequest {
        collection_id: collection.id.clone(),
        name: Some("Updated Name".to_string()),
        description: None,
        notes: None,
    });

    let update_response = server.update_collection(update_request).await.unwrap();
    let updated_collection = update_response.into_inner().collection.unwrap();
    let after_update = chrono::Utc::now().timestamp();

    // Verify update timestamps
    assert_eq!(updated_collection.created_at, collection.created_at); // Should not change
    assert!(updated_collection.modified_at >= before_update);
    assert!(updated_collection.modified_at <= after_update);
    assert!(updated_collection.modified_at > collection.modified_at); // Should be newer
}

#[tokio::test]
async fn test_get_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection for Get".to_string(),
        description: Some("Test Description".to_string()),
        notes: Some("Test Notes".to_string()),
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let created_collection = create_response.into_inner().collection.unwrap();
    let collection_id = created_collection.id.clone();

    // Get the collection by ID
    let get_request = Request::new(GetCollectionRequest {
        collection_id: collection_id.clone(),
    });

    let get_response = server.get_collection(get_request).await.unwrap();
    let collection = get_response
        .into_inner()
        .collection
        .expect("Collection should be present");

    // Verify the collection details
    assert_eq!(collection.id, collection_id);
    assert_eq!(collection.name, "Test Collection for Get");
    assert_eq!(collection.description, Some("Test Description".to_string()));
    assert_eq!(collection.notes, Some("Test Notes".to_string()));
    assert_eq!(collection.project_ids.len(), 0);
    assert!(collection.created_at > 0);
    assert!(collection.modified_at > 0);
}

#[tokio::test]
async fn test_get_collection_not_found() {
    setup("error");
    let server = create_test_server().await;

    // Try to get a non-existent collection
    let get_request = Request::new(GetCollectionRequest {
        collection_id: Uuid::new_v4().to_string(),
    });

    let get_response = server.get_collection(get_request).await.unwrap();
    let collection = get_response.into_inner().collection;

    // Verify the collection is None
    assert!(collection.is_none());
}

#[tokio::test]
async fn test_get_collection_with_projects() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Collection with Projects".to_string(),
        description: None,
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let collection_id = create_response.into_inner().collection.unwrap().id;

    // Create and add test projects
    let project_id1 = create_test_project_in_db(server.db()).await;
    let project_id2 = create_test_project_in_db(server.db()).await;

    // Add projects to collection
    for project_id in [&project_id1, &project_id2] {
        let add_request = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id.clone(),
            position: None,
        });
        server.add_project_to_collection(add_request).await.unwrap();
    }

    // Get the collection by ID
    let get_request = Request::new(GetCollectionRequest {
        collection_id: collection_id.clone(),
    });

    let get_response = server.get_collection(get_request).await.unwrap();
    let collection = get_response
        .into_inner()
        .collection
        .expect("Collection should be present");

    // Verify the collection has the projects
    assert_eq!(collection.id, collection_id);
    assert_eq!(collection.name, "Collection with Projects");
    assert_eq!(collection.project_ids.len(), 2);
    assert!(collection.project_ids.contains(&project_id1));
    assert!(collection.project_ids.contains(&project_id2));
    assert_eq!(collection.project_count, 2);
}

#[tokio::test]
async fn test_reorder_collection() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Reorder Test Collection".to_string(),
        description: Some("Testing reordering".to_string()),
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let created_collection = create_response.into_inner().collection.unwrap();
    let collection_id = created_collection.id.clone();

    // Create some test projects
    let project1_id = create_test_project_in_db(server.db()).await;
    let project2_id = create_test_project_in_db(server.db()).await;
    let project3_id = create_test_project_in_db(server.db()).await;

    // Add projects to collection
    let add_request1 = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project1_id.clone(),
        position: None,
    });
    server.add_project_to_collection(add_request1).await.unwrap();

    let add_request2 = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project2_id.clone(),
        position: None,
    });
    server.add_project_to_collection(add_request2).await.unwrap();

    let add_request3 = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project3_id.clone(),
        position: None,
    });
    server.add_project_to_collection(add_request3).await.unwrap();

    // Get collection to verify initial order
    let get_request = Request::new(GetCollectionRequest {
        collection_id: collection_id.clone(),
    });
    let get_response = server.get_collection(get_request).await.unwrap();
    let collection = get_response.into_inner().collection.unwrap();
    
    let initial_order = collection.project_ids.clone();
    assert_eq!(initial_order.len(), 3);
    assert_eq!(initial_order[0], project1_id);
    assert_eq!(initial_order[1], project2_id);
    assert_eq!(initial_order[2], project3_id);

    // Reorder the collection (reverse the order)
    let reorder_request = Request::new(ReorderCollectionRequest {
        collection_id: collection_id.clone(),
        project_ids: vec![
            project3_id.clone(),
            project2_id.clone(),
            project1_id.clone(),
        ],
    });

    let reorder_response = server.reorder_collection(reorder_request).await.unwrap();
    let reorder_result = reorder_response.into_inner();
    assert!(reorder_result.success);

    // Get collection again to verify new order
    let get_request = Request::new(GetCollectionRequest {
        collection_id: collection_id.clone(),
    });
    let get_response = server.get_collection(get_request).await.unwrap();
    let collection = get_response.into_inner().collection.unwrap();
    
    let new_order = collection.project_ids;
    assert_eq!(new_order.len(), 3);
    assert_eq!(new_order[0], project3_id);
    assert_eq!(new_order[1], project2_id);
    assert_eq!(new_order[2], project1_id);
}

#[tokio::test]
async fn test_reorder_collection_invalid_project_ids() {
    setup("error");
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Invalid Reorder Test Collection".to_string(),
        description: Some("Testing invalid reordering".to_string()),
        notes: None,
    });

    let create_response = server.create_collection(create_request).await.unwrap();
    let created_collection = create_response.into_inner().collection.unwrap();
    let collection_id = created_collection.id.clone();

    // Create test projects
    let project1_id = create_test_project_in_db(server.db()).await;
    let project2_id = create_test_project_in_db(server.db()).await;

    // Add only one project to collection
    let add_request = Request::new(AddProjectToCollectionRequest {
        collection_id: collection_id.clone(),
        project_id: project1_id.clone(),
        position: None,
    });
    server.add_project_to_collection(add_request).await.unwrap();

    // Try to reorder with project IDs that don't match the collection
    let reorder_request = Request::new(ReorderCollectionRequest {
        collection_id: collection_id.clone(),
        project_ids: vec![
            project1_id.clone(),
            project2_id.clone(), // This project is not in the collection
        ],
    });

    let reorder_response = server.reorder_collection(reorder_request).await;
    assert!(reorder_response.is_err());
    
    let status = reorder_response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("Project IDs must match exactly"));
}

#[tokio::test]
async fn test_reorder_collection_nonexistent_collection() {
    setup("error");
    let server = create_test_server().await;

    // Try to reorder a collection that doesn't exist
    let reorder_request = Request::new(ReorderCollectionRequest {
        collection_id: "nonexistent-collection-id".to_string(),
        project_ids: vec!["project1".to_string(), "project2".to_string()],
    });

    let reorder_response = server.reorder_collection(reorder_request).await;
    assert!(reorder_response.is_err());
    
    let status = reorder_response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
    assert!(status.message().contains("Collection not found"));
}

#[tokio::test]
async fn test_get_collections_pagination() {
    setup("error");
    let server = create_test_server().await;

    // Create multiple collections
    for i in 0..5 {
        let create_request = Request::new(CreateCollectionRequest {
            name: format!("Collection {}", i),
            description: Some(format!("Description {}", i)),
            notes: None,
        });
        server.create_collection(create_request).await.unwrap();
    }

    // Test pagination with limit
    let request = Request::new(GetCollectionsRequest {
        limit: Some(3),
        offset: None,
        sort_by: None,
        sort_desc: None,
    });

    let response = server.get_collections(request).await.unwrap();
    let collections_response = response.into_inner();
    
    assert_eq!(collections_response.collections.len(), 3);
    assert_eq!(collections_response.total_count, 5);

    // Test pagination with offset
    let request = Request::new(GetCollectionsRequest {
        limit: Some(2),
        offset: Some(2),
        sort_by: None,
        sort_desc: None,
    });

    let response = server.get_collections(request).await.unwrap();
    let collections_response = response.into_inner();
    
    assert_eq!(collections_response.collections.len(), 2);
    assert_eq!(collections_response.total_count, 5);

    // Test sorting
    let request = Request::new(GetCollectionsRequest {
        limit: None,
        offset: None,
        sort_by: Some("name".to_string()),
        sort_desc: Some(true),
    });

    let response = server.get_collections(request).await.unwrap();
    let collections_response = response.into_inner();
    
    assert_eq!(collections_response.collections.len(), 5);
    assert_eq!(collections_response.total_count, 5);
    
    // Verify descending order by name
    let names: Vec<&str> = collections_response.collections.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["Collection 4", "Collection 3", "Collection 2", "Collection 1", "Collection 0"]);
}

#[tokio::test]
async fn test_search_collections() {
    setup("error");
    let server = create_test_server().await;

    // Create multiple collections with different names and descriptions
    let create_requests = vec![
        CreateCollectionRequest {
            name: "Electronic Music".to_string(),
            description: Some("Collection of electronic music projects".to_string()),
            notes: Some("EDM, techno, house".to_string()),
        },
        CreateCollectionRequest {
            name: "Rock Band Projects".to_string(),
            description: Some("Rock and alternative music".to_string()),
            notes: None,
        },
        CreateCollectionRequest {
            name: "Jazz Standards".to_string(),
            description: Some("Jazz music collection".to_string()),
            notes: Some("Traditional jazz standards".to_string()),
        },
        CreateCollectionRequest {
            name: "Film Scoring".to_string(),
            description: Some("Film and video game music".to_string()),
            notes: Some("Orchestral and cinematic".to_string()),
        },
    ];

    for create_request in create_requests {
        let request = Request::new(create_request);
        server.create_collection(request).await.unwrap();
    }

    // Test search by name
    let search_request = Request::new(SearchCollectionsRequest {
        query: "Electronic".to_string(),
        limit: None,
        offset: None,
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 1);
    assert_eq!(search_response.total_count, 1);
    assert_eq!(search_response.collections[0].name, "Electronic Music");

    // Test search by description
    let search_request = Request::new(SearchCollectionsRequest {
        query: "jazz".to_string(),
        limit: None,
        offset: None,
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 1);
    assert_eq!(search_response.total_count, 1);
    assert_eq!(search_response.collections[0].name, "Jazz Standards");

    // Test search by notes
    let search_request = Request::new(SearchCollectionsRequest {
        query: "orchestral".to_string(),
        limit: None,
        offset: None,
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 1);
    assert_eq!(search_response.total_count, 1);
    assert_eq!(search_response.collections[0].name, "Film Scoring");

    // Test search with pagination
    let search_request = Request::new(SearchCollectionsRequest {
        query: "music".to_string(),
        limit: Some(2),
        offset: None,
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 2);
    assert_eq!(search_response.total_count, 4); // "Electronic Music", "Rock Band Projects", "Jazz Standards", "Film Scoring"

    // Test search with offset
    let search_request = Request::new(SearchCollectionsRequest {
        query: "music".to_string(),
        limit: Some(1),
        offset: Some(1),
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 1);
    assert_eq!(search_response.total_count, 4);

    // Test search with no results
    let search_request = Request::new(SearchCollectionsRequest {
        query: "nonexistent".to_string(),
        limit: None,
        offset: None,
    });

    let response = server.search_collections(search_request).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.collections.len(), 0);
    assert_eq!(search_response.total_count, 0);
}
