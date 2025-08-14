//! Task service tests

use crate::grpc::*;
use crate::grpc::server_setup::{setup_test_server, create_test_project};
use seula::grpc::tasks::*;
use seula::grpc::tasks::task_service_server::TaskService;

#[tokio::test]
async fn test_search_tasks() {
    let (server, _db) = setup_test_server().await;

    // Create a test project
    let project_id = create_test_project(&server, "Test Project", "/path/to/project.als").await;

    // Create some test tasks
    let task1_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Finish the intro".to_string(),
    });
    let task1_response = server.create_task(task1_req).await.unwrap();
    let _task1 = task1_response.into_inner().task.unwrap();

    let task2_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Mix the vocals".to_string(),
    });
    let task2_response = server.create_task(task2_req).await.unwrap();
    let task2 = task2_response.into_inner().task.unwrap();

    let task3_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Master the track".to_string(),
    });
    let task3_response = server.create_task(task3_req).await.unwrap();
    let _task3 = task3_response.into_inner().task.unwrap();

    // Mark one task as completed
    let complete_req = Request::new(UpdateTaskRequest {
        task_id: task2.id.clone(),
        description: None,
        completed: Some(true),
    });
    server.update_task(complete_req).await.unwrap();

    // Test searching for tasks with "intro"
    let search_req = Request::new(SearchTasksRequest {
        project_id: project_id.clone(),
        query: "intro".to_string(),
        limit: None,
        offset: None,
        completed_only: None,
        pending_only: None,
    });
    let search_response = server.search_tasks(search_req).await.unwrap();
    let search_result = search_response.into_inner();
    
    assert_eq!(search_result.tasks.len(), 1);
    assert_eq!(search_result.total_count, 1);
    assert_eq!(search_result.tasks[0].description, "Finish the intro");

    // Test searching for completed tasks only
    let completed_search_req = Request::new(SearchTasksRequest {
        project_id: project_id.clone(),
        query: "".to_string(), // Empty query to match all
        limit: None,
        offset: None,
        completed_only: Some(true),
        pending_only: None,
    });
    let completed_response = server.search_tasks(completed_search_req).await.unwrap();
    let completed_result = completed_response.into_inner();
    
    assert_eq!(completed_result.tasks.len(), 1);
    assert_eq!(completed_result.total_count, 1);
    assert_eq!(completed_result.tasks[0].description, "Mix the vocals");
    assert_eq!(completed_result.tasks[0].completed, true);

    // Test searching for pending tasks only
    let pending_search_req = Request::new(SearchTasksRequest {
        project_id: project_id.clone(),
        query: "".to_string(), // Empty query to match all
        limit: None,
        offset: None,
        completed_only: None,
        pending_only: Some(true),
    });
    let pending_response = server.search_tasks(pending_search_req).await.unwrap();
    let pending_result = pending_response.into_inner();
    
    assert_eq!(pending_result.tasks.len(), 2);
    assert_eq!(pending_result.total_count, 2);
    assert!(pending_result.tasks.iter().all(|task| !task.completed));
}

#[tokio::test]
async fn test_get_task_statistics() {
    let (server, _db) = setup_test_server().await;

    // Create a test project
    let project_id = create_test_project(&server, "Test Project", "/path/to/project.als").await;

    // Create some test tasks
    let task1_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Task 1".to_string(),
    });
    server.create_task(task1_req).await.unwrap();

    let task2_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Task 2".to_string(),
    });
    let task2_response = server.create_task(task2_req).await.unwrap();
    let task2 = task2_response.into_inner().task.unwrap();

    let task3_req = Request::new(CreateTaskRequest {
        project_id: project_id.clone(),
        description: "Task 3".to_string(),
    });
    let task3_response = server.create_task(task3_req).await.unwrap();
    let task3 = task3_response.into_inner().task.unwrap();

    // Mark two tasks as completed
    let complete_req1 = Request::new(UpdateTaskRequest {
        task_id: task2.id.clone(),
        description: None,
        completed: Some(true),
    });
    server.update_task(complete_req1).await.unwrap();

    let complete_req2 = Request::new(UpdateTaskRequest {
        task_id: task3.id.clone(),
        description: None,
        completed: Some(true),
    });
    server.update_task(complete_req2).await.unwrap();

    // Test getting statistics for the specific project
    let stats_req = Request::new(GetTaskStatisticsRequest {
        project_id: Some(project_id.clone()),
    });
    let stats_response = server.get_task_statistics(stats_req).await.unwrap();
    let stats = stats_response.into_inner().statistics.unwrap();

    assert_eq!(stats.total_tasks, 3);
    assert_eq!(stats.completed_tasks, 2);
    assert_eq!(stats.pending_tasks, 1);
    assert!((stats.completion_rate - 66.66666666666667).abs() < 0.1); // ~66.67%

    // Test getting global statistics (all projects)
    let global_stats_req = Request::new(GetTaskStatisticsRequest {
        project_id: None,
    });
    let global_stats_response = server.get_task_statistics(global_stats_req).await.unwrap();
    let global_stats = global_stats_response.into_inner().statistics.unwrap();

    assert_eq!(global_stats.total_tasks, 3);
    assert_eq!(global_stats.completed_tasks, 2);
    assert_eq!(global_stats.pending_tasks, 1);
    assert!((global_stats.completion_rate - 66.66666666666667).abs() < 0.1); // ~66.67%
}

#[tokio::test]
async fn test_search_tasks_pagination() {
    let (server, _db) = setup_test_server().await;

    // Create a test project
    let project_id = create_test_project(&server, "Test Project", "/path/to/project.als").await;

    // Create multiple tasks
    for i in 1..=5 {
        let task_req = Request::new(CreateTaskRequest {
            project_id: project_id.clone(),
            description: format!("Task {}", i),
        });
        server.create_task(task_req).await.unwrap();
    }

    // Test pagination with limit
    let search_req = Request::new(SearchTasksRequest {
        project_id: project_id.clone(),
        query: "Task".to_string(),
        limit: Some(3),
        offset: Some(0),
        completed_only: None,
        pending_only: None,
    });
    let search_response = server.search_tasks(search_req).await.unwrap();
    let search_result = search_response.into_inner();

    assert_eq!(search_result.tasks.len(), 3);
    assert_eq!(search_result.total_count, 5);

    // Test pagination with offset
    let search_req2 = Request::new(SearchTasksRequest {
        project_id: project_id.clone(),
        query: "Task".to_string(),
        limit: Some(3),
        offset: Some(3),
        completed_only: None,
        pending_only: None,
    });
    let search_response2 = server.search_tasks(search_req2).await.unwrap();
    let search_result2 = search_response2.into_inner();

    assert_eq!(search_result2.tasks.len(), 2); // Remaining 2 tasks
    assert_eq!(search_result2.total_count, 5);
}
