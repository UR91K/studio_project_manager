use crate::error::ConfigError;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::path::PathBuf;

/// Maximum depth to traverse when searching for config file relative to executable
pub const MAX_DIRECTORY_TRAVERSAL_DEPTH: usize = 5;

/// Default gRPC port
pub const DEFAULT_GRPC_PORT: u16 = 50051;

/// Default maximum cover art size in MB
pub const DEFAULT_MAX_COVER_ART_SIZE_MB: u32 = 10;

/// Default maximum audio file size in MB
pub const DEFAULT_MAX_AUDIO_FILE_SIZE_MB: u32 = 50;

/// Default log level
pub const DEFAULT_LOG_LEVEL: &str = "info";

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
/// live_database_dir = "C:\\Users\\username\\AppData\\Roaming\\Ableton\\Live Database"
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
    #[serde(default = "default_database_path")]
    pub database_path: Option<String>,
    /// Directory containing Ableton Live database files
    pub live_database_dir: String,
    /// gRPC server port (can be overridden by STUDIO_PROJECT_MANAGER_GRPC_PORT env var)
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    /// Logging level
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Directory for storing media files
    pub media_storage_dir: String,
    /// Maximum cover art file size in MB (0 = no limit, None = use media module default)
    #[serde(default = "default_max_cover_art_size")]
    pub max_cover_art_size_mb: Option<u32>,
    /// Maximum audio file size in MB (0 = no limit, None = use media module default)
    #[serde(default = "default_max_audio_file_size")]
    pub max_audio_file_size_mb: Option<u32>,
}

impl Config {
    /// Creates a new Config instance by loading from the config file
    fn new() -> Result<Self, ConfigError> {
        let config_path = find_config_file()?;
        let config_str =
            std::fs::read_to_string(&config_path).map_err(|e| ConfigError::IoError(e))?;

        let mut config: Config =
            toml::from_str(&config_str).map_err(|e| ConfigError::ParseError(e))?;

        let home_dir = dirs::home_dir().ok_or(ConfigError::HomeDirError)?;
        let home_dir_str = home_dir.to_str().ok_or_else(|| {
            ConfigError::InvalidPath("Home directory path is not valid UTF-8".into())
        })?;

        // Replace {USER_HOME} in all paths
        config.paths = config
            .paths
            .iter()
            .map(|path| path.replace("{USER_HOME}", home_dir_str))
            .collect();
        config.database_path = config
            .database_path
            .map(|path| path.replace("{USER_HOME}", home_dir_str));
        config.live_database_dir = config
            .live_database_dir
            .replace("{USER_HOME}", home_dir_str);
        config.media_storage_dir = config
            .media_storage_dir
            .replace("{USER_HOME}", home_dir_str);

        // If database_path is None or empty, set it to the user's data directory
        if config.database_path.is_none()
            || config
                .database_path
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            let data_dir = dirs::data_dir()
                .ok_or_else(|| ConfigError::InvalidPath("Could not get data directory".into()))?;
            let app_data_dir = data_dir.join("StudioProjectManager");
            std::fs::create_dir_all(&app_data_dir).map_err(|e| ConfigError::IoError(e))?;

            config.database_path = Some(
                app_data_dir
                    .join("ableton_live_sets.db")
                    .to_str()
                    .ok_or_else(|| {
                        ConfigError::InvalidPath("Database path is not valid UTF-8".into())
                    })?
                    .to_string(),
            );
        }

        // Validate the configuration and collect warnings
        let warnings = config.validate()?;
        
        // Log warnings if any
        for warning in warnings {
            eprintln!("Config warning: {}", warning);
        }

        Ok(config)
    }

    /// Validates the configuration for logical constraints
    fn validate(&self) -> Result<Vec<String>, ConfigError> {
        let mut warnings = Vec::new();
        
        if self.paths.is_empty() {
            return Err(ConfigError::InvalidValue(
                "At least one path must be specified".into(),
            ));
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
    fn validate_paths(&self) -> Result<Vec<String>, ConfigError> {
        let mut warnings = Vec::new();
        
        for path in &self.paths {
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
                    // Check write permissions for directories that need to be writable
                    if let Ok(metadata) = path_buf.metadata() {
                        if metadata.permissions().readonly() {
                            warnings.push(format!("Directory is read-only: {}", path));
                        }
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
        let live_db_path = PathBuf::from(&self.live_database_dir);
        if !live_db_path.exists() {
            warnings.push(format!("Live database directory does not exist: {}", self.live_database_dir));
        }

        // Validate media storage directory
        let media_storage_path = PathBuf::from(&self.media_storage_dir);
        if !media_storage_path.exists() {
            // Try to create the media storage directory
            if let Err(e) = std::fs::create_dir_all(&media_storage_path) {
                warnings.push(format!("Could not create media storage directory: {}", e));
            }
        }
        
        Ok(warnings)
    }

    /// Returns the gRPC port with environment variable override support
    pub fn grpc_port(&self) -> u16 {
        std::env::var("STUDIO_PROJECT_MANAGER_GRPC_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(self.grpc_port)
    }

    /// Returns the log level with environment variable override support
    pub fn log_level(&self) -> String {
        std::env::var("STUDIO_PROJECT_MANAGER_LOG_LEVEL")
            .unwrap_or_else(|_| self.log_level.clone())
    }

    /// Returns the database path with environment variable override support
    pub fn database_path(&self) -> Option<String> {
        std::env::var("STUDIO_PROJECT_MANAGER_DATABASE_PATH")
            .ok()
            .or(self.database_path.clone())
    }
}

/// Finds the configuration file using the search strategy:
/// 1. Environment variable STUDIO_PROJECT_MANAGER_CONFIG
/// 2. AppData directory (primary location for deployed apps)
/// 3. Relative to executable (for development/portable use)
/// 4. Creates default config in AppData if none found
fn find_config_file() -> Result<PathBuf, ConfigError> {
    // First check environment variable
    if let Ok(config_path) = std::env::var("STUDIO_PROJECT_MANAGER_CONFIG") {
        let path = PathBuf::from(config_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // Check AppData directory first (primary location for deployed apps)
    let config_dir = dirs::config_dir()
        .ok_or_else(|| ConfigError::InvalidPath("Could not get config directory".into()))?;
    let app_config_dir = config_dir.join("StudioProjectManager");
    let appdata_config_path = app_config_dir.join("config.toml");

    if appdata_config_path.exists() {
        return Ok(appdata_config_path);
    }

    // Fall back to searching relative to executable (for development/portable use)
    let mut dir = std::env::current_exe().map_err(|e| ConfigError::IoError(e))?;
    dir.pop(); // Remove the executable name to get the directory

    // Navigate up the directory tree until we find the config file or reach the root
    for _ in 0..MAX_DIRECTORY_TRAVERSAL_DEPTH {
        let config_path = dir.join("config.toml");
        if config_path.exists() {
            return Ok(config_path);
        }

        // Try to go up one directory level
        if !dir.pop() {
            // Reached the root directory, break out of the loop
            break;
        }
    }

    // If no config file found, create one in the user's AppData directory
    std::fs::create_dir_all(&app_config_dir).map_err(|e| ConfigError::IoError(e))?;

    // Generate and write default config
    let default_config = generate_default_config()?;
    std::fs::write(&appdata_config_path, default_config).map_err(|e| ConfigError::IoError(e))?;

    Ok(appdata_config_path)
}

fn default_max_cover_art_size() -> Option<u32> {
    None // Use media module default
}

fn default_max_audio_file_size() -> Option<u32> {
    None // Use media module default
}

fn default_grpc_port() -> u16 {
    DEFAULT_GRPC_PORT
}

fn default_database_path() -> Option<String> {
    None // Default to None, which will be replaced by executable path
}

fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
}

/// Generates a default configuration file content
fn generate_default_config() -> Result<String, ConfigError> {
    // Get proper directories using dirs crate
    let data_dir = dirs::data_dir()
        .ok_or_else(|| ConfigError::InvalidPath("Could not get data directory".into()))?;

    let live_database_path = data_dir.join("Ableton").join("Live Database");
    let media_storage_path = data_dir.join("StudioProjectManager").join("media");

    let config_content = format!(
        r#"# config.toml

# if you change this file while the program is running, you need to restart the program for changes to take effect.

paths = [
    # put your project folder paths here
]

# use {{USER_HOME}} as a shortcut to your user folder

# Database configuration
# If database_path is not specified or empty, it will default to the user's data directory
# database_path = ''

live_database_dir = '{}'

# gRPC server configuration
grpc_port = {}

# Logging configuration
# Options: error, warn, info, debug, trace
log_level = "{}"

# Media storage configuration
media_storage_dir = '{}'

# Media file size limits (in MB) - Optional, 0 = no limit, omit to use defaults
# max_cover_art_size_mb = 10
# max_audio_file_size_mb = 50
"#,
        live_database_path.display(),
        DEFAULT_GRPC_PORT,
        DEFAULT_LOG_LEVEL,
        media_storage_path.display()
    );

    Ok(config_content)
}

/// Global configuration instance loaded lazily
pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);
