//! Scanning integration tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following from src/main.rs tests module (around line 395):
//! - test_process_projects_integration() (line ~400)
//! - test_process_projects_with_progress() (line ~468)

use crate::common::setup;
use studio_project_manager::{
    config::CONFIG,
    scan::project_scanner::ProjectPathScanner,
    database::LiveSetDatabase,
    process_projects,
    process_projects_with_progress,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

// TODO: Move test_process_projects_integration() from src/main.rs (around line 400)
// TODO: Move test_process_projects_with_progress() from src/main.rs (around line 468)
// TODO: These are the main integration tests that test the full scanning workflow
// Total: ~2 tests to move 

#[test]
fn test_process_projects_integration() {
    setup("debug");

    // Get expected project paths from config
    let config = CONFIG.as_ref().expect("Failed to load config");
    let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
    
    // Scan configured paths to know what we expect to find
    let mut expected_projects = HashSet::new();
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            let projects = scanner.scan_directory(&path).expect("Failed to scan directory");
            expected_projects.extend(projects);
        }
    }
    
    assert!(!expected_projects.is_empty(), "No projects found in configured paths");
    let expected_project_names: HashSet<String> = expected_projects
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    // Run process_projects
    process_projects().expect("process_projects failed");

    // Open database and verify contents
    let database_path = config.database_path.as_ref().expect("Database path should be set by config initialization");
    let db = LiveSetDatabase::new(PathBuf::from(database_path))
        .expect("Failed to open database");

    // Get actual project names from database
    let mut stmt = db.conn.prepare("SELECT name FROM projects").expect("Failed to prepare query");
    let project_names: HashSet<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("Failed to execute query")
        .map(|r| r.expect("Failed to get project name"))
        .collect();

    // Verify all expected projects were processed
    for expected_name in &expected_project_names {
        assert!(
            project_names.contains(expected_name),
            "Project '{}' not found in database", expected_name
        );
    }

    // Get some statistics
    let project_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .expect("Failed to count projects");
    let plugin_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .expect("Failed to count plugins");
    let sample_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))
        .expect("Failed to count samples");

    println!("\nDatabase Statistics:");
    println!("Projects found: {}", project_count);
    println!("Plugins found: {}", plugin_count);
    println!("Samples found: {}", sample_count);
    println!("\nProjects processed:");
    for name in &project_names {
        println!("- {}", name);
    }
}

#[test]
fn test_process_projects_with_progress() {
    setup("info");

    // Get expected project paths from config
    let config = CONFIG.as_ref().expect("Failed to load config");
    let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
    
    // Scan configured paths to know what we expect to find
    let mut expected_projects = HashSet::new();
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            let projects = scanner.scan_directory(&path).expect("Failed to scan directory");
            expected_projects.extend(projects);
        }
    }
    
    if expected_projects.is_empty() {
        println!("No projects found in configured paths, skipping test");
        return;
    }

    // Track progress updates
    let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(u32, u32, f32, String, String)>::new()));
    let progress_updates_clone = Arc::clone(&progress_updates);

    // Create progress callback that captures all updates
    let progress_callback = move |completed: u32, total: u32, progress: f32, message: String, phase: &str| {
        let mut updates = progress_updates_clone.lock().unwrap();
        updates.push((completed, total, progress, message.clone(), phase.to_string()));
        println!("Progress: {}/{} ({:.1}%) - {} [{}]", completed, total, progress * 100.0, message, phase);
    };

    // Run process_projects_with_progress
    let result = process_projects_with_progress(Some(progress_callback));
    assert!(result.is_ok(), "process_projects_with_progress failed: {:?}", result.err());

    // Verify progress updates
    let updates = progress_updates.lock().unwrap();
    assert!(!updates.is_empty(), "No progress updates received");

    // Check that we got the expected progression of phases
    let phases: Vec<String> = updates.iter().map(|(_, _, _, _, phase)| phase.clone()).collect();
    println!("Received phases: {:?}", phases);

    // Should have at least starting and completed phases
    assert!(phases.contains(&"starting".to_string()), "Missing 'starting' phase");
    assert!(phases.contains(&"completed".to_string()) || phases.contains(&"preprocessing".to_string()), 
        "Missing completion phase");

    // Check that progress values make sense
    for (i, (completed, total, progress, _, _)) in updates.iter().enumerate() {
        // Progress should be between 0 and 1
        assert!(*progress >= 0.0 && *progress <= 1.0, 
            "Progress out of range at update {}: {}", i, progress);
        
        // If total > 0, completed should not exceed total
        if *total > 0 {
            assert!(*completed <= *total, 
                "Completed {} exceeds total {} at update {}", completed, total, i);
        }
    }

    // Check final progress should be complete
    if let Some((_, _, final_progress, _, final_phase)) = updates.last() {
        if final_phase == "completed" {
            assert_eq!(*final_progress, 1.0, "Final progress should be 1.0, got {}", final_progress);
        }
    }

    println!("✅ Progress streaming test passed with {} updates", updates.len());
}