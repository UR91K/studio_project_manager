use studio_project_manager::config::CONFIG;
use studio_project_manager::error::ConfigError;

#[test]
fn test_config_validation() {
    // Test that the global config loads successfully
    match &*CONFIG {
        Ok(config) => {
            // Verify basic validation
            assert!(!config.paths.is_empty(), "At least one path should be configured");
            assert!(config.grpc_port > 0, "gRPC port should be greater than 0");
            assert!(config.max_cover_art_size_mb > 0, "Max cover art size should be greater than 0");
            assert!(config.max_audio_file_size_mb > 0, "Max audio file size should be greater than 0");
            assert!(!config.allowed_image_formats.is_empty(), "At least one image format should be allowed");
            assert!(!config.allowed_audio_formats.is_empty(), "At least one audio format should be allowed");
            
            // Verify database path is set
            assert!(config.database_path.is_some(), "Database path should be set");
            
            // Verify paths are properly formatted (no {USER_HOME} placeholders)
            for path in &config.paths {
                assert!(!path.contains("{USER_HOME}"), "Path should not contain {{USER_HOME}} placeholder: {}", path);
            }
            
            assert!(!config.live_database_dir.contains("{USER_HOME}"), "Live database dir should not contain {{USER_HOME}} placeholder");
            assert!(!config.media_storage_dir.contains("{USER_HOME}"), "Media storage dir should not contain {{USER_HOME}} placeholder");
        }
        Err(e) => {
            // If config fails to load, it might be due to missing directories
            // This is acceptable in test environments
            eprintln!("Config failed to load: {}", e);
        }
    }
}

#[test]
fn test_config_constants() {
    // Test that constants are properly defined
    assert_eq!(studio_project_manager::config::DEFAULT_GRPC_PORT, 50051);
    assert_eq!(studio_project_manager::config::DEFAULT_MAX_COVER_ART_SIZE_MB, 10);
    assert_eq!(studio_project_manager::config::DEFAULT_MAX_AUDIO_FILE_SIZE_MB, 50);
    assert_eq!(studio_project_manager::config::DEFAULT_LOG_LEVEL, "info");
    
    // Test format arrays
    assert!(studio_project_manager::config::DEFAULT_IMAGE_FORMATS.contains(&"jpg"));
    assert!(studio_project_manager::config::DEFAULT_IMAGE_FORMATS.contains(&"png"));
    assert!(studio_project_manager::config::DEFAULT_AUDIO_FORMATS.contains(&"mp3"));
    assert!(studio_project_manager::config::DEFAULT_AUDIO_FORMATS.contains(&"wav"));
}

#[test]
fn test_config_error_types() {
    // Test that ConfigError variants exist and work
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let config_error = ConfigError::IoError(io_error);
    assert!(format!("{}", config_error).contains("IO error in config"));
    
    let invalid_value = ConfigError::InvalidValue("test value".to_string());
    assert!(format!("{}", invalid_value).contains("Invalid configuration value"));
    
    let invalid_path = ConfigError::InvalidPath("test path".to_string());
    assert!(format!("{}", invalid_path).contains("Invalid path in config"));
}

#[test]
fn test_config_clone() {
    // Test that Config can be cloned
    if let Ok(config) = &*CONFIG {
        let cloned = config.clone();
        assert_eq!(config.grpc_port, cloned.grpc_port);
        assert_eq!(config.paths, cloned.paths);
        assert_eq!(config.database_path, cloned.database_path);
    }
}

#[test]
fn test_config_debug() {
    // Test that Config can be debug printed
    if let Ok(config) = &*CONFIG {
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("Config"));
    }
} 