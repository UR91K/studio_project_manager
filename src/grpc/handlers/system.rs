use chrono;
use log::{debug, error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Request, Response, Status};

use super::utils::convert_live_set_to_proto;
use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::grpc::proto::*;
use crate::live_set::LiveSet;
use crate::process_projects_with_progress;
use crate::watcher::file_watcher::{FileEvent, FileWatcher};

pub struct SystemHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
    pub scan_status: Arc<Mutex<ScanStatus>>,
    pub scan_progress: Arc<Mutex<Option<ScanProgressResponse>>>,
    pub watcher: Arc<Mutex<Option<FileWatcher>>>,
    pub watcher_events: Arc<Mutex<Option<std::sync::mpsc::Receiver<FileEvent>>>>,
    pub start_time: Instant,
}

impl SystemHandler {
    pub fn new(
        db: Arc<Mutex<LiveSetDatabase>>,
        scan_status: Arc<Mutex<ScanStatus>>,
        scan_progress: Arc<Mutex<Option<ScanProgressResponse>>>,
        watcher: Arc<Mutex<Option<FileWatcher>>>,
        watcher_events: Arc<Mutex<Option<std::sync::mpsc::Receiver<FileEvent>>>>,
        start_time: Instant,
    ) -> Self {
        Self {
            db,
            scan_status,
            scan_progress,
            watcher,
            watcher_events,
            start_time,
        }
    }

    pub async fn scan_directories(
        &self,
        request: Request<ScanDirectoriesRequest>,
    ) -> Result<Response<ReceiverStream<Result<ScanProgressResponse, Status>>>, Status> {
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
            let progress_callback =
                move |completed: u32, total: u32, progress: f32, message: String, phase: &str| {
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

    pub async fn get_scan_status(
        &self,
        _request: Request<GetScanStatusRequest>,
    ) -> Result<Response<GetScanStatusResponse>, Status> {
        let status_guard = self.scan_status.lock().await;
        let status = *status_guard;
        drop(status_guard);

        let progress_guard = self.scan_progress.lock().await;
        let current_progress = progress_guard.clone();
        drop(progress_guard);

        let response = GetScanStatusResponse {
            status: status as i32,
            current_progress,
        };

        Ok(Response::new(response))
    }

    pub async fn add_single_project(
        &self,
        request: Request<AddSingleProjectRequest>,
    ) -> Result<Response<AddSingleProjectResponse>, Status> {
        debug!("AddSingleProject request: {:?}", request);

        let req = request.into_inner();
        let file_path = PathBuf::from(&req.file_path);

        // Validate file path
        if !file_path.exists() {
            return Ok(Response::new(AddSingleProjectResponse {
                success: false,
                project: None,
                error_message: Some("File does not exist".to_string()),
            }));
        }

        // Validate file extension
        if !file_path.extension().map_or(false, |ext| ext == "als") {
            return Ok(Response::new(AddSingleProjectResponse {
                success: false,
                project: None,
                error_message: Some("File must have .als extension".to_string()),
            }));
        }

        // Parse the file
        match LiveSet::new(file_path.clone()) {
            Ok(live_set) => {
                debug!("Successfully parsed project: {}", live_set.name);

                // Insert into database
                let mut db = self.db.lock().await;
                match db.insert_project(&live_set) {
                    Ok(()) => {
                        debug!(
                            "Successfully inserted project into database: {}",
                            live_set.name
                        );

                        // Convert to proto project
                        match convert_live_set_to_proto(live_set, &mut *db) {
                            Ok(proto_project) => {
                                info!("Successfully added single project: {}", req.file_path);
                                Ok(Response::new(AddSingleProjectResponse {
                                    success: true,
                                    project: Some(proto_project),
                                    error_message: None,
                                }))
                            }
                            Err(e) => {
                                error!("Failed to convert project to proto: {:?}", e);
                                Ok(Response::new(AddSingleProjectResponse {
                                    success: false,
                                    project: None,
                                    error_message: Some(format!(
                                        "Failed to convert project: {}",
                                        e
                                    )),
                                }))
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to insert project into database: {:?}", e);
                        Ok(Response::new(AddSingleProjectResponse {
                            success: false,
                            project: None,
                            error_message: Some(format!("Database error: {}", e)),
                        }))
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse project file {}: {:?}", req.file_path, e);
                Ok(Response::new(AddSingleProjectResponse {
                    success: false,
                    project: None,
                    error_message: Some(format!("Failed to parse project: {}", e)),
                }))
            }
        }
    }

    pub async fn start_watcher(
        &self,
        _request: Request<StartWatcherRequest>,
    ) -> Result<Response<StartWatcherResponse>, Status> {
        debug!("Starting file watcher");

        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;

        // Check if watcher is already active
        if watcher_guard.is_some() {
            return Ok(Response::new(StartWatcherResponse { success: true }));
        }

        // Create new watcher
        match FileWatcher::new(Arc::clone(&self.db)) {
            Ok((mut watcher, event_receiver)) => {
                // Add configured watch paths
                let config = CONFIG
                    .as_ref()
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
                Ok(Response::new(StartWatcherResponse { success: true }))
            }
            Err(e) => {
                error!("Failed to create file watcher: {}", e);
                Err(Status::internal(format!("Failed to start watcher: {}", e)))
            }
        }
    }

    pub async fn stop_watcher(
        &self,
        _request: Request<StopWatcherRequest>,
    ) -> Result<Response<StopWatcherResponse>, Status> {
        debug!("Stopping file watcher");

        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;

        // Check if watcher is active
        if watcher_guard.is_none() {
            return Ok(Response::new(StopWatcherResponse { success: true }));
        }

        // Stop the watcher by dropping it
        *watcher_guard = None;
        *events_guard = None;

        info!("File watcher stopped successfully");
        Ok(Response::new(StopWatcherResponse { success: true }))
    }

    pub async fn get_watcher_events(
        &self,
        _request: Request<GetWatcherEventsRequest>,
    ) -> Result<Response<ReceiverStream<Result<WatcherEventResponse, Status>>>, Status> {
        debug!("Getting watcher events stream");

        let mut events_guard = self.watcher_events.lock().await;

        // Check if watcher is active and has events
        if let Some(event_receiver) = events_guard.take() {
            let (tx, rx) = mpsc::channel(100);

            // Clone the database reference for the spawn task
            let db_clone = Arc::clone(&self.db);

            // Spawn a task to convert FileEvent to WatcherEventResponse
            tokio::spawn(async move {
                while let Ok(file_event) = event_receiver.recv() {
                    // Handle database operations for file events
                    match &file_event {
                        FileEvent::Deleted(path) => {
                            debug!("Processing file deletion for database update: {:?}", path);
                            let mut db = db_clone.lock().await;

                            // Try to find the project by path and mark it as deleted
                            match db.get_project_by_path(&path.to_string_lossy()) {
                                Ok(Some(project)) => {
                                    debug!(
                                        "Found project for deleted file: {} ({})",
                                        project.name, project.id
                                    );
                                    if let Err(e) = db.mark_project_deleted(&project.id) {
                                        warn!("Failed to mark project as deleted: {}", e);
                                    } else {
                                        info!(
                                            "Successfully marked project as deleted: {}",
                                            project.name
                                        );
                                    }
                                }
                                Ok(None) => {
                                    debug!("No project found for deleted file: {:?}", path);
                                }
                                Err(e) => {
                                    warn!("Error looking up project for deleted file: {}", e);
                                }
                            }
                        }
                        FileEvent::Created(_) => {
                            // TODO: Could trigger a scan of the new file in the future
                            debug!("File created event - no database action needed");
                        }
                        FileEvent::Modified(_) => {
                            // TODO: Could trigger a re-scan of the modified file in the future
                            debug!("File modified event - no database action needed");
                        }
                        FileEvent::Renamed { from, to: _ } => {
                            // TODO: Could update the project path in the database
                            debug!(
                                "File renamed event - no database action implemented for: {:?}",
                                from
                            );
                        }
                    }

                    // Convert to WatcherEventResponse for streaming
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

    pub async fn get_system_info(
        &self,
        _request: Request<GetSystemInfoRequest>,
    ) -> Result<Response<GetSystemInfoResponse>, Status> {
        let config = CONFIG
            .as_ref()
            .map_err(|e| Status::new(Code::Internal, format!("Config error: {}", e)))?;

        // Check if watcher is active
        let watcher_guard = self.watcher.lock().await;
        let watcher_active = watcher_guard.is_some();

        let uptime_seconds = self.start_time.elapsed().as_secs() as i64;

        let response = GetSystemInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            watch_paths: config.paths.clone(),
            watcher_active,
            uptime_seconds,
        };

        Ok(Response::new(response))
    }

    pub async fn get_statistics(
        &self,
        request: Request<GetStatisticsRequest>,
    ) -> Result<Response<GetStatisticsResponse>, Status> {
        debug!("Getting comprehensive statistics: {:?}", request);
        let _req = request.into_inner();

        let mut db = self.db.lock().await;

        // TODO: Implement filtering based on _req.date_range, _req.collection_ids, _req.tag_ids, _req.ableton_version_filter
        // For now, we'll use the existing implementation without filtering

        // Basic counts
        let (
            total_projects,
            total_plugins,
            total_samples,
            total_collections,
            total_tags,
            total_tasks,
        ) = db
            .get_basic_counts()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // Plugin statistics
        let top_plugins = db
            .get_top_plugins(10)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(
                |(name, vendor, count)| crate::grpc::proto::PluginStatistic {
                    name,
                    vendor,
                    usage_count: count,
                },
            )
            .collect();

        let top_vendors = db
            .get_top_vendors(10)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(
                |(vendor, plugin_count, usage_count)| crate::grpc::proto::VendorStatistic {
                    vendor,
                    plugin_count,
                    usage_count,
                },
            )
            .collect();

        // Musical statistics
        let tempo_distribution = db
            .get_tempo_distribution()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(tempo, count)| crate::grpc::proto::TempoStatistic { tempo, count })
            .collect();

        let key_distribution = db
            .get_key_distribution()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(key, count)| crate::grpc::proto::KeyStatistic { key, count })
            .collect();

        let time_signature_distribution = db
            .get_time_signature_distribution()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(
                |(numerator, denominator, count)| crate::grpc::proto::TimeSignatureStatistic {
                    numerator,
                    denominator,
                    count,
                },
            )
            .collect();

        // Project analytics
        let projects_per_year = db
            .get_projects_per_year()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(year, count)| crate::grpc::proto::YearStatistic { year, count })
            .collect();

        let projects_per_month: Vec<crate::grpc::proto::MonthStatistic> = db
            .get_projects_per_month(12)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(year, month, count)| crate::grpc::proto::MonthStatistic { year, month, count })
            .collect();

        // Calculate average monthly projects
        let total_months = projects_per_month.len() as f64;
        let average_monthly_projects = if total_months > 0.0 {
            projects_per_month
                .iter()
                .map(|m| m.count as f64)
                .sum::<f64>()
                / total_months
        } else {
            0.0
        };

        // Duration analytics
        let (average_project_duration_seconds, projects_under_40_seconds, longest_project_id) = db
            .get_duration_analytics()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let longest_project = if let Some(project_id) = longest_project_id {
            match db.get_project_by_id(&project_id) {
                Ok(Some(project)) => match convert_live_set_to_proto(project, &mut *db) {
                    Ok(proto_project) => Some(proto_project),
                    Err(_) => None,
                },
                _ => None,
            }
        } else {
            None
        };

        // Complexity metrics
        let (average_plugins_per_project, average_samples_per_project) = db
            .get_complexity_metrics()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let most_complex_projects = db
            .get_most_complex_projects(5)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        let mut proto_complex_projects = Vec::new();

        for (project_id, plugin_count, sample_count, complexity_score) in most_complex_projects {
            if let Ok(Some(project)) = db.get_project_by_id(&project_id) {
                if let Ok(proto_project) = convert_live_set_to_proto(project, &mut *db) {
                    proto_complex_projects.push(crate::grpc::proto::ProjectComplexityStatistic {
                        project: Some(proto_project),
                        plugin_count,
                        sample_count,
                        complexity_score,
                    });
                }
            }
        }

        // Sample statistics
        let top_samples = db
            .get_top_samples(10)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(
                |(name, path, usage_count)| crate::grpc::proto::SampleStatistic {
                    name,
                    path,
                    usage_count,
                },
            )
            .collect();

        // Tag statistics
        let top_tags = db
            .get_top_tags(10)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(name, usage_count)| crate::grpc::proto::TagStatistic { name, usage_count })
            .collect();

        // Task statistics
        let (completed_tasks, pending_tasks, task_completion_rate) = db
            .get_task_statistics()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // Recent activity
        let recent_activity = db
            .get_recent_activity(30)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(year, month, day, projects_created, projects_modified)| {
                crate::grpc::proto::ActivityTrendStatistic {
                    year,
                    month,
                    day,
                    projects_created,
                    projects_modified,
                }
            })
            .collect();

        // Version statistics
        let ableton_versions = db
            .get_ableton_version_stats()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(|(version, count)| crate::grpc::proto::VersionStatistic { version, count })
            .collect();

        // Collection analytics
        let (average_projects_per_collection, largest_collection_id) = db
            .get_collection_analytics()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let largest_collection = if let Some(collection_id) = largest_collection_id {
            match db.get_collection_by_id(&collection_id) {
                Ok(Some((
                    id,
                    name,
                    description,
                    notes,
                    created_at,
                    modified_at,
                    project_ids,
                    cover_art_id,
                ))) => {
                    // Get collection statistics
                    let (total_duration_seconds, project_count) =
                        db.get_collection_statistics(&id).unwrap_or((None, 0));

                    Some(crate::grpc::proto::Collection {
                        id,
                        name,
                        description,
                        notes,
                        created_at,
                        modified_at,
                        project_ids,
                        cover_art_id,
                        total_duration_seconds,
                        project_count,
                    })
                }
                _ => None,
            }
        } else {
            None
        };

        // Task completion trends
        let task_completion_trends = db
            .get_task_completion_trends(12)
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .into_iter()
            .map(
                |(year, month, completed_tasks, total_tasks, completion_rate)| {
                    crate::grpc::proto::TaskCompletionTrendStatistic {
                        year,
                        month,
                        completed_tasks,
                        total_tasks,
                        completion_rate,
                    }
                },
            )
            .collect();

        let response = GetStatisticsResponse {
            total_projects,
            total_plugins,
            total_samples,
            total_collections,
            total_tags,
            total_tasks,
            top_plugins,
            top_vendors,
            tempo_distribution,
            key_distribution,
            time_signature_distribution,
            projects_per_year,
            projects_per_month,
            average_monthly_projects,
            average_project_duration_seconds,
            projects_under_40_seconds,
            longest_project,
            most_complex_projects: proto_complex_projects,
            average_plugins_per_project,
            average_samples_per_project,
            top_samples,
            top_tags,
            completed_tasks,
            pending_tasks,
            task_completion_rate,
            recent_activity,
            ableton_versions,
            average_projects_per_collection,
            largest_collection,
            task_completion_trends,
        };

        debug!("Successfully gathered comprehensive statistics");
        Ok(Response::new(response))
    }

    pub async fn export_statistics(
        &self,
        request: Request<ExportStatisticsRequest>,
    ) -> Result<Response<ExportStatisticsResponse>, Status> {
        debug!("ExportStatistics request: {:?}", request);
        let req = request.into_inner();

        // Get statistics data (optionally with filters)
        let stats_request = req.filters.clone().unwrap_or_default();
        let stats_response = self.get_statistics(Request::new(stats_request)).await?;
        let stats = stats_response.into_inner();

        match req.format() {
            ExportFormat::ExportCsv => {
                // Generate CSV export
                let csv_data = self.generate_csv_export(stats).await?;
                let filename = format!(
                    "statistics_{}.csv",
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                );

                Ok(Response::new(ExportStatisticsResponse {
                    data: csv_data,
                    filename,
                    success: true,
                    error_message: None,
                }))
            }
        }
    }

    async fn generate_csv_export(&self, stats: GetStatisticsResponse) -> Result<Vec<u8>, Status> {
        let mut csv_content = String::new();

        // Basic statistics
        csv_content.push_str("Category,Value\n");
        csv_content.push_str(&format!("Total Projects,{}\n", stats.total_projects));
        csv_content.push_str(&format!("Total Plugins,{}\n", stats.total_plugins));
        csv_content.push_str(&format!("Total Samples,{}\n", stats.total_samples));
        csv_content.push_str(&format!("Total Collections,{}\n", stats.total_collections));
        csv_content.push_str(&format!("Total Tags,{}\n", stats.total_tags));
        csv_content.push_str(&format!("Total Tasks,{}\n", stats.total_tasks));
        csv_content.push_str(&format!("Completed Tasks,{}\n", stats.completed_tasks));
        csv_content.push_str(&format!("Pending Tasks,{}\n", stats.pending_tasks));
        csv_content.push_str(&format!(
            "Task Completion Rate,{:.2}%\n",
            stats.task_completion_rate * 100.0
        ));
        csv_content.push_str(&format!(
            "Average Project Duration,{:.2} seconds\n",
            stats.average_project_duration_seconds
        ));
        csv_content.push_str(&format!(
            "Average Projects per Collection,{:.2}\n",
            stats.average_projects_per_collection
        ));
        csv_content.push_str(&format!(
            "Average Plugins per Project,{:.2}\n",
            stats.average_plugins_per_project
        ));
        csv_content.push_str(&format!(
            "Average Samples per Project,{:.2}\n",
            stats.average_samples_per_project
        ));
        csv_content.push_str("\n");

        // Top plugins
        csv_content.push_str("Top Plugins\n");
        csv_content.push_str("Plugin Name,Vendor,Usage Count\n");
        for plugin in stats.top_plugins {
            csv_content.push_str(&format!(
                "{},{},{}\n",
                plugin.name, plugin.vendor, plugin.usage_count
            ));
        }
        csv_content.push_str("\n");

        // Tempo distribution
        csv_content.push_str("Tempo Distribution\n");
        csv_content.push_str("Tempo,Count\n");
        for tempo in stats.tempo_distribution {
            csv_content.push_str(&format!("{},{}\n", tempo.tempo, tempo.count));
        }
        csv_content.push_str("\n");

        // Key distribution
        csv_content.push_str("Key Distribution\n");
        csv_content.push_str("Key,Count\n");
        for key in stats.key_distribution {
            csv_content.push_str(&format!("{},{}\n", key.key, key.count));
        }

        Ok(csv_content.into_bytes())
    }
}
