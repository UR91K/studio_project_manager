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
use crate::process_projects_with_progress;

use super::proto::*;

pub struct StudioProjectManagerServer {
    pub db: Arc<Mutex<LiveSetDatabase>>,
    pub scan_status: Arc<Mutex<ScanStatus>>,
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

    async fn update_project_name(
        &self,
        request: Request<UpdateProjectNameRequest>,
    ) -> Result<Response<UpdateProjectNameResponse>, Status> {
        debug!("UpdateProjectName request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.set_project_name(&req.project_id, &req.name) {
            Ok(()) => {
                let response = UpdateProjectNameResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to update project name for {}: {:?}", req.project_id, e);
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
        let scan_status = Arc::clone(&self.scan_status);
        let tx_for_callback = tx.clone();
        let scan_status_for_callback = Arc::clone(&scan_status);
        let tx_for_error = tx.clone();
        let scan_status_for_error = Arc::clone(&scan_status);
        
        // Spawn the scanning task
        tokio::spawn(async move {
            // Create progress callback that sends updates through the channel
            let progress_callback = move |completed: u32, total: u32, progress: f32, message: String, phase: &str| {
                let status = match phase {
                    "starting" => ScanStatus::ScanStarting,
                    "discovering" => ScanStatus::ScanDiscovering,
                    "preprocessing" | "parsing" => ScanStatus::ScanParsing,
                    "inserting" => ScanStatus::ScanInserting,
                    "completed" => ScanStatus::ScanCompleted,
                    _ => ScanStatus::ScanStarting,
                };
                
                // Update global scan status
                let scan_status_clone = Arc::clone(&scan_status_for_callback);
                tokio::spawn(async move {
                    *scan_status_clone.lock().await = status;
                });
                
                // Send progress update
                let response = ScanProgressResponse {
                    completed,
                    total,
                    progress,
                    message,
                    status: status as i32,
                };
                
                if let Err(e) = tx_for_callback.try_send(Ok(response)) {
                    error!("Failed to send progress update: {:?}", e);
                }
            };
            
            // Run the scanning process with progress callbacks
            match process_projects_with_progress(Some(progress_callback)) {
                Ok(()) => {
                    info!("Scan completed successfully");
                    *scan_status.lock().await = ScanStatus::ScanCompleted;
                }
                Err(e) => {
                    error!("Scan failed: {:?}", e);
                    *scan_status_for_error.lock().await = ScanStatus::ScanError;
                    let _ = tx_for_error.try_send(Ok(ScanProgressResponse {
                        completed: 0,
                        total: 1,
                        progress: 0.0,
                        message: format!("Scan failed: {}", e),
                        status: ScanStatus::ScanError as i32,
                    }));
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
        debug!("GetCollections request");
        
        let mut db = self.db.lock().await;
        match db.list_collections() {
            Ok(collections_data) => {
                let mut collections = Vec::new();
                
                for (id, name, description) in collections_data {
                    // Get the full collection details including project IDs
                    match db.get_collection_by_id(&id) {
                        Ok(Some((_, _, _, notes, created_at, modified_at, project_ids))) => {
                            collections.push(Collection {
                                id: id.clone(),
                                name: name.clone(),
                                description,
                                notes,
                                created_at,
                                modified_at,
                                project_ids,
                            });
                        }
                        Ok(None) => {
                            // Collection was deleted between list_collections and get_collection_by_id
                            debug!("Collection {} not found during detailed lookup", id);
                        }
                        Err(e) => {
                            error!("Failed to get collection details for {}: {:?}", id, e);
                            return Err(Status::new(Code::Internal, format!("Database error: {}", e)));
                        }
                    }
                }
                
                let response = GetCollectionsResponse { collections };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get collections: {:?}", e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> Result<Response<CreateCollectionResponse>, Status> {
        debug!("CreateCollection request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.create_collection(&req.name, req.description.as_deref(), req.notes.as_deref()) {
            Ok(collection_id) => {
                // Get the created collection details to return in response
                match db.get_collection_by_id(&collection_id) {
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                        };
                        
                        let response = CreateCollectionResponse { collection: Some(collection) };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} was created but not found", collection_id);
                        Err(Status::new(Code::Internal, "Collection creation failed"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve created collection {}: {:?}", collection_id, e);
                        Err(Status::new(Code::Internal, format!("Database error: {}", e)))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create collection '{}': {:?}", req.name, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn update_collection(
        &self,
        request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<UpdateCollectionResponse>, Status> {
        debug!("UpdateCollection request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.update_collection(
            &req.collection_id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.notes.as_deref(),
        ) {
            Ok(()) => {
                // Get the updated collection details to return in response
                match db.get_collection_by_id(&req.collection_id) {
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                        };
                        
                        let response = UpdateCollectionResponse { collection: Some(collection) };
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Collection {} not found after update", req.collection_id);
                        Err(Status::new(Code::NotFound, "Collection not found"))
                    }
                    Err(e) => {
                        error!("Failed to retrieve updated collection {}: {:?}", req.collection_id, e);
                        Err(Status::new(Code::Internal, format!("Database error: {}", e)))
                    }
                }
            }
            Err(e) => {
                error!("Failed to update collection '{}': {:?}", req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn add_project_to_collection(
        &self,
        request: Request<AddProjectToCollectionRequest>,
    ) -> Result<Response<AddProjectToCollectionResponse>, Status> {
        debug!("AddProjectToCollection request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.add_project_to_collection(&req.collection_id, &req.project_id) {
            Ok(()) => {
                debug!("Successfully added project {} to collection {}", req.project_id, req.collection_id);
                let response = AddProjectToCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to add project {} to collection {}: {:?}", req.project_id, req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn remove_project_from_collection(
        &self,
        request: Request<RemoveProjectFromCollectionRequest>,
    ) -> Result<Response<RemoveProjectFromCollectionResponse>, Status> {
        debug!("RemoveProjectFromCollection request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.remove_project_from_collection(&req.collection_id, &req.project_id) {
            Ok(()) => {
                debug!("Successfully removed project {} from collection {}", req.project_id, req.collection_id);
                let response = RemoveProjectFromCollectionResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove project {} from collection {}: {:?}", req.project_id, req.collection_id, e);
                Err(Status::new(Code::Internal, format!("Database error: {}", e)))
            }
        }
    }

    async fn get_tags(
        &self,
        _request: Request<GetTagsRequest>,
    ) -> Result<Response<GetTagsResponse>, Status> {
        debug!("GetTags request");
        
        let mut db = self.db.lock().await;
        match db.list_tags() {
            Ok(tags_data) => {
                let tags = tags_data
                    .into_iter()
                    .map(|(id, name, created_at)| super::proto::Tag {
                        id,
                        name,
                        created_at,
                    })
                    .collect();
                
                let response = GetTagsResponse { tags };
                debug!("Successfully retrieved {} tags", response.tags.len());
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get tags: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to get tags: {}", e),
                ))
            }
        }
    }

    async fn create_tag(
        &self,
        request: Request<CreateTagRequest>,
    ) -> Result<Response<CreateTagResponse>, Status> {
        debug!("CreateTag request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.add_tag(&req.name) {
            Ok(tag_id) => {
                // Get the created tag details to return in response
                match db.get_tag_by_id(&tag_id) {
                    Ok(Some((id, name, created_at))) => {
                        let tag = super::proto::Tag {
                            id,
                            name,
                            created_at,
                        };
                        
                        let response = CreateTagResponse { tag: Some(tag) };
                        debug!("Successfully created tag: {}", req.name);
                        Ok(Response::new(response))
                    }
                    Ok(None) => {
                        error!("Created tag not found after creation");
                        Err(Status::new(
                            Code::Internal,
                            "Created tag not found after creation".to_string(),
                        ))
                    }
                    Err(e) => {
                        error!("Failed to retrieve created tag: {}", e);
                        Err(Status::new(
                            Code::Internal,
                            format!("Failed to retrieve created tag: {}", e),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to create tag: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to create tag: {}", e),
                ))
            }
        }
    }

    async fn tag_project(
        &self,
        request: Request<TagProjectRequest>,
    ) -> Result<Response<TagProjectResponse>, Status> {
        debug!("TagProject request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.tag_project(&req.project_id, &req.tag_id) {
            Ok(()) => {
                debug!("Successfully tagged project {} with tag {}", req.project_id, req.tag_id);
                let response = TagProjectResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to tag project: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to tag project: {}", e),
                ))
            }
        }
    }

    async fn untag_project(
        &self,
        request: Request<UntagProjectRequest>,
    ) -> Result<Response<UntagProjectResponse>, Status> {
        debug!("UntagProject request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.untag_project(&req.project_id, &req.tag_id) {
            Ok(()) => {
                debug!("Successfully untagged project {} from tag {}", req.project_id, req.tag_id);
                let response = UntagProjectResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to untag project: {}", e);
                Err(Status::new(
                    Code::Internal,
                    format!("Failed to untag project: {}", e),
                ))
            }
        }
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
