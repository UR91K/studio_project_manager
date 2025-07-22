//! Batch operations gRPC tests

use studio_project_manager::grpc::projects::*;
use studio_project_manager::grpc::collections::*;
use studio_project_manager::grpc::tags::*;
use studio_project_manager::grpc::tasks::*;
use studio_project_manager::grpc::projects::project_service_server::ProjectService;
use studio_project_manager::grpc::collections::collection_service_server::CollectionService;
use studio_project_manager::grpc::tags::tag_service_server::TagService;
use studio_project_manager::grpc::tasks::task_service_server::TaskService;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_batch_archive_projects() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create multiple test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Verify projects are initially active
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
    assert_eq!(active_projects.len(), 3);

    // Archive multiple projects
    let batch_archive_req = BatchMarkProjectsAsArchivedRequest {
        project_ids: project_ids.clone(),
        archived: true,
    };
    let batch_archive_resp = server
        .batch_mark_projects_as_archived(Request::new(batch_archive_req))
        .await
        .unwrap();
    let result = batch_archive_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 3);
    assert_eq!(result.failed_count, 0);

    // Verify projects are now archived
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

    let get_archived_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_archived_resp = server
        .get_projects_by_deletion_status(Request::new(get_archived_req))
        .await
        .unwrap();
    let archived_projects = get_archived_resp.into_inner().projects;
    assert_eq!(archived_projects.len(), 3);

    // Verify all archived project IDs match
    let archived_ids: Vec<String> = archived_projects.iter().map(|p| p.id.clone()).collect();
    for project_id in &project_ids {
        assert!(archived_ids.contains(project_id));
    }
}

#[tokio::test]
async fn test_batch_delete_projects() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create multiple test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // First archive them
    let batch_archive_req = BatchMarkProjectsAsArchivedRequest {
        project_ids: project_ids.clone(),
        archived: true,
    };
    server
        .batch_mark_projects_as_archived(Request::new(batch_archive_req))
        .await
        .unwrap();

    // Verify they're archived
    let get_archived_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_archived_resp = server
        .get_projects_by_deletion_status(Request::new(get_archived_req))
        .await
        .unwrap();
    let archived_projects = get_archived_resp.into_inner().projects;
    assert_eq!(archived_projects.len(), 3);

    // Permanently delete them
    let batch_delete_req = BatchDeleteProjectsRequest {
        project_ids: project_ids.clone(),
    };
    let batch_delete_resp = server
        .batch_delete_projects(Request::new(batch_delete_req))
        .await
        .unwrap();
    let result = batch_delete_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 3);
    assert_eq!(result.failed_count, 0);

    // Verify they're completely gone
    let get_archived_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_archived_resp = server
        .get_projects_by_deletion_status(Request::new(get_archived_req))
        .await
        .unwrap();
    let archived_projects = get_archived_resp.into_inner().projects;
    assert_eq!(archived_projects.len(), 0);

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
}

#[tokio::test]
async fn test_batch_tag_projects() {
    setup("info");

    let server = create_test_server().await;
    let db = server.db();

    // Create test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create test tags
    let tag1_req = CreateTagRequest {
        name: "Test Tag 1".to_string(),
    };
    let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
    let tag1_id = tag1_resp.into_inner().tag.unwrap().id;

    let tag2_req = CreateTagRequest {
        name: "Test Tag 2".to_string(),
    };
    let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
    let tag2_id = tag2_resp.into_inner().tag.unwrap().id;

    // Tag multiple projects with multiple tags
    let batch_tag_req = BatchTagProjectsRequest {
        project_ids: project_ids.clone(),
        tag_ids: vec![tag1_id.clone(), tag2_id.clone()],
    };
    let batch_tag_resp = server
        .batch_tag_projects(Request::new(batch_tag_req))
        .await
        .unwrap();
    let result = batch_tag_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 4);
    assert_eq!(result.failed_count, 0);

    // Verify tags were applied
    for project_id in &project_ids {
        let get_project_req = GetProjectRequest {
            project_id: project_id.clone(),
        };
        let get_project_resp = server
            .get_project(Request::new(get_project_req))
            .await
            .unwrap();
        let project = get_project_resp.into_inner().project.unwrap();

        assert_eq!(project.tags.len(), 2);
        let project_tag_ids: Vec<String> = project.tags.iter().map(|t| t.id.clone()).collect();
        assert!(project_tag_ids.contains(&tag1_id));
        assert!(project_tag_ids.contains(&tag2_id));
    }
}

#[tokio::test]
async fn test_batch_untag_projects() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create test tags
    let tag1_req = CreateTagRequest {
        name: "Test Tag 1".to_string(),
    };
    let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
    let tag1_id = tag1_resp.into_inner().tag.unwrap().id;

    let tag2_req = CreateTagRequest {
        name: "Test Tag 2".to_string(),
    };
    let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
    let tag2_id = tag2_resp.into_inner().tag.unwrap().id;

    // First tag the projects
    let batch_tag_req = BatchTagProjectsRequest {
        project_ids: project_ids.clone(),
        tag_ids: vec![tag1_id.clone(), tag2_id.clone()],
    };
    server
        .batch_tag_projects(Request::new(batch_tag_req))
        .await
        .unwrap();

    // Verify tags were applied
    for project_id in &project_ids {
        let get_project_req = GetProjectRequest {
            project_id: project_id.clone(),
        };
        let get_project_resp = server
            .get_project(Request::new(get_project_req))
            .await
            .unwrap();
        let project = get_project_resp.into_inner().project.unwrap();
        assert_eq!(project.tags.len(), 2);
    }

    // Now untag with one tag
    let batch_untag_req = BatchUntagProjectsRequest {
        project_ids: project_ids.clone(),
        tag_ids: vec![tag1_id.clone()],
    };
    let batch_untag_resp = server
        .batch_untag_projects(Request::new(batch_untag_req))
        .await
        .unwrap();
    let result = batch_untag_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 2);
    assert_eq!(result.failed_count, 0);

    // Verify only tag2 remains
    for project_id in &project_ids {
        let get_project_req = GetProjectRequest {
            project_id: project_id.clone(),
        };
        let get_project_resp = server
            .get_project(Request::new(get_project_req))
            .await
            .unwrap();
        let project = get_project_resp.into_inner().project.unwrap();

        assert_eq!(project.tags.len(), 1);
        assert_eq!(project.tags[0].id, tag2_id);
    }
}

#[tokio::test]
async fn test_batch_add_projects_to_collection() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create a test collection
    let collection_req = CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: Some("Test collection for batch operations".to_string()),
        notes: Some("Test notes".to_string()),
    };
    let collection_resp = server
        .create_collection(Request::new(collection_req))
        .await
        .unwrap();
    let collection_id = collection_resp.into_inner().collection.unwrap().id;

    // Add multiple projects to collection
    let batch_add_req = BatchAddToCollectionRequest {
        project_ids: project_ids.clone(),
        collection_id: collection_id.clone(),
    };
    let batch_add_resp = server
        .batch_add_to_collection(Request::new(batch_add_req))
        .await
        .unwrap();
    let result = batch_add_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 3);
    assert_eq!(result.failed_count, 0);

    // Verify projects were added to collection
    let get_collection_req = GetCollectionRequest {
        collection_id: collection_id.clone(),
    };
    let get_collection_resp = server
        .get_collection(Request::new(get_collection_req))
        .await
        .unwrap();
    let collection = get_collection_resp.into_inner().collection.unwrap();

    assert_eq!(collection.project_count, 3);
    let collection_project_ids: Vec<String> = collection.project_ids;
    for project_id in &project_ids {
        assert!(collection_project_ids.contains(project_id));
    }
}

#[tokio::test]
async fn test_batch_remove_projects_from_collection() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create a test collection
    let collection_req = CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: Some("Test collection for batch operations".to_string()),
        notes: Some("Test notes".to_string()),
    };
    let collection_resp = server
        .create_collection(Request::new(collection_req))
        .await
        .unwrap();
    let collection_id = collection_resp.into_inner().collection.unwrap().id;

    // First add all projects to collection
    let batch_add_req = BatchAddToCollectionRequest {
        project_ids: project_ids.clone(),
        collection_id: collection_id.clone(),
    };
    server
        .batch_add_to_collection(Request::new(batch_add_req))
        .await
        .unwrap();

    // Verify all projects are in collection
    let get_collection_req = GetCollectionRequest {
        collection_id: collection_id.clone(),
    };
    let get_collection_resp = server
        .get_collection(Request::new(get_collection_req))
        .await
        .unwrap();
    let collection = get_collection_resp.into_inner().collection.unwrap();
    assert_eq!(collection.project_count, 3);

    // Remove some projects from collection
    let projects_to_remove = vec![project_ids[0].clone(), project_ids[1].clone()];
    let batch_remove_req = BatchRemoveFromCollectionRequest {
        project_ids: projects_to_remove.clone(),
        collection_id: collection_id.clone(),
    };
    let batch_remove_resp = server
        .batch_remove_from_collection(Request::new(batch_remove_req))
        .await
        .unwrap();
    let result = batch_remove_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));

    // Verify only one project remains in collection
    let get_collection_req = GetCollectionRequest {
        collection_id: collection_id.clone(),
    };
    let get_collection_resp = server
        .get_collection(Request::new(get_collection_req))
        .await
        .unwrap();
    let collection = get_collection_resp.into_inner().collection.unwrap();

    assert_eq!(collection.project_count, 1);
    assert_eq!(collection.project_ids[0], project_ids[2]);
}

#[tokio::test]
async fn test_batch_create_collection_from_projects() {
    setup("trace");

    let server = create_test_server().await;
    let db = server.db();

    // Create test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create collection from projects
    let batch_create_req = BatchCreateCollectionFromRequest {
        project_ids: project_ids.clone(),
        collection_name: "New Collection from Batch".to_string(),
        notes: Some("Collection created from batch operation".to_string()),
        description: None,
    };
    let batch_create_resp = server
        .batch_create_collection_from(Request::new(batch_create_req))
        .await
        .unwrap();
    let result = batch_create_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert!(result.collection.is_some());

    let collection = result.collection.unwrap();
    assert_eq!(collection.name, "New Collection from Batch");
    assert_eq!(
        collection.notes,
        Some("Collection created from batch operation".to_string())
    );
    assert_eq!(collection.project_count, 2);

    // Verify all projects are in the new collection
    let collection_project_ids: Vec<String> = collection.project_ids;
    for project_id in &project_ids {
        assert!(collection_project_ids.contains(project_id));
    }
}

#[tokio::test]
async fn test_batch_update_task_status() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Create multiple tasks
    let task1_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "First task".to_string(),
    };

    let task1_resp = server.create_task(Request::new(task1_req)).await.unwrap();
    let task1_id = task1_resp.into_inner().task.unwrap().id;

    let task2_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Second task".to_string(),
    };
    let task2_resp = server.create_task(Request::new(task2_req)).await.unwrap();
    let task2_id = task2_resp.into_inner().task.unwrap().id;

    let task3_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Third task".to_string(),
    };
    let task3_resp = server.create_task(Request::new(task3_req)).await.unwrap();
    let task3_id = task3_resp.into_inner().task.unwrap().id;

    // Verify initial statuses
    let get_tasks_req = GetProjectTasksRequest {
        project_id: project_id.clone(),
    };
    let get_tasks_resp = server
        .get_project_tasks(Request::new(get_tasks_req))
        .await
        .unwrap();
    let tasks = get_tasks_resp.into_inner().tasks;
    assert_eq!(tasks.len(), 3);

    // Update status of multiple tasks
    let task_ids = vec![task1_id, task2_id, task3_id];
    let batch_update_req = BatchUpdateTaskStatusRequest {
        task_ids: task_ids.clone(),
        completed: true,
    };
    let batch_update_resp = server
        .batch_update_task_status(Request::new(batch_update_req))
        .await
        .unwrap();
    let result = batch_update_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 3);
    assert_eq!(result.failed_count, 0);

    // Verify all tasks are now completed
    let get_tasks_req = GetProjectTasksRequest {
        project_id: project_id.clone(),
    };
    let get_tasks_resp = server
        .get_project_tasks(Request::new(get_tasks_req))
        .await
        .unwrap();
    let tasks = get_tasks_resp.into_inner().tasks;

    for task in tasks {
        assert_eq!(task.completed, true);
    }
}

#[tokio::test]
async fn test_batch_delete_tasks() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let project_id = create_test_project_in_db(db).await;

    // Create multiple tasks
    let task1_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "First task".to_string(),
    };
    let task1_resp = server.create_task(Request::new(task1_req)).await.unwrap();
    let task1_id = task1_resp.into_inner().task.unwrap().id;

    let task2_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Second task".to_string(),
    };
    let task2_resp = server.create_task(Request::new(task2_req)).await.unwrap();
    let task2_id = task2_resp.into_inner().task.unwrap().id;

    let task3_req = CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Third task".to_string(),
    };
    let task3_resp = server.create_task(Request::new(task3_req)).await.unwrap();
    let task3_id = task3_resp.into_inner().task.unwrap().id;

    // Verify initial task count
    let get_tasks_req = GetProjectTasksRequest {
        project_id: project_id.clone(),
    };
    let get_tasks_resp = server
        .get_project_tasks(Request::new(get_tasks_req))
        .await
        .unwrap();
    let tasks = get_tasks_resp.into_inner().tasks;
    assert_eq!(tasks.len(), 3);

    // Delete multiple tasks
    let task_ids = vec![task1_id, task2_id];
    let batch_delete_req = BatchDeleteTasksRequest {
        task_ids: task_ids.clone(),
    };
    let batch_delete_resp = server
        .batch_delete_tasks(Request::new(batch_delete_req))
        .await
        .unwrap();
    let result = batch_delete_resp.into_inner();

    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 2);
    assert_eq!(result.failed_count, 0);

    // Verify only one task remains
    let get_tasks_req = GetProjectTasksRequest {
        project_id: project_id.clone(),
    };
    let get_tasks_resp = server
        .get_project_tasks(Request::new(get_tasks_req))
        .await
        .unwrap();
    let tasks = get_tasks_resp.into_inner().tasks;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task3_id);
}

#[tokio::test]
async fn test_batch_operations_with_mixed_success_failure() {
    setup("trace");

    let server = create_test_server().await;
    let db = server.db();

    // Create one valid project
    let valid_project_id = create_test_project_in_db(db).await;

    // Try to tag with non-existent project IDs and valid ones
    let mixed_project_ids = vec![
        valid_project_id.clone(),
        "non-existent-project-1".to_string(),
        "non-existent-project-2".to_string(),
    ];

    // Create a valid tag
    let tag_req = CreateTagRequest {
        name: "Test Tag".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag_id = tag_resp.into_inner().tag.unwrap().id;

    // Try to tag mixed valid/invalid projects
    let batch_tag_req = BatchTagProjectsRequest {
        project_ids: mixed_project_ids,
        tag_ids: vec![tag_id.clone()],
    };
    let batch_tag_resp = server
        .batch_tag_projects(Request::new(batch_tag_req))
        .await
        .unwrap();
    let result = batch_tag_resp.into_inner();

    // Should have mixed success/failure results
    assert_eq!(result.successful_count, 1); // Only the valid project
    assert_eq!(result.failed_count, 2); // The two non-existent projects

    // Verify the valid project was tagged
    let get_project_req = GetProjectRequest {
        project_id: valid_project_id.clone(),
    };
    let get_project_resp = server
        .get_project(Request::new(get_project_req))
        .await
        .unwrap();
    let project = get_project_resp.into_inner().project.unwrap();
    assert_eq!(project.tags.len(), 1);
    assert_eq!(project.tags[0].id, tag_id);
}

#[tokio::test]
async fn test_batch_operations_workflow() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create multiple test projects
    let project_ids = vec![
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
        create_test_project_in_db(db).await,
    ];

    // Create tags
    let tag1_req = CreateTagRequest {
        name: "Workflow Tag 1".to_string(),
    };
    let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
    let tag1_id = tag1_resp.into_inner().tag.unwrap().id;

    let tag2_req = CreateTagRequest {
        name: "Workflow Tag 2".to_string(),
    };
    let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
    let tag2_id = tag2_resp.into_inner().tag.unwrap().id;

    // Step 1: Tag all projects
    let batch_tag_req = BatchTagProjectsRequest {
        project_ids: project_ids.clone(),
        tag_ids: vec![tag1_id.clone(), tag2_id.clone()],
    };
    let batch_tag_resp = server
        .batch_tag_projects(Request::new(batch_tag_req))
        .await
        .unwrap();
    let result = batch_tag_resp.into_inner();
    assert!(result.results.iter().all(|r| r.success));
    assert_eq!(result.successful_count, 6); // 3 projects × 2 tags

    // Step 2: Create collection from projects
    let batch_create_collection_req = BatchCreateCollectionFromRequest {
        project_ids: project_ids.clone(),
        collection_name: "Workflow Collection".to_string(),
        notes: Some("Collection created during workflow test".to_string()),
        description: None,
    };
    let batch_create_collection_resp = server
        .batch_create_collection_from(Request::new(batch_create_collection_req))
        .await
        .unwrap();
    let collection_result = batch_create_collection_resp.into_inner();
    assert!(collection_result.results.iter().all(|r| r.success));
    let collection_id = collection_result.collection.unwrap().id;

    // Step 3: Create tasks for each project
    let mut task_ids = Vec::new();
    for project_id in &project_ids {
        let task_req = CreateTaskRequest {
            project_id: project_id.clone(),
            description: format!("Task for project {}", project_id),
        };
        let task_resp = server.create_task(Request::new(task_req)).await.unwrap();
        let task_id = task_resp.into_inner().task.unwrap().id;
        task_ids.push(task_id);
    }

    // Step 4: Update all task statuses
    let batch_update_tasks_req = BatchUpdateTaskStatusRequest {
        task_ids: task_ids.clone(),
        completed: false,
    };
    let batch_update_tasks_resp = server
        .batch_update_task_status(Request::new(batch_update_tasks_req))
        .await
        .unwrap();
    let task_result = batch_update_tasks_resp.into_inner();
    assert!(task_result.results.iter().all(|r| r.success));
    assert_eq!(task_result.successful_count, 3);

    // Step 5: Archive some projects
    let projects_to_archive = vec![project_ids[0].clone(), project_ids[1].clone()];
    let batch_archive_req = BatchMarkProjectsAsArchivedRequest {
        project_ids: projects_to_archive.clone(),
        archived: true,
    };
    let batch_archive_resp = server
        .batch_mark_projects_as_archived(Request::new(batch_archive_req))
        .await
        .unwrap();
    let archive_result = batch_archive_resp.into_inner();
    assert!(archive_result.results.iter().all(|r| r.success));
    assert_eq!(archive_result.successful_count, 2);

    // Step 6: Remove some projects from collection
    let batch_remove_req = BatchRemoveFromCollectionRequest {
        project_ids: vec![project_ids[2].clone()],
        collection_id: collection_id.clone(),
    };
    let batch_remove_resp = server
        .batch_remove_from_collection(Request::new(batch_remove_req))
        .await
        .unwrap();
    let remove_result = batch_remove_resp.into_inner();
    assert!(remove_result.results.iter().all(|r| r.success));
    assert_eq!(remove_result.successful_count, 1);

    // Step 7: Untag some projects
    let batch_untag_req = BatchUntagProjectsRequest {
        project_ids: vec![project_ids[0].clone()],
        tag_ids: vec![tag1_id.clone()],
    };
    let batch_untag_resp = server
        .batch_untag_projects(Request::new(batch_untag_req))
        .await
        .unwrap();
    let untag_result = batch_untag_resp.into_inner();
    assert!(untag_result.results.iter().all(|r| r.success));
    assert_eq!(untag_result.successful_count, 1);

    // Verify final state
    // Check archived projects
    let get_archived_req = GetProjectsByDeletionStatusRequest {
        is_deleted: true,
        limit: Some(10),
        offset: Some(0),
    };
    let get_archived_resp = server
        .get_projects_by_deletion_status(Request::new(get_archived_req))
        .await
        .unwrap();
    let archived_projects = get_archived_resp.into_inner().projects;
    assert_eq!(archived_projects.len(), 2);

    // Check collection has 2 projects (3 added - 1 removed)
    let get_collection_req = GetCollectionRequest {
        collection_id: collection_id.clone(),
    };
    let get_collection_resp = server
        .get_collection(Request::new(get_collection_req))
        .await
        .unwrap();
    let collection = get_collection_resp.into_inner().collection.unwrap();
    assert_eq!(collection.project_count, 2);

    // Check tasks are in progress
    for project_id in &project_ids {
        let get_tasks_req = GetProjectTasksRequest {
            project_id: project_id.clone(),
        };
        let get_tasks_resp = server
            .get_project_tasks(Request::new(get_tasks_req))
            .await
            .unwrap();
        let tasks = get_tasks_resp.into_inner().tasks;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].completed, false);
    }
}
