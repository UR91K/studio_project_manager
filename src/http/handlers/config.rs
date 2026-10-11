//! Config domain HTTP handlers (ADR-0024). Thin over `ConfigService`. A config that
//! fails to load is a 500; a mutation that fails is a 400 when the path or setting
//! cannot be used and a 500 when the file cannot be written.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::http::dto::config::{
    AddPathRequest, ConfigDto, ConfigStatusResponse, GetConfigResponse, PathMutationResponse,
    ReloadConfigResponse, RemovePathRequest, RemovePathResponse, UpdatePathsRequest,
    UpdateSettingsRequest, ValidateConfigResponse,
};
use crate::http::error::ApiError;
use crate::http::state::AppState;

fn load_error(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(format!("Failed to load config: {}", e))
}

pub async fn get_config(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let config = state.services.config.get().map_err(load_error)?;
    Ok(Json(GetConfigResponse {
        config: ConfigDto::from(&*config),
    }))
}

pub async fn get_config_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let config = state.services.config.get().map_err(load_error)?;
    Ok(Json(ConfigStatusResponse {
        needs_setup: config.needs_setup(),
        is_ready_for_operation: config.is_ready_for_operation(),
        status_message: config.get_status_message(),
        configured_paths_count: config.paths.len() as i32,
        reload_error: state.services.config.reload_error(),
        restart_required: state.services.config.restart_required(),
    }))
}

pub async fn update_paths(
    State(state): State<AppState>,
    Json(req): Json<UpdatePathsRequest>,
) -> Result<Json<PathMutationResponse>, ApiError> {
    let (_, warnings) = state.services.config.update_paths(req.paths)?;
    Ok(Json(PathMutationResponse {
        validation_warnings: warnings,
    }))
}

pub async fn add_path(
    State(state): State<AppState>,
    Json(req): Json<AddPathRequest>,
) -> Result<Json<PathMutationResponse>, ApiError> {
    let (_, warnings) = state.services.config.add_path(req.path)?;
    Ok(Json(PathMutationResponse {
        validation_warnings: warnings,
    }))
}

pub async fn remove_path(
    State(state): State<AppState>,
    Json(req): Json<RemovePathRequest>,
) -> Result<Json<RemovePathResponse>, ApiError> {
    let config = state.services.config.remove_path(&req.path)?;
    Ok(Json(RemovePathResponse {
        remaining_paths_count: config.paths.len() as i32,
    }))
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(req): Json<UpdateSettingsRequest>,
) -> Result<Json<PathMutationResponse>, ApiError> {
    let (_, warnings) = state.services.config.update_settings(
        req.database_path,
        req.log_level,
        req.media_storage_dir,
        req.max_cover_art_size_mb,
        req.max_audio_file_size_mb,
        req.vst_search_paths,
        req.vst_scan_timeout_secs,
    )?;
    Ok(Json(PathMutationResponse {
        validation_warnings: warnings,
    }))
}

pub async fn reload_config(
    State(state): State<AppState>,
) -> Result<Json<ReloadConfigResponse>, ApiError> {
    let (new_config, warnings) = state.services.config.reload()?;
    Ok(Json(ReloadConfigResponse {
        validation_warnings: warnings,
        config: ConfigDto::from(&*new_config),
    }))
}

pub async fn validate_config(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let needs_setup = state
        .services
        .config
        .get()
        .map_err(load_error)?
        .needs_setup();

    Ok(Json(match state.services.config.validate() {
        Ok(warnings) => ValidateConfigResponse {
            is_valid: true,
            warnings,
            errors: vec![],
            needs_setup,
        },
        Err(e) => ValidateConfigResponse {
            is_valid: false,
            warnings: vec![],
            errors: vec![e.to_string()],
            needs_setup,
        },
    }))
}
