use crate::error::ConfigError;
use dirs;

/// Default gRPC port
pub const DEFAULT_GRPC_PORT: u16 = 50051;

/// Default log level
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default maximum cover art size in MB
pub const DEFAULT_MAX_COVER_ART_SIZE_MB: u32 = 10;

/// Default maximum audio file size in MB
pub const DEFAULT_MAX_AUDIO_FILE_SIZE_MB: u32 = 50;

/// Generates a default configuration file content
pub fn generate_default_config() -> Result<String, ConfigError> {
    let local_data_dir = dirs::data_local_dir()
        .ok_or_else(|| ConfigError::InvalidPath("Could not get local data directory".into()))?;
    let roaming_data_dir = dirs::data_dir()
        .ok_or_else(|| ConfigError::InvalidPath("Could not get roaming data directory".into()))?;

    let live_database_path = local_data_dir.join("Ableton").join("Live Database");
    let media_storage_path = roaming_data_dir.join("Seula").join("media");

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

/// Default value functions for serde deserialization

pub fn default_max_cover_art_size() -> Option<u32> {
    None // Use media module default
}

pub fn default_max_audio_file_size() -> Option<u32> {
    None // Use media module default
}

pub fn default_grpc_port() -> u16 {
    DEFAULT_GRPC_PORT
}

pub fn default_database_path() -> Option<String> {
    None // Default to None, which will be replaced by executable path
}

pub fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
} 