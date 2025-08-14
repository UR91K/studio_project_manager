//! Statistics-related gRPC tests

use seula::grpc::system::system_service_server::SystemService;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_get_statistics_with_trace_logging() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project to ensure we have some data
    let _ = create_test_project_in_db(db).await;

    // Test the get_statistics endpoint
    let get_stats_req = GetStatisticsRequest {
        date_range: None,
        collection_ids: vec![],
        tag_ids: vec![],
        ableton_version_filter: None,
    };

    let get_stats_resp = server.get_statistics(Request::new(get_stats_req)).await;

    match get_stats_resp {
        Ok(response) => {
            let stats = response.into_inner();
            println!("[OK] Statistics endpoint succeeded!");
            println!("Total projects: {}", stats.total_projects);
            println!("Total plugins: {}", stats.total_plugins);
            println!("Total samples: {}", stats.total_samples);
            println!("Total collections: {}", stats.total_collections);
            println!("Total tags: {}", stats.total_tags);
            println!("Total tasks: {}", stats.total_tasks);
            println!("Projects per year: {:?}", stats.projects_per_year);
            println!("Projects per month: {:?}", stats.projects_per_month);
            println!("Recent activity: {:?}", stats.recent_activity);
            println!("Task completion trends: {:?}", stats.task_completion_trends);

            // Basic assertions
            assert!(stats.total_projects >= 1); // We created at least one project
        }
        Err(e) => {
            println!("[ERROR] Statistics endpoint failed with error: {:?}", e);
            panic!("Statistics endpoint should not fail: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_get_statistics_with_empty_database() {
    setup("error");

    let server = create_test_server().await;

    // Test with empty database
    let get_stats_req = GetStatisticsRequest {
        date_range: None,
        collection_ids: vec![],
        tag_ids: vec![],
        ableton_version_filter: None,
    };

    let get_stats_resp = server.get_statistics(Request::new(get_stats_req)).await;

    match get_stats_resp {
        Ok(response) => {
            let stats = response.into_inner();
            println!("[OK] Statistics endpoint succeeded with empty database!");
            println!("Total projects: {}", stats.total_projects);

            // Should work with empty database
            assert_eq!(stats.total_projects, 0);
            assert_eq!(stats.total_plugins, 0);
            assert_eq!(stats.total_samples, 0);
            assert_eq!(stats.total_collections, 0);
            assert_eq!(stats.total_tags, 0);
            assert_eq!(stats.total_tasks, 0);
        }
        Err(e) => {
            println!("[ERROR] Statistics endpoint failed with error: {:?}", e);
            panic!(
                "Statistics endpoint should not fail even with empty database: {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_individual_statistics_functions() {
    setup("error");

    let server = create_test_server().await;
    let db = server.db();

    // Create a test project
    let _ = create_test_project_in_db(db).await;

    // Test individual statistics functions that might be causing issues
    {
        let db_guard = db.lock().await;

        println!("Testing get_projects_per_year...");
        match db_guard.get_projects_per_year() {
            Ok(projects_per_year) => {
                println!(
                    "[OK] get_projects_per_year succeeded: {:?}",
                    projects_per_year
                );
            }
            Err(e) => {
                println!("[ERROR] get_projects_per_year failed: {:?}", e);
                panic!("get_projects_per_year should not fail: {:?}", e);
            }
        }

        println!("Testing get_projects_per_month...");
        match db_guard.get_projects_per_month(12) {
            Ok(projects_per_month) => {
                println!(
                    "[OK] get_projects_per_month succeeded: {:?}",
                    projects_per_month
                );
            }
            Err(e) => {
                println!("[ERROR] get_projects_per_month failed: {:?}", e);
                panic!("get_projects_per_month should not fail: {:?}", e);
            }
        }

        println!("Testing get_recent_activity...");
        match db_guard.get_recent_activity(30) {
            Ok(recent_activity) => {
                println!("[OK] get_recent_activity succeeded: {:?}", recent_activity);
            }
            Err(e) => {
                println!("[ERROR] get_recent_activity failed: {:?}", e);
                panic!("get_recent_activity should not fail: {:?}", e);
            }
        }
    }

    // Test the task completion trends function specifically
    {
        let mut db_guard = db.lock().await;

        println!("Testing get_task_completion_trends...");
        match db_guard.get_task_completion_trends(12) {
            Ok(task_trends) => {
                println!(
                    "[OK] get_task_completion_trends succeeded: {:?}",
                    task_trends
                );
            }
            Err(e) => {
                println!("[ERROR] get_task_completion_trends failed: {:?}", e);
                panic!("get_task_completion_trends should not fail: {:?}", e);
            }
        }
    }
}
