// /src/config.rs
use crate::error::ConfigError;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Config {
    pub paths: Vec<String>,
    pub database_path: String,
    pub live_database_dir: String,
    // gRPC server configuration
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    // Media storage configuration
    pub media_storage_dir: String,
    #[serde(default = "default_max_cover_art_size")]
    pub max_cover_art_size_mb: u32,
    #[serde(default = "default_max_audio_file_size")]
    pub max_audio_file_size_mb: u32,
    #[serde(default = "default_allowed_image_formats")]
    pub allowed_image_formats: Vec<String>,
    #[serde(default = "default_allowed_audio_formats")]
    pub allowed_audio_formats: Vec<String>,
}

impl Config {
    fn new() -> Result<Self, ConfigError> {
        let config_path = find_config_file()?;
        let config_str =
            std::fs::read_to_string(&config_path).map_err(|e| ConfigError::ReadError(e))?;

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
        config.database_path = config.database_path.replace("{USER_HOME}", home_dir_str);
        config.live_database_dir = config
            .live_database_dir
            .replace("{USER_HOME}", home_dir_str);
        config.media_storage_dir = config
            .media_storage_dir
            .replace("{USER_HOME}", home_dir_str);

        Ok(config)
    }
}

fn find_config_file() -> Result<PathBuf, ConfigError> {
    // First check environment variable
    if let Ok(config_path) = std::env::var("STUDIO_PROJECT_MANAGER_CONFIG") {
        let path = PathBuf::from(config_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // Fall back to searching relative to executable
    let mut dir = std::env::current_exe().map_err(|e| ConfigError::ReadError(e))?;

    // Navigate up the directory tree until we find the config file or reach the root
    while dir.pop() {
        let config_path = dir.join("config.toml");
        if config_path.exists() {
            return Ok(config_path);
        }
    }

    Err(ConfigError::ReadError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Could not find config.toml",
    )))
}

fn default_max_cover_art_size() -> u32 {
    10 // 10MB
}

fn default_max_audio_file_size() -> u32 {
    50 // 50MB
}

fn default_allowed_image_formats() -> Vec<String> {
    vec!["jpg".to_string(), "jpeg".to_string(), "png".to_string(), "webp".to_string()]
}

fn default_allowed_audio_formats() -> Vec<String> {
    vec!["mp3".to_string(), "wav".to_string(), "m4a".to_string(), "flac".to_string()]
}

fn default_grpc_port() -> u16 {
    50051 // Default gRPC port
}

pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);
