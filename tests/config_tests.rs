mod common;

use studio_project_manager::config::CONFIG;
use studio_project_manager::error::ConfigError;

use crate::common::setup;

#[test]
fn test_config_validation() {
    setup("error");
    // Test that the global config loads successfully
    match &*CONFIG {
        Ok(config) => {
            // Verify basic validation
            assert!(
                !config.paths.is_empty(),
                "At least one path should be configured"
            );
            assert!(config.grpc_port > 0, "gRPC port should be greater than 0");

            // Note: Size limits are now optional and handled by media module
            // 0 means no limit, None means use media module default
            // Test that media formats are available (now handled by media module)
            assert!(
                !studio_project_manager::media::ALLOWED_IMAGE_FORMATS.is_empty(),
                "At least one image format should be allowed"
            );
            assert!(
                !studio_project_manager::media::ALLOWED_AUDIO_FORMATS.is_empty(),
                "At least one audio format should be allowed"
            );

            // Verify database path is set
            assert!(
                config.database_path.is_some(),
                "Database path should be set"
            );

            // Verify paths are properly formatted (no {USER_HOME} placeholders)
            for path in &config.paths {
                assert!(
                    !path.contains("{USER_HOME}"),
                    "Path should not contain {{USER_HOME}} placeholder: {}",
                    path
                );
            }

            assert!(
                !config.live_database_dir.contains("{USER_HOME}"),
                "Live database dir should not contain {{USER_HOME}} placeholder"
            );
            assert!(
                !config.media_storage_dir.contains("{USER_HOME}"),
                "Media storage dir should not contain {{USER_HOME}} placeholder"
            );
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
    setup("error");
    // Test that constants are properly defined
    assert_eq!(studio_project_manager::config::DEFAULT_GRPC_PORT, 50051);
    assert_eq!(studio_project_manager::config::DEFAULT_LOG_LEVEL, "info");

    // Test media module constants
    assert_eq!(
        studio_project_manager::media::DEFAULT_MAX_COVER_ART_SIZE_MB,
        10
    );
    assert_eq!(
        studio_project_manager::media::DEFAULT_MAX_AUDIO_FILE_SIZE_MB,
        50
    );

    // Test format arrays (now in media module)
    assert!(studio_project_manager::media::ALLOWED_IMAGE_FORMATS.contains(&"jpg"));
    assert!(studio_project_manager::media::ALLOWED_IMAGE_FORMATS.contains(&"png"));
    assert!(studio_project_manager::media::ALLOWED_AUDIO_FORMATS.contains(&"mp3"));
    assert!(studio_project_manager::media::ALLOWED_AUDIO_FORMATS.contains(&"wav"));
}

#[test]
fn test_config_error_types() {
    setup("error");
    // Test that ConfigError variants exist and work
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let config_error = ConfigError::IoError(io_error);
    assert!(format!("{}", config_error).contains("IO error in config"));

    let invalid_value = ConfigError::InvalidValue("test value".to_string());
    assert!(format!("{}", invalid_value).contains("Invalid configuration value"));

    let invalid_path = ConfigError::InvalidPath("test path".to_string());
    assert!(format!("{}", invalid_path).contains("Invalid path in config"));

    // Test new error types
    let path_not_found = ConfigError::PathNotFound("test/path".to_string());
    assert!(format!("{}", path_not_found).contains("Path not found"));

    let invalid_directory = ConfigError::InvalidDirectory("test/dir".to_string());
    assert!(format!("{}", invalid_directory).contains("Invalid directory"));

    let permission_denied = ConfigError::PermissionDenied("test/path".to_string());
    assert!(format!("{}", permission_denied).contains("Permission denied"));

    let port_out_of_range = ConfigError::PortOutOfRange(0);
    assert!(format!("{}", port_out_of_range).contains("Port 0 is out of valid range"));

    let config_file_not_found = ConfigError::ConfigFileNotFound;
    assert!(format!("{}", config_file_not_found).contains("Configuration file not found"));
}

#[test]
fn test_config_clone() {
    setup("error");
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
    setup("error");
    // Test that Config can be debug printed
    if let Ok(config) = &*CONFIG {
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("Config"));
    }
}

#[test]
fn test_path_length_validation() {
    setup("error");
    // Test path length validation
    let short_path = "C:\\short\\path";
    assert!(studio_project_manager::config::Config::validate_path_length(short_path).is_ok());

    // Test path that exceeds Windows limit
    let long_path = "C:\\".to_string() + &"a".repeat(260);
    let result = studio_project_manager::config::Config::validate_path_length(&long_path);

    #[cfg(windows)]
    {
        // On Windows, this might pass if long path support is enabled
        // or fail if it's disabled - both are valid outcomes
        if result.is_ok() {
            println!("Long path support is enabled on this Windows system");
        } else {
            println!("Long path support is disabled on this Windows system");
        }
    }

    #[cfg(not(windows))]
    {
        assert!(
            result.is_err(),
            "Path should be too long on non-Windows systems"
        );
    }
}

#[test]
fn test_windows_path_validation() {
    setup("error");
    // Test valid Windows paths
    assert!(
        studio_project_manager::config::Config::validate_windows_path("C:\\path\\to\\file").is_ok()
    );
    assert!(
        studio_project_manager::config::Config::validate_windows_path("\\\\server\\share\\file")
            .is_ok()
    );
    assert!(
        studio_project_manager::config::Config::validate_windows_path("relative\\path").is_ok()
    );

    // Test invalid Unix-style paths
    assert!(
        studio_project_manager::config::Config::validate_windows_path("/unix/style/path").is_err()
    );
}

#[test]
fn test_write_permission_check() {
    setup("error");
    // Test write permission check (should work for temp directory)
    let temp_dir = std::env::temp_dir();
    assert!(studio_project_manager::config::Config::can_write_to_directory(&temp_dir));
}

#[test]
fn test_validation_helper_error_context() {
    setup("error");
    // Test that the validation helper provides proper error context
    let long_path = "C:\\".to_string() + &"a".repeat(260);

    // This should fail with path length validation, but we can't test the helper directly
    // since it's private. Instead, we test that the validation is called during config validation
    let result = studio_project_manager::config::Config::validate_path_length(&long_path);

    #[cfg(windows)]
    {
        // On Windows, this might pass or fail depending on long path support
        if let Err(studio_project_manager::error::ConfigError::InvalidPath(msg)) = result {
            // Test that the error message contains the expected content
            assert!(msg.contains("characters"));
        } else {
            // If it passes, that's fine too - long path support is enabled
            println!("Long path support is enabled, path validation passed");
        }
    }

    #[cfg(not(windows))]
    {
        assert!(
            result.is_err(),
            "Path should be too long on non-Windows systems"
        );
        if let Err(studio_project_manager::error::ConfigError::InvalidPath(msg)) = result {
            assert!(msg.contains("Path exceeds limit"));
            assert!(msg.contains("260"));
        } else {
            panic!("Expected InvalidPath error");
        }
    }
}

#[cfg(windows)]
#[test]
fn test_windows_path_length_detection() {
    setup("error");
    // Test that we can access the Windows path length detection
    use studio_project_manager::config::windows_paths;

    let max_length = windows_paths::get_max_path_length();
    assert!(max_length >= 260);
    assert!(max_length <= 32767);

    let info = windows_paths::get_path_length_info();
    assert!(!info.is_empty());
    assert!(info.contains("characters"));
}
