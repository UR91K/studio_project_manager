use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use super::super::plugins::*;
use super::super::common::*;
use super::utils::convert_live_set_to_proto;
use crate::database::LiveSetDatabase;

#[derive(Clone)]
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

        match db.get_all_plugins(
            req.limit,
            req.offset,
            req.sort_by,
            req.sort_desc,
            req.vendor_filter,
            req.format_filter,
            req.installed_only,
            req.min_usage_count,
        ) {
            Ok((grpc_plugins, total_count)) => {
                let proto_plugins = grpc_plugins
                    .into_iter()
                    .map(|grpc_plugin| Plugin {
                        id: grpc_plugin.plugin.id.to_string(),
                        ableton_plugin_id: grpc_plugin.plugin.plugin_id,
                        ableton_module_id: grpc_plugin.plugin.module_id,
                        dev_identifier: grpc_plugin.plugin.dev_identifier,
                        name: grpc_plugin.plugin.name,
                        format: grpc_plugin.plugin.plugin_format.to_string(),
                        installed: grpc_plugin.plugin.installed,
                        vendor: grpc_plugin.plugin.vendor,
                        version: grpc_plugin.plugin.version,
                        sdk_version: grpc_plugin.plugin.sdk_version,
                        flags: grpc_plugin.plugin.flags,
                        scanstate: grpc_plugin.plugin.scanstate,
                        enabled: grpc_plugin.plugin.enabled,
                        usage_count: Some(grpc_plugin.usage_count),
                        project_count: Some(grpc_plugin.project_count),
                    })
                    .collect();

                let response = GetAllPluginsResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get all plugins: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
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
                let proto_plugins = plugins
                    .into_iter()
                    .map(|plugin| Plugin {
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
                        usage_count: None, // This method doesn't include usage data
                        project_count: None, // This method doesn't include usage data
                    })
                    .collect();

                let response = GetPluginByInstalledStatusResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugins by installed status: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
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
                let proto_plugins = plugins
                    .into_iter()
                    .map(|plugin| Plugin {
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
                        usage_count: None, // This method doesn't include usage data
                        project_count: None, // This method doesn't include usage data
                    })
                    .collect();

                let response = SearchPluginsResponse {
                    plugins: proto_plugins,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to search plugins: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
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
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_plugin_vendors(
        &self,
        request: Request<GetPluginVendorsRequest>,
    ) -> Result<Response<GetPluginVendorsResponse>, Status> {
        debug!("GetPluginVendors request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_plugin_vendors(req.limit, req.offset, req.sort_by, req.sort_desc) {
            Ok((vendors, total_count)) => {
                let proto_vendors = vendors
                    .into_iter()
                    .map(|vendor| VendorInfo {
                        vendor: vendor.vendor,
                        plugin_count: vendor.plugin_count,
                        installed_plugins: vendor.installed_plugins,
                        missing_plugins: vendor.missing_plugins,
                        total_usage_count: vendor.total_usage_count,
                        unique_projects_using: vendor.unique_projects_using,
                        plugins_by_format: vendor.plugins_by_format,
                    })
                    .collect();

                let response = GetPluginVendorsResponse {
                    vendors: proto_vendors,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugin vendors: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_plugin_formats(
        &self,
        request: Request<GetPluginFormatsRequest>,
    ) -> Result<Response<GetPluginFormatsResponse>, Status> {
        debug!("GetPluginFormats request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_plugin_formats(req.limit, req.offset, req.sort_by, req.sort_desc) {
            Ok((formats, total_count)) => {
                let proto_formats = formats
                    .into_iter()
                    .map(|format| FormatInfo {
                        format: format.format,
                        plugin_count: format.plugin_count,
                        installed_plugins: format.installed_plugins,
                        missing_plugins: format.missing_plugins,
                        total_usage_count: format.total_usage_count,
                        unique_projects_using: format.unique_projects_using,
                        plugins_by_vendor: format.plugins_by_vendor,
                    })
                    .collect();

                let response = GetPluginFormatsResponse {
                    formats: proto_formats,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get plugin formats: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_plugin(
        &self,
        request: Request<GetPluginRequest>,
    ) -> Result<Response<GetPluginResponse>, Status> {
        debug!("GetPlugin request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_plugin_by_id(&req.plugin_id) {
            Ok(Some(grpc_plugin)) => {
                let proto_plugin = Plugin {
                    id: grpc_plugin.plugin.id.to_string(),
                    ableton_plugin_id: grpc_plugin.plugin.plugin_id,
                    ableton_module_id: grpc_plugin.plugin.module_id,
                    dev_identifier: grpc_plugin.plugin.dev_identifier,
                    name: grpc_plugin.plugin.name,
                    format: grpc_plugin.plugin.plugin_format.to_string(),
                    installed: grpc_plugin.plugin.installed,
                    vendor: grpc_plugin.plugin.vendor,
                    version: grpc_plugin.plugin.version,
                    sdk_version: grpc_plugin.plugin.sdk_version,
                    flags: grpc_plugin.plugin.flags,
                    scanstate: grpc_plugin.plugin.scanstate,
                    enabled: grpc_plugin.plugin.enabled,
                    usage_count: Some(grpc_plugin.usage_count),
                    project_count: Some(grpc_plugin.project_count),
                };

                let response = GetPluginResponse {
                    plugin: Some(proto_plugin),
                    usage_count: grpc_plugin.usage_count,
                    project_count: grpc_plugin.project_count,
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                Err(Status::new(
                    Code::NotFound,
                    format!("Plugin with ID {} not found", req.plugin_id),
                ))
            }
            Err(e) => {
                error!("Failed to get plugin: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
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
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn refresh_plugin_installation_status(
        &self,
        _request: Request<RefreshPluginInstallationStatusRequest>,
    ) -> Result<Response<RefreshPluginInstallationStatusResponse>, Status> {
        debug!("RefreshPluginInstallationStatus request");

        let mut db = self.db.lock().await;

        match db.refresh_plugin_installation_status() {
            Ok(result) => {
                let response = RefreshPluginInstallationStatusResponse {
                    total_plugins_checked: result.total_plugins_checked,
                    plugins_now_installed: result.plugins_now_installed,
                    plugins_now_missing: result.plugins_now_missing,
                    plugins_unchanged: result.plugins_unchanged,
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to refresh plugin installation status: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }
}
