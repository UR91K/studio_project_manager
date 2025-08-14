//! Server setup utilities for gRPC tests

use super::*;
use crate::common::{setup, LiveSetBuilder};
use std::path::PathBuf;
use studio_project_manager::grpc::StudioProjectManagerServer;
use studio_project_manager::media::{MediaConfig, MediaStorageManager};

pub async fn create_test_server() -> StudioProjectManagerServer {
    setup("error");

    // Create in-memory database
    let db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create test database");

    // Create a test media config (use defaults)
    let media_config = MediaConfig::default();

    // Create temporary directory for test media storage
    let temp_dir = std::env::temp_dir().join("studio_project_manager_test_media");
    let media_storage = MediaStorageManager::new(temp_dir, media_config)
        .expect("Failed to create test media storage");

    StudioProjectManagerServer::new_for_test(db, media_storage)
}

pub async fn create_test_project_in_db(db: &Arc<Mutex<LiveSetDatabase>>) -> String {
    let test_project = LiveSetBuilder::new()
        .with_plugin("Serum")
        .with_sample("kick.wav")
        .with_tempo(140.0)
        .with_time_signature(4, 4)
        .with_key_signature(studio_project_manager::models::KeySignature {
            tonic: studio_project_manager::models::Tonic::C,
            scale: studio_project_manager::models::Scale::Major,
        })
        .with_version(11, 0, 0, false)
        .build();

    let unique_id = uuid::Uuid::new_v4();
    let unique_name = format!("Test Project {}.als", unique_id);

    let test_live_set = studio_project_manager::live_set::LiveSet {
        is_active: true,
        id: unique_id,
        file_path: PathBuf::from(&unique_name),
        name: unique_name.clone(),
        file_hash: format!("test_hash_{}", unique_id),
        created_time: chrono::Local::now(),
        modified_time: chrono::Local::now(),
        last_parsed_timestamp: chrono::Local::now(),
        tempo: test_project.tempo,
        time_signature: test_project.time_signature,
        key_signature: test_project.key_signature,
        furthest_bar: test_project.furthest_bar,
        estimated_duration: None,
        ableton_version: test_project.version,
        plugins: test_project.plugins,
        samples: test_project.samples,
        tags: std::collections::HashSet::new(),
    };

    let project_id = test_live_set.id.to_string();
    let mut db_guard = db.lock().await;
    db_guard
        .insert_project(&test_live_set)
        .expect("Failed to insert test project");

    project_id
}

/// Setup test server and return both server and database reference
pub async fn setup_test_server() -> (StudioProjectManagerServer, Arc<Mutex<LiveSetDatabase>>) {
    let server = create_test_server().await;
    let db = server.db().clone();
    (server, db)
}

/// Create a test project with the given name and path
pub async fn create_test_project(server: &StudioProjectManagerServer, name: &str, path: &str) -> String {
    let db = server.db();
    let project_id = uuid::Uuid::new_v4().to_string();
    
    {
        let db_lock = db.lock().await;
        db_lock.conn.execute(
            "INSERT INTO projects (
                id, name, path, hash, created_at, modified_at, last_parsed_at,
                tempo, time_signature_numerator, time_signature_denominator,
                ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta
            ) VALUES (?, ?, ?, ?, datetime('now'), datetime('now'), datetime('now'), ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![project_id, name, path, format!("test_hash_{}", project_id), 120.0, 4, 4, 11, 0, 0, false],
        ).expect("Failed to insert test project");
    }
    
    project_id
}

/// Create a test sample with the given parameters
pub async fn create_test_sample(server: &StudioProjectManagerServer, name: &str, path: &str, is_present: bool) -> String {
    let db = server.db();
    let sample_id = uuid::Uuid::new_v4().to_string();
    
    {
        let db_lock = db.lock().await;
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample_id, name, path, is_present],
        ).expect("Failed to insert test sample");
    }
    
    sample_id
}

/// Link a sample to a project (create usage relationship)
pub async fn add_sample_to_project(server: &StudioProjectManagerServer, project_id: &str, sample_id: &str) {
    let db = server.db();
    
    {
        let db_lock = db.lock().await;
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project_id, sample_id],
        ).expect("Failed to link sample to project");
    }
}

/// Create a Sample struct for testing (type-safe alternative to raw database insertion)
pub fn create_sample_struct(name: &str, path: &str, is_present: bool) -> studio_project_manager::models::Sample {
    use studio_project_manager::models::Sample;
    use std::path::PathBuf;
    use uuid::Uuid;
    
    Sample {
        id: Uuid::new_v4(),
        name: name.to_string(),
        path: PathBuf::from(path),
        is_present,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_project_manager::grpc::scanning::*;
    use studio_project_manager::grpc::common::*;
    use tonic::Request;

    #[tokio::test]
    async fn test_scan_status_progress_tracking() {
        let server = create_test_server().await;

        // Initially, there should be no progress
        let request = Request::new(GetScanStatusRequest {});
        let response = server.get_scan_status(request).await.unwrap();
        let scan_status_response = response.into_inner();

        assert_eq!(scan_status_response.status, ScanStatus::ScanUnknown as i32);
        assert!(scan_status_response.current_progress.is_none());

        // Simulate setting progress manually
        let progress_response = ScanProgressResponse {
            completed: 50,
            total: 100,
            progress: 0.5,
            message: "Test progress".to_string(),
            status: ScanStatus::ScanParsing as i32,
        };

        // Set the progress using the system handler
        *server.system_handler.scan_progress.lock().await = Some(progress_response.clone());
        *server.system_handler.scan_status.lock().await = ScanStatus::ScanParsing;

        // Now check that the progress is returned
        let request = Request::new(GetScanStatusRequest {});
        let response = server.get_scan_status(request).await.unwrap();
        let scan_status_response = response.into_inner();

        assert_eq!(scan_status_response.status, ScanStatus::ScanParsing as i32);
        assert!(scan_status_response.current_progress.is_some());

        let current_progress = scan_status_response.current_progress.unwrap();
        assert_eq!(current_progress.completed, 50);
        assert_eq!(current_progress.total, 100);
        assert_eq!(current_progress.progress, 0.5);
        assert_eq!(current_progress.message, "Test progress");
    }
}
