pub mod windows_paths;
pub mod loader;
pub mod validator;
pub mod paths;
pub mod defaults;

use crate::error::ConfigError;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

// Re-export constants from submodules for backward compatibility
pub use defaults::{
    DEFAULT_GRPC_PORT, DEFAULT_LOG_LEVEL, DEFAULT_MAX_COVER_ART_SIZE_MB, DEFAULT_MAX_AUDIO_FILE_SIZE_MB,
};
pub use paths::MAX_PATH_LENGTH;
pub use loader::MAX_DIRECTORY_TRAVERSAL_DEPTH;

/// Configuration for the Studio Project Manager application
///
/// # Example Configuration File
/// ```toml
/// # List of paths to scan for music projects
/// paths = [
///     "C:\\Users\\username\\Documents\\Music Projects",
///     "{USER_HOME}\\Documents\\Ableton Projects"
/// ]
///
/// # Database file path (optional, defaults to user data directory)
/// # database_path = "C:\\Users\\username\\AppData\\Roaming\\StudioProjectManager\\ableton_live_sets.db"
///
/// # Directory containing Ableton Live database files
/// live_database_dir = "C:\\Users\\username\\AppData\\Local\\Ableton\\Live Database"
///
/// # gRPC server port (can be overridden by STUDIO_PROJECT_MANAGER_GRPC_PORT env var)
/// grpc_port = 50051
///
/// # Logging level: error, warn, info, debug, trace
/// log_level = "info"
///
/// # Directory for storing media files
/// media_storage_dir = "C:\\Users\\username\\AppData\\Roaming\\StudioProjectManager\\media"
///
/// # Media file size limits (optional, 0 = no limit, omit to use defaults)
/// # max_cover_art_size_mb = 10
/// # max_audio_file_size_mb = 50
/// ```
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    /// List of paths to scan for music projects
    pub paths: Vec<String>,
    /// Database file path (optional, defaults to user data directory)
    #[serde(default = "defaults::default_database_path")]
    pub database_path: Option<String>,
    /// Directory containing Ableton Live database files
    pub live_database_dir: String,
    /// gRPC server port (can be overridden by STUDIO_PROJECT_MANAGER_GRPC_PORT env var)
    #[serde(default = "defaults::default_grpc_port")]
    pub grpc_port: u16,
    /// Logging level
    #[serde(default = "defaults::default_log_level")]
    pub log_level: String,
    /// Directory for storing media files
    pub media_storage_dir: String,
    /// Maximum cover art file size in MB (0 = no limit, None = use media module default)
    #[serde(default = "defaults::default_max_cover_art_size")]
    pub max_cover_art_size_mb: Option<u32>,
    /// Maximum audio file size in MB (0 = no limit, None = use media module default)
    #[serde(default = "defaults::default_max_audio_file_size")]
    pub max_audio_file_size_mb: Option<u32>,
}

impl Config {
    /// Returns the gRPC port with environment variable override support
    pub fn grpc_port(&self) -> u16 {
        std::env::var("STUDIO_PROJECT_MANAGER_GRPC_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(self.grpc_port)
    }

    /// Returns the log level with environment variable override support
    pub fn log_level(&self) -> String {
        std::env::var("STUDIO_PROJECT_MANAGER_LOG_LEVEL").unwrap_or_else(|_| self.log_level.clone())
    }

    /// Returns the database path with environment variable override support
    pub fn database_path(&self) -> Option<String> {
        std::env::var("STUDIO_PROJECT_MANAGER_DATABASE_PATH")
            .ok()
            .or(self.database_path.clone())
    }

    /// Returns true if the application needs initial setup (no paths configured)
    pub fn needs_setup(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns true if the application is ready for normal operation
    pub fn is_ready_for_operation(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Returns a user-friendly status message about the configuration state
    pub fn get_status_message(&self) -> String {
        if self.needs_setup() {
            "Configuration incomplete: No project paths specified. Please add paths to begin scanning projects.".to_string()
        } else {
            format!("Configuration ready: {} project path(s) configured", self.paths.len())
        }
    }

    /// Updates the project paths and saves to config file
    pub fn update_paths(&mut self, new_paths: Vec<String>) -> Result<Vec<String>, ConfigError> {
        self.paths = new_paths;
        let warnings = self.validate()?;
        self.save_to_file()?;
        Ok(warnings)
    }

    /// Adds a path to the configuration and saves to config file
    pub fn add_path(&mut self, path: String) -> Result<Vec<String>, ConfigError> {
        if !self.paths.contains(&path) {
            self.paths.push(path);
            let warnings = self.validate()?;
            self.save_to_file()?;
            Ok(warnings)
        } else {
            Ok(vec!["Path already exists in configuration".to_string()])
        }
    }

    /// Removes a path from the configuration and saves to config file
    pub fn remove_path(&mut self, path: &str) -> Result<(), ConfigError> {
        self.paths.retain(|p| p != path);
        self.validate()?; // This will now allow empty paths
        self.save_to_file()?;
        Ok(())
    }

    /// Updates configuration settings and saves to config file
    pub fn update_settings(
        &mut self,
        database_path: Option<String>,
        live_database_dir: Option<String>,
        grpc_port: Option<u16>,
        log_level: Option<String>,
        media_storage_dir: Option<String>,
        max_cover_art_size_mb: Option<Option<u32>>,
        max_audio_file_size_mb: Option<Option<u32>>,
    ) -> Result<Vec<String>, ConfigError> {
        if let Some(db_path) = database_path {
            self.database_path = Some(db_path);
        }
        if let Some(live_db_dir) = live_database_dir {
            self.live_database_dir = live_db_dir;
        }
        if let Some(port) = grpc_port {
            self.grpc_port = port;
        }
        if let Some(level) = log_level {
            self.log_level = level;
        }
        if let Some(media_dir) = media_storage_dir {
            self.media_storage_dir = media_dir;
        }
        if let Some(cover_size) = max_cover_art_size_mb {
            self.max_cover_art_size_mb = cover_size;
        }
        if let Some(audio_size) = max_audio_file_size_mb {
            self.max_audio_file_size_mb = audio_size;
        }

        let warnings = self.validate()?;
        self.save_to_file()?;
        Ok(warnings)
    }

    /// Saves the current configuration to the config file
    pub fn save_to_file(&self) -> Result<(), ConfigError> {
        let config_path = loader::find_config_file()?;
        let config_content = self.to_toml_string()?;
        std::fs::write(&config_path, config_content).map_err(|e| ConfigError::IoError(e))?;
        Ok(())
    }

    /// Converts the configuration to TOML string format
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        // Create a simplified structure for TOML serialization
        let config_toml = toml::to_string(self).map_err(|e| ConfigError::SerializeError(e))?;
        Ok(config_toml)
    }

    /// Reloads configuration from file
    pub fn reload() -> Result<(Self, Vec<String>), ConfigError> {
        let new_config = Config::new()?;
        // The warnings are already logged during Config::new(), but we return them too
        let warnings = new_config.validate()?;
        Ok((new_config, warnings))
    }
}

/// Global configuration instance loaded lazily
pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);
