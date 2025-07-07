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
            match crate::process_projects_with_progress(progress_callback) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::sync::Mutex;
    use uuid::Uuid;
    use crate::test_utils::{LiveSetBuilder, setup};
    use studio_project_manager_server::StudioProjectManager;

    async fn create_test_server() -> StudioProjectManagerServer {
        setup("debug");
        
        // Create in-memory database
        let db = LiveSetDatabase::new(PathBuf::from(":memory:"))
            .expect("Failed to create test database");
        
        StudioProjectManagerServer {
            db: Arc::new(Mutex::new(db)),
            scan_status: Arc::new(Mutex::new(ScanStatus::ScanUnknown)),
        }
    }

    async fn create_test_project_in_db(db: &Arc<Mutex<LiveSetDatabase>>) -> String {
        let test_project = LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_sample("kick.wav")
            .with_tempo(140.0)
            .build();
        
        let unique_id = uuid::Uuid::new_v4();
        let unique_name = format!("Test Project {}.als", unique_id);
        
        let test_live_set = crate::live_set::LiveSet {
            is_active: true,
            id: unique_id,
            file_path: PathBuf::from(&unique_name),
            name: unique_name.clone(),
            file_hash: format!("test_hash_{}", unique_id),
            created_time: chrono::Local::now(),
            modified_time: chrono::Local::now(),
            last_parsed_timestamp: chrono::Local::now(),
            tempo: test_project.tempo,
            time_signature: test_project.time_signature,
            key_signature: test_project.key_signature,
            furthest_bar: test_project.furthest_bar,
            estimated_duration: None,
            ableton_version: test_project.version,
            plugins: test_project.plugins,
            samples: test_project.samples,
            tags: std::collections::HashSet::new(),
        };
        
        let project_id = test_live_set.id.to_string();
        let mut db_guard = db.lock().await;
        db_guard.insert_project(&test_live_set).expect("Failed to insert test project");
        
        project_id
    }

    #[tokio::test]
    async fn test_get_collections_empty() {
        let server = create_test_server().await;
        let request = Request::new(GetCollectionsRequest {});
        
        let response = server.get_collections(request).await.unwrap();
        let collections = response.into_inner().collections;
        
        assert_eq!(collections.len(), 0);
    }

    #[tokio::test]
    async fn test_create_collection() {
        let server = create_test_server().await;
        let request = Request::new(CreateCollectionRequest {
            name: "My Test Collection".to_string(),
            description: Some("A collection for testing".to_string()),
            notes: Some("Test notes".to_string()),
        });
        
        let response = server.create_collection(request).await.unwrap();
        let collection = response.into_inner().collection.expect("Collection should be present");
        
        assert_eq!(collection.name, "My Test Collection");
        assert_eq!(collection.description, Some("A collection for testing".to_string()));
        assert_eq!(collection.notes, Some("Test notes".to_string()));
        assert!(!collection.id.is_empty());
        assert!(collection.created_at > 0);
        assert!(collection.modified_at > 0);
        assert_eq!(collection.project_ids.len(), 0);
    }

    #[tokio::test]
    async fn test_get_collections_with_data() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Test Collection".to_string(),
            description: Some("Test Description".to_string()),
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let created_collection = create_response.into_inner().collection.unwrap();
        
        // Get all collections
        let get_request = Request::new(GetCollectionsRequest {});
        let get_response = server.get_collections(get_request).await.unwrap();
        let collections = get_response.into_inner().collections;
        
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, created_collection.id);
        assert_eq!(collections[0].name, "Test Collection");
        assert_eq!(collections[0].description, Some("Test Description".to_string()));
        assert_eq!(collections[0].notes, None);
    }

    #[tokio::test]
    async fn test_update_collection() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Original Name".to_string(),
            description: Some("Original Description".to_string()),
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Update the collection
        let update_request = Request::new(UpdateCollectionRequest {
            collection_id: collection_id.clone(),
            name: Some("Updated Name".to_string()),
            description: Some("Updated Description".to_string()),
            notes: Some("Updated Notes".to_string()),
        });
        
        let update_response = server.update_collection(update_request).await.unwrap();
        let updated_collection = update_response.into_inner().collection.unwrap();
        
        assert_eq!(updated_collection.id, collection_id);
        assert_eq!(updated_collection.name, "Updated Name");
        assert_eq!(updated_collection.description, Some("Updated Description".to_string()));
        assert_eq!(updated_collection.notes, Some("Updated Notes".to_string()));
    }

    #[tokio::test]
    async fn test_update_collection_partial() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Original Name".to_string(),
            description: Some("Original Description".to_string()),
            notes: Some("Original Notes".to_string()),
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Update only the name
        let update_request = Request::new(UpdateCollectionRequest {
            collection_id: collection_id.clone(),
            name: Some("Updated Name Only".to_string()),
            description: None,
            notes: None,
        });
        
        let update_response = server.update_collection(update_request).await.unwrap();
        let updated_collection = update_response.into_inner().collection.unwrap();
        
        assert_eq!(updated_collection.name, "Updated Name Only");
        assert_eq!(updated_collection.description, Some("Original Description".to_string()));
        assert_eq!(updated_collection.notes, Some("Original Notes".to_string()));
    }

    #[tokio::test]
    async fn test_update_nonexistent_collection() {
        let server = create_test_server().await;
        
        let update_request = Request::new(UpdateCollectionRequest {
            collection_id: Uuid::new_v4().to_string(),
            name: Some("Should Fail".to_string()),
            description: None,
            notes: None,
        });
        
        let result = server.update_collection(update_request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), Code::NotFound);
    }

    #[tokio::test]
    async fn test_add_project_to_collection() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Test Collection".to_string(),
            description: None,
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Create a test project
        let project_id = create_test_project_in_db(&server.db).await;
        
        // Add project to collection
        let add_request = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id.clone(),
            position: None,
        });
        
        let add_response = server.add_project_to_collection(add_request).await.unwrap();
        assert!(add_response.into_inner().success);
        
        // Verify the project was added
        let get_request = Request::new(GetCollectionsRequest {});
        let get_response = server.get_collections(get_request).await.unwrap();
        let collections = get_response.into_inner().collections;
        
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].project_ids.len(), 1);
        assert_eq!(collections[0].project_ids[0], project_id);
    }

    #[tokio::test]
    async fn test_add_multiple_projects_to_collection() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Multi-Project Collection".to_string(),
            description: None,
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Create multiple test projects
        let project_id1 = create_test_project_in_db(&server.db).await;
        let project_id2 = create_test_project_in_db(&server.db).await;
        
        // Add first project
        let add_request1 = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id1.clone(),
            position: None,
        });
        
        let add_response1 = server.add_project_to_collection(add_request1).await.unwrap();
        assert!(add_response1.into_inner().success);
        
        // Add second project
        let add_request2 = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id2.clone(),
            position: None,
        });
        
        let add_response2 = server.add_project_to_collection(add_request2).await.unwrap();
        assert!(add_response2.into_inner().success);
        
        // Verify both projects were added in order
        let get_request = Request::new(GetCollectionsRequest {});
        let get_response = server.get_collections(get_request).await.unwrap();
        let collections = get_response.into_inner().collections;
        
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].project_ids.len(), 2);
        assert_eq!(collections[0].project_ids[0], project_id1);
        assert_eq!(collections[0].project_ids[1], project_id2);
    }

    #[tokio::test]
    async fn test_remove_project_from_collection() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Test Collection".to_string(),
            description: None,
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Create and add a test project
        let project_id = create_test_project_in_db(&server.db).await;
        
        let add_request = Request::new(AddProjectToCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id.clone(),
            position: None,
        });
        
        server.add_project_to_collection(add_request).await.unwrap();
        
        // Remove the project
        let remove_request = Request::new(RemoveProjectFromCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id.clone(),
        });
        
        let remove_response = server.remove_project_from_collection(remove_request).await.unwrap();
        assert!(remove_response.into_inner().success);
        
        // Verify the project was removed
        let get_request = Request::new(GetCollectionsRequest {});
        let get_response = server.get_collections(get_request).await.unwrap();
        let collections = get_response.into_inner().collections;
        
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].project_ids.len(), 0);
    }

    #[tokio::test]
    async fn test_remove_project_maintains_order() {
        let server = create_test_server().await;
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: "Order Test Collection".to_string(),
            description: None,
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection_id = create_response.into_inner().collection.unwrap().id;
        
        // Create three test projects
        let project_id1 = create_test_project_in_db(&server.db).await;
        let project_id2 = create_test_project_in_db(&server.db).await;
        let project_id3 = create_test_project_in_db(&server.db).await;
        
        // Add all projects
        for project_id in [&project_id1, &project_id2, &project_id3] {
            let add_request = Request::new(AddProjectToCollectionRequest {
                collection_id: collection_id.clone(),
                project_id: project_id.clone(),
                position: None,
            });
            server.add_project_to_collection(add_request).await.unwrap();
        }
        
        // Remove the middle project
        let remove_request = Request::new(RemoveProjectFromCollectionRequest {
            collection_id: collection_id.clone(),
            project_id: project_id2.clone(),
        });
        
        server.remove_project_from_collection(remove_request).await.unwrap();
        
        // Verify the remaining projects maintain their relative order
        let get_request = Request::new(GetCollectionsRequest {});
        let get_response = server.get_collections(get_request).await.unwrap();
        let collections = get_response.into_inner().collections;
        
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].project_ids.len(), 2);
        assert_eq!(collections[0].project_ids[0], project_id1);
        assert_eq!(collections[0].project_ids[1], project_id3);
    }

    #[tokio::test]
    async fn test_add_project_to_nonexistent_collection() {
        let server = create_test_server().await;
        
        let project_id = create_test_project_in_db(&server.db).await;
        
        let add_request = Request::new(AddProjectToCollectionRequest {
            collection_id: Uuid::new_v4().to_string(),
            project_id,
            position: None,
        });
        
        let result = server.add_project_to_collection(add_request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), Code::Internal);
    }

    #[tokio::test]
    async fn test_remove_project_from_nonexistent_collection() {
        let server = create_test_server().await;
        
        let project_id = create_test_project_in_db(&server.db).await;
        
        let remove_request = Request::new(RemoveProjectFromCollectionRequest {
            collection_id: Uuid::new_v4().to_string(),
            project_id,
        });
        
        let result = server.remove_project_from_collection(remove_request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), Code::Internal);
    }

    #[tokio::test]
    async fn test_collection_timestamps() {
        let server = create_test_server().await;
        
        // Create a collection and record creation time
        let before_create = chrono::Utc::now().timestamp();
        
        let create_request = Request::new(CreateCollectionRequest {
            name: "Timestamp Test".to_string(),
            description: None,
            notes: None,
        });
        
        let create_response = server.create_collection(create_request).await.unwrap();
        let collection = create_response.into_inner().collection.unwrap();
        let after_create = chrono::Utc::now().timestamp();
        
        // Verify creation timestamps
        assert!(collection.created_at >= before_create);
        assert!(collection.created_at <= after_create);
        assert!(collection.modified_at >= before_create);
        assert!(collection.modified_at <= after_create);
        
        // Wait a bit then update (ensure timestamp difference)
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        let before_update = chrono::Utc::now().timestamp();
        
        let update_request = Request::new(UpdateCollectionRequest {
            collection_id: collection.id.clone(),
            name: Some("Updated Name".to_string()),
            description: None,
            notes: None,
        });
        
        let update_response = server.update_collection(update_request).await.unwrap();
        let updated_collection = update_response.into_inner().collection.unwrap();
        let after_update = chrono::Utc::now().timestamp();
        
        // Verify update timestamps
        assert_eq!(updated_collection.created_at, collection.created_at); // Should not change
        assert!(updated_collection.modified_at >= before_update);
        assert!(updated_collection.modified_at <= after_update);
        assert!(updated_collection.modified_at > collection.modified_at); // Should be newer
    }

    // === TAG TESTS ===

    #[tokio::test]
    async fn test_get_tags_empty() {
        setup("debug");
        
        let server = create_test_server().await;
        
        let request = GetTagsRequest {};
        let response = server.get_tags(Request::new(request)).await.unwrap();
        let tags = response.into_inner().tags;
        
        assert_eq!(tags.len(), 0);
    }

    #[tokio::test]
    async fn test_create_tag() {
        setup("debug");
        
        let server = create_test_server().await;
        
        let request = CreateTagRequest {
            name: "Electronic".to_string(),
        };
        let response = server.create_tag(Request::new(request)).await.unwrap();
        let tag = response.into_inner().tag.unwrap();
        
        assert_eq!(tag.name, "Electronic");
        assert!(!tag.id.is_empty());
        
        // Verify timestamp is recent
        let now = chrono::Utc::now().timestamp();
        assert!((tag.created_at - now).abs() < 5);
    }

    #[allow(unused)]
    #[tokio::test]
    async fn test_get_tags_with_data() {
        setup("debug");
        
        let server = create_test_server().await;
        
        // Create multiple tags
        let tag1_req = CreateTagRequest {
            name: "Rock".to_string(),
        };
        let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
        // let tag1 = tag1_resp.into_inner().tag.unwrap();
        
        let tag2_req = CreateTagRequest {
            name: "Electronic".to_string(),
        };
        let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
        // let tag2 = tag2_resp.into_inner().tag.unwrap();
        
        let tag3_req = CreateTagRequest {
            name: "Ambient".to_string(),
        };
        let tag3_resp = server.create_tag(Request::new(tag3_req)).await.unwrap();
        // let tag3 = tag3_resp.into_inner().tag.unwrap();
        
        // Get all tags
        let request = GetTagsRequest {};
        let response = server.get_tags(Request::new(request)).await.unwrap();
        let tags = response.into_inner().tags;
        
        assert_eq!(tags.len(), 3);
        
        // Verify tags are sorted by name (as per database query)
        let tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tag_names, vec!["Ambient", "Electronic", "Rock"]);
        
        // Verify all tags have valid IDs and timestamps
        for tag in tags {
            assert!(!tag.id.is_empty());
            assert!(!tag.name.is_empty());
            assert!(tag.created_at > 0);
        }
    }

    #[tokio::test]
    async fn test_tag_project() {
        setup("debug");
        
        let server = create_test_server().await;
        let db = &server.db;
        
        // Create a test project
        let project_id = create_test_project_in_db(db).await;
        
        // Create a tag
        let tag_req = CreateTagRequest {
            name: "Work In Progress".to_string(),
        };
        let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
        let tag = tag_resp.into_inner().tag.unwrap();
        
        // Tag the project
        let tag_project_req = TagProjectRequest {
            project_id: project_id.clone(),
            tag_id: tag.id.clone(),
        };
        let tag_project_resp = server.tag_project(Request::new(tag_project_req)).await.unwrap();
        let result = tag_project_resp.into_inner();
        
        assert!(result.success);
        
        // Verify the project is tagged by checking database directly
        let mut db_guard = db.lock().await;
        let project_tags = db_guard.get_project_tags(&project_id).unwrap();
        assert_eq!(project_tags.len(), 1);
        assert!(project_tags.contains("Work In Progress"));
    }

    #[tokio::test]
    async fn test_untag_project() {
        setup("debug");
        
        let server = create_test_server().await;
        let db = &server.db;
        
        // Create a test project
        let project_id = create_test_project_in_db(db).await;
        
        // Create multiple tags
        let tag1_req = CreateTagRequest {
            name: "Tag 1".to_string(),
        };
        let tag1_resp = server.create_tag(Request::new(tag1_req)).await.unwrap();
        let tag1 = tag1_resp.into_inner().tag.unwrap();
        
        let tag2_req = CreateTagRequest {
            name: "Tag 2".to_string(),
        };
        let tag2_resp = server.create_tag(Request::new(tag2_req)).await.unwrap();
        let tag2 = tag2_resp.into_inner().tag.unwrap();
        
        // Tag the project with both tags
        let tag_project_req1 = TagProjectRequest {
            project_id: project_id.clone(),
            tag_id: tag1.id.clone(),
        };
        server.tag_project(Request::new(tag_project_req1)).await.unwrap();
        
        let tag_project_req2 = TagProjectRequest {
            project_id: project_id.clone(),
            tag_id: tag2.id.clone(),
        };
        server.tag_project(Request::new(tag_project_req2)).await.unwrap();
        
        // Verify both tags are applied
        {
            let mut db_guard = db.lock().await;
            let project_tags = db_guard.get_project_tags(&project_id).unwrap();
            assert_eq!(project_tags.len(), 2);
            assert!(project_tags.contains("Tag 1"));
            assert!(project_tags.contains("Tag 2"));
        }
        
        // Untag one tag
        let untag_project_req = UntagProjectRequest {
            project_id: project_id.clone(),
            tag_id: tag1.id.clone(),
        };
        let untag_project_resp = server.untag_project(Request::new(untag_project_req)).await.unwrap();
        let result = untag_project_resp.into_inner();
        
        assert!(result.success);
        
        // Verify only one tag remains
        {
            let mut db_guard = db.lock().await;
            let project_tags = db_guard.get_project_tags(&project_id).unwrap();
            assert_eq!(project_tags.len(), 1);
            assert!(project_tags.contains("Tag 2"));
            assert!(!project_tags.contains("Tag 1"));
        }
    }

    #[tokio::test]
    async fn test_tag_project_nonexistent_project() {
        setup("debug");
        
        let server = create_test_server().await;
        
        // Create a tag
        let tag_req = CreateTagRequest {
            name: "Test Tag".to_string(),
        };
        let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
        let tag = tag_resp.into_inner().tag.unwrap();
        
        // Try to tag a non-existent project
        let tag_project_req = TagProjectRequest {
            project_id: "non-existent-project-id".to_string(),
            tag_id: tag.id,
        };
        let result = server.tag_project(Request::new(tag_project_req)).await;
        
        // Should fail due to foreign key constraint (project doesn't exist)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tag_project_nonexistent_tag() {
        setup("debug");
        
        let server = create_test_server().await;
        let db = &server.db;
        
        // Create a test project
        let project_id = create_test_project_in_db(db).await;
        
        // Try to tag with a non-existent tag
        let tag_project_req = TagProjectRequest {
            project_id,
            tag_id: "non-existent-tag-id".to_string(),
        };
        let result = server.tag_project(Request::new(tag_project_req)).await;
        
        // Should fail due to foreign key constraint
        assert!(result.is_err());
    }

    #[allow(unused)]
    #[tokio::test]
    async fn test_create_duplicate_tag() {
        setup("debug");
        
        let server = create_test_server().await;
        
        // Create a tag
        let tag_req = CreateTagRequest {
            name: "Duplicate Tag".to_string(),
        };
        let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
        let tag = tag_resp.into_inner().tag.unwrap();
        
        // Try to create another tag with the same name
        let duplicate_req = CreateTagRequest {
            name: "Duplicate Tag".to_string(),
        };
        let result = server.create_tag(Request::new(duplicate_req)).await;
        
        // Should fail due to unique constraint
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tag_project_idempotent() {
        setup("debug");
        
        let server = create_test_server().await;
        let db = &server.db;
        
        // Create a test project
        let project_id = create_test_project_in_db(db).await;
        
        // Create a tag
        let tag_req = CreateTagRequest {
            name: "Idempotent Tag".to_string(),
        };
        let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
        let tag = tag_resp.into_inner().tag.unwrap();
        
        // Tag the project multiple times
        let tag_project_req = TagProjectRequest {
            project_id: project_id.clone(),
            tag_id: tag.id.clone(),
        };
        let result1 = server.tag_project(Request::new(tag_project_req.clone())).await.unwrap();
        let result2 = server.tag_project(Request::new(tag_project_req.clone())).await.unwrap();
        let result3 = server.tag_project(Request::new(tag_project_req)).await.unwrap();
        
        // All should succeed
        assert!(result1.into_inner().success);
        assert!(result2.into_inner().success);
        assert!(result3.into_inner().success);
        
        // Verify only one tag association exists
        {
            let mut db_guard = db.lock().await;
            let project_tags = db_guard.get_project_tags(&project_id).unwrap();
            assert_eq!(project_tags.len(), 1);
            assert!(project_tags.contains("Idempotent Tag"));
        }
    }
} 