use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, Code};
use log::{debug, error};

use crate::database::LiveSetDatabase;
use super::super::proto::*;
use super::utils::convert_live_set_to_proto;

pub struct PluginsHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl PluginsHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_all_plugins(
        &self,
        request: Request<GetAllPluginsRequest>,
    ) -> Result<Response<GetAllPluginsResponse>, Status> {
        debug!("GetAllPlugins request: {:?}", request);
        
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        match db.get_all_plugins(req.limit, req.offset, req.sort_by, req.sort_desc) {
            Ok((plugins, total_count)) => {
                let proto_plugins = plugins.into_iter().map(|plugin| {
                    Plugin {
                        id: plugin.id.to_string(),
                        ableton_plugin_id: plugin.plugin_id,
                        ableton_module_id: plugin.module_id,
                        dev_identifier: plugin.dev_identifier,
                        name: plugin.name,
                        format: plugin.plugin_format.to_string(),
                        installed: plugin.installed,
                        vendor: plugin.vendor,
                        version: plugin.version,
                        sdk_version: plugin.sdk_version,
                        flags: plugin.flags,
                        scanstate: plugin.scanstate,
                        enabled: plugin.enabled,
                    }
                }).collect();

                let response = GetAllPluginsResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get all plugins: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn get_plugin_by_installed_status(
        &self,
        request: Request<GetPluginByInstalledStatusRequest>,
    ) -> Result<Response<GetPluginByInstalledStatusResponse>, Status> {
        debug!("GetPluginByInstalledStatus request: {:?}", request);
        
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        match db.get_plugins_by_installed_status(
            req.installed,
            req.limit,
            req.offset,
            req.sort_by,
            req.sort_desc,
        ) {
            Ok((plugins, total_count)) => {
                let proto_plugins = plugins.into_iter().map(|plugin| {
                    Plugin {
                        id: plugin.id.to_string(),
                        ableton_plugin_id: plugin.plugin_id,
                        ableton_module_id: plugin.module_id,
                        dev_identifier: plugin.dev_identifier,
                        name: plugin.name,
                        format: plugin.plugin_format.to_string(),
                        installed: plugin.installed,
                        vendor: plugin.vendor,
                        version: plugin.version,
                        sdk_version: plugin.sdk_version,
                        flags: plugin.flags,
                        scanstate: plugin.scanstate,
                        enabled: plugin.enabled,
                    }
                }).collect();

                let response = GetPluginByInstalledStatusResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugins by installed status: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn search_plugins(
        &self,
        request: Request<SearchPluginsRequest>,
    ) -> Result<Response<SearchPluginsResponse>, Status> {
        debug!("SearchPlugins request: {:?}", request);
        
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        match db.search_plugins(
            &req.query,
            req.limit,
            req.offset,
            req.installed_only,
            req.vendor_filter,
            req.format_filter,
        ) {
            Ok((plugins, total_count)) => {
                let proto_plugins = plugins.into_iter().map(|plugin| {
                    Plugin {
                        id: plugin.id.to_string(),
                        ableton_plugin_id: plugin.plugin_id,
                        ableton_module_id: plugin.module_id,
                        dev_identifier: plugin.dev_identifier,
                        name: plugin.name,
                        format: plugin.plugin_format.to_string(),
                        installed: plugin.installed,
                        vendor: plugin.vendor,
                        version: plugin.version,
                        sdk_version: plugin.sdk_version,
                        flags: plugin.flags,
                        scanstate: plugin.scanstate,
                        enabled: plugin.enabled,
                    }
                }).collect();

                let response = SearchPluginsResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to search plugins: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn get_plugin_stats(
        &self,
        _request: Request<GetPluginStatsRequest>,
    ) -> Result<Response<GetPluginStatsResponse>, Status> {
        debug!("GetPluginStats request");
        
        let db = self.db.lock().await;
        
        match db.get_plugin_stats() {
            Ok(stats) => {
                let response = GetPluginStatsResponse {
                    total_plugins: stats.total_plugins,
                    installed_plugins: stats.installed_plugins,
                    missing_plugins: stats.missing_plugins,
                    unique_vendors: stats.unique_vendors,
                    plugins_by_format: stats.plugins_by_format,
                    plugins_by_vendor: stats.plugins_by_vendor,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugin stats: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn get_all_plugin_usage_numbers(
        &self,
        _request: Request<GetAllPluginUsageNumbersRequest>,
    ) -> Result<Response<GetAllPluginUsageNumbersResponse>, Status> {
        debug!("GetAllPluginUsageNumbers request");
        
        let db = self.db.lock().await;
        
        match db.get_all_plugin_usage_numbers() {
            Ok(usage_info) => {
                let plugin_usages = usage_info.into_iter().map(|info| {
                    PluginUsage {
                        plugin_id: info.plugin_id,
                        name: info.name,
                        vendor: info.vendor.unwrap_or_default(), // TODO: plugin should not ever be in incorrect state.
                        usage_count: info.usage_count,
                        project_count: info.project_count,
                    }
                }).collect();

                let response = GetAllPluginUsageNumbersResponse {
                    plugin_usages,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugin usage numbers: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    pub async fn get_projects_by_plugin(
        &self,
        request: Request<GetProjectsByPluginRequest>,
    ) -> Result<Response<GetProjectsByPluginResponse>, Status> {
        debug!("GetProjectsByPlugin request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.get_projects_by_plugin_id(&req.plugin_id, req.limit, req.offset) {
            Ok((projects, total_count)) => {
                let mut proto_projects = Vec::new();
                
                for project in projects {
                    match convert_live_set_to_proto(project, &mut *db) {
                        Ok(proto_project) => proto_projects.push(proto_project),
                        Err(e) => {
                            error!("Failed to convert project to proto: {:?}", e);
                            return Err(Status::internal(format!("Database error: {}", e)));
                        }
                    }
                }

                let response = GetProjectsByPluginResponse {
                    projects: proto_projects,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get projects by plugin: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }
} 