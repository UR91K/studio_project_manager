use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::media::{MediaConfig, MediaStorageManager};

use super::handlers::*;
use super::common::*;
use super::projects::*;
use super::collections::*;
use super::tasks::*;
use super::search::*;
use super::tags::*;
use super::media::*;
use super::system::*;
use super::plugins::*;
use super::samples::*;
use super::scanning::*;
use super::watcher::*;

#[derive(Clone)]
pub struct StudioProjectManagerServer {
    pub projects_handler: ProjectsHandler,
    pub search_handler: SearchHandler,
    pub collections_handler: CollectionsHandler,
    pub tags_handler: TagsHandler,
    pub tasks_handler: TasksHandler,
    pub media_handler: MediaHandler,
    pub system_handler: SystemHandler,
    pub plugins_handler: PluginsHandler,
    pub samples_handler: SamplesHandler,
}

impl StudioProjectManagerServer {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = CONFIG
            .as_ref()
            .map_err(|e| format!("Failed to load config: {}", e))?;

        let database_path = config
            .database_path
            .as_ref()
            .expect("Database path should be set by config initialization");
        let db_path = PathBuf::from(database_path);
        let db = LiveSetDatabase::new(db_path)
            .map_err(|e| format!("Failed to initialize database: {}", e))?;
        let db = Arc::new(Mutex::new(db));

        let media_config = MediaConfig::from(config);
        let media_storage = Arc::new(MediaStorageManager::new(
            std::path::PathBuf::from(&config.media_storage_dir),
            media_config,
        )?);

        let scan_status = Arc::new(Mutex::new(ScanStatus::ScanUnknown));
        let scan_progress = Arc::new(Mutex::new(None));
        let watcher = Arc::new(Mutex::new(None));
        let watcher_events = Arc::new(Mutex::new(None));
        let start_time = Instant::now();

        Ok(Self {
            projects_handler: ProjectsHandler::new(Arc::clone(&db)),
            search_handler: SearchHandler::new(Arc::clone(&db)),
            collections_handler: CollectionsHandler::new(Arc::clone(&db)),
            tags_handler: TagsHandler::new(Arc::clone(&db)),
            tasks_handler: TasksHandler::new(Arc::clone(&db)),
            media_handler: MediaHandler::new(Arc::clone(&db), Arc::clone(&media_storage)),
            system_handler: SystemHandler::new(
                Arc::clone(&db),
                scan_status,
                scan_progress,
                watcher,
                watcher_events,
                start_time,
            ),
            plugins_handler: PluginsHandler::new(Arc::clone(&db)),
            samples_handler: SamplesHandler::new(Arc::clone(&db)),
        })
    }

    pub fn new_for_test(db: LiveSetDatabase, media_storage: MediaStorageManager) -> Self {
        let db = Arc::new(Mutex::new(db));
        let media_storage = Arc::new(media_storage);
        let scan_status = Arc::new(Mutex::new(ScanStatus::ScanUnknown));
        let scan_progress = Arc::new(Mutex::new(None));
        let watcher = Arc::new(Mutex::new(None));
        let watcher_events = Arc::new(Mutex::new(None));
        let start_time = Instant::now();

        Self {
            projects_handler: ProjectsHandler::new(Arc::clone(&db)),
            search_handler: SearchHandler::new(Arc::clone(&db)),
            collections_handler: CollectionsHandler::new(Arc::clone(&db)),
            tags_handler: TagsHandler::new(Arc::clone(&db)),
            tasks_handler: TasksHandler::new(Arc::clone(&db)),
            media_handler: MediaHandler::new(Arc::clone(&db), Arc::clone(&media_storage)),
            system_handler: SystemHandler::new(
                Arc::clone(&db),
                scan_status,
                scan_progress,
                watcher,
                watcher_events,
                start_time,
            ),
            plugins_handler: PluginsHandler::new(Arc::clone(&db)),
            samples_handler: SamplesHandler::new(Arc::clone(&db)),
        }
    }

    pub fn db(&self) -> &Arc<Mutex<LiveSetDatabase>> {
        &self.projects_handler.db
    }

    pub fn media_storage(&self) -> &Arc<MediaStorageManager> {
        &self.media_handler.media_storage
    }
}

// Project Service Implementation
#[tonic::async_trait]
impl project_service_server::ProjectService for StudioProjectManagerServer {
    async fn get_projects(
        &self,
        request: Request<GetProjectsRequest>,
    ) -> Result<Response<GetProjectsResponse>, Status> {
        self.projects_handler.get_projects(request).await
    }

    async fn get_project(
        &self,
        request: Request<GetProjectRequest>,
    ) -> Result<Response<GetProjectResponse>, Status> {
        self.projects_handler.get_project(request).await
    }

    async fn update_project_notes(
        &self,
        request: Request<UpdateProjectNotesRequest>,
    ) -> Result<Response<UpdateProjectNotesResponse>, Status> {
        self.projects_handler.update_project_notes(request).await
    }

    async fn update_project_name(
        &self,
        request: Request<UpdateProjectNameRequest>,
    ) -> Result<Response<UpdateProjectNameResponse>, Status> {
        self.projects_handler.update_project_name(request).await
    }

    async fn mark_project_deleted(
        &self,
        request: Request<MarkProjectDeletedRequest>,
    ) -> Result<Response<MarkProjectDeletedResponse>, Status> {
        self.projects_handler.mark_project_deleted(request).await
    }

    async fn reactivate_project(
        &self,
        request: Request<ReactivateProjectRequest>,
    ) -> Result<Response<ReactivateProjectResponse>, Status> {
        self.projects_handler.reactivate_project(request).await
    }

    async fn get_projects_by_deletion_status(
        &self,
        request: Request<GetProjectsByDeletionStatusRequest>,
    ) -> Result<Response<GetProjectsByDeletionStatusResponse>, Status> {
        self.projects_handler
            .get_projects_by_deletion_status(request)
            .await
    }

    async fn permanently_delete_project(
        &self,
        request: Request<PermanentlyDeleteProjectRequest>,
    ) -> Result<Response<PermanentlyDeleteProjectResponse>, Status> {
        self.projects_handler
            .permanently_delete_project(request)
            .await
    }

    async fn batch_mark_projects_as_archived(
        &self,
        request: Request<BatchMarkProjectsAsArchivedRequest>,
    ) -> Result<Response<BatchMarkProjectsAsArchivedResponse>, Status> {
        self.projects_handler
            .batch_mark_projects_as_archived(request)
            .await
    }

    async fn batch_delete_projects(
        &self,
        request: Request<BatchDeleteProjectsRequest>,
    ) -> Result<Response<BatchDeleteProjectsResponse>, Status> {
        self.projects_handler.batch_delete_projects(request).await
    }
}

// Search Service Implementation
#[tonic::async_trait]
impl search_service_server::SearchService for StudioProjectManagerServer {
    async fn search(
        &self,
        request: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        self.search_handler.search(request).await
    }
}

// Collection Service Implementation
#[tonic::async_trait]
impl collection_service_server::CollectionService for StudioProjectManagerServer {
    async fn get_collections(
        &self,
        request: Request<GetCollectionsRequest>,
    ) -> Result<Response<GetCollectionsResponse>, Status> {
        self.collections_handler.get_collections(request).await
    }

    async fn get_collection(
        &self,
        request: Request<GetCollectionRequest>,
    ) -> Result<Response<GetCollectionResponse>, Status> {
        self.collections_handler.get_collection(request).await
    }

    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> Result<Response<CreateCollectionResponse>, Status> {
        self.collections_handler.create_collection(request).await
    }

    async fn update_collection(
        &self,
        request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<UpdateCollectionResponse>, Status> {
        self.collections_handler.update_collection(request).await
    }

    async fn delete_collection(
        &self,
        request: Request<DeleteCollectionRequest>,
    ) -> Result<Response<DeleteCollectionResponse>, Status> {
        self.collections_handler.delete_collection(request).await
    }

    async fn duplicate_collection(
        &self,
        request: Request<DuplicateCollectionRequest>,
    ) -> Result<Response<DuplicateCollectionResponse>, Status> {
        self.collections_handler.duplicate_collection(request).await
    }

    async fn add_project_to_collection(
        &self,
        request: Request<AddProjectToCollectionRequest>,
    ) -> Result<Response<AddProjectToCollectionResponse>, Status> {
        self.collections_handler
            .add_project_to_collection(request)
            .await
    }

    async fn remove_project_from_collection(
        &self,
        request: Request<RemoveProjectFromCollectionRequest>,
    ) -> Result<Response<RemoveProjectFromCollectionResponse>, Status> {
        self.collections_handler
            .remove_project_from_collection(request)
            .await
    }

    async fn reorder_collection(
        &self,
        request: Request<ReorderCollectionRequest>,
    ) -> Result<Response<ReorderCollectionResponse>, Status> {
        self.collections_handler.reorder_collection(request).await
    }

    async fn get_collection_tasks(
        &self,
        request: Request<GetCollectionTasksRequest>,
    ) -> Result<Response<GetCollectionTasksResponse>, Status> {
        self.collections_handler.get_collection_tasks(request).await
    }

    async fn search_collections(
        &self,
        request: Request<SearchCollectionsRequest>,
    ) -> Result<Response<SearchCollectionsResponse>, Status> {
        self.collections_handler.search_collections(request).await
    }

    async fn get_collection_statistics(
        &self,
        request: Request<GetCollectionStatisticsRequest>,
    ) -> Result<Response<GetCollectionStatisticsResponse>, Status> {
        self.collections_handler.get_collection_statistics(request).await
    }

    async fn batch_add_to_collection(
        &self,
        request: Request<BatchAddToCollectionRequest>,
    ) -> Result<Response<BatchAddToCollectionResponse>, Status> {
        self.collections_handler
            .batch_add_to_collection(request)
            .await
    }

    async fn batch_remove_from_collection(
        &self,
        request: Request<BatchRemoveFromCollectionRequest>,
    ) -> Result<Response<BatchRemoveFromCollectionResponse>, Status> {
        self.collections_handler
            .batch_remove_from_collection(request)
            .await
    }

    async fn batch_create_collection_from(
        &self,
        request: Request<BatchCreateCollectionFromRequest>,
    ) -> Result<Response<BatchCreateCollectionFromResponse>, Status> {
        self.collections_handler
            .batch_create_collection_from(request)
            .await
    }

    async fn set_collection_cover_art(
        &self,
        request: Request<SetCollectionCoverArtRequest>,
    ) -> Result<Response<SetCollectionCoverArtResponse>, Status> {
        self.media_handler.set_collection_cover_art(request).await
    }

    async fn remove_collection_cover_art(
        &self,
        request: Request<RemoveCollectionCoverArtRequest>,
    ) -> Result<Response<RemoveCollectionCoverArtResponse>, Status> {
        self.media_handler
            .remove_collection_cover_art(request)
            .await
    }
}

// Tag Service Implementation
#[tonic::async_trait]
impl tag_service_server::TagService for StudioProjectManagerServer {
    async fn get_tags(
        &self,
        request: Request<GetTagsRequest>,
    ) -> Result<Response<GetTagsResponse>, Status> {
        self.tags_handler.get_tags(request).await
    }

    async fn create_tag(
        &self,
        request: Request<CreateTagRequest>,
    ) -> Result<Response<CreateTagResponse>, Status> {
        self.tags_handler.create_tag(request).await
    }

    async fn update_tag(
        &self,
        request: Request<UpdateTagRequest>,
    ) -> Result<Response<UpdateTagResponse>, Status> {
        self.tags_handler.update_tag(request).await
    }

    async fn delete_tag(
        &self,
        request: Request<DeleteTagRequest>,
    ) -> Result<Response<DeleteTagResponse>, Status> {
        self.tags_handler.delete_tag(request).await
    }

    async fn tag_project(
        &self,
        request: Request<TagProjectRequest>,
    ) -> Result<Response<TagProjectResponse>, Status> {
        self.tags_handler.tag_project(request).await
    }

    async fn untag_project(
        &self,
        request: Request<UntagProjectRequest>,
    ) -> Result<Response<UntagProjectResponse>, Status> {
        self.tags_handler.untag_project(request).await
    }

    async fn batch_tag_projects(
        &self,
        request: Request<BatchTagProjectsRequest>,
    ) -> Result<Response<BatchTagProjectsResponse>, Status> {
        self.tags_handler.batch_tag_projects(request).await
    }

    async fn batch_untag_projects(
        &self,
        request: Request<BatchUntagProjectsRequest>,
    ) -> Result<Response<BatchUntagProjectsResponse>, Status> {
        self.tags_handler.batch_untag_projects(request).await
    }
}

// Task Service Implementation
#[tonic::async_trait]
impl task_service_server::TaskService for StudioProjectManagerServer {
    async fn get_project_tasks(
        &self,
        request: Request<GetProjectTasksRequest>,
    ) -> Result<Response<GetProjectTasksResponse>, Status> {
        self.tasks_handler.get_project_tasks(request).await
    }

    async fn create_task(
        &self,
        request: Request<CreateTaskRequest>,
    ) -> Result<Response<CreateTaskResponse>, Status> {
        self.tasks_handler.create_task(request).await
    }

    async fn update_task(
        &self,
        request: Request<UpdateTaskRequest>,
    ) -> Result<Response<UpdateTaskResponse>, Status> {
        self.tasks_handler.update_task(request).await
    }

    async fn delete_task(
        &self,
        request: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        self.tasks_handler.delete_task(request).await
    }

    async fn batch_update_task_status(
        &self,
        request: Request<BatchUpdateTaskStatusRequest>,
    ) -> Result<Response<BatchUpdateTaskStatusResponse>, Status> {
        self.tasks_handler.batch_update_task_status(request).await
    }

    async fn batch_delete_tasks(
        &self,
        request: Request<BatchDeleteTasksRequest>,
    ) -> Result<Response<BatchDeleteTasksResponse>, Status> {
        self.tasks_handler.batch_delete_tasks(request).await
    }
}

// Media Service Implementation
#[tonic::async_trait]
impl media_service_server::MediaService for StudioProjectManagerServer {
    async fn upload_cover_art(
        &self,
        request: Request<tonic::Streaming<UploadCoverArtRequest>>,
    ) -> Result<Response<UploadCoverArtResponse>, Status> {
        self.media_handler.upload_cover_art(request).await
    }

    async fn upload_audio_file(
        &self,
        request: Request<tonic::Streaming<UploadAudioFileRequest>>,
    ) -> Result<Response<UploadAudioFileResponse>, Status> {
        self.media_handler.upload_audio_file(request).await
    }

    type DownloadMediaStream = ReceiverStream<Result<DownloadMediaResponse, Status>>;

    async fn download_media(
        &self,
        request: Request<DownloadMediaRequest>,
    ) -> Result<Response<Self::DownloadMediaStream>, Status> {
        self.media_handler.download_media(request).await
    }

    async fn delete_media(
        &self,
        request: Request<DeleteMediaRequest>,
    ) -> Result<Response<DeleteMediaResponse>, Status> {
        self.media_handler.delete_media(request).await
    }

    async fn set_project_audio_file(
        &self,
        request: Request<SetProjectAudioFileRequest>,
    ) -> Result<Response<SetProjectAudioFileResponse>, Status> {
        self.media_handler.set_project_audio_file(request).await
    }

    async fn remove_project_audio_file(
        &self,
        request: Request<RemoveProjectAudioFileRequest>,
    ) -> Result<Response<RemoveProjectAudioFileResponse>, Status> {
        self.media_handler.remove_project_audio_file(request).await
    }

    async fn list_media_files(
        &self,
        request: Request<ListMediaFilesRequest>,
    ) -> Result<Response<ListMediaFilesResponse>, Status> {
        self.media_handler.list_media_files(request).await
    }

    async fn get_media_files_by_type(
        &self,
        request: Request<GetMediaFilesByTypeRequest>,
    ) -> Result<Response<GetMediaFilesByTypeResponse>, Status> {
        self.media_handler.get_media_files_by_type(request).await
    }

    async fn get_orphaned_media_files(
        &self,
        request: Request<GetOrphanedMediaFilesRequest>,
    ) -> Result<Response<GetOrphanedMediaFilesResponse>, Status> {
        self.media_handler.get_orphaned_media_files(request).await
    }

    async fn get_media_statistics(
        &self,
        request: Request<GetMediaStatisticsRequest>,
    ) -> Result<Response<GetMediaStatisticsResponse>, Status> {
        self.media_handler.get_media_statistics(request).await
    }

    async fn cleanup_orphaned_media(
        &self,
        request: Request<CleanupOrphanedMediaRequest>,
    ) -> Result<Response<CleanupOrphanedMediaResponse>, Status> {
        self.media_handler.cleanup_orphaned_media(request).await
    }
}

// System Service Implementation
#[tonic::async_trait]
impl system_service_server::SystemService for StudioProjectManagerServer {
    async fn get_system_info(
        &self,
        request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        self.system_handler.get_system_info(request).await
    }

    async fn get_statistics(
        &self,
        request: Request<GetStatisticsRequest>,
    ) -> Result<Response<GetStatisticsResponse>, Status> {
        self.system_handler.get_statistics(request).await
    }

    async fn export_statistics(
        &self,
        request: Request<ExportStatisticsRequest>,
    ) -> Result<Response<ExportStatisticsResponse>, Status> {
        self.system_handler.export_statistics(request).await
    }
}

// Scanning Service Implementation
#[tonic::async_trait]
impl scanning_service_server::ScanningService for StudioProjectManagerServer {
    type ScanDirectoriesStream = ReceiverStream<Result<ScanProgressResponse, Status>>;

    async fn scan_directories(
        &self,
        request: Request<ScanDirectoriesRequest>,
    ) -> Result<Response<Self::ScanDirectoriesStream>, Status> {
        self.system_handler.scan_directories(request).await
    }

    async fn get_scan_status(
        &self,
        request: Request<GetScanStatusRequest>,
    ) -> Result<Response<GetScanStatusResponse>, Status> {
        self.system_handler.get_scan_status(request).await
    }

    async fn add_single_project(
        &self,
        request: Request<AddSingleProjectRequest>,
    ) -> Result<Response<AddSingleProjectResponse>, Status> {
        self.system_handler.add_single_project(request).await
    }

    async fn add_multiple_projects(
        &self,
        request: Request<AddMultipleProjectsRequest>,
    ) -> Result<Response<AddMultipleProjectsResponse>, Status> {
        self.system_handler.add_multiple_projects(request).await
    }
}

// Watcher Service Implementation
#[tonic::async_trait]
impl watcher_service_server::WatcherService for StudioProjectManagerServer {
    async fn start_watcher(
        &self,
        request: Request<StartWatcherRequest>,
    ) -> Result<Response<StartWatcherResponse>, Status> {
        self.system_handler.start_watcher(request).await
    }

    async fn stop_watcher(
        &self,
        request: Request<StopWatcherRequest>,
    ) -> Result<Response<StopWatcherResponse>, Status> {
        self.system_handler.stop_watcher(request).await
    }

    type GetWatcherEventsStream = ReceiverStream<Result<WatcherEventResponse, Status>>;

    async fn get_watcher_events(
        &self,
        request: Request<GetWatcherEventsRequest>,
    ) -> Result<Response<Self::GetWatcherEventsStream>, Status> {
        self.system_handler.get_watcher_events(request).await
    }
}

// Plugin Service Implementation
#[tonic::async_trait]
impl plugin_service_server::PluginService for StudioProjectManagerServer {
    async fn get_all_plugins(
        &self,
        request: Request<GetAllPluginsRequest>,
    ) -> Result<Response<GetAllPluginsResponse>, Status> {
        self.plugins_handler.get_all_plugins(request).await
    }

    async fn get_plugin_by_installed_status(
        &self,
        request: Request<GetPluginByInstalledStatusRequest>,
    ) -> Result<Response<GetPluginByInstalledStatusResponse>, Status> {
        self.plugins_handler
            .get_plugin_by_installed_status(request)
            .await
    }

    async fn search_plugins(
        &self,
        request: Request<SearchPluginsRequest>,
    ) -> Result<Response<SearchPluginsResponse>, Status> {
        self.plugins_handler.search_plugins(request).await
    }

    async fn get_plugin_stats(
        &self,
        request: Request<GetPluginStatsRequest>,
    ) -> Result<Response<GetPluginStatsResponse>, Status> {
        self.plugins_handler.get_plugin_stats(request).await
    }

    async fn get_plugin_vendors(
        &self,
        request: Request<GetPluginVendorsRequest>,
    ) -> Result<Response<GetPluginVendorsResponse>, Status> {
        self.plugins_handler.get_plugin_vendors(request).await
    }

    async fn get_plugin_formats(
        &self,
        request: Request<GetPluginFormatsRequest>,
    ) -> Result<Response<GetPluginFormatsResponse>, Status> {
        self.plugins_handler.get_plugin_formats(request).await
    }

    async fn get_plugin(
        &self,
        request: Request<GetPluginRequest>,
    ) -> Result<Response<GetPluginResponse>, Status> {
        self.plugins_handler.get_plugin(request).await
    }

    async fn get_projects_by_plugin(
        &self,
        request: Request<GetProjectsByPluginRequest>,
    ) -> Result<Response<GetProjectsByPluginResponse>, Status> {
        self.plugins_handler.get_projects_by_plugin(request).await
    }

    async fn refresh_plugin_installation_status(
        &self,
        request: Request<RefreshPluginInstallationStatusRequest>,
    ) -> Result<Response<RefreshPluginInstallationStatusResponse>, Status> {
        self.plugins_handler.refresh_plugin_installation_status(request).await
    }
}

// Sample Service Implementation
#[tonic::async_trait]
impl sample_service_server::SampleService for StudioProjectManagerServer {
    async fn get_all_samples(
        &self,
        request: Request<GetAllSamplesRequest>,
    ) -> Result<Response<GetAllSamplesResponse>, Status> {
        self.samples_handler.get_all_samples(request).await
    }

    async fn get_sample_by_presence(
        &self,
        request: Request<GetSampleByPresenceRequest>,
    ) -> Result<Response<GetSampleByPresenceResponse>, Status> {
        self.samples_handler.get_sample_by_presence(request).await
    }

    async fn search_samples(
        &self,
        request: Request<SearchSamplesRequest>,
    ) -> Result<Response<SearchSamplesResponse>, Status> {
        self.samples_handler.search_samples(request).await
    }

    async fn get_sample_stats(
        &self,
        request: Request<GetSampleStatsRequest>,
    ) -> Result<Response<GetSampleStatsResponse>, Status> {
        self.samples_handler.get_sample_stats(request).await
    }

    async fn get_all_sample_usage_numbers(
        &self,
        request: Request<GetAllSampleUsageNumbersRequest>,
    ) -> Result<Response<GetAllSampleUsageNumbersResponse>, Status> {
        self.samples_handler
            .get_all_sample_usage_numbers(request)
            .await
    }

    async fn get_projects_by_sample(
        &self,
        request: Request<GetProjectsBySampleRequest>,
    ) -> Result<Response<GetProjectsBySampleResponse>, Status> {
        self.samples_handler.get_projects_by_sample(request).await
    }

    async fn refresh_sample_presence_status(
        &self,
        request: Request<RefreshSamplePresenceStatusRequest>,
    ) -> Result<Response<RefreshSamplePresenceStatusResponse>, Status> {
        self.samples_handler.refresh_sample_presence_status(request).await
    }
}
