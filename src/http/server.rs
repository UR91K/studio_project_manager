//! HTTP route table (ADR-0024).
//!
//! `/health` plus the tags domain (the pattern-proof domain). Remaining CRUD
//! domains and the streaming endpoints are added incrementally, one at a time,
//! per the ADR's implementation order.

use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::handlers::collections as collection_handlers;
use super::handlers::config as config_handlers;
use super::handlers::media as media_handlers;
use super::handlers::plugins as plugin_handlers;
use super::handlers::projects as project_handlers;
use super::handlers::samples as sample_handlers;
use super::handlers::search as search_handlers;
use super::handlers::system as system_handlers;
use super::handlers::tags as tag_handlers;
use super::handlers::tasks as task_handlers;
use super::state::AppState;

/// Builds the axum router for the HTTP adapter.
///
/// CORS is scoped permissively to localhost origins, per ADR-0024: the only
/// consumers are a Tauri webview and a local browser page, neither of which shares
/// the server's origin, and nothing here is intended to cross a real network.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin.to_str().map(is_local_origin).unwrap_or(false)
        }))
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/tags",
            get(tag_handlers::list_tags).post(tag_handlers::create_tag),
        )
        .route("/api/v1/tags/search", get(tag_handlers::search_tags))
        .route(
            "/api/v1/tags/statistics",
            get(tag_handlers::get_tag_statistics),
        )
        .route(
            "/api/v1/tags/with-usage",
            get(tag_handlers::get_all_tags_with_usage),
        )
        .route(
            "/api/v1/tags/batch-tag",
            post(tag_handlers::batch_tag_projects),
        )
        .route(
            "/api/v1/tags/batch-untag",
            post(tag_handlers::batch_untag_projects),
        )
        .route(
            "/api/v1/tags/{tag_id}",
            get(tag_handlers::get_tag)
                .put(tag_handlers::update_tag)
                .delete(tag_handlers::delete_tag),
        )
        .route(
            "/api/v1/tags/{tag_id}/projects",
            get(tag_handlers::get_projects_by_tag),
        )
        .route(
            "/api/v1/projects/{project_id}/tags/{tag_id}",
            post(tag_handlers::tag_project).delete(tag_handlers::untag_project),
        )
        .route("/api/v1/projects", get(project_handlers::list_projects))
        .route(
            "/api/v1/projects/statistics",
            get(project_handlers::get_statistics),
        )
        .route(
            "/api/v1/projects/batch-archive",
            post(project_handlers::batch_mark_archived),
        )
        .route(
            "/api/v1/projects/batch-delete",
            post(project_handlers::batch_delete),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(project_handlers::get_project).delete(project_handlers::mark_project_deleted),
        )
        .route(
            "/api/v1/projects/{project_id}/permanent",
            axum::routing::delete(project_handlers::permanently_delete_project),
        )
        .route(
            "/api/v1/projects/{project_id}/notes",
            put(project_handlers::update_project_notes),
        )
        .route(
            "/api/v1/projects/{project_id}/name",
            put(project_handlers::update_project_name),
        )
        .route(
            "/api/v1/projects/{project_id}/reactivate",
            post(project_handlers::reactivate_project),
        )
        .route(
            "/api/v1/projects/{project_id}/rescan",
            post(project_handlers::rescan_project),
        )
        .route(
            "/api/v1/collections",
            get(collection_handlers::list_collections).post(collection_handlers::create_collection),
        )
        .route(
            "/api/v1/collections/search",
            get(collection_handlers::search_collections),
        )
        .route(
            "/api/v1/collections/batch-create",
            post(collection_handlers::batch_create_collection_from),
        )
        .route(
            "/api/v1/collections/{collection_id}",
            get(collection_handlers::get_collection)
                .put(collection_handlers::update_collection)
                .delete(collection_handlers::delete_collection),
        )
        .route(
            "/api/v1/collections/{collection_id}/duplicate",
            post(collection_handlers::duplicate_collection),
        )
        .route(
            "/api/v1/collections/{collection_id}/projects",
            get(collection_handlers::get_collection_projects),
        )
        .route(
            "/api/v1/collections/{collection_id}/projects/{project_id}",
            post(collection_handlers::add_project_to_collection)
                .delete(collection_handlers::remove_project_from_collection),
        )
        .route(
            "/api/v1/collections/{collection_id}/reorder",
            put(collection_handlers::reorder_collection),
        )
        .route(
            "/api/v1/collections/{collection_id}/tasks",
            get(collection_handlers::get_collection_tasks),
        )
        .route(
            "/api/v1/collections/{collection_id}/statistics",
            get(collection_handlers::get_collection_statistics),
        )
        .route(
            "/api/v1/collections/{collection_id}/batch-add",
            post(collection_handlers::batch_add_to_collection),
        )
        .route(
            "/api/v1/collections/{collection_id}/batch-remove",
            post(collection_handlers::batch_remove_from_collection),
        )
        .route("/api/v1/search", get(search_handlers::search))
        .route(
            "/api/v1/projects/{project_id}/tasks",
            get(task_handlers::get_project_tasks).post(task_handlers::create_task),
        )
        .route(
            "/api/v1/projects/{project_id}/tasks/search",
            get(task_handlers::search_tasks),
        )
        .route(
            "/api/v1/tasks/statistics",
            get(task_handlers::get_task_statistics),
        )
        .route(
            "/api/v1/tasks/batch-update-status",
            post(task_handlers::batch_update_task_status),
        )
        .route(
            "/api/v1/tasks/batch-delete",
            post(task_handlers::batch_delete_tasks),
        )
        .route(
            "/api/v1/tasks/{task_id}",
            put(task_handlers::update_task).delete(task_handlers::delete_task),
        )
        .route("/api/v1/plugins", get(plugin_handlers::get_all_plugins))
        .route(
            "/api/v1/plugins/by-status",
            get(plugin_handlers::get_plugins_by_installed_status),
        )
        .route(
            "/api/v1/plugins/search",
            get(plugin_handlers::search_plugins),
        )
        .route(
            "/api/v1/plugins/stats",
            get(plugin_handlers::get_plugin_stats),
        )
        .route(
            "/api/v1/plugins/vendors",
            get(plugin_handlers::get_plugin_vendors),
        )
        .route(
            "/api/v1/plugins/formats",
            get(plugin_handlers::get_plugin_formats),
        )
        .route(
            "/api/v1/plugins/refresh-installation-status",
            post(plugin_handlers::refresh_plugin_installation_status),
        )
        .route("/api/v1/plugins/scan", post(plugin_handlers::scan_plugins))
        .route(
            "/api/v1/plugins/failed-files",
            get(plugin_handlers::get_failed_plugin_files),
        )
        .route(
            "/api/v1/plugins/{plugin_id}",
            get(plugin_handlers::get_plugin),
        )
        .route(
            "/api/v1/plugins/{plugin_id}/projects",
            get(plugin_handlers::get_projects_by_plugin),
        )
        .route("/api/v1/samples", get(sample_handlers::get_all_samples))
        .route(
            "/api/v1/samples/by-presence",
            get(sample_handlers::get_samples_by_presence),
        )
        .route(
            "/api/v1/samples/search",
            get(sample_handlers::search_samples),
        )
        .route(
            "/api/v1/samples/stats",
            get(sample_handlers::get_sample_stats),
        )
        .route(
            "/api/v1/samples/usage",
            get(sample_handlers::get_all_sample_usage_numbers),
        )
        .route(
            "/api/v1/samples/analytics",
            get(sample_handlers::get_sample_analytics),
        )
        .route(
            "/api/v1/samples/formats",
            get(sample_handlers::get_sample_formats),
        )
        .route(
            "/api/v1/samples/refresh-presence-status",
            post(sample_handlers::refresh_sample_presence_status),
        )
        .route(
            "/api/v1/samples/check",
            post(sample_handlers::check_samples),
        )
        .route(
            "/api/v1/samples/{sample_id}",
            get(sample_handlers::get_sample),
        )
        .route(
            "/api/v1/samples/{sample_id}/projects",
            get(sample_handlers::get_projects_by_sample),
        )
        .route("/api/v1/config", get(config_handlers::get_config))
        .route(
            "/api/v1/config/status",
            get(config_handlers::get_config_status),
        )
        .route(
            "/api/v1/config/paths",
            put(config_handlers::update_paths).post(config_handlers::add_path),
        )
        .route(
            "/api/v1/config/paths/remove",
            post(config_handlers::remove_path),
        )
        .route(
            "/api/v1/config/settings",
            put(config_handlers::update_settings),
        )
        .route(
            "/api/v1/config/reload",
            post(config_handlers::reload_config),
        )
        .route(
            "/api/v1/config/validate",
            get(config_handlers::validate_config),
        )
        .route(
            "/api/v1/media/cover-art",
            post(media_handlers::upload_cover_art),
        )
        .route(
            "/api/v1/media/audio-file",
            post(media_handlers::upload_audio_file),
        )
        .route("/api/v1/media", get(media_handlers::list_media_files))
        .route(
            "/api/v1/media/by-type",
            get(media_handlers::get_media_files_by_type),
        )
        .route(
            "/api/v1/media/orphaned",
            get(media_handlers::get_orphaned_media_files),
        )
        .route(
            "/api/v1/media/statistics",
            get(media_handlers::get_media_statistics),
        )
        .route(
            "/api/v1/media/cleanup-orphaned",
            post(media_handlers::cleanup_orphaned_media),
        )
        .route(
            "/api/v1/media/{media_file_id}",
            get(media_handlers::download_media).delete(media_handlers::delete_media),
        )
        .route(
            "/api/v1/collections/{collection_id}/cover-art",
            put(media_handlers::set_collection_cover_art)
                .delete(media_handlers::remove_collection_cover_art),
        )
        .route(
            "/api/v1/projects/{project_id}/audio-file",
            put(media_handlers::set_project_audio_file)
                .delete(media_handlers::remove_project_audio_file),
        )
        .route(
            "/api/v1/projects/{project_id}/audio-files",
            get(media_handlers::list_project_audio_files)
                .post(media_handlers::add_project_audio_file),
        )
        .route(
            "/api/v1/projects/{project_id}/audio-files/{media_file_id}",
            delete(media_handlers::remove_project_audio_file_from_list),
        )
        .route("/api/v1/system/info", get(system_handlers::get_system_info))
        .route("/api/v1/system/shutdown", post(system_handlers::shutdown))
        .route(
            "/api/v1/system/statistics",
            get(system_handlers::get_statistics),
        )
        .route(
            "/api/v1/system/statistics/export",
            get(system_handlers::export_statistics),
        )
        .route(
            "/api/v1/system/scan-status",
            get(system_handlers::get_scan_status),
        )
        .route(
            "/api/v1/system/scan",
            post(system_handlers::scan_directories),
        )
        .route(
            "/api/v1/system/watcher/start",
            post(system_handlers::start_watcher),
        )
        .route(
            "/api/v1/system/watcher/stop",
            post(system_handlers::stop_watcher),
        )
        .route(
            "/api/v1/system/watcher/events",
            get(system_handlers::get_watcher_events),
        )
        .route(
            "/api/v1/projects/add",
            post(system_handlers::add_single_project),
        )
        .route(
            "/api/v1/projects/add-multiple",
            post(system_handlers::add_multiple_projects),
        )
        .layer(cors)
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

/// Whether an `Origin` header names a page on this machine.
///
/// The host is matched exactly, never by prefix: a prefix check lets
/// `http://localhost.example.com` through. The Tauri webview's origin depends on
/// platform and version: `tauri://localhost` on macOS and Linux, and on Windows
/// `http://tauri.localhost` in Tauri 2 (the default) or `https://tauri.localhost` in
/// Tauri 1 and Tauri 2 with `useHttpsScheme`.
fn is_local_origin(origin: &str) -> bool {
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => authority,
    };
    match scheme {
        "http" | "https" => matches!(host, "localhost" | "127.0.0.1" | "tauri.localhost"),
        "tauri" => host == "localhost",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_origin;

    #[test]
    fn local_and_tauri_origins_are_allowed() {
        for origin in [
            "http://localhost",
            "http://localhost:5173",
            "https://localhost:5173",
            "http://127.0.0.1:8080",
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
        ] {
            assert!(is_local_origin(origin), "{origin} should be allowed");
        }
    }

    #[test]
    fn hosts_that_only_start_like_a_local_one_are_refused() {
        for origin in [
            "http://localhost.example.com",
            "http://127.0.0.1.example.com",
            "https://tauri.localhost.example.com",
            "tauri://localhost.example.com",
            "http://localhost:80.example.com",
            "ftp://localhost",
            "null",
            "",
        ] {
            assert!(!is_local_origin(origin), "{origin} should be refused");
        }
    }
}
