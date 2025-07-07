use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Code};
use log::{debug, info, error};

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::database::search::SearchQuery;
use crate::live_set::LiveSet;
use crate::process_projects;

use super::proto::*;

pub struct StudioProjectManagerServer {
    db: Arc<Mutex<LiveSetDatabase>>,
    scan_status: Arc<Mutex<ScanStatus>>,
}

impl StudioProjectManagerServer {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = CONFIG.as_ref()
            .map_err(|e| format!("Failed to load config: {}", e))?;
        
        let db_path = PathBuf::from(&config.database_path);
        let db = LiveSetDatabase::new(db_path)
            .map_err(|e| format!("Failed to initialize database: {}", e))?;
        
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            scan_status: Arc::new(Mutex::new(ScanStatus::ScanUnknown)),
        })
    }
}

#[tonic::async_trait]
impl studio_project_manager_server::StudioProjectManager for StudioProjectManagerServer {
    async fn get_projects(
        &self,
        request: Request<GetProjectsRequest>,
    ) -> Result<Response<GetProjectsResponse>, Status> {
        debug!("GetProjects request: {:?}", request);
        
        let req = request.into_inner();
        let db = self.db.lock().await;
        match db.get_all_projects_with_status(None) {
            Ok(projects) => {
                let total_count = projects.len() as i32;
                
                // Apply pagination: offset is always applied, limit only if specified
                // This allows returning all ~4000 projects when no limit is set
                let projects_iter = projects.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let proto_projects: Vec<Project> = if let Some(limit) = req.limit {
                    projects_iter.take(limit as usize).map(|p| convert_live_set_to_proto(p)).collect()
                } else {
                    projects_iter.map(|p| convert_live_set_to_proto(p)).collect()
                };
                
                let response = GetProjectsResponse {
                    projects: proto_projects,
                    total_count,
                };
                
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get projects: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn get_project(
        &self,
        request: Request<GetProjectRequest>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        debug!("GetProject request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.get_project(&req.project_id) {
            Ok(Some(project)) => {
                let response = GetProjectResponse {
                    project: Some(convert_live_set_to_proto(project)),
                };
                Ok(Response::new(response))
            }
            Ok(None) => {
                let response = GetProjectResponse { project: None };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get project {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn update_project_notes(
        &self,
        request: Request<UpdateProjectNotesRequest>,
    ) -> Result<Response<UpdateProjectNotesResponse>, Status> {
        debug!("UpdateProjectNotes request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.set_project_notes(&req.project_id, &req.notes) {
            Ok(()) => {
                let response = UpdateProjectNotesResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to update project notes for {}: {:?}", req.project_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        debug!("Search request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        let search_query = SearchQuery::parse(&req.query);
        
        match db.search_fts(&search_query) {
            Ok(search_results) => {
                let total_count = search_results.len() as i32;
                
                // Apply pagination: offset is always applied, limit only if specified
                // This allows returning all search results when no limit is set
                let results_iter = search_results.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let projects: Vec<Project> = if let Some(limit) = req.limit {
                    results_iter.take(limit as usize).map(|result| convert_live_set_to_proto(result.project)).collect()
                } else {
                    results_iter.map(|result| convert_live_set_to_proto(result.project)).collect()
                };
                
                let response = SearchResponse {
                    projects,
                    total_count,
                };
                
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Search failed for query '{}': {:?}", req.query, e);
                Err(Status::new(Code::Internal, format!("Search error: {}", e)))
            }
        }
    }

    type ScanDirectoriesStream = ReceiverStream<Result<ScanProgressResponse, Status>>;

    async fn scan_directories(
        &self,
        request: Request<ScanDirectoriesRequest>,
    ) -> Result<Response<Self::ScanDirectoriesStream>, Status> {
        info!("ScanDirectories request: {:?}", request);
        
        let _req = request.into_inner();
        let (tx, rx) = mpsc::channel(100);
        
        // Update scan status
        *self.scan_status.lock().await = ScanStatus::ScanStarting;
        
        // Clone necessary data for the task
        let _db = Arc::clone(&self.db);
        let scan_status = Arc::clone(&self.scan_status);
        
        // Spawn the scanning task
        tokio::spawn(async move {
            // Send initial progress
            let _ = tx.send(Ok(ScanProgressResponse {
                completed: 0,
                total: 0,
                progress: 0.0,
                message: "Starting scan...".to_string(),
                status: ScanStatus::ScanStarting as i32,
            })).await;
            
            // Update status to discovering
            *scan_status.lock().await = ScanStatus::ScanDiscovering;
            let _ = tx.send(Ok(ScanProgressResponse {
                completed: 0,
                total: 0,
                progress: 0.0,
                message: "Discovering projects...".to_string(),
                status: ScanStatus::ScanDiscovering as i32,
            })).await;
            
            // For now, use the existing process_projects function
            // TODO: Integrate proper progress streaming
            match process_projects() {
                Ok(()) => {
                    *scan_status.lock().await = ScanStatus::ScanCompleted;
                    let _ = tx.send(Ok(ScanProgressResponse {
                        completed: 1,
                        total: 1,
                        progress: 1.0,
                        message: "Scan completed successfully".to_string(),
                        status: ScanStatus::ScanCompleted as i32,
                    })).await;
                }
                Err(e) => {
                    error!("Scan failed: {:?}", e);
                    *scan_status.lock().await = ScanStatus::ScanError;
                    let _ = tx.send(Ok(ScanProgressResponse {
                        completed: 0,
                        total: 1,
                        progress: 0.0,
                        message: format!("Scan failed: {}", e),
                        status: ScanStatus::ScanError as i32,
                    })).await;
                }
            }
        });
        
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_scan_status(
        &self,
        _request: Request<GetScanStatusRequest>,
    ) -> Result<Response<GetScanStatusResponse>, Status> {
        let status = *self.scan_status.lock().await;
        
        let response = GetScanStatusResponse {
            status: status as i32,
            current_progress: None, // TODO: Implement actual progress tracking
        };
        
        Ok(Response::new(response))
    }

    // Placeholder implementations for other methods
    async fn get_collections(
        &self,
        _request: Request<GetCollectionsRequest>,
    ) -> Result<Response<GetCollectionsResponse>, Status> {
        // TODO: Implement collections
        Ok(Response::new(GetCollectionsResponse {
            collections: vec![],
        }))
    }

    async fn create_collection(
        &self,
        _request: Request<CreateCollectionRequest>,
    ) -> Result<Response<CreateCollectionResponse>, Status> {
        Err(Status::unimplemented("Collections not yet implemented"))
    }

    async fn update_collection(
        &self,
        _request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<UpdateCollectionResponse>, Status> {
        Err(Status::unimplemented("Collections not yet implemented"))
    }

    async fn add_project_to_collection(
        &self,
        _request: Request<AddProjectToCollectionRequest>,
    ) -> Result<Response<AddProjectToCollectionResponse>, Status> {
        Err(Status::unimplemented("Collections not yet implemented"))
    }

    async fn remove_project_from_collection(
        &self,
        _request: Request<RemoveProjectFromCollectionRequest>,
    ) -> Result<Response<RemoveProjectFromCollectionResponse>, Status> {
        Err(Status::unimplemented("Collections not yet implemented"))
    }

    async fn get_tags(
        &self,
        _request: Request<GetTagsRequest>,
    ) -> Result<Response<GetTagsResponse>, Status> {
        // TODO: Implement tags
        Ok(Response::new(GetTagsResponse { tags: vec![] }))
    }

    async fn create_tag(
        &self,
        _request: Request<CreateTagRequest>,
    ) -> Result<Response<CreateTagResponse>, Status> {
        Err(Status::unimplemented("Tags not yet implemented"))
    }

    async fn tag_project(
        &self,
        _request: Request<TagProjectRequest>,
    ) -> Result<Response<TagProjectResponse>, Status> {
        Err(Status::unimplemented("Tags not yet implemented"))
    }

    async fn untag_project(
        &self,
        _request: Request<UntagProjectRequest>,
    ) -> Result<Response<UntagProjectResponse>, Status> {
        Err(Status::unimplemented("Tags not yet implemented"))
    }

    async fn get_project_tasks(
        &self,
        _request: Request<GetProjectTasksRequest>,
    ) -> Result<Response<GetProjectTasksResponse>, Status> {
        // TODO: Implement tasks
        Ok(Response::new(GetProjectTasksResponse { tasks: vec![] }))
    }

    async fn create_task(
        &self,
        _request: Request<CreateTaskRequest>,
    ) -> Result<Response<CreateTaskResponse>, Status> {
        Err(Status::unimplemented("Tasks not yet implemented"))
    }

    async fn update_task(
        &self,
        _request: Request<UpdateTaskRequest>,
    ) -> Result<Response<UpdateTaskResponse>, Status> {
        Err(Status::unimplemented("Tasks not yet implemented"))
    }

    async fn delete_task(
        &self,
        _request: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        Err(Status::unimplemented("Tasks not yet implemented"))
    }

    async fn start_watcher(
        &self,
        _request: Request<StartWatcherRequest>,
    ) -> Result<Response<StartWatcherResponse>, Status> {
        Err(Status::unimplemented("File watcher not yet implemented"))
    }

    async fn stop_watcher(
        &self,
        _request: Request<StopWatcherRequest>,
    ) -> Result<Response<StopWatcherResponse>, Status> {
        Err(Status::unimplemented("File watcher not yet implemented"))
    }

    type GetWatcherEventsStream = ReceiverStream<Result<WatcherEventResponse, Status>>;

    async fn get_watcher_events(
        &self,
        _request: Request<GetWatcherEventsRequest>,
    ) -> Result<Response<Self::GetWatcherEventsStream>, Status> {
        Err(Status::unimplemented("File watcher not yet implemented"))
    }

    async fn get_system_info(
        &self,
        _request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        let config = CONFIG.as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Config error: {}", e)))?;
        
        let response = GetSystemInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            watch_paths: config.paths.clone(),
            watcher_active: false, // TODO: Implement actual watcher status
            uptime_seconds: 0, // TODO: Track actual uptime
        };
        
        Ok(Response::new(response))
    }

    async fn get_statistics(
        &self,
        _request: Request<GetStatisticsRequest>,
    ) -> Result<Response<GetStatisticsResponse>, Status> {
        // TODO: Implement actual statistics gathering
        let response = GetStatisticsResponse {
            total_projects: 0,
            total_plugins: 0,
            total_samples: 0,
            total_collections: 0,
            total_tags: 0,
            total_tasks: 0,
            top_plugins: vec![],
            tempo_distribution: vec![],
            key_distribution: vec![],
        };
        
        Ok(Response::new(response))
    }
}

// Helper function to convert LiveSet to protobuf Project
fn convert_live_set_to_proto(live_set: LiveSet) -> Project {
    Project {
        id: live_set.id.to_string(),
        name: live_set.name,
        path: live_set.file_path.to_string_lossy().to_string(),
        hash: live_set.file_hash,
        notes: String::new(), // TODO: Load actual notes from database
        created_at: live_set.created_time.timestamp(),
        modified_at: live_set.modified_time.timestamp(),
        last_parsed_at: live_set.last_parsed_timestamp.timestamp(),
        
        tempo: live_set.tempo,
        time_signature: Some(TimeSignature {
            numerator: live_set.time_signature.numerator as i32,
            denominator: live_set.time_signature.denominator as i32,
        }),
        key_signature: live_set.key_signature.map(|ks| KeySignature {
            tonic: ks.tonic.to_string(),
            scale: ks.scale.to_string(),
        }),
        duration_seconds: live_set.estimated_duration.map(|d| d.num_seconds() as f64),
        furthest_bar: live_set.furthest_bar,
        
        ableton_version: Some(AbletonVersion {
            major: live_set.ableton_version.major,
            minor: live_set.ableton_version.minor,
            patch: live_set.ableton_version.patch,
            beta: live_set.ableton_version.beta,
        }),
        
        plugins: live_set.plugins.into_iter().map(|p| Plugin {
            id: p.id.to_string(),
            ableton_plugin_id: p.plugin_id,
            ableton_module_id: p.module_id,
            dev_identifier: p.dev_identifier,
            name: p.name,
            format: p.plugin_format.to_string(),
            installed: p.installed,
            vendor: Some(p.vendor.unwrap_or_default()),
            version: Some(p.version.unwrap_or_default()),
            sdk_version: Some(p.sdk_version.unwrap_or_default()),
            flags: p.flags,
            scanstate: p.scanstate,
            enabled: p.enabled,
        }).collect(),
        
        samples: live_set.samples.into_iter().map(|s| Sample {
            id: s.id.to_string(),
            name: s.name,
            path: s.path.to_string_lossy().to_string(),
            is_present: s.is_present,
        }).collect(),
        
        tags: live_set.tags.into_iter().map(|tag_name| super::proto::Tag {
            id: tag_name.clone(), // TODO: Use actual tag ID from database
            name: tag_name,
            created_at: 0, // TODO: Use actual creation timestamp from database
        }).collect(),
        tasks: vec![], // TODO: Load actual tasks
        collection_ids: vec![], // TODO: Load actual collection associations
    }
} 