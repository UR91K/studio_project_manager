//! Server setup utilities for gRPC tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following from src/grpc/server.rs:
//! - create_test_server() function (line ~798)
//! - create_test_project_in_db() function (line ~813)
//! - Any other shared test setup functions

use super::*;
use crate::common::{setup, LiveSetBuilder};

// TODO: Move create_test_server() function from src/grpc/server.rs (around line 798)
// TODO: Move create_test_project_in_db() function from src/grpc/server.rs (around line 813)
// TODO: Move any other shared setup functions used by multiple gRPC tests
// Make sure all functions are public: pub async fn create_test_server() { ... } 

pub async fn create_test_server() -> StudioProjectManagerServer {
    setup("debug");
    
    // Create in-memory database
    let db = LiveSetDatabase::new(PathBuf::from(":memory:"))
        .expect("Failed to create test database");
    
    StudioProjectManagerServer {
        db: Arc::new(Mutex::new(db)),
        scan_status: Arc::new(Mutex::new(ScanStatus::ScanUnknown)),
    }
}

pub async fn create_test_project_in_db(db: &Arc<Mutex<LiveSetDatabase>>) -> String {
    let test_project = LiveSetBuilder::new()
        .with_plugin("Serum")
        .with_sample("kick.wav")
        .with_tempo(140.0)
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
    db_guard.insert_project(&test_live_set).expect("Failed to insert test project");
    
    project_id
}
