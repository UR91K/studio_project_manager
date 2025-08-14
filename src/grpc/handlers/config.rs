use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use crate::config::{Config, CONFIG};
use crate::database::LiveSetDatabase;
use super::super::config::*;

#[derive(Clone)]
pub struct ConfigHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl ConfigHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_config(
        &self,
        _request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        debug!("GetConfig request");

        let config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?;

        let config_data = ConfigData {
            paths: config.paths.clone(),
            database_path: config.database_path.clone(),
            live_database_dir: config.live_database_dir.clone(),
            grpc_port: config.grpc_port as u32,
            log_level: config.log_level.clone(),
            media_storage_dir: config.media_storage_dir.clone(),
            max_cover_art_size_mb: config.max_cover_art_size_mb,
            max_audio_file_size_mb: config.max_audio_file_size_mb,
            needs_setup: config.needs_setup(),
            status_message: config.get_status_message(),
        };

        let response = GetConfigResponse {
            config: Some(config_data),
        };
        Ok(Response::new(response))
    }

    pub async fn get_config_status(
        &self,
        _request: Request<GetConfigStatusRequest>,
    ) -> Result<Response<GetConfigStatusResponse>, Status> {
        debug!("GetConfigStatus request");

        let config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?;

        let response = GetConfigStatusResponse {
            needs_setup: config.needs_setup(),
            is_ready_for_operation: config.is_ready_for_operation(),
            status_message: config.get_status_message(),
            configured_paths_count: config.paths.len() as i32,
        };
        Ok(Response::new(response))
    }

    pub async fn update_paths(
        &self,
        request: Request<UpdatePathsRequest>,
    ) -> Result<Response<UpdatePathsResponse>, Status> {
        debug!("UpdatePaths request: {:?}", request);

        let req = request.into_inner();
        
        // Since CONFIG is a static lazy, we need to reload it to get a mutable version
        // For now, we'll work with a temporary config and save it
        let mut config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?
            .clone();

        match config.update_paths(req.paths) {
            Ok(warnings) => {
                debug!("Successfully updated paths");
                let response = UpdatePathsResponse {
                    success: true,
                    error_message: None,
                    validation_warnings: warnings,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to update paths: {:?}", e);
                let response = UpdatePathsResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                    validation_warnings: vec![],
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn add_path(
        &self,
        request: Request<AddPathRequest>,
    ) -> Result<Response<AddPathResponse>, Status> {
        debug!("AddPath request: {:?}", request);

        let req = request.into_inner();
        
        let mut config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?
            .clone();

        match config.add_path(req.path) {
            Ok(warnings) => {
                debug!("Successfully added path");
                let response = AddPathResponse {
                    success: true,
                    error_message: None,
                    validation_warnings: warnings,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to add path: {:?}", e);
                let response = AddPathResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                    validation_warnings: vec![],
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn remove_path(
        &self,
        request: Request<RemovePathRequest>,
    ) -> Result<Response<RemovePathResponse>, Status> {
        debug!("RemovePath request: {:?}", request);

        let req = request.into_inner();
        
        let mut config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?
            .clone();

        match config.remove_path(&req.path) {
            Ok(()) => {
                debug!("Successfully removed path");
                let response = RemovePathResponse {
                    success: true,
                    error_message: None,
                    remaining_paths_count: config.paths.len() as i32,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove path: {:?}", e);
                let response = RemovePathResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                    remaining_paths_count: config.paths.len() as i32,
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn update_settings(
        &self,
        request: Request<UpdateSettingsRequest>,
    ) -> Result<Response<UpdateSettingsResponse>, Status> {
        debug!("UpdateSettings request: {:?}", request);

        let req = request.into_inner();
        
        let mut config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?
            .clone();

        match config.update_settings(
            req.database_path,
            req.live_database_dir,
            req.grpc_port.map(|p| p as u16),
            req.log_level,
            req.media_storage_dir,
            req.max_cover_art_size_mb.map(Some),
            req.max_audio_file_size_mb.map(Some),
        ) {
            Ok(warnings) => {
                debug!("Successfully updated settings");
                let response = UpdateSettingsResponse {
                    success: true,
                    error_message: None,
                    validation_warnings: warnings,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to update settings: {:?}", e);
                let response = UpdateSettingsResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                    validation_warnings: vec![],
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn reload_config(
        &self,
        _request: Request<ReloadConfigRequest>,
    ) -> Result<Response<ReloadConfigResponse>, Status> {
        debug!("ReloadConfig request");

        match Config::reload() {
            Ok((new_config, warnings)) => {
                debug!("Successfully reloaded config");
                
                let config_data = ConfigData {
                    paths: new_config.paths.clone(),
                    database_path: new_config.database_path.clone(),
                    live_database_dir: new_config.live_database_dir.clone(),
                    grpc_port: new_config.grpc_port as u32,
                    log_level: new_config.log_level.clone(),
                    media_storage_dir: new_config.media_storage_dir.clone(),
                    max_cover_art_size_mb: new_config.max_cover_art_size_mb,
                    max_audio_file_size_mb: new_config.max_audio_file_size_mb,
                    needs_setup: new_config.needs_setup(),
                    status_message: new_config.get_status_message(),
                };

                let response = ReloadConfigResponse {
                    success: true,
                    error_message: None,
                    validation_warnings: warnings,
                    config: Some(config_data),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to reload config: {:?}", e);
                let response = ReloadConfigResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                    validation_warnings: vec![],
                    config: None,
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn validate_config(
        &self,
        _request: Request<ValidateConfigRequest>,
    ) -> Result<Response<ValidateConfigResponse>, Status> {
        debug!("ValidateConfig request");

        let config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Failed to load config: {}", e)))?;

        match config.validate() {
            Ok(warnings) => {
                debug!("Config validation successful with {} warnings", warnings.len());
                let response = ValidateConfigResponse {
                    is_valid: true,
                    warnings,
                    errors: vec![],
                    needs_setup: config.needs_setup(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                debug!("Config validation failed: {:?}", e);
                let response = ValidateConfigResponse {
                    is_valid: false,
                    warnings: vec![],
                    errors: vec![e.to_string()],
                    needs_setup: config.needs_setup(),
                };
                Ok(Response::new(response))
            }
        }
    }
}
