use crate::config::{Config, paths};
use crate::error::ConfigError;
use std::path::PathBuf;

impl Config {
    /// Validates the configuration for logical constraints
    pub fn validate(&self) -> Result<Vec<String>, ConfigError> {
        let mut warnings = Vec::new();

        // Allow empty paths but issue a warning - this enables "setup required" mode
        if self.paths.is_empty() {
            warnings.push("No project paths configured - application will run in setup mode".to_string());
        }

        // Validate gRPC port range (u16 is already limited to 0-65535, so just check for 0)
        if self.grpc_port == 0 {
            return Err(ConfigError::PortOutOfRange(0));
        }

        // Validate log level
        let valid_log_levels = ["error", "warn", "info", "debug", "trace"];
        if !valid_log_levels.contains(&self.log_level.as_str()) {
            return Err(ConfigError::InvalidValue(format!(
                "Invalid log level '{}'. Must be one of: {:?}",
                self.log_level, valid_log_levels
            )));
        }

        // Validate paths and collect warnings
        let path_warnings = self.validate_paths()?;
        warnings.extend(path_warnings);

        Ok(warnings)
    }

    /// Validates that configured paths exist and are accessible
    /// Returns a list of warnings for non-critical issues
    pub fn validate_paths(&self) -> Result<Vec<String>, ConfigError> {
        let mut warnings = Vec::new();

        for path in &self.paths {
            // Validate path length and format
            paths::validate_single_path(path, "Project path")?;

            let path_buf = PathBuf::from(path);

            if !path_buf.exists() {
                warnings.push(format!("Path does not exist: {}", path));
                continue;
            }

            if !path_buf.is_dir() {
                return Err(ConfigError::InvalidDirectory(path.clone()));
            }

            // Check read permissions
            match std::fs::read_dir(&path_buf) {
                Ok(_) => {
                    // Check write permissions using actual file operations
                    if !paths::can_write_to_directory(&path_buf) {
                        warnings.push(format!("Directory is not writable: {}", path));
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        return Err(ConfigError::PermissionDenied(path.clone()));
                    } else {
                        return Err(ConfigError::IoError(e));
                    }
                }
            }
        }

        // Validate live database directory
        paths::validate_single_path(&self.live_database_dir, "Live database directory")?;
        let live_db_path = PathBuf::from(&self.live_database_dir);
        if !live_db_path.exists() {
            warnings.push(format!(
                "Live database directory does not exist: {}",
                self.live_database_dir
            ));
        }

        // Validate database_path if it exists
        if let Some(ref db_path) = self.database_path {
            paths::validate_single_path(db_path, "Database path")?;
        }

        // Validate media storage directory
        paths::validate_single_path(&self.media_storage_dir, "Media storage directory")?;
        let media_storage_path = PathBuf::from(&self.media_storage_dir);
        if !media_storage_path.exists() {
            // Try to create the media storage directory
            if let Err(e) = std::fs::create_dir_all(&media_storage_path) {
                warnings.push(format!("Could not create media storage directory: {}", e));
            }
        }

        Ok(warnings)
    }
} 