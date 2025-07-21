pub mod windows_paths;
pub mod loader;
pub mod validator;
pub mod paths;
pub mod defaults;

use crate::error::ConfigError;
use once_cell::sync::Lazy;
use serde::Deserialize;

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
#[derive(Deserialize, Debug, Clone)]
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
}

/// Global configuration instance loaded lazily
pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);
