use crate::config::{self, Config};
use crate::error::ConfigError;
use std::sync::Arc;

/// The config lives in the swappable cell in `config::live` (ADR-0072), not in
/// `ProjectDatabase` -- this service holds no state of its own. Every mutation saves
/// `config.toml` and puts the result in force at once, so the next scan uses it. The
/// CLI's `config` command reads the startup `CONFIG` directly and has no mutation
/// subcommands to route through this yet.
#[derive(Clone, Default)]
pub struct ConfigService;

impl ConfigService {
    pub fn new() -> Self {
        Self
    }

    /// The config in force now.
    pub fn get(&self) -> Result<Arc<Config>, ConfigError> {
        config::current()
    }

    pub fn update_paths(
        &self,
        paths: Vec<String>,
    ) -> Result<(Arc<Config>, Vec<String>), ConfigError> {
        config::live::mutate(|c| c.update_paths(paths))
    }

    pub fn add_path(&self, path: String) -> Result<(Arc<Config>, Vec<String>), ConfigError> {
        config::live::mutate(|c| c.add_path(path))
    }

    pub fn remove_path(&self, path: &str) -> Result<Arc<Config>, ConfigError> {
        config::live::mutate(|c| c.remove_path(path)).map(|(config, ())| config)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_settings(
        &self,
        database_path: Option<String>,
        log_level: Option<String>,
        media_storage_dir: Option<String>,
        max_cover_art_size_mb: Option<Option<u32>>,
        max_audio_file_size_mb: Option<Option<u32>>,
        vst_search_paths: Option<Vec<String>>,
        vst_scan_timeout_secs: Option<u64>,
    ) -> Result<(Arc<Config>, Vec<String>), ConfigError> {
        config::live::mutate(|c| {
            c.update_settings(
                database_path,
                log_level,
                media_storage_dir,
                max_cover_art_size_mb,
                max_audio_file_size_mb,
                vst_search_paths,
                vst_scan_timeout_secs,
            )
        })
    }

    /// Reads `config.toml` again and puts it in force. A file that does not load changes
    /// nothing and is reported by `reload_error`.
    pub fn reload(&self) -> Result<(Arc<Config>, Vec<String>), ConfigError> {
        config::live::reload()
    }

    /// Why the last edit of the file was not applied, if it was not.
    pub fn reload_error(&self) -> Option<String> {
        config::reload_error()
    }

    /// Keys whose file value differs from the running one.
    pub fn restart_required(&self) -> Vec<String> {
        config::restart_required()
    }

    pub fn validate(&self) -> Result<Vec<String>, ConfigError> {
        self.get()?.validate()
    }
}
