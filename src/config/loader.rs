use crate::config::{Config, defaults};
use crate::error::ConfigError;
use dirs;
use std::path::PathBuf;
use toml;

/// Maximum depth to traverse when searching for config file relative to executable
pub const MAX_DIRECTORY_TRAVERSAL_DEPTH: usize = 5;

impl Config {
    /// Creates a new Config instance by loading from the config file
    pub fn new() -> Result<Self, ConfigError> {
        let config_path = find_config_file()?;
        let config_str = std::fs::read_to_string(&config_path).map_err(|e| {
            ConfigError::IoError(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                ),
            ))
        })?;

        let mut config: Config =
            toml::from_str(&config_str).map_err(|e| ConfigError::ParseError(e))?;

        // Process {USER_HOME} placeholders
        process_user_home_placeholders(&mut config)?;

        // Set default database path if needed
        set_default_database_path(&mut config)?;

        // Validate the configuration and collect warnings
        let warnings = config.validate()?;

        // Log warnings if any
        for warning in warnings {
            eprintln!("Config warning: {}", warning);
        }

        Ok(config)
    }
}

/// Processes {USER_HOME} placeholders in all configuration paths
fn process_user_home_placeholders(config: &mut Config) -> Result<(), ConfigError> {
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
        .as_ref()
        .map(|path| path.replace("{USER_HOME}", home_dir_str));
    config.live_database_dir = config
        .live_database_dir
        .replace("{USER_HOME}", home_dir_str);
    config.media_storage_dir = config
        .media_storage_dir
        .replace("{USER_HOME}", home_dir_str);

    Ok(())
}

/// Sets the default database path if none is specified
fn set_default_database_path(config: &mut Config) -> Result<(), ConfigError> {
    // If database_path is None or empty, set it to the user's data directory
    if config
        .database_path
        .as_deref()
        .map_or(true, |s| s.trim().is_empty())
    {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| ConfigError::InvalidPath("Could not get data directory".into()))?;
        let app_data_dir = data_dir.join("Seula");
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

    Ok(())
}

/// Finds the configuration file using the search strategy:
/// 1. Environment variable STUDIO_PROJECT_MANAGER_CONFIG
/// 2. AppData directory (primary location for deployed apps)
/// 3. Relative to executable (for development/portable use)
/// 4. Creates default config in AppData if none found
pub fn find_config_file() -> Result<PathBuf, ConfigError> {
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
    let app_config_dir = config_dir.join("Seula");
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
        let default_config = defaults::generate_default_config()?;
    std::fs::write(&appdata_config_path, default_config).map_err(|e| ConfigError::IoError(e))?;

    Ok(appdata_config_path)
} 