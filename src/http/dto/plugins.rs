//! HTTP wire types for the plugins domain (ADR-0024).
//!
//! `PluginStats`, `VendorInfo`, `FormatInfo`, `PluginRefreshResult`
//! (`src/database/plugins.rs`) and `PluginDetails` (`src/database/plugin_details.rs`)
//! already derive `Serialize` and are returned as-is, same reasoning as the other
//! domains' reuse of already-serde types.

use serde::{Deserialize, Serialize};

use crate::database::plugins::InstallState;
use crate::models::{Plugin as DomainPlugin, PluginWithUsage};

/// Parses a comma-separated `install_states` query value into the database
/// layer's tri-state set (ADR-0025). Unrecognised values are dropped rather
/// than rejected: an
/// empty set means "no filtering", which is what a request naming no state (or
/// only invalid ones) is asking for.
pub fn parse_install_states(raw: Option<&str>) -> Vec<InstallState> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    raw.split(',')
        .filter_map(|s| match s.trim() {
            "installed" => Some(InstallState::Installed),
            "absent" => Some(InstallState::Absent),
            "unscanned" => Some(InstallState::Unscanned),
            _ => None,
        })
        .collect()
}

/// `project_count` is always populated (ADR-0034); the old `usage_count` always equalled
/// it and is gone.
#[derive(Serialize)]
pub struct PluginDto {
    pub id: String,
    pub dev_identifier: String,
    pub name: String,
    pub format: String,
    pub installed: Option<bool>,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub project_count: i32,
    /// How its file failed the last scan, when every file of this plugin did (ADR-0067):
    /// `crashed`, `timeout`, `load_failed`, `result_lost` or `invalid_format`. It loaded once, so it is
    /// known; the plugins view shows it as Failed. The handler fills it in.
    pub scan_error: Option<String>,
}

impl From<PluginWithUsage> for PluginDto {
    fn from(plugin_with_usage: PluginWithUsage) -> Self {
        Self {
            id: plugin_with_usage.plugin.id.to_string(),
            dev_identifier: plugin_with_usage.plugin.dev_identifier,
            name: plugin_with_usage.plugin.name,
            format: plugin_with_usage.plugin.plugin_format.to_string(),
            installed: plugin_with_usage.plugin.installed,
            vendor: plugin_with_usage.plugin.vendor,
            version: plugin_with_usage.plugin.version,
            project_count: plugin_with_usage.project_count,
            scan_error: None,
        }
    }
}

impl PluginDto {
    /// For the routes that return plain plugins; the handler supplies the count.
    pub fn new(plugin: DomainPlugin, project_count: i32) -> Self {
        Self {
            id: plugin.id.to_string(),
            dev_identifier: plugin.dev_identifier,
            name: plugin.name,
            format: plugin.plugin_format.to_string(),
            installed: plugin.installed,
            vendor: plugin.vendor,
            version: plugin.version,
            project_count,
            scan_error: None,
        }
    }
}

#[derive(Deserialize)]
pub struct GetAllPluginsQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub vendor_filter: Option<String>,
    pub format_filter: Option<String>,
    pub install_states: Option<String>,
    pub min_project_count: Option<i32>,
    /// `active` (default) or `all`; see `parse_project_scope`.
    pub scope: Option<String>,
}

/// Maps the HTTP sort key onto the database layer's, which still calls it
/// `usage_count` (ADR-0034).
pub fn plugin_sort_key(sort_by: Option<String>) -> Option<String> {
    sort_by.map(|s| {
        if s == "project_count" {
            "usage_count".to_string()
        } else {
            s
        }
    })
}

#[derive(Deserialize)]
pub struct ByInstalledStatusQuery {
    pub install_states: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub scope: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchPluginsQuery {
    pub query: String,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub install_states: Option<String>,
    pub vendor_filter: Option<String>,
    pub format_filter: Option<String>,
    pub scope: Option<String>,
}

#[derive(Serialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginDto>,
    pub total_count: i32,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    /// For the vendor and format rollups' projects-using counts.
    pub scope: Option<String>,
}

/// Routes whose only option is the project scope.
#[derive(Deserialize)]
pub struct ScopeQuery {
    pub scope: Option<String>,
}

#[derive(Serialize)]
pub struct VendorListResponse {
    pub vendors: Vec<crate::database::plugins::VendorInfo>,
    pub total_count: i32,
}

#[derive(Serialize)]
pub struct FormatListResponse {
    pub formats: Vec<crate::database::plugins::FormatInfo>,
    pub total_count: i32,
}

/// `plugin` is the list row; `details` is everything the scan recorded, for the
/// inspector. A plugin no scan has found has `null` scanner fields and empty class and
/// bus lists.
#[derive(Serialize)]
pub struct GetPluginResponse {
    pub plugin: PluginDto,
    pub details: crate::database::plugin_details::PluginDetails,
}

/// The list and search filters, so the status bar counts what the list shows. All
/// optional; none given counts every plugin.
#[derive(Deserialize)]
pub struct PluginStatsQuery {
    pub query: Option<String>,
    pub vendor_filter: Option<String>,
    pub format_filter: Option<String>,
    pub install_states: Option<String>,
}

/// One event of the plugin scan stream (ADR-0038): the same progress fields as a
/// project scan, and on the final `completed` event, what the scan changed.
#[derive(Serialize)]
pub struct PluginScanEventDto {
    #[serde(flatten)]
    pub progress: crate::http::dto::system::ScanProgressDto,
    pub result: Option<crate::database::plugins::PluginRefreshResult>,
}

/// `?mode=changes` (the default) loads only new and changed plugin files; `?mode=all`
/// loads every one, retrying failures (ADR-0067).
#[derive(Deserialize)]
pub struct ScanModeQuery {
    pub mode: Option<String>,
}

/// A plugin file that failed and has never loaded, so no plugin row stands for it
/// (ADR-0067).
#[derive(Serialize)]
pub struct FailedPluginFileDto {
    pub path: String,
    /// `crashed`, `timeout`, `load_failed` or `result_lost`.
    pub error_type: String,
    pub error_message: Option<String>,
    /// Epoch seconds.
    pub scanned_at: i64,
}

#[derive(Serialize)]
pub struct FailedPluginFilesResponse {
    pub files: Vec<FailedPluginFileDto>,
}

#[derive(Deserialize)]
pub struct ProjectsByPluginQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    /// `all` includes archived projects, each marked by its `is_active`.
    pub scope: Option<String>,
}
