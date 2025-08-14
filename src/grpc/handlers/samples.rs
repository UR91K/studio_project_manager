use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use super::super::samples::*;
use super::super::common::*;
use super::utils::convert_live_set_to_proto;
use crate::database::LiveSetDatabase;

#[derive(Clone)]
pub struct SamplesHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl SamplesHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_all_samples(
        &self,
        request: Request<GetAllSamplesRequest>,
    ) -> Result<Response<GetAllSamplesResponse>, Status> {
        debug!("GetAllSamples request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_all_samples(
            req.limit,
            req.offset,
            req.sort_by,
            req.sort_desc,
            req.present_only,
            req.missing_only,
            req.extension_filter,
            req.min_usage_count,
            req.max_usage_count,
        ) {
            Ok((samples, total_count)) => {
                let proto_samples = samples
                    .into_iter()
                    .map(|sample| Sample {
                        id: sample.id.to_string(),
                        name: sample.name,
                        path: sample.path.to_string_lossy().to_string(),
                        is_present: sample.is_present,
                    })
                    .collect();

                let response = GetAllSamplesResponse {
                    samples: proto_samples,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get all samples: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_sample(
        &self,
        request: Request<GetSampleRequest>,
    ) -> Result<Response<GetSampleResponse>, Status> {
        debug!("GetSample request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_sample_by_id(&req.sample_id) {
            Ok(Some(sample)) => {
                let proto_sample = Sample {
                    id: sample.id.to_string(),
                    name: sample.name,
                    path: sample.path.to_string_lossy().to_string(),
                    is_present: sample.is_present,
                };

                let response = GetSampleResponse {
                    sample: Some(proto_sample),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                debug!("Sample not found with ID: {}", req.sample_id);
                Err(Status::new(
                    Code::NotFound,
                    format!("Sample not found with ID: {}", req.sample_id),
                ))
            }
            Err(e) => {
                error!("Failed to get sample: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_sample_by_presence(
        &self,
        request: Request<GetSampleByPresenceRequest>,
    ) -> Result<Response<GetSampleByPresenceResponse>, Status> {
        debug!("GetSampleByPresence request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.get_samples_by_presence(
            req.is_present,
            req.limit,
            req.offset,
            req.sort_by,
            req.sort_desc,
        ) {
            Ok((samples, total_count)) => {
                let proto_samples = samples
                    .into_iter()
                    .map(|sample| Sample {
                        id: sample.id.to_string(),
                        name: sample.name,
                        path: sample.path.to_string_lossy().to_string(),
                        is_present: sample.is_present,
                    })
                    .collect();

                let response = GetSampleByPresenceResponse {
                    samples: proto_samples,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get samples by presence: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn search_samples(
        &self,
        request: Request<SearchSamplesRequest>,
    ) -> Result<Response<SearchSamplesResponse>, Status> {
        debug!("SearchSamples request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        match db.search_samples(
            &req.query,
            req.limit,
            req.offset,
            req.present_only,
            req.extension_filter,
        ) {
            Ok((samples, total_count)) => {
                let proto_samples = samples
                    .into_iter()
                    .map(|sample| Sample {
                        id: sample.id.to_string(),
                        name: sample.name,
                        path: sample.path.to_string_lossy().to_string(),
                        is_present: sample.is_present,
                    })
                    .collect();

                let response = SearchSamplesResponse {
                    samples: proto_samples,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to search samples: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_sample_stats(
        &self,
        _request: Request<GetSampleStatsRequest>,
    ) -> Result<Response<GetSampleStatsResponse>, Status> {
        debug!("GetSampleStats request");

        let db = self.db.lock().await;

        match db.get_sample_stats() {
            Ok(stats) => {
                let response = GetSampleStatsResponse {
                    total_samples: stats.total_samples,
                    present_samples: stats.present_samples,
                    missing_samples: stats.missing_samples,
                    unique_paths: stats.unique_paths,
                    samples_by_extension: stats.samples_by_extension,
                    total_estimated_size_bytes: stats.total_estimated_size_bytes,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get sample stats: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_all_sample_usage_numbers(
        &self,
        _request: Request<GetAllSampleUsageNumbersRequest>,
    ) -> Result<Response<GetAllSampleUsageNumbersResponse>, Status> {
        debug!("GetAllSampleUsageNumbers request");

        let db = self.db.lock().await;

        match db.get_all_sample_usage_numbers() {
            Ok(usage_info) => {
                let sample_usages = usage_info
                    .into_iter()
                    .map(|info| SampleUsage {
                        sample_id: info.sample_id,
                        name: info.name,
                        path: info.path,
                        usage_count: info.usage_count,
                        project_count: info.project_count,
                    })
                    .collect();

                let response = GetAllSampleUsageNumbersResponse { sample_usages };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get sample usage numbers: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_projects_by_sample(
        &self,
        request: Request<GetProjectsBySampleRequest>,
    ) -> Result<Response<GetProjectsBySampleResponse>, Status> {
        debug!("GetProjectsBySample request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.get_projects_by_sample_id(&req.sample_id, req.limit, req.offset) {
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

                let response = GetProjectsBySampleResponse {
                    projects: proto_projects,
                    total_count,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get projects by sample: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn refresh_sample_presence_status(
        &self,
        _request: Request<RefreshSamplePresenceStatusRequest>,
    ) -> Result<Response<RefreshSamplePresenceStatusResponse>, Status> {
        debug!("RefreshSamplePresenceStatus request");

        let mut db = self.db.lock().await;

        match db.refresh_sample_presence_status() {
            Ok(result) => {
                let response = RefreshSamplePresenceStatusResponse {
                    total_samples_checked: result.total_samples_checked,
                    samples_now_present: result.samples_now_present,
                    samples_now_missing: result.samples_now_missing,
                    samples_unchanged: result.samples_unchanged,
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to refresh sample presence status: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_sample_analytics(
        &self,
        _request: Request<GetSampleAnalyticsRequest>,
    ) -> Result<Response<GetSampleAnalyticsResponse>, Status> {
        debug!("GetSampleAnalytics request");

        let db = self.db.lock().await;

        match db.get_sample_analytics() {
            Ok(analytics) => {
                // Convert top_used_samples to proto format
                let top_used_samples = analytics.top_used_samples
                    .into_iter()
                    .map(|info| SampleUsage {
                        sample_id: info.sample_id,
                        name: info.name,
                        path: info.path,
                        usage_count: info.usage_count,
                        project_count: info.project_count,
                    })
                    .collect();

                // Convert extensions to proto format
                let extensions = analytics.extensions
                    .into_iter()
                    .map(|(key, value)| {
                        (key, super::super::samples::ExtensionAnalytics {
                            count: value.count,
                            total_size_bytes: value.total_size_bytes,
                            present_count: value.present_count,
                            missing_count: value.missing_count,
                            average_usage_count: value.average_usage_count,
                        })
                    })
                    .collect();

                let proto_analytics = super::super::samples::SampleAnalytics {
                    most_used_samples_count: analytics.most_used_samples_count,
                    moderately_used_samples_count: analytics.moderately_used_samples_count,
                    rarely_used_samples_count: analytics.rarely_used_samples_count,
                    unused_samples_count: analytics.unused_samples_count,
                    extensions,
                    missing_samples_percentage: analytics.missing_samples_percentage,
                    present_samples_percentage: analytics.present_samples_percentage,
                    total_storage_bytes: analytics.total_storage_bytes,
                    present_storage_bytes: analytics.present_storage_bytes,
                    missing_storage_bytes: analytics.missing_storage_bytes,
                    top_used_samples,
                    recently_added_samples: analytics.recently_added_samples,
                };

                let response = GetSampleAnalyticsResponse {
                    analytics: Some(proto_analytics),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get sample analytics: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    pub async fn get_sample_extensions(
        &self,
        _request: Request<GetSampleExtensionsRequest>,
    ) -> Result<Response<GetSampleExtensionsResponse>, Status> {
        debug!("GetSampleExtensions request");

        let db = self.db.lock().await;

        match db.get_sample_extensions() {
            Ok(extensions) => {
                // Convert extensions to proto format
                let proto_extensions = extensions
                    .into_iter()
                    .map(|(key, value)| {
                        (key, super::super::samples::ExtensionAnalytics {
                            count: value.count,
                            total_size_bytes: value.total_size_bytes,
                            present_count: value.present_count,
                            missing_count: value.missing_count,
                            average_usage_count: value.average_usage_count,
                        })
                    })
                    .collect();

                let response = GetSampleExtensionsResponse {
                    extensions: proto_extensions,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get sample extensions: {:?}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }
}
