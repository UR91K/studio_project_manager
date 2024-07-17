use once_cell::sync::Lazy;
use serde::Deserialize;
use std::path::PathBuf;
use crate::errors::ConfigError;

#[derive(Deserialize)]
pub struct Config {
    pub live_database_dir: PathBuf,
}

impl Config {
    fn new() -> Result<Self, ConfigError> {
        let config_str = std::fs::read_to_string("config.toml")?;
        let mut config: Config = toml::from_str(&config_str)?;

        let home_dir = dirs::home_dir().ok_or(ConfigError::HomeDirError)?;
        let home_dir_str = home_dir.to_str().ok_or_else(|| ConfigError::InvalidPath("Home directory path is not valid UTF-8".into()))?;

        let live_database_dir_str = config.live_database_dir
            .to_str()
            .ok_or_else(|| ConfigError::InvalidPath("live_database_dir path is not valid UTF-8".into()))?
            .replace("{USER_HOME}", home_dir_str);

        config.live_database_dir = PathBuf::from(live_database_dir_str);

        Ok(config)
    }
}

pub static CONFIG: Lazy<Result<Config, ConfigError>> = Lazy::new(Config::new);