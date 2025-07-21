use crate::config::Config;
use crate::error::ConfigError;
use std::path::{Path, PathBuf};

/// Maximum path length for Windows compatibility (260 characters)
/// Note: Use windows_paths::get_max_path_length() for dynamic detection
pub const MAX_PATH_LENGTH: usize = 260;

impl Config {
    /// Validates that a path doesn't exceed Windows path length limits
    pub fn validate_path_length(path: &str) -> Result<(), ConfigError> {
        #[cfg(windows)]
        {
            use crate::config::windows_paths::validate_path_with_long_path_support;
            validate_path_with_long_path_support(path).map_err(|msg| ConfigError::InvalidPath(msg))
        }

        #[cfg(not(windows))]
        {
            if path.len() > MAX_PATH_LENGTH {
                return Err(ConfigError::InvalidPath(format!(
                    "Path exceeds limit of {} characters: {}",
                    MAX_PATH_LENGTH, path
                )));
            }
            Ok(())
        }
    }

    /// Tests if a directory is writable by attempting to create and remove a test file
    pub fn can_write_to_directory(path: &Path) -> bool {
        let test_file = path.join(".temp_write_test");
        std::fs::File::create(&test_file)
            .and_then(|_| std::fs::remove_file(&test_file))
            .is_ok()
    }

    /// Validates Windows path format (drive letter, UNC paths, etc.)
    pub fn validate_windows_path(path: &str) -> Result<(), ConfigError> {
        // Check for Unix-style absolute paths (starting with /)
        if path.starts_with('/') {
            return Err(ConfigError::InvalidPath(format!(
                "Unix-style absolute path not supported on Windows: {}",
                path
            )));
        }

        let path_buf = PathBuf::from(path);

        if path_buf.is_absolute() {
            // Check for valid Windows path components
            let mut components = path_buf.components();

            if let Some(first) = components.next() {
                match first {
                    std::path::Component::Prefix(_prefix) => {
                        // Valid Windows path prefix (drive letter or UNC)
                        return Ok(());
                    }
                    std::path::Component::RootDir => {
                        // This shouldn't happen after the string check above, but just in case
                        return Err(ConfigError::InvalidPath(format!(
                            "Unix-style absolute path not supported on Windows: {}",
                            path
                        )));
                    }
                    _ => {
                        // Other components shouldn't be first in absolute paths
                        return Err(ConfigError::InvalidPath(format!(
                            "Invalid Windows path format: {}",
                            path
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Validates a single path with length and format checks, providing context in error messages
    pub fn validate_single_path(path: &str, path_name: &str) -> Result<(), ConfigError> {
        Self::validate_path_length(path)
            .map_err(|e| ConfigError::InvalidPath(format!("{}: {}", path_name, e)))?;
        Self::validate_windows_path(path)
            .map_err(|e| ConfigError::InvalidPath(format!("{}: {}", path_name, e)))?;
        Ok(())
    }
}

// Re-export the path validation functions for easier access
pub fn validate_path_length(path: &str) -> Result<(), ConfigError> {
    Config::validate_path_length(path)
}

pub fn can_write_to_directory(path: &Path) -> bool {
    Config::can_write_to_directory(path)
}

pub fn validate_windows_path(path: &str) -> Result<(), ConfigError> {
    Config::validate_windows_path(path)
}

pub fn validate_single_path(path: &str, path_name: &str) -> Result<(), ConfigError> {
    Config::validate_single_path(path, path_name)
} 