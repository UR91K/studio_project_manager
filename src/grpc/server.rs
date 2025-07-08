use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Code};
use log::{debug, error, info, warn};

use crate::live_set::LiveSet;
use crate::process_projects_with_progress;
use crate::error::DatabaseError;
use crate::media::{MediaStorageManager, MediaConfig, MediaType};
use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::database::search::SearchQuery;
use crate::watcher::file_watcher::{FileWatcher, FileEvent};

use super::proto::*;

pub struct StudioProjectManagerServer {
    pub db: Arc<Mutex<LiveSetDatabase>>,
    pub scan_status: Arc<Mutex<ScanStatus>>,
    pub scan_progress: Arc<Mutex<Option<ScanProgressResponse>>>,
    pub media_storage: Arc<MediaStorageManager>,
    pub watcher: Arc<Mutex<Option<FileWatcher>>>,
    pub watcher_events: Arc<Mutex<Option<std::sync::mpsc::Receiver<FileEvent>>>>,
}

impl StudioProjectManagerServer {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = CONFIG.as_ref().map_err(|e| {
            format!("Failed to load config: {}", e)
        })?;
        
        let db_path = PathBuf::from(&config.database_path);
        let db = LiveSetDatabase::new(db_path)
            .map_err(|e| format!("Failed to initialize database: {}", e))?;
        
        let media_config = MediaConfig::from(config);
        let media_storage = MediaStorageManager::new(
            std::path::PathBuf::from(&config.media_storage_dir),
            media_config
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            scan_status: Arc::new(Mutex::new(ScanStatus::ScanUnknown)),
            scan_progress: Arc::new(Mutex::new(None)),
            media_storage: Arc::new(media_storage),
            watcher: Arc::new(Mutex::new(None)),
            watcher_events: Arc::new(Mutex::new(None)),
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
        let mut db = self.db.lock().await;
        match db.get_all_projects_with_status(None) {
            Ok(projects) => {
                let total_count = projects.len() as i32;
                let projects_iter = projects.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let mut proto_projects = Vec::new();
                
                let projects_to_convert: Vec<LiveSet> = if let Some(limit) = req.limit {
                    projects_iter.take(limit as usize).collect()
                } else {
                    projects_iter.collect()
                };
                
                for project in projects_to_convert {
                    match convert_live_set_to_proto(project, &mut *db) {
                        Ok(proto_project) => proto_projects.push(proto_project),
                        Err(e) => {
                            error!("Failed to convert project to proto: {:?}", e);
                            return Err(Status::internal(format!("Database error: {}", e)));
                        }
                    }
                }
                
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
                match convert_live_set_to_proto(project, &mut *db) {
                    Ok(proto_project) => {
                        let response = GetProjectResponse {
                            project: Some(proto_project),
                        };
                        Ok(Response::new(response))
                    }
                    Err(e) => {
                        error!("Failed to convert project to proto: {:?}", e);
                        Err(Status::internal(format!("Database error: {}", e)))
                    }
                }
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
        
        let _search_query = SearchQuery::parse(&req.query);
        
        match db.search(&req.query) {
            Ok(search_results) => {
                let total_count = search_results.len() as i32;
                let results_iter = search_results.into_iter().skip(req.offset.unwrap_or(0) as usize);
                let mut proto_projects = Vec::new();
                
                let projects_to_convert: Vec<LiveSet> = if let Some(limit) = req.limit {
                    results_iter.take(limit as usize).collect()
                } else {
                    results_iter.collect()
                };
                
                for project in projects_to_convert {
                    match convert_live_set_to_proto(project, &mut *db) {
                        Ok(proto_project) => proto_projects.push(proto_project),
                        Err(e) => {
                            error!("Failed to convert project to proto: {:?}", e);
                            return Err(Status::internal(format!("Database error: {}", e)));
                        }
                    }
                }
                
                let response = SearchResponse {
                    projects: proto_projects,
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
        
        // Update scan status and clear progress
        *self.scan_status.lock().await = ScanStatus::ScanStarting;
        *self.scan_progress.lock().await = None;
        
        // Clone necessary data for the task
        let scan_status = Arc::clone(&self.scan_status);
        let scan_progress = Arc::clone(&self.scan_progress);
        let tx_for_callback = tx.clone();
        let scan_status_for_callback = Arc::clone(&scan_status);
        let scan_progress_for_callback = Arc::clone(&scan_progress);
        let tx_for_error = tx.clone();
        let scan_status_for_error = Arc::clone(&scan_status);
        let scan_progress_for_error = Arc::clone(&scan_progress);
        
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
                
                // Create the progress response
                let response = ScanProgressResponse {
                    completed,
                    total,
                    progress,
                    message: message.clone(),
                    status: status as i32,
                };
                
                // Update global scan status and progress
                let scan_status_clone = Arc::clone(&scan_status_for_callback);
                let scan_progress_clone = Arc::clone(&scan_progress_for_callback);
                let response_clone = response.clone();
                tokio::spawn(async move {
                    *scan_status_clone.lock().await = status;
                    *scan_progress_clone.lock().await = Some(response_clone);
                });
                
                // Send progress update
                if let Err(e) = tx_for_callback.try_send(Ok(response)) {
                    error!("Failed to send progress update: {:?}", e);
                }
            };
            
            // Run the scanning process with progress callbacks
            match process_projects_with_progress(Some(progress_callback)) {
                Ok(()) => {
                    info!("Scan completed successfully");
                    let final_status = ScanStatus::ScanCompleted;
                    let final_progress = ScanProgressResponse {
                        completed: 100,
                        total: 100,
                        progress: 1.0,
                        message: "Scan completed successfully".to_string(),
                        status: final_status as i32,
                    };
                    
                    *scan_status.lock().await = final_status;
                    *scan_progress.lock().await = Some(final_progress);
                }
                Err(e) => {
                    error!("Scan failed: {:?}", e);
                    let error_status = ScanStatus::ScanError;
                    let error_progress = ScanProgressResponse {
                        completed: 0,
                        total: 1,
                        progress: 0.0,
                        message: format!("Scan failed: {}", e),
                        status: error_status as i32,
                    };
                    
                    *scan_status_for_error.lock().await = error_status;
                    *scan_progress_for_error.lock().await = Some(error_progress.clone());
                    let _ = tx_for_error.try_send(Ok(error_progress));
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
        let current_progress = self.scan_progress.lock().await.clone();
        
        let response = GetScanStatusResponse {
            status: status as i32,
            current_progress,
        };
        
        Ok(Response::new(response))
    }

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
                        Ok(Some((_, _, _, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                            collections.push(Collection {
                                id: id.clone(),
                                name: name.clone(),
                                description,
                                notes,
                                created_at,
                                modified_at,
                                project_ids,
                                cover_art_id,
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
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
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
                    Ok(Some((id, name, description, notes, created_at, modified_at, project_ids, cover_art_id))) => {
                        let collection = Collection {
                            id,
                            name,
                            description,
                            notes,
                            created_at,
                            modified_at,
                            project_ids,
                            cover_art_id,
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
        debug!("Starting file watcher");
        
        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;
        
        // Check if watcher is already active
        if watcher_guard.is_some() {
            return Ok(Response::new(StartWatcherResponse {
                success: true,
            }));
        }
        
        // Create new watcher
        match FileWatcher::new(Arc::clone(&self.db)) {
            Ok((mut watcher, event_receiver)) => {
                // Add configured watch paths
                let config = CONFIG.as_ref()
                    .map_err(|e| Status::internal(format!("Config error: {}", e)))?;
                
                for path in &config.paths {
                    if let Err(e) = watcher.add_watch_path(PathBuf::from(path)) {
                        warn!("Failed to add watch path {}: {}", path, e);
                    }
                }
                
                // Store watcher and event receiver
                *watcher_guard = Some(watcher);
                *events_guard = Some(event_receiver);
                
                info!("File watcher started successfully");
                Ok(Response::new(StartWatcherResponse {
                    success: true,
                }))
            }
            Err(e) => {
                error!("Failed to create file watcher: {}", e);
                Err(Status::internal(format!("Failed to start watcher: {}", e)))
            }
        }
    }

    async fn stop_watcher(
        &self,
        _request: Request<StopWatcherRequest>,
    ) -> Result<Response<StopWatcherResponse>, Status> {
        debug!("Stopping file watcher");
        
        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;
        
        // Check if watcher is active
        if watcher_guard.is_none() {
            return Ok(Response::new(StopWatcherResponse {
                success: true,
            }));
        }
        
        // Stop the watcher by dropping it
        *watcher_guard = None;
        *events_guard = None;
        
        info!("File watcher stopped successfully");
        Ok(Response::new(StopWatcherResponse {
            success: true,
        }))
    }

    type GetWatcherEventsStream = ReceiverStream<Result<WatcherEventResponse, Status>>;

    async fn get_watcher_events(
        &self,
        _request: Request<GetWatcherEventsRequest>,
    ) -> Result<Response<Self::GetWatcherEventsStream>, Status> {
        debug!("Getting watcher events stream");
        
        let mut events_guard = self.watcher_events.lock().await;
        
        // Check if watcher is active and has events
        if let Some(event_receiver) = events_guard.take() {
            let (tx, rx) = mpsc::channel(100);
            
            // Spawn a task to convert FileEvent to WatcherEventResponse
            tokio::spawn(async move {
                while let Ok(file_event) = event_receiver.recv() {
                    let watcher_event = match file_event {
                        FileEvent::Created(path) => WatcherEventResponse {
                            event_type: WatcherEventType::WatcherCreated as i32,
                            path: path.to_string_lossy().to_string(),
                            new_path: None,
                            timestamp: chrono::Utc::now().timestamp(),
                        },
                        FileEvent::Modified(path) => WatcherEventResponse {
                            event_type: WatcherEventType::WatcherModified as i32,
                            path: path.to_string_lossy().to_string(),
                            new_path: None,
                            timestamp: chrono::Utc::now().timestamp(),
                        },
                        FileEvent::Deleted(path) => WatcherEventResponse {
                            event_type: WatcherEventType::WatcherDeleted as i32,
                            path: path.to_string_lossy().to_string(),
                            new_path: None,
                            timestamp: chrono::Utc::now().timestamp(),
                        },
                        FileEvent::Renamed { from, to } => WatcherEventResponse {
                            event_type: WatcherEventType::WatcherRenamed as i32,
                            path: from.to_string_lossy().to_string(),
                            new_path: Some(to.to_string_lossy().to_string()),
                            timestamp: chrono::Utc::now().timestamp(),
                        },
                    };
                    
                    if tx.send(Ok(watcher_event)).await.is_err() {
                        break;
                    }
                }
            });
            
            let stream = ReceiverStream::new(rx);
            Ok(Response::new(stream))
        } else {
            Err(Status::failed_precondition("Watcher not active"))
        }
    }

    async fn get_system_info(
        &self,
        _request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        let config = CONFIG.as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Config error: {}", e)))?;
        
        // Check if watcher is active
        let watcher_guard = self.watcher.lock().await;
        let watcher_active = watcher_guard.is_some();
        
        let response = GetSystemInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            watch_paths: config.paths.clone(),
            watcher_active,
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
    
    // Media Management - Streaming implementations
    async fn upload_cover_art(
        &self,
        request: Request<tonic::Streaming<UploadCoverArtRequest>>,
    ) -> Result<Response<UploadCoverArtResponse>, Status> {
        debug!("UploadCoverArt streaming request received");
        
        let mut stream = request.into_inner();
        let mut collection_id: Option<String> = None;
        let mut filename: Option<String> = None;
        let mut data_chunks: Vec<u8> = Vec::new();
        
        // Process the streaming request
        while let Some(chunk_result) = stream.message().await? {
            let chunk = chunk_result;
            
            if let Some(data) = chunk.data {
                match data {
                    super::proto::upload_cover_art_request::Data::CollectionId(id) => {
                        collection_id = Some(id);
                    }
                    super::proto::upload_cover_art_request::Data::Filename(name) => {
                        filename = Some(name);
                    }
                    super::proto::upload_cover_art_request::Data::Chunk(bytes) => {
                        data_chunks.extend(bytes);
                    }
                }
            }
        }
        
        // Validate we have all required data
        let collection_id = collection_id.ok_or_else(|| {
            Status::invalid_argument("Collection ID is required")
        })?;
        
        // we dont seem to actually need a file name here, but ill leave it for now
        let filename = filename.ok_or_else(|| {
            Status::invalid_argument("Filename is required")
        })?;
        
        if data_chunks.is_empty() {
            return Err(Status::invalid_argument("No file data received"));
        }
        
        // Store the file using MediaStorageManager
        let media_file = match self.media_storage.store_file(&data_chunks, &filename, MediaType::CoverArt) {
            Ok(file) => file,
            Err(e) => {
                error!("Failed to store cover art file: {:?}", e);
                return Ok(Response::new(UploadCoverArtResponse {
                    media_file_id: String::new(),
                    success: false,
                    error_message: Some(format!("Failed to store file: {}", e)),
                }));
            }
        };
        
        // Store the media file metadata in the database
        let mut db = self.db.lock().await;
        if let Err(e) = db.insert_media_file(&media_file) {
            error!("Failed to insert media file into database: {:?}", e);
            // Clean up the stored file
            if let Err(cleanup_err) = self.media_storage.delete_file(&media_file.id, &media_file.file_extension, &media_file.media_type) {
                error!("Failed to cleanup stored file after database error: {:?}", cleanup_err);
            }
            return Ok(Response::new(UploadCoverArtResponse {
                media_file_id: String::new(),
                success: false,
                error_message: Some(format!("Failed to store metadata: {}", e)),
            }));
        }
        
        // Optionally set as collection cover art if collection_id was provided
        if let Err(e) = db.update_collection_cover_art(&collection_id, Some(&media_file.id)) {
            warn!("Failed to set collection cover art: {:?}", e);
            // Don't fail the upload, just log the warning
        }
        
        info!("Successfully uploaded cover art: {} bytes for collection {}", 
              data_chunks.len(), collection_id);
        
        let response = UploadCoverArtResponse {
            media_file_id: media_file.id,
            success: true,
            error_message: None,
        };
        
        Ok(Response::new(response))
    }
    
    async fn upload_audio_file(
        &self,
        request: Request<tonic::Streaming<UploadAudioFileRequest>>,
    ) -> Result<Response<UploadAudioFileResponse>, Status> {
        debug!("UploadAudioFile streaming request received");
        
        let mut stream = request.into_inner();
        let mut project_id: Option<String> = None;
        let mut filename: Option<String> = None;
        let mut data_chunks: Vec<u8> = Vec::new();
        
        // Process the streaming request
        while let Some(chunk_result) = stream.message().await? {
            let chunk = chunk_result;
            
            if let Some(data) = chunk.data {
                match data {
                    super::proto::upload_audio_file_request::Data::ProjectId(id) => {
                        project_id = Some(id);
                    }
                    super::proto::upload_audio_file_request::Data::Filename(name) => {
                        filename = Some(name);
                    }
                    super::proto::upload_audio_file_request::Data::Chunk(bytes) => {
                        data_chunks.extend(bytes);
                    }
                }
            }
        }
        
        // Validate we have all required data
        let project_id = project_id.ok_or_else(|| {
            Status::invalid_argument("Project ID is required")
        })?;
        
        let filename = filename.ok_or_else(|| {
            Status::invalid_argument("Filename is required")
        })?;
        
        if data_chunks.is_empty() {
            return Err(Status::invalid_argument("No file data received"));
        }
        
        // Store the file using MediaStorageManager
        let media_file = match self.media_storage.store_file(&data_chunks, &filename, MediaType::AudioFile) {
            Ok(file) => file,
            Err(e) => {
                error!("Failed to store audio file: {:?}", e);
                return Ok(Response::new(UploadAudioFileResponse {
                    media_file_id: String::new(),
                    success: false,
                    error_message: Some(format!("Failed to store file: {}", e)),
                }));
            }
        };
        
        // Store the media file metadata in the database
        let mut db = self.db.lock().await;
        if let Err(e) = db.insert_media_file(&media_file) {
            error!("Failed to insert media file into database: {:?}", e);
            // Clean up the stored file
            if let Err(cleanup_err) = self.media_storage.delete_file(&media_file.id, &media_file.file_extension, &media_file.media_type) {
                error!("Failed to cleanup stored file after database error: {:?}", cleanup_err);
            }
            return Ok(Response::new(UploadAudioFileResponse {
                media_file_id: String::new(),
                success: false,
                error_message: Some(format!("Failed to store metadata: {}", e)),
            }));
        }
        
        // Optionally set as project audio file if project_id was provided
        if let Err(e) = db.update_project_audio_file(&project_id, Some(&media_file.id)) {
            warn!("Failed to set project audio file: {:?}", e);
            // Don't fail the upload, just log the warning
        }
        
        info!("Successfully uploaded audio file: {} bytes for project {}", 
              data_chunks.len(), project_id);
        
        let response = UploadAudioFileResponse {
            media_file_id: media_file.id,
            success: true,
            error_message: None,
        };
        
        Ok(Response::new(response))
    }
    
    type DownloadMediaStream = ReceiverStream<Result<DownloadMediaResponse, Status>>;
    
    async fn download_media(
        &self,
        request: Request<DownloadMediaRequest>,
    ) -> Result<Response<Self::DownloadMediaStream>, Status> {
        debug!("DownloadMedia request: {:?}", request);
        
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        // Get media file metadata
        let media_file = match db.get_media_file(&req.media_file_id) {
            Ok(Some(file)) => file,
            Ok(None) => {
                return Err(Status::not_found("Media file not found"));
            }
            Err(e) => {
                error!("Failed to get media file: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Clone values needed for later use
        let file_id = media_file.id.clone();
        let file_extension = media_file.file_extension.clone();
        let media_type = media_file.media_type.clone();
        
        let (tx, rx) = mpsc::channel(100);
        
        // Convert our MediaFile to protobuf MediaFile
        let proto_media_file = super::proto::MediaFile {
            id: media_file.id,
            original_filename: media_file.original_filename,
            file_extension: media_file.file_extension,
            media_type: media_file.media_type.as_str().to_string(),
            file_size_bytes: media_file.file_size_bytes as i64,
            mime_type: media_file.mime_type,
            uploaded_at: media_file.uploaded_at.timestamp(),
            checksum: media_file.checksum,
        };
        
        // Send metadata first
        let metadata_response = DownloadMediaResponse {
            data: Some(super::proto::download_media_response::Data::Metadata(proto_media_file)),
        };
        
        if tx.send(Ok(metadata_response)).await.is_err() {
            return Err(Status::internal("Failed to send metadata"));
        }
        
        // Get the file path and stream the actual file data
        let file_path = match self.media_storage.get_file_path(&file_id, &file_extension, &media_type) {
            Ok(path) => path,
            Err(e) => {
                error!("Failed to get file path: {:?}", e);
                return Err(Status::internal(format!("Failed to get file path: {}", e)));
            }
        };
        
        // Read and stream the file in chunks
        match tokio::fs::read(&file_path).await {
            Ok(file_data) => {
                // Stream the file in chunks (e.g., 64KB chunks)
                const CHUNK_SIZE: usize = 64 * 1024;
                for chunk in file_data.chunks(CHUNK_SIZE) {
                    let chunk_response = DownloadMediaResponse {
                        data: Some(super::proto::download_media_response::Data::Chunk(chunk.to_vec())),
                    };
                    
                    if tx.send(Ok(chunk_response)).await.is_err() {
                        return Err(Status::internal("Failed to send file chunk"));
                    }
                }
            }
            Err(e) => {
                error!("Failed to read file: {:?}", e);
                return Err(Status::internal(format!("Failed to read file: {}", e)));
            }
        }
        
        Ok(Response::new(ReceiverStream::new(rx)))
    }
    
    async fn delete_media(
        &self,
        request: Request<DeleteMediaRequest>,
    ) -> Result<Response<DeleteMediaResponse>, Status> {
        debug!("DeleteMedia request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        // First check if the media file exists and get its info
        match db.get_media_file(&req.media_file_id) {
            Ok(Some(media_file)) => {
                // Clone the values we need for later use
                let file_id = media_file.id.clone();
                let file_extension = media_file.file_extension.clone();
                let media_type = media_file.media_type.clone();
                
                // Delete from database first
                match db.delete_media_file(&req.media_file_id) {
                    Ok(()) => {
                        // Also delete physical file from storage
                        if let Err(e) = self.media_storage.delete_file(&file_id, &file_extension, &media_type) {
                            warn!("Failed to delete physical file from storage: {:?}", e);
                            // Don't fail the operation if physical file deletion fails
                        }
                        
                        info!("Successfully deleted media file: {}", req.media_file_id);
                        let response = DeleteMediaResponse {
                            success: true,
                            error_message: None,
                        };
                        Ok(Response::new(response))
                    }
                    Err(e) => {
                        error!("Failed to delete media file from database: {:?}", e);
                        let response = DeleteMediaResponse {
                            success: false,
                            error_message: Some(format!("Database error: {}", e)),
                        };
                        Ok(Response::new(response))
                    }
                }
            }
            Ok(None) => {
                let response = DeleteMediaResponse {
                    success: false,
                    error_message: Some("Media file not found".to_string()),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to check media file existence: {:?}", e);
                let response = DeleteMediaResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }
    
    async fn set_collection_cover_art(
        &self,
        request: Request<SetCollectionCoverArtRequest>,
    ) -> Result<Response<SetCollectionCoverArtResponse>, Status> {
        debug!("SetCollectionCoverArt request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.update_collection_cover_art(&req.collection_id, Some(&req.media_file_id)) {
            Ok(()) => {
                let response = SetCollectionCoverArtResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to set collection cover art: {:?}", e);
                let response = SetCollectionCoverArtResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }
    
    async fn remove_collection_cover_art(
        &self,
        request: Request<RemoveCollectionCoverArtRequest>,
    ) -> Result<Response<RemoveCollectionCoverArtResponse>, Status> {
        debug!("RemoveCollectionCoverArt request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.update_collection_cover_art(&req.collection_id, None) {
            Ok(()) => {
                let response = RemoveCollectionCoverArtResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove collection cover art: {:?}", e);
                let response = RemoveCollectionCoverArtResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }
    
    async fn set_project_audio_file(
        &self,
        request: Request<SetProjectAudioFileRequest>,
    ) -> Result<Response<SetProjectAudioFileResponse>, Status> {
        debug!("SetProjectAudioFile request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.update_project_audio_file(&req.project_id, Some(&req.media_file_id)) {
            Ok(()) => {
                let response = SetProjectAudioFileResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to set project audio file: {:?}", e);
                let response = SetProjectAudioFileResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }
    
    async fn remove_project_audio_file(
        &self,
        request: Request<RemoveProjectAudioFileRequest>,
    ) -> Result<Response<RemoveProjectAudioFileResponse>, Status> {
        debug!("RemoveProjectAudioFile request: {:?}", request);
        
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        match db.update_project_audio_file(&req.project_id, None) {
            Ok(()) => {
                let response = RemoveProjectAudioFileResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove project audio file: {:?}", e);
                let response = RemoveProjectAudioFileResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }
    
    /// List all media files with optional pagination
    async fn list_media_files(&self, request: Request<ListMediaFilesRequest>) -> Result<Response<ListMediaFilesResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        // Get media files
        let media_files = match db.list_media_files(req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to list media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Get total count
        let total_count = match db.get_media_files_count() {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get media files count: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Convert to proto format
        let proto_files = media_files.into_iter().map(|file| {
            super::proto::MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            }
        }).collect();
        
        Ok(Response::new(ListMediaFilesResponse {
            media_files: proto_files,
            total_count,
        }))
    }
    
    /// Get media files by type
    async fn get_media_files_by_type(&self, request: Request<GetMediaFilesByTypeRequest>) -> Result<Response<GetMediaFilesByTypeResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        // Get media files by type
        let media_files = match db.get_media_files_by_type(&req.media_type, req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get media files by type: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Get total count for this type
        let total_count = match db.get_media_files_count_by_type(&req.media_type) {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get media files count by type: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Convert to proto format
        let proto_files = media_files.into_iter().map(|file| {
            super::proto::MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            }
        }).collect();
        
        Ok(Response::new(GetMediaFilesByTypeResponse {
            media_files: proto_files,
            total_count,
        }))
    }
    
    /// Get orphaned media files
    async fn get_orphaned_media_files(&self, request: Request<GetOrphanedMediaFilesRequest>) -> Result<Response<GetOrphanedMediaFilesResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;
        
        // Get orphaned media files
        let orphaned_files = match db.get_orphaned_media_files(req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get orphaned media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Get total count of orphaned files
        let total_count = match db.get_orphaned_media_files_count() {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get orphaned media files count: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Convert to proto format
        let proto_files = orphaned_files.into_iter().map(|file| {
            super::proto::MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            }
        }).collect();
        
        Ok(Response::new(GetOrphanedMediaFilesResponse {
            orphaned_files: proto_files,
            total_count,
        }))
    }
    
    /// Get media statistics
    async fn get_media_statistics(&self, _request: Request<GetMediaStatisticsRequest>) -> Result<Response<GetMediaStatisticsResponse>, Status> {
        let db = self.db.lock().await;
        
        let (total_files, total_size, cover_art_count, audio_file_count, orphaned_count, orphaned_size) = match db.get_media_statistics() {
            Ok(stats) => stats,
            Err(e) => {
                error!("Failed to get media statistics: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        // Create a map of files by type
        let mut files_by_type = std::collections::HashMap::new();
        files_by_type.insert("cover_art".to_string(), cover_art_count);
        files_by_type.insert("audio_file".to_string(), audio_file_count);
        
        Ok(Response::new(GetMediaStatisticsResponse {
            total_files,
            total_size_bytes: total_size,
            cover_art_count,
            audio_file_count,
            orphaned_files_count: orphaned_count,
            orphaned_files_size_bytes: orphaned_size,
            files_by_type,
        }))
    }
    
    /// Cleanup orphaned media files
    async fn cleanup_orphaned_media(&self, request: Request<CleanupOrphanedMediaRequest>) -> Result<Response<CleanupOrphanedMediaResponse>, Status> {
        let req = request.into_inner();
        let mut db = self.db.lock().await;
        
        // Get orphaned files first
        let orphaned_files = match db.get_orphaned_media_files(None, None) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get orphaned media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };
        
        let mut deleted_file_ids = Vec::new();
        let mut bytes_freed = 0i64;
        
        if !req.dry_run {
            // Actually delete the files
            for file in &orphaned_files {
                // Delete from storage
                if let Err(e) = self.media_storage.delete_file(&file.id, &file.file_extension, &file.media_type) {
                    warn!("Failed to delete physical file from storage: {:?}", e);
                    // Continue with database deletion even if physical file deletion fails
                }
                
                // Delete from database
                if let Err(e) = db.delete_media_file(&file.id) {
                    error!("Failed to delete media file from database: {:?}", e);
                    continue;
                }
                
                deleted_file_ids.push(file.id.clone());
                bytes_freed += file.file_size_bytes as i64;
            }
        } else {
            // Dry run - just calculate what would be deleted
            for file in &orphaned_files {
                deleted_file_ids.push(file.id.clone());
                bytes_freed += file.file_size_bytes as i64;
            }
        }
        
        Ok(Response::new(CleanupOrphanedMediaResponse {
            files_cleaned: deleted_file_ids.len() as i32,
            bytes_freed,
            deleted_file_ids,
            success: true,
            error_message: None,
        }))
    }
}

// Helper function to convert LiveSet to protobuf Project
fn convert_live_set_to_proto(live_set: LiveSet, db: &mut LiveSetDatabase) -> Result<Project, DatabaseError> {
    let project_id = live_set.id.to_string();
    
    // Load notes from database
    let notes = db.get_project_notes(&project_id)?
        .unwrap_or_default();
    
    // Load audio file ID from database
    let audio_file_id = db.get_project_audio_file(&project_id)?
        .map(|media_file| media_file.id);
    
    // Load collection associations from database
    let collection_ids = db.get_collections_for_project(&project_id)?;
    
    // Load tag data from database
    let tag_data = db.get_project_tag_data(&project_id)?;
    
    // Load tasks from database
    let tasks = db.get_project_tasks(&project_id)?
        .into_iter()
        .map(|(task_id, description, completed, created_at)| super::proto::Task {
            id: task_id,
            description,
            completed,
            project_id: project_id.clone(), // Add project_id to Task
            created_at,
        })
        .collect();
    
    // Convert tags with proper IDs and creation timestamps
    let tags = tag_data.into_iter()
        .map(|(tag_id, tag_name, created_at)| super::proto::Tag {
            id: tag_id,
            name: tag_name,
            created_at,
        })
        .collect();
    
    Ok(Project {
        id: project_id,
        name: live_set.name,
        path: live_set.file_path.to_string_lossy().to_string(),
        hash: live_set.file_hash,
        notes,
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
        
        tags,
        tasks,
        collection_ids,
        audio_file_id,
    })
}
