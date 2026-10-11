//! HTTP wire types for the config domain (ADR-0024).
//!
//! A mutation that fails is an HTTP error (400 for a path or setting that cannot be
//! used, 500 for a config file that cannot be written); one that succeeds answers with
//! any validation warnings.

use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Serialize)]
pub struct ConfigDto {
    pub paths: Vec<String>,
    pub database_path: Option<String>,
    pub log_level: String,
    pub media_storage_dir: String,
    pub max_cover_art_size_mb: Option<u32>,
    pub max_audio_file_size_mb: Option<u32>,
    /// Empty means the platform defaults.
    pub vst_search_paths: Vec<String>,
    pub vst_scan_timeout_secs: u64,
    /// Read-only over the API: it belongs to the listening socket (ADR-0072).
    pub http_port: u16,
    pub needs_setup: bool,
    pub status_message: String,
}

impl From<&Config> for ConfigDto {
    fn from(config: &Config) -> Self {
        Self {
            paths: config.paths.clone(),
            database_path: config.database_path.clone(),
            log_level: config.log_level.clone(),
            media_storage_dir: config.media_storage_dir.clone(),
            max_cover_art_size_mb: config.max_cover_art_size_mb,
            max_audio_file_size_mb: config.max_audio_file_size_mb,
            vst_search_paths: config.vst_search_paths.clone(),
            vst_scan_timeout_secs: config.vst_scan_timeout_secs,
            http_port: config.http_port(),
            needs_setup: config.needs_setup(),
            status_message: config.get_status_message(),
        }
    }
}

#[derive(Serialize)]
pub struct GetConfigResponse {
    pub config: ConfigDto,
}

#[derive(Serialize)]
pub struct ConfigStatusResponse {
    pub needs_setup: bool,
    pub is_ready_for_operation: bool,
    pub status_message: String,
    pub configured_paths_count: i32,
    /// Why the last edit of config.toml was not applied; null once a load succeeds.
    pub reload_error: Option<String>,
    /// Keys whose file value differs from the running one.
    pub restart_required: Vec<String>,
}

#[derive(Deserialize)]
pub struct UpdatePathsRequest {
    pub paths: Vec<String>,
}

#[derive(Deserialize)]
pub struct AddPathRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct RemovePathRequest {
    pub path: String,
}

#[derive(Serialize)]
pub struct PathMutationResponse {
    pub validation_warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct RemovePathResponse {
    pub remaining_paths_count: i32,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub database_path: Option<String>,
    pub log_level: Option<String>,
    pub media_storage_dir: Option<String>,
    pub max_cover_art_size_mb: Option<Option<u32>>,
    pub max_audio_file_size_mb: Option<Option<u32>>,
    pub vst_search_paths: Option<Vec<String>>,
    pub vst_scan_timeout_secs: Option<u64>,
}

#[derive(Serialize)]
pub struct ReloadConfigResponse {
    pub validation_warnings: Vec<String>,
    pub config: ConfigDto,
}

#[derive(Serialize)]
pub struct ValidateConfigResponse {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub needs_setup: bool,
}
