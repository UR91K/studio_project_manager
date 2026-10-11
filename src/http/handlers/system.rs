//! System domain HTTP handlers (ADR-0024). Thin over `SystemService` and
//! `AppState.system`.
//!
//! The two streaming endpoints (scan progress, watcher events) are
//! Server-Sent Events, per ADR-0024's Streaming section: both wrap the same
//! callback-driven `SystemService` methods, forwarding each update as an SSE
//! `data:` event.

use std::convert::Infallible;
use std::path::PathBuf;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use tokio_stream::wrappers::ReceiverStream;

use crate::http::dto::parse_project_scope;
use crate::http::dto::projects::{project_to_dto, KeySignatureDto};
use crate::http::dto::system::{
    AddMultipleProjectsRequest, AddMultipleProjectsResponse, AddProjectResponse,
    AddSingleProjectRequest, ExportStatisticsQuery, ScanProgressDto, ScanStatusResponse,
    StatisticsDto, StatisticsQuery, SystemInfoResponse, WatcherActionResponse, WatcherEventDto,
};
use crate::http::error::ApiError;
use crate::http::state::AppState;
use crate::services::system::send_scan_update;
use crate::services::Statistics;

pub async fn get_system_info(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (version, watch_paths, watcher_active, uptime_seconds) = state
        .system
        .get_system_info()
        .await
        .map_err(|e| ApiError::Internal(format!("Config error: {}", e)))?;

    Ok(Json(SystemInfoResponse {
        version,
        watch_paths,
        watcher_active,
        uptime_seconds,
        config_path: crate::config::loader::find_config_file()
            .ok()
            .map(|p| p.display().to_string()),
        log_dir: crate::logging::log_dir().map(|p| p.display().to_string()),
    }))
}

/// 202, then the daemon stops (ADR-0076). Firing the switch here is what the tray's Quit
/// does too; the server's graceful shutdown lets this response finish first.
pub async fn shutdown(State(state): State<AppState>) -> StatusCode {
    state.shutdown.trigger();
    StatusCode::ACCEPTED
}

pub async fn get_scan_status(State(state): State<AppState>) -> impl IntoResponse {
    let (status, progress) = state.system.get_scan_status().await;
    Json(ScanStatusResponse {
        status: status.name().to_string(),
        current_progress: progress.map(ScanProgressDto::from),
    })
}

/// 409 when a scan, of projects or plugins, is already running (ADR-0038).
pub async fn scan_directories(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(100);

    let started = state
        .system
        .start_scan(move |response| {
            let status = response.status;
            let dto = ScanProgressDto::from(response);
            if let Ok(json) = serde_json::to_string(&dto) {
                send_scan_update(&tx, status, Ok(Event::default().data(json)));
            }
        })
        .await;
    if !started {
        return Err(ApiError::Conflict("A scan is already running".to_string()));
    }

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

pub async fn add_single_project(
    State(state): State<AppState>,
    Json(req): Json<AddSingleProjectRequest>,
) -> Json<AddProjectResponse> {
    let file_path = PathBuf::from(&req.file_path);

    match state.system.add_single_project(&file_path).await {
        Ok(project) => {
            let db_arc = state.system.db_handle();
            let mut db = db_arc.lock().await;
            match project_to_dto(project, &mut db) {
                Ok(dto) => Json(AddProjectResponse {
                    success: true,
                    project: Some(dto),
                    error_message: None,
                }),
                Err(e) => Json(AddProjectResponse {
                    success: false,
                    project: None,
                    error_message: Some(format!("Failed to convert project: {}", e)),
                }),
            }
        }
        Err(message) => Json(AddProjectResponse {
            success: false,
            project: None,
            error_message: Some(message),
        }),
    }
}

pub async fn add_multiple_projects(
    State(state): State<AppState>,
    Json(req): Json<AddMultipleProjectsRequest>,
) -> Json<AddMultipleProjectsResponse> {
    let total_requested = req.file_paths.len() as i32;
    let (successes, failures) = state.system.add_multiple_projects(req.file_paths).await;

    let mut failed_paths: Vec<String> = failures.iter().map(|(path, _)| path.clone()).collect();
    let mut error_messages: Vec<String> = failures.into_iter().map(|(_, msg)| msg).collect();

    let mut projects = Vec::new();
    if !successes.is_empty() {
        let db_arc = state.system.db_handle();
        let mut db = db_arc.lock().await;
        for (path, project) in successes {
            match project_to_dto(project, &mut db) {
                Ok(dto) => projects.push(dto),
                Err(e) => {
                    failed_paths.push(path);
                    error_messages.push(format!("Failed to convert project: {}", e));
                }
            }
        }
    }

    let successful_imports = projects.len() as i32;
    let failed_imports = total_requested - successful_imports;

    Json(AddMultipleProjectsResponse {
        success: failed_imports == 0,
        projects,
        failed_paths,
        error_messages,
        total_requested,
        successful_imports,
        failed_imports,
    })
}

pub async fn start_watcher(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let success = state
        .system
        .start_watcher()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to start watcher: {}", e)))?;

    Ok(Json(WatcherActionResponse { success }))
}

pub async fn stop_watcher(State(state): State<AppState>) -> Json<WatcherActionResponse> {
    state.system.stop_watcher().await;
    Json(WatcherActionResponse { success: true })
}

pub async fn get_watcher_events(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(100);

    let started = state
        .system
        .start_watcher_event_stream(move |event| {
            let dto = WatcherEventDto {
                event_type: event.event_type.name().to_string(),
                path: event.path,
                new_path: event.new_path,
                timestamp: event.timestamp,
            };
            if let Ok(json) = serde_json::to_string(&dto) {
                let _ = tx.try_send(Ok(Event::default().data(json)));
            }
        })
        .await;

    if !started {
        return Err(ApiError::InvalidRequest("Watcher not active".to_string()));
    }

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

pub async fn get_statistics(
    State(state): State<AppState>,
    Query(query): Query<StatisticsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let scope = parse_project_scope(query.scope.as_deref())?;
    let stats = state
        .system
        .get_statistics(scope)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let db_arc = state.system.db_handle();
    let mut db = db_arc.lock().await;
    let dto = StatisticsDto::build(stats, &mut db)
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;
    Ok(Json(dto))
}

pub async fn export_statistics(
    State(state): State<AppState>,
    Query(query): Query<ExportStatisticsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Only CSV exists today: an unrecognised or missing format falls back to it
    // rather than erroring, since it's the only variant that has ever existed.
    let _ = query.format;
    let scope = parse_project_scope(query.scope.as_deref())?;

    let stats = state
        .system
        .get_statistics(scope)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let csv_data = generate_csv_export(stats);
    let filename = format!(
        "statistics_{}.csv",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );

    let headers = [
        (axum::http::header::CONTENT_TYPE, "text/csv".to_string()),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, csv_data))
}

fn generate_csv_export(stats: Statistics) -> Vec<u8> {
    let mut csv_content = String::new();

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
    csv_content.push('\n');

    csv_content.push_str("Top Plugins\n");
    csv_content.push_str("Plugin Name,Vendor,Usage Count\n");
    for plugin in stats.top_plugins {
        csv_content.push_str(&format!(
            "{},{},{}\n",
            plugin.name, plugin.vendor, plugin.usage_count
        ));
    }
    csv_content.push('\n');

    csv_content.push_str("Tempo Distribution\n");
    csv_content.push_str("Tempo,Count\n");
    for tempo in stats.tempo_distribution {
        csv_content.push_str(&format!("{},{}\n", tempo.tempo, tempo.count));
    }
    csv_content.push('\n');

    csv_content.push_str("Key Distribution\n");
    csv_content.push_str("Key,Count\n");
    for key in stats.key_distribution {
        let name = key
            .key
            .as_deref()
            .and_then(KeySignatureDto::from_joined)
            .map(|k| k.sharp)
            .unwrap_or_else(|| "No key".to_string());
        csv_content.push_str(&format!("{},{}\n", name, key.count));
    }

    csv_content.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::generate_csv_export;
    use crate::services::Statistics;

    /// Regression: the rate arrived as a percentage and was multiplied by 100 again,
    /// so half the tasks done exported as "5000.00%".
    #[test]
    fn csv_completion_rate_is_a_percentage_once() {
        let stats = Statistics {
            task_completion_rate: 0.5,
            ..Default::default()
        };
        let csv = String::from_utf8(generate_csv_export(stats)).unwrap();
        assert!(csv.contains("Task Completion Rate,50.00%"), "{csv}");
    }
}
