use crate::error::ConfigError;
use dirs;

/// Default HTTP port (ADR-0024).
pub const DEFAULT_HTTP_PORT: u16 = 50052;

/// Default log level. `info` so a log file records what the daemon did, not only what
/// failed (ADR-0071); a config that already says `error` keeps it.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default maximum cover art size in MB
pub const DEFAULT_MAX_COVER_ART_SIZE_MB: u32 = 10;

/// Default maximum audio file size in MB
pub const DEFAULT_MAX_AUDIO_FILE_SIZE_MB: u32 = 50;

/// Default plugin-scan timeout, in seconds
pub const DEFAULT_VST_SCAN_TIMEOUT_SECS: u64 = 30;

/// Generates a default configuration file content
pub fn generate_default_config() -> Result<String, ConfigError> {
    let roaming_data_dir = dirs::data_dir()
        .ok_or_else(|| ConfigError::InvalidPath("Could not get roaming data directory".into()))?;

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

# HTTP server configuration (can be overridden by SEULA_HTTP_PORT env var)
http_port = {}

# Logging configuration
# Options: error, warn, info, debug, trace
log_level = "{}"

# Media storage configuration
media_storage_dir = '{}'

# Media file size limits (in MB) - Optional, 0 = no limit, omit to use defaults
# max_cover_art_size_mb = 10
# max_audio_file_size_mb = 50

# VST plugin scanning
# Directories to search for installed plugins. Leave empty to use the platform defaults.
vst_search_paths = []

# Seconds a single plugin may take to load before the scanner gives up on it.
# vst_scan_timeout_secs = 30
"#,
        DEFAULT_HTTP_PORT,
        DEFAULT_LOG_LEVEL,
        media_storage_path.display()
    );

    Ok(config_content)
}

/// Default value functions for serde deserialization

pub fn default_vst_scan_timeout_secs() -> u64 {
    DEFAULT_VST_SCAN_TIMEOUT_SECS
}

/// The conventional install locations for VST plugins on this platform.
///
/// Used when `vst_search_paths` is left empty. Paths that do not exist are dropped by
/// discovery, so listing all the usual suspects here is harmless.
pub fn default_vst_search_paths() -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let program_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
        let common = std::path::PathBuf::from(&program_files).join("Common Files");
        vec![
            common.join("VST3"),
            common.join("VST2"),
            std::path::PathBuf::from(&program_files).join("VSTPlugins"),
            std::path::PathBuf::from(&program_files)
                .join("Steinberg")
                .join("VSTPlugins"),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        let mut paths = vec![
            std::path::PathBuf::from("/Library/Audio/Plug-Ins/VST3"),
            std::path::PathBuf::from("/Library/Audio/Plug-Ins/VST"),
        ];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Library/Audio/Plug-Ins/VST3"));
            paths.push(home.join("Library/Audio/Plug-Ins/VST"));
        }
        paths
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let mut paths = vec![
            std::path::PathBuf::from("/usr/lib/vst3"),
            std::path::PathBuf::from("/usr/lib/vst"),
        ];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".vst3"));
            paths.push(home.join(".vst"));
        }
        paths
    }
}

pub fn default_max_cover_art_size() -> Option<u32> {
    None // Use media module default
}

pub fn default_max_audio_file_size() -> Option<u32> {
    None // Use media module default
}

pub fn default_http_port() -> u16 {
    DEFAULT_HTTP_PORT
}

pub fn default_database_path() -> Option<String> {
    None // Default to None, which will be replaced by executable path
}

pub fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
}
