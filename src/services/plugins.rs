use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::config;
use crate::database::plugin_details::PluginDetails;
use crate::database::plugins::{
    FormatInfo, InstallState, PluginFilter, PluginRefreshResult, PluginStats, VendorInfo,
};
use crate::database::{ProjectDatabase, ProjectScope};
use crate::error::DatabaseError;
use crate::models::PluginWithUsage;
use crate::project::Project;
use crate::scan::plugins::{scan_files, PluginScan, ScanMode};

#[derive(Clone)]
pub struct PluginsService {
    db: Arc<Mutex<ProjectDatabase>>,
}

impl PluginsService {
    pub fn new(db: Arc<Mutex<ProjectDatabase>>) -> Self {
        Self { db }
    }

    pub fn db_handle(&self) -> Arc<Mutex<ProjectDatabase>> {
        Arc::clone(&self.db)
    }

    /// How many projects use each plugin, keyed by id (ADR-0034).
    pub async fn project_counts(
        &self,
        ids: &[String],
        scope: ProjectScope,
    ) -> Result<std::collections::HashMap<String, i32>, DatabaseError> {
        self.db.lock().await.plugin_project_counts(ids, scope)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_all_plugins(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
        vendor_filter: Option<String>,
        format_filter: Option<String>,
        install_states: &[InstallState],
        min_usage_count: Option<i32>,
        scope: ProjectScope,
    ) -> Result<(Vec<PluginWithUsage>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.get_all_plugins(
            limit,
            offset,
            sort_by,
            sort_desc,
            vendor_filter,
            format_filter,
            install_states,
            min_usage_count,
            scope,
        )
    }

    pub async fn get_plugins_by_installed_status(
        &self,
        install_states: &[InstallState],
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
    ) -> Result<(Vec<crate::models::Plugin>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugins_by_installed_status(install_states, limit, offset, sort_by, sort_desc)
    }

    pub async fn search_plugins(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        install_states: &[InstallState],
        vendor_filter: Option<String>,
        format_filter: Option<String>,
    ) -> Result<(Vec<crate::models::Plugin>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.search_plugins(
            query,
            limit,
            offset,
            install_states,
            vendor_filter,
            format_filter,
        )
    }

    pub async fn get_plugin_stats(&self) -> Result<PluginStats, DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_stats()
    }

    /// The same counts over only the plugins `filter` selects.
    pub async fn get_plugin_stats_filtered(
        &self,
        filter: &PluginFilter,
    ) -> Result<PluginStats, DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_stats_filtered(filter)
    }

    pub async fn get_plugin_details(
        &self,
        plugin_id: &str,
    ) -> Result<Option<PluginDetails>, DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_details(plugin_id)
    }

    pub async fn get_plugin_vendors(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
        scope: ProjectScope,
    ) -> Result<(Vec<VendorInfo>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_vendors(limit, offset, sort_by, sort_desc, scope)
    }

    pub async fn get_plugin_formats(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
        scope: ProjectScope,
    ) -> Result<(Vec<FormatInfo>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_formats(limit, offset, sort_by, sort_desc, scope)
    }

    pub async fn get_plugin(
        &self,
        plugin_id: &str,
        scope: ProjectScope,
    ) -> Result<Option<PluginWithUsage>, DatabaseError> {
        let db = self.db.lock().await;
        db.get_plugin_by_id(plugin_id, scope)
    }

    pub async fn get_projects_by_plugin(
        &self,
        plugin_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        scope: ProjectScope,
    ) -> Result<(Vec<Project>, i32), DatabaseError> {
        let db = self.db.lock().await;
        db.get_projects_by_plugin_id(plugin_id, limit, offset, scope)
    }

    /// Plugins whose every file failed the last scan, by id, with how (ADR-0067).
    pub async fn scan_errors(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, DatabaseError> {
        self.db.lock().await.plugin_scan_errors()
    }

    /// Plugin files that failed and have never loaded (ADR-0067).
    pub async fn failed_files(
        &self,
    ) -> Result<Vec<crate::database::plugin_scan::FailedPluginFile>, DatabaseError> {
        self.db.lock().await.failed_plugin_files()
    }

    /// Rescan the system's plugins and record the result. Returns when the scan is
    /// done, which takes minutes; the database is locked only to read what is unchanged
    /// and to write the result (ADR-0038). The HTTP API's background scan is
    /// `SystemService::start_plugin_scan`.
    pub async fn refresh_plugin_installation_status(
        &self,
        mode: ScanMode,
    ) -> Result<PluginRefreshResult, DatabaseError> {
        let db = Arc::clone(&self.db);
        let scan = tokio::task::spawn_blocking(move || {
            scan_configured_plugins(mode, &db, &mut |_, _, _| {})
        })
        .await
        .map_err(|e| DatabaseError::ConnectionError(format!("Plugin scan task failed: {}", e)))??;
        self.db.lock().await.record_plugin_refresh(&scan)
    }
}

/// Scan the configured plugin search paths with the configured timeout, loading the
/// files `mode` asks for and reporting each as it is attempted (ADR-0067). Blocking and
/// slow: run it on a blocking thread. It locks `db` only to read which files are
/// unchanged, never while loading.
pub(crate) fn scan_configured_plugins(
    mode: ScanMode,
    db: &Mutex<ProjectDatabase>,
    on_progress: &mut dyn FnMut(usize, usize, &Path),
) -> Result<PluginScan, DatabaseError> {
    let config = config::current().map_err(DatabaseError::ConfigError)?;
    let roots: Vec<PathBuf> = config.vst_search_paths.iter().map(PathBuf::from).collect();
    let timeout = Duration::from_secs(config.vst_scan_timeout_secs);
    let unchanged = |files: &[_]| {
        db.blocking_lock()
            .unchanged_plugin_files(files)
            .unwrap_or_else(|e| {
                // Then nothing counts as unchanged, and the scan loads every file.
                tracing::warn!(
                    "Could not read the last plugin scan, so loading every file: {}",
                    e
                );
                Default::default()
            })
    };
    scan_files(&roots, timeout, mode, unchanged, on_progress)
        .map_err(|e| DatabaseError::ConnectionError(e.to_string()))
}
