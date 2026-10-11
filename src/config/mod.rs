pub mod defaults;
pub mod live;
pub mod loader;
pub mod paths;
pub mod validator;
pub mod windows_paths;

use crate::error::ConfigError;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

// Re-export constants from submodules for backward compatibility
pub use defaults::{
    DEFAULT_HTTP_PORT, DEFAULT_LOG_LEVEL, DEFAULT_MAX_AUDIO_FILE_SIZE_MB,
    DEFAULT_MAX_COVER_ART_SIZE_MB,
};
pub use live::{current, reload_error, restart_required, subscribe};
pub use loader::MAX_DIRECTORY_TRAVERSAL_DEPTH;
pub use paths::MAX_PATH_LENGTH;

/// Configuration for the Seula application
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
/// # database_path = "C:\\Users\\username\\AppData\\Roaming\\Seula\\seula.db"
///
///
/// # HTTP server port (can be overridden by SEULA_HTTP_PORT env var)
/// http_port = 50052
///
/// # Logging level: error, warn, info, debug, trace
/// log_level = "info"
///
/// # Directory for storing media files
/// media_storage_dir = "C:\\Users\\username\\AppData\\Roaming\\Seula\\media"
///
/// # Media file size limits (optional, 0 = no limit, omit to use defaults)
/// # max_cover_art_size_mb = 10
/// # max_audio_file_size_mb = 50
/// ```
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Config {
    /// List of paths to scan for music projects
    pub paths: Vec<String>,
    /// Database file path (optional, defaults to user data directory)
    #[serde(default = "defaults::default_database_path")]
    pub database_path: Option<String>,
    /// HTTP server port (can be overridden by SEULA_HTTP_PORT env var)
    #[serde(default = "defaults::default_http_port")]
    pub http_port: u16,
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
    /// Directories to search for installed VST plugins. Empty means "use the platform
    /// defaults" -- see `defaults::default_vst_search_paths`.
    #[serde(default)]
    pub vst_search_paths: Vec<String>,
    /// How long the plugin scanner may go without producing output before the worker
    /// is treated as hung and killed. Generous by default: some legitimate plugins
    /// take ten seconds or more on a cold first load.
    #[serde(default = "defaults::default_vst_scan_timeout_secs")]
    pub vst_scan_timeout_secs: u64,
}

impl Config {
    /// Returns the HTTP port with environment variable override support
    pub fn http_port(&self) -> u16 {
        std::env::var("SEULA_HTTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(self.http_port)
    }

    /// Returns the log level with environment variable override support
    pub fn log_level(&self) -> String {
        std::env::var("SEULA_LOG_LEVEL").unwrap_or_else(|_| self.log_level.clone())
    }

    /// Returns the database path with environment variable override support
    pub fn database_path(&self) -> Option<String> {
        std::env::var("SEULA_DATABASE_PATH")
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
            format!(
                "Configuration ready: {} project path(s) configured",
                self.paths.len()
            )
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
        log_level: Option<String>,
        media_storage_dir: Option<String>,
        max_cover_art_size_mb: Option<Option<u32>>,
        max_audio_file_size_mb: Option<Option<u32>>,
        vst_search_paths: Option<Vec<String>>,
        vst_scan_timeout_secs: Option<u64>,
    ) -> Result<Vec<String>, ConfigError> {
        if let Some(db_path) = database_path {
            self.database_path = Some(db_path);
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
        if let Some(search_paths) = vst_search_paths {
            self.vst_search_paths = search_paths;
        }
        if let Some(timeout) = vst_scan_timeout_secs {
            self.vst_scan_timeout_secs = timeout;
        }

        let warnings = self.validate()?;
        self.save_to_file()?;
        Ok(warnings)
    }

    /// Saves the current configuration to the config file
    pub fn save_to_file(&self) -> Result<(), ConfigError> {
        let config_path = loader::find_config_file()?;
        let config_content = self.to_toml_string()?;
        write_atomically(&config_path, &config_content).map_err(ConfigError::IoError)?;
        Ok(())
    }

    /// Converts the configuration to TOML string format
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        // Create a simplified structure for TOML serialization
        let config_toml = toml::to_string(self).map_err(ConfigError::SerializeError)?;
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

/// Writes `content` to a sibling temporary file and renames it over `path`, so the file
/// watcher (and anything else reading) never sees a half-written config. If the rename is
/// refused (the target held open, say) it falls back to writing in place.
fn write_atomically(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".seula-tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, content)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(&tmp);
            std::fs::write(path, content)
        }
    }
}

/// The configuration as it was when the process started, read once. The CLI's one-shot
/// commands use it; the daemon reads `current()` instead, which follows edits (ADR-0072),
/// and compares against this to say which changes still need a restart.
pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);
