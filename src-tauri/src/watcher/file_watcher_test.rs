#![allow(dead_code, unused)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::sleep;
use log::{debug, info};

use crate::database::LiveSetDatabase;
use crate::watcher::file_watcher::{FileEvent, FileWatcher};

struct TestEnvironment {
    temp_dir: TempDir,
    watcher: FileWatcher,
    rx: std::sync::mpsc::Receiver<FileEvent>,
    _db: Arc<Mutex<LiveSetDatabase>>,
}

impl TestEnvironment {
    async fn new() -> Self {
        debug!("Creating new test environment");
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        debug!("Created temp directory at {:?}", temp_dir.path());
        
        let db = Arc::new(Mutex::new(
            LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create test database")
        ));
        debug!("Created in-memory test database");
        
        let (mut watcher, rx) = FileWatcher::new(db.clone()).expect("Failed to create file watcher");
        debug!("Created file watcher");
        
        // Add temp directory to watch paths
        watcher
            .add_watch_path(temp_dir.path().to_path_buf())
            .expect("Failed to add watch path");
        debug!("Added temp directory to watch paths");

        let env = Self {
            temp_dir,
            watcher,
            rx,
            _db: db,
        };

        // Small delay to ensure watcher is ready
        debug!("Waiting for watcher to initialize...");
        sleep(Duration::from_millis(200)).await;
        debug!("Test environment ready");
        env
    }

    fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        debug!("Creating test file: {:?}", path);
        let mut file = File::create(&path).expect("Failed to create test file");
        file.write_all(content.as_bytes())
            .expect("Failed to write test content");
        file.sync_all().expect("Failed to sync file to disk");
        debug!("Created and synced test file: {:?}", path);
        path
    }

    fn modify_file(&self, path: &Path, content: &str) {
        debug!("Modifying file: {:?}", path);
        let mut file = File::create(path).expect("Failed to modify test file");
        file.write_all(content.as_bytes())
            .expect("Failed to write modified content");
        file.sync_all().expect("Failed to sync file to disk");
        debug!("Modified and synced file: {:?}", path);
    }

    fn delete_file(&self, path: &Path) {
        debug!("Deleting file: {:?}", path);
        fs::remove_file(path).expect("Failed to delete test file");
        debug!("Deleted file: {:?}", path);
    }

    fn rename_file(&self, from: &Path, to: &str) -> PathBuf {
        let new_path = self.temp_dir.path().join(to);
        debug!("Renaming file: {:?} -> {:?}", from, new_path);
        fs::rename(from, &new_path).expect("Failed to rename test file");
        debug!("Renamed file successfully");
        new_path
    }

    async fn expect_event(&self, timeout_ms: u64) -> Option<FileEvent> {
        debug!("Waiting up to {}ms for event...", timeout_ms);
        let interval_ms = 20;
        let attempts = (timeout_ms / interval_ms) + 1;
        
        for i in 0..attempts {
            if let Ok(event) = self.rx.try_recv() {
                debug!("Received event after {}ms: {:?}", i * interval_ms, event);
                return Some(event);
            }
            sleep(Duration::from_millis(interval_ms)).await;
        }
        debug!("No event received after {}ms", timeout_ms);
        None
    }

    async fn expect_events(&self, timeout_ms: u64) -> Vec<FileEvent> {
        debug!("Waiting up to {}ms for events...", timeout_ms);
        let mut events = Vec::new();
        let interval_ms = 20;
        let attempts = (timeout_ms / interval_ms) + 1;
        
        for i in 0..attempts {
            while let Ok(event) = self.rx.try_recv() {
                debug!("Received event: {:?}", event);
                events.push(event);
            }
            if !events.is_empty() {
                debug!("Collected {} events after {}ms", events.len(), i * interval_ms);
                break;
            }
            sleep(Duration::from_millis(interval_ms)).await;
        }
        if events.is_empty() {
            debug!("No events received after {}ms", timeout_ms);
        }
        events
    }

    async fn wait_for_watcher(&self) {
        debug!("Waiting for watcher to settle...");
        sleep(Duration::from_millis(200)).await;
        debug!("Watcher should be ready");
    }
}

//#[tokio::test]
async fn test_file_creation() {
    let env = TestEnvironment::new().await;
    env.wait_for_watcher().await;
    
    // Create a test file
    let path = env.create_file("test.als", "test content");
    
    // Wait for and verify the event
    if let Some(FileEvent::Created(event_path)) = env.expect_event(500).await {
        assert_eq!(event_path, path);
    } else {
        panic!("Did not receive expected file creation event");
    }
}

//#[tokio::test]
async fn test_file_modification() {
    let env = TestEnvironment::new().await;
    env.wait_for_watcher().await;
    
    // Create and wait for initial file
    let path = env.create_file("test.als", "initial content");
    env.expect_event(500).await; // Consume creation event
    
    // Modify the file
    env.modify_file(&path, "modified content");
    
    // Wait for and verify the event
    if let Some(FileEvent::Modified(event_path)) = env.expect_event(500).await {
        assert_eq!(event_path, path);
    } else {
        panic!("Did not receive expected file modification event");
    }
}

//#[tokio::test]
async fn test_file_deletion() {
    let env = TestEnvironment::new().await;
    env.wait_for_watcher().await;
    
    // Create and wait for initial file
    let path = env.create_file("test.als", "test content");
    env.expect_event(500).await; // Consume creation event
    
    // Delete the file
    env.delete_file(&path);
    
    // Wait for and verify the event
    if let Some(FileEvent::Deleted(event_path)) = env.expect_event(500).await {
        assert_eq!(event_path, path);
    } else {
        panic!("Did not receive expected file deletion event");
    }
}

//#[tokio::test]
async fn test_file_rename() {
    let env = TestEnvironment::new().await;
    env.wait_for_watcher().await;
    
    // Create and wait for initial file
    let path = env.create_file("old.als", "test content");
    env.expect_event(500).await; // Consume creation event
    
    // Rename the file
    let new_path = env.rename_file(&path, "new.als");
    
    // Wait for and verify the event
    if let Some(FileEvent::Renamed { from, to }) = env.expect_event(500).await {
        assert_eq!(from, path);
        assert_eq!(to, new_path);
    } else {
        panic!("Did not receive expected file rename event");
    }
}

//#[tokio::test]
async fn test_non_als_file_ignored() {
    let env = TestEnvironment::new().await;
    
    // Create a non-.als file
    env.create_file("test.txt", "test content");
    
    // Verify no event is received
    assert!(env.expect_event(100).await.is_none(), "Received unexpected event for non-.als file");
}

//#[tokio::test]
async fn test_offline_changes() {
    let env = TestEnvironment::new().await;
    env.wait_for_watcher().await;
    
    // Create test files
    let path1 = env.create_file("test1.als", "content1");
    let path2 = env.create_file("test2.als", "content2");
    
    // Consume initial creation events
    let _ = env.expect_events(500).await;
    
    // Simulate offline changes
    env.modify_file(&path1, "modified content");
    env.delete_file(&path2);
    
    // Check offline changes
    env.watcher.check_offline_changes().await.expect("Failed to check offline changes");
    
    // Verify events
    let events = env.expect_events(500).await;
    let mut received_modified = false;
    let mut received_deleted = false;
    
    for event in events {
        match event {
            FileEvent::Modified(path) if path == path1 => received_modified = true,
            FileEvent::Deleted(path) if path == path2 => received_deleted = true,
            _ => {}
        }
    }
    
    assert!(received_modified, "Did not receive expected modification event");
    assert!(received_deleted, "Did not receive expected deletion event");
}

//#[tokio::test]
async fn test_scan_for_new_files() {
    let env = TestEnvironment::new().await;
    
    // Create test files before scanning
    let path1 = env.create_file("existing1.als", "content1");
    let path2 = env.create_file("existing2.als", "content2");
    
    // Consume initial creation events
    env.expect_event(100).await;
    env.expect_event(100).await;
    
    // Scan for new files
    env.watcher.scan_for_new_files().await.expect("Failed to scan for new files");
    
    // Verify events for existing files
    let mut found_files = HashSet::new();
    for _ in 0..2 {
        if let Some(FileEvent::Created(path)) = env.expect_event(100).await {
            found_files.insert(path);
        }
    }
    
    assert!(found_files.contains(&path1), "Did not find existing1.als");
    assert!(found_files.contains(&path2), "Did not find existing2.als");
} 