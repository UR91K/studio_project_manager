//! Tests for configuration and startup scenarios

use std::fs;
use tempfile::TempDir;
use seula::config::Config;

/// Helper function to escape Windows paths for TOML
fn escape_path_for_toml(path: &std::path::Path) -> String {
    path.display().to_string().replace("\\", "\\\\")
}

/// Single comprehensive test that runs all config startup scenarios in order
/// This prevents test interference from parallel execution and shared environment variables
#[test]
fn test_all_config_startup_scenarios() {
    println!("=== Running all config startup scenarios in sequence ===");
    
    // Run each test scenario in order
    test_config_loads_with_empty_paths_impl();
    test_config_validation_with_empty_paths_impl();
    test_config_with_valid_paths_impl();
    test_config_status_messages_impl();
    test_config_path_manipulation_impl();
    test_scanning_with_empty_paths_impl();
    test_config_reload_impl();
    
    println!("=== All config startup scenarios completed successfully ===");
}

/// Test that configuration can be loaded with empty paths without crashing
fn test_config_loads_with_empty_paths_impl() {
    println!("Running: test_config_loads_with_empty_paths");
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Create a config with empty paths (using Windows-compatible paths)
    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    // This should not panic or crash
    let result = Config::new();
    assert!(result.is_ok(), "Config should load successfully with empty paths");

    let config = result.unwrap();
    assert!(config.needs_setup(), "Config should indicate setup is needed");
    assert!(!config.is_ready_for_operation(), "Config should not be ready for operation");
    assert_eq!(config.paths.len(), 0, "Config should have no paths");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test that configuration validation allows empty paths but issues warnings
fn test_config_validation_with_empty_paths_impl() {
    println!("Running: test_config_validation_with_empty_paths");
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    let config = Config::new().unwrap();
    let validation_result = config.validate();

    assert!(validation_result.is_ok(), "Validation should succeed with empty paths");
    
    let warnings = validation_result.unwrap();
    assert!(!warnings.is_empty(), "Should have warnings about empty paths");
    assert!(warnings.iter().any(|w| w.contains("No project paths configured")), 
            "Should warn about no paths configured");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test that configuration with valid paths works normally

fn test_config_with_valid_paths_impl() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let test_project_dir = temp_dir.path().join("projects");
    fs::create_dir_all(&test_project_dir).unwrap();

    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let config_content = format!(r#"
paths = ["{}"]
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&test_project_dir), escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    let config = Config::new().unwrap();
    assert!(!config.needs_setup(), "Config should not need setup with valid paths");
    assert!(config.is_ready_for_operation(), "Config should be ready for operation");
    assert_eq!(config.paths.len(), 1, "Config should have one path");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test the status message functionality

fn test_config_status_messages_impl() {
    // Test empty paths
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let empty_config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, empty_config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    let config = Config::new().unwrap();
    let status = config.get_status_message();
    assert!(status.contains("Configuration incomplete"), "Should indicate incomplete configuration");
    assert!(status.contains("No project paths specified"), "Should mention no paths");

    // Test with paths
    let test_project_dir = temp_dir.path().join("projects");
    fs::create_dir_all(&test_project_dir).unwrap();

    let another_path = temp_dir.path().join("another_path");
    fs::create_dir_all(&another_path).unwrap();

    let config_with_paths = format!(r#"
paths = ["{}", "{}"]
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&test_project_dir), escape_path_for_toml(&another_path), escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, config_with_paths).unwrap();
    
    // Use reload instead of new() to avoid lazy static caching issues
    let (config, _warnings) = Config::reload().unwrap();
    let status = config.get_status_message();
    assert!(status.contains("Configuration ready"), "Should indicate ready configuration");
    assert!(status.contains("2 project path(s)"), "Should mention number of paths");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test config path manipulation methods

fn test_config_path_manipulation_impl() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let test_project_dir1 = temp_dir.path().join("projects1");
    let test_project_dir2 = temp_dir.path().join("projects2");
    fs::create_dir_all(&test_project_dir1).unwrap();
    fs::create_dir_all(&test_project_dir2).unwrap();

    // Start with empty config
    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let empty_config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, empty_config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    let mut config = Config::new().unwrap();
    assert!(config.needs_setup(), "Should need setup initially");

    // Add a path
    let result = config.add_path(test_project_dir1.to_string_lossy().to_string());
    assert!(result.is_ok(), "Should be able to add path");
    assert!(!config.needs_setup(), "Should not need setup after adding path");

    // Add another path
    let result = config.add_path(test_project_dir2.to_string_lossy().to_string());
    assert!(result.is_ok(), "Should be able to add second path");
    assert_eq!(config.paths.len(), 2, "Should have two paths");

    // Try to add duplicate path
    let result = config.add_path(test_project_dir1.to_string_lossy().to_string());
    assert!(result.is_ok(), "Should handle duplicate path gracefully");
    let warnings = result.unwrap();
    assert!(warnings.iter().any(|w| w.contains("already exists")), "Should warn about duplicate");

    // Remove a path
    let result = config.remove_path(&test_project_dir1.to_string_lossy());
    assert!(result.is_ok(), "Should be able to remove path");
    assert_eq!(config.paths.len(), 1, "Should have one path after removal");

    // Remove last path
    let result = config.remove_path(&test_project_dir2.to_string_lossy());
    assert!(result.is_ok(), "Should be able to remove last path");
    assert!(config.needs_setup(), "Should need setup after removing all paths");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test that the main scanning function handles empty paths gracefully

fn test_scanning_with_empty_paths_impl() {
    use seula::process_projects;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let empty_config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, empty_config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    // This should not panic or crash, but return Ok(()) indicating setup is needed
    let result = process_projects();
    assert!(result.is_ok(), "process_projects should handle empty paths gracefully");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}

/// Test config reload functionality

fn test_config_reload_impl() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Start with empty config
    let live_db_dir = temp_dir.path().join("ableton");
    let media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&live_db_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    let empty_config_content = format!(r#"
paths = []
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, empty_config_content).unwrap();
    std::env::set_var("STUDIO_PROJECT_MANAGER_CONFIG", config_path.to_str().unwrap());

    // Test reload
    let (config, warnings) = Config::reload().unwrap();
    assert!(config.needs_setup(), "Reloaded config should need setup");
    assert!(!warnings.is_empty(), "Should have warnings about empty paths");

    // Update config file
    let test_project_dir = temp_dir.path().join("projects");
    fs::create_dir_all(&test_project_dir).unwrap();

    let updated_config_content = format!(r#"
paths = ["{}"]
live_database_dir = "{}"
grpc_port = 50051
log_level = "info"
media_storage_dir = "{}"
"#, escape_path_for_toml(&test_project_dir), escape_path_for_toml(&live_db_dir), escape_path_for_toml(&media_dir));

    fs::write(&config_path, updated_config_content).unwrap();

    // Reload again
    let (config, _) = Config::reload().unwrap();
    assert!(!config.needs_setup(), "Reloaded config should not need setup");
    assert_eq!(config.paths.len(), 1, "Should have one path after reload");

    // Clean up
    std::env::remove_var("STUDIO_PROJECT_MANAGER_CONFIG");
}
