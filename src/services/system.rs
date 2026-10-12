use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use super::scan::{ScanProgress, ScanStatus, WatcherEvent, WatcherEventType};
use super::statistics::*;
use super::CollectionDetail;
use crate::config;
use crate::database::batch::BatchInsertManager;
use crate::database::plugins::PluginRefreshResult;
use crate::database::samples::SampleRefreshResult;
use crate::database::{ProjectDatabase, ProjectScope};
use crate::error::{DatabaseError, LiveSetError};
use crate::process_projects_into;
use crate::project::Project;
use crate::scan::sample_check::{check_sample_files, default_threads};
use crate::watcher::file_watcher::{FileEvent, FileWatcher};

/// What a project scan reports through: completed, total, progress, message, phase.
type ProgressCallback = Box<dyn FnMut(u32, u32, f32, String, &str) + Send>;

/// Queues one scan update on a stream's channel. A client that falls behind misses
/// updates rather than slowing the scan, but never the last one (`completed` or
/// `error`, by `status`), which carries the result: that one waits for room.
///
/// Call it only from a scan's progress callback. `start_scan`, `start_plugin_scan` and
/// `start_sample_check` run those on a blocking thread, where waiting is allowed; in a
/// task it would panic.
pub fn send_scan_update<T>(tx: &tokio::sync::mpsc::Sender<T>, status: ScanStatus, update: T) {
    if status.is_final() {
        let _ = tx.blocking_send(update);
    } else {
        let _ = tx.try_send(update);
    }
}

/// Owns the state shared across the scanning/watcher/statistics RPCs: scan
/// status/progress, the active file watcher (if any), and process start time.
#[derive(Clone)]
pub struct SystemService {
    db: Arc<Mutex<ProjectDatabase>>,
    scan_status: Arc<Mutex<ScanStatus>>,
    scan_progress: Arc<Mutex<Option<ScanProgress>>>,
    watcher: Arc<Mutex<Option<FileWatcher>>>,
    watcher_events: Arc<Mutex<Option<std::sync::mpsc::Receiver<FileEvent>>>>,
    start_time: Instant,
}

impl SystemService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<Mutex<ProjectDatabase>>,
        scan_status: Arc<Mutex<ScanStatus>>,
        scan_progress: Arc<Mutex<Option<ScanProgress>>>,
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

    pub fn db_handle(&self) -> Arc<Mutex<ProjectDatabase>> {
        Arc::clone(&self.db)
    }

    /// Escape hatch for tests that need to simulate scan progress without
    /// running an actual scan.
    pub fn scan_status_handle(&self) -> Arc<Mutex<ScanStatus>> {
        Arc::clone(&self.scan_status)
    }

    pub fn scan_progress_handle(&self) -> Arc<Mutex<Option<ScanProgress>>> {
        Arc::clone(&self.scan_progress)
    }

    pub async fn get_system_info(&self) -> Result<(String, Vec<String>, bool, i64), DatabaseError> {
        let config = config::current().map_err(DatabaseError::ConfigError)?;
        let watcher_active = self.watcher.lock().await.is_some();
        let uptime_seconds = self.start_time.elapsed().as_secs() as i64;

        Ok((
            env!("CARGO_PKG_VERSION").to_string(),
            config.paths.clone(),
            watcher_active,
            uptime_seconds,
        ))
    }

    pub async fn get_scan_status(&self) -> (ScanStatus, Option<ScanProgress>) {
        let status = *self.scan_status.lock().await;
        let progress = self.scan_progress.lock().await.clone();
        (status, progress)
    }

    /// Claims the scan status for a new scan, or returns false when one is already
    /// running. Project scans, plugin scans and sample checks share the status, so one
    /// runs at a time (ADR-0038, ADR-0041).
    async fn begin_scan(&self, status: ScanStatus) -> bool {
        let mut current = self.scan_status.lock().await;
        if current.is_running() {
            return false;
        }
        *current = status;
        *self.scan_progress.lock().await = None;
        true
    }

    /// Resets scan status/progress and starts the scan in the background,
    /// invoking `on_progress` with every update (including the final
    /// success/error one) so the caller can forward it to its own transport.
    /// Returns false, and starts nothing, when a scan is already running.
    pub async fn start_scan<F>(&self, on_progress: F) -> bool
    where
        F: Fn(ScanProgress) + Send + Sync + 'static,
    {
        let db = Arc::clone(&self.db);
        self.start_scan_with(on_progress, move |callback| {
            // One snapshot for the whole scan: an edit made meanwhile applies to the next.
            let config = config::current().map_err(LiveSetError::ConfigError)?;
            process_projects_into(&db, &config, Some(callback))
        })
        .await
    }

    /// `start_scan` with the scan itself passed in, so tests can drive the status
    /// handling without a configured project folder.
    async fn start_scan_with<F, S>(&self, on_progress: F, scan: S) -> bool
    where
        F: Fn(ScanProgress) + Send + Sync + 'static,
        S: FnOnce(ProgressCallback) -> Result<(), LiveSetError> + Send + 'static,
    {
        if !self.begin_scan(ScanStatus::Starting).await {
            return false;
        }

        let scan_status = Arc::clone(&self.scan_status);
        let scan_progress = Arc::clone(&self.scan_progress);

        // A blocking thread, like the plugin scan: the scan is synchronous, and writing
        // the status here, in order, is what stops an update landing after the final
        // one. Written from spawned tasks, one could, and it left the scan "running".
        tokio::task::spawn_blocking(move || {
            // blocking_lock is right here: this is a blocking thread, not a task.
            let report = Arc::new(move |response: ScanProgress, status: ScanStatus| {
                *scan_status.blocking_lock() = status;
                *scan_progress.blocking_lock() = Some(response.clone());
                on_progress(response);
            });
            let report_for_callback = Arc::clone(&report);

            let progress_callback =
                move |completed: u32, total: u32, progress: f32, message: String, phase: &str| {
                    let status = match phase {
                        "starting" => ScanStatus::Starting,
                        "scanning_plugins" => ScanStatus::ScanningPlugins,
                        "discovering" => ScanStatus::Discovering,
                        "preprocessing" | "parsing" => ScanStatus::Parsing,
                        "inserting" => ScanStatus::Inserting,
                        "completed" => ScanStatus::Completed,
                        _ => ScanStatus::Starting,
                    };

                    let response = ScanProgress {
                        completed,
                        total,
                        progress,
                        message,
                        status,
                    };
                    report_for_callback(response, status);
                };

            match scan(Box::new(progress_callback)) {
                Ok(()) => {
                    let final_status = ScanStatus::Completed;
                    let final_progress = ScanProgress {
                        completed: 100,
                        total: 100,
                        progress: 1.0,
                        message: "Scan completed successfully".to_string(),
                        status: final_status,
                    };
                    report(final_progress, final_status);
                }
                Err(e) => {
                    let error_status = ScanStatus::Error;
                    let error_progress = ScanProgress {
                        completed: 0,
                        total: 1,
                        progress: 0.0,
                        message: format!("Scan failed: {}", e),
                        status: error_status,
                    };
                    report(error_progress, error_status);
                }
            }
        });
        true
    }

    /// Rescans the system's plugins in the background (ADR-0038), reporting through
    /// the same status and progress as a project scan: `scanning_plugins` with one
    /// update per plugin binary, then `completed` or `error`. The final update also
    /// carries what the scan changed.
    ///
    /// The scan runs on a blocking thread without the database, which is locked only to
    /// write the result. Returns false, and starts nothing, when a scan is already
    /// running.
    pub async fn start_plugin_scan<F>(
        &self,
        mode: crate::scan::plugins::ScanMode,
        on_progress: F,
    ) -> bool
    where
        F: Fn(ScanProgress, Option<PluginRefreshResult>) + Send + Sync + 'static,
    {
        if !self.begin_scan(ScanStatus::ScanningPlugins).await {
            return false;
        }

        let db = Arc::clone(&self.db);
        let scan_status = Arc::clone(&self.scan_status);
        let scan_progress = Arc::clone(&self.scan_progress);

        tokio::task::spawn_blocking(move || {
            // blocking_lock is right here: this is a blocking thread, not a task.
            let report = |completed: u32, total: u32, message: String, status: ScanStatus| {
                let response = ScanProgress {
                    completed,
                    total,
                    progress: if total == 0 {
                        0.0
                    } else {
                        completed as f32 / total as f32
                    },
                    message,
                    status,
                };
                *scan_status.blocking_lock() = status;
                *scan_progress.blocking_lock() = Some(response.clone());
                response
            };

            on_progress(
                report(
                    0,
                    0,
                    "Finding plugins...".to_string(),
                    ScanStatus::ScanningPlugins,
                ),
                None,
            );

            let mut on_plugin = |index: usize, total: usize, path: &Path| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("plugin");
                on_progress(
                    report(
                        index as u32,
                        total as u32,
                        // Counted like the project scan's messages; this one is
                        // being loaded, not yet done.
                        format!("Scanning {} ({}/{})", name, index + 1, total),
                        ScanStatus::ScanningPlugins,
                    ),
                    None,
                );
            };
            let outcome =
                crate::services::plugins::scan_configured_plugins(mode, &db, &mut on_plugin)
                    .and_then(|scan| {
                        tracing::debug!(
                            "Plugin scan returned {} result(s); waiting for the database",
                            scan.report.results.len()
                        );
                        let lock_started = std::time::Instant::now();
                        let mut guard = db.blocking_lock();
                        tracing::debug!(
                            "Database locked after {} ms; recording the plugin scan",
                            lock_started.elapsed().as_millis()
                        );
                        let record_started = std::time::Instant::now();
                        let recorded = guard.record_plugin_refresh(&scan);
                        tracing::debug!(
                            "Plugin scan recorded in {} ms",
                            record_started.elapsed().as_millis()
                        );
                        recorded
                    });

            match outcome {
                Ok(result) => {
                    // Zero counts are left out: "0 failed to load" after a scan that
                    // skipped two failing files as unchanged would say they were fixed.
                    let counts = [
                        (result.unchanged, "unchanged"),
                        (result.scan_failures, "failed to load"),
                        (result.plugins_missing, "missing"),
                    ];
                    let mut message = format!(
                        "Plugin scan completed: {} scanned",
                        result.candidates_scanned
                    );
                    for (n, what) in counts.iter().filter(|(n, _)| *n > 0) {
                        message.push_str(&format!(", {n} {what}"));
                    }
                    let total = result.candidates_scanned.max(0) as u32;
                    on_progress(
                        report(total, total, message, ScanStatus::Completed),
                        Some(result),
                    );
                }
                Err(e) => {
                    let message = format!("Plugin scan failed: {}", e);
                    on_progress(report(0, 1, message, ScanStatus::Error), None);
                }
            }
        });
        true
    }

    /// Checks every sample file in the background (ADR-0041): whether it is still
    /// there, and its size. Reports like the other scans, as `checking_samples` with
    /// progress by folder, then `completed` (carrying what changed) or `error`.
    ///
    /// The database is locked only to read the paths and to write the result. Returns
    /// false, and starts nothing, when a scan is already running.
    pub async fn start_sample_check<F>(&self, on_progress: F) -> bool
    where
        F: Fn(ScanProgress, Option<SampleRefreshResult>) + Send + Sync + 'static,
    {
        if !self.begin_scan(ScanStatus::CheckingSamples).await {
            return false;
        }

        let db = Arc::clone(&self.db);
        let scan_status = Arc::clone(&self.scan_status);
        let scan_progress = Arc::clone(&self.scan_progress);

        tokio::task::spawn_blocking(move || {
            // blocking_lock is right here: this is a blocking thread, not a task.
            let report = |completed: u32, total: u32, message: String, status: ScanStatus| {
                let response = ScanProgress {
                    completed,
                    total,
                    progress: if total == 0 {
                        0.0
                    } else {
                        completed as f32 / total as f32
                    },
                    message,
                    status,
                };
                *scan_status.blocking_lock() = status;
                *scan_progress.blocking_lock() = Some(response.clone());
                response
            };

            // Its own statement: a guard taken inside the `and_then` chain would live to
            // the end of it, holding the database through the whole check, and then
            // deadlock on the lock below (the trap ADR-0002 records).
            let paths = db.blocking_lock().sample_paths();
            let outcome = paths.and_then(|paths| {
                on_progress(
                    report(
                        0,
                        0,
                        format!("Checking {} samples...", paths.len()),
                        ScanStatus::CheckingSamples,
                    ),
                    None,
                );
                // One event per folder would be thousands on a big library; about 200
                // is plenty for a progress bar.
                let mut on_folder = |done: usize, total: usize, folder: &Path| {
                    if done != total && !done.is_multiple_of((total / 200).max(1)) {
                        return;
                    }
                    let name = folder
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("folder");
                    on_progress(
                        report(
                            done as u32,
                            total as u32,
                            format!("Checked {} ({}/{})", name, done, total),
                            ScanStatus::CheckingSamples,
                        ),
                        None,
                    );
                };
                let found = check_sample_files(&paths, default_threads(), &mut on_folder);
                db.blocking_lock().record_sample_check(&found)
            });

            match outcome {
                Ok(result) => {
                    let message = format!(
                        "Sample check completed: {} checked, {} now missing, {} found again",
                        result.total_samples_checked,
                        result.samples_now_missing,
                        result.samples_now_present
                    );
                    let total = result.total_samples_checked.max(0) as u32;
                    on_progress(
                        report(total, total, message, ScanStatus::Completed),
                        Some(result),
                    );
                }
                Err(e) => {
                    let message = format!("Sample check failed: {}", e);
                    on_progress(report(0, 1, message, ScanStatus::Error), None);
                }
            }
        });
        true
    }

    /// Validates, parses, and inserts a single project via BatchInsertManager,
    /// returning the freshly-inserted domain Project. Error strings match the
    /// exact wording the API already commits to.
    pub async fn add_single_project(&self, file_path: &Path) -> Result<Project, String> {
        if !file_path.exists() {
            return Err("File does not exist".to_string());
        }
        if file_path.extension().is_none_or(|ext| ext != "als") {
            return Err("File must have .als extension".to_string());
        }

        // A full gunzip and parse: a blocking thread, not the runtime's.
        let path = file_path.to_path_buf();
        let live_set = tokio::task::spawn_blocking(move || Project::new(path))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
            .map_err(|e| format!("Failed to parse project: {}", e))?;
        // By path, not by the parse's id: a known path keeps the id it has (ADR-0052).
        let stored_path = live_set.file_path.to_string_lossy().to_string();

        let mut db = self.db.lock().await;
        let mut batch_manager = BatchInsertManager::new(&mut db.conn, Arc::new(vec![live_set]));
        batch_manager
            .execute()
            .map_err(|e| format!("Database error: {}", e))?;

        db.get_project_by_path(&stored_path)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| "Project inserted but not found".to_string())
    }

    /// Same shape as `add_single_project`, batched: every path is validated and
    /// parsed independently (a bad file doesn't block the others), then every
    /// successfully-parsed project is inserted in one BatchInsertManager
    /// transaction. Returns (path, project) pairs for what succeeded end to end,
    /// and (path, error message) pairs for everything that didn't.
    pub async fn add_multiple_projects(
        &self,
        file_paths: Vec<String>,
    ) -> (Vec<(String, Project)>, Vec<(String, String)>) {
        // Parsing is a full gunzip and parse per file: a blocking thread, not the
        // runtime's.
        let (mut failures, parsed) = tokio::task::spawn_blocking(move || {
            let mut failures = Vec::new();
            let mut parsed: Vec<(String, Project)> = Vec::new();

            for file_path_str in file_paths {
                let file_path = PathBuf::from(&file_path_str);

                if !file_path.exists() {
                    failures.push((file_path_str, "File does not exist".to_string()));
                    continue;
                }
                if file_path.extension().is_none_or(|ext| ext != "als") {
                    failures.push((file_path_str, "File must have .als extension".to_string()));
                    continue;
                }

                match Project::new(file_path.clone()) {
                    Ok(live_set) => parsed.push((file_path_str, live_set)),
                    Err(e) => {
                        failures.push((file_path_str, format!("Failed to parse project: {}", e)))
                    }
                }
            }

            (failures, parsed)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()));

        let mut successes = Vec::new();
        if !parsed.is_empty() {
            // By path, not by the parse's id: a known path keeps the id it has (ADR-0052).
            let stored_paths: Vec<(String, String)> = parsed
                .iter()
                .map(|(path, p)| (path.clone(), p.file_path.to_string_lossy().to_string()))
                .collect();
            let projects_to_insert: Vec<Project> = parsed.into_iter().map(|(_, p)| p).collect();

            let mut db = self.db.lock().await;
            let mut batch_manager =
                BatchInsertManager::new(&mut db.conn, Arc::new(projects_to_insert));
            match batch_manager.execute() {
                Ok(_) => {
                    for (path, stored_path) in stored_paths {
                        match db.get_project_by_path(&stored_path) {
                            Ok(Some(inserted)) => successes.push((path, inserted)),
                            Ok(None) => {
                                failures.push((path, "Project inserted but not found".to_string()))
                            }
                            Err(e) => failures.push((path, format!("Database error: {}", e))),
                        }
                    }
                }
                Err(e) => {
                    for (path, _) in stored_paths {
                        failures.push((path, format!("Database error: {}", e)));
                    }
                }
            }
        }

        (successes, failures)
    }

    /// Starts the file watcher if one isn't already active. Returns `Ok(true)`
    /// if a watcher is active on return (whether newly started or already was).
    pub async fn start_watcher(&self) -> Result<bool, String> {
        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;

        if watcher_guard.is_some() {
            return Ok(true);
        }

        let (mut watcher, event_receiver) =
            FileWatcher::new(Arc::clone(&self.db)).map_err(|e| e.to_string())?;

        let config = config::current().map_err(|e| format!("Config error: {}", e))?;
        for path in &config.paths {
            if let Err(e) = watcher.add_watch_path(PathBuf::from(path)) {
                tracing::warn!("Failed to add watch path {}: {}", path, e);
            }
        }

        *watcher_guard = Some(watcher);
        *events_guard = Some(event_receiver);
        Ok(true)
    }

    /// Makes a running folder watcher follow `paths` as the config changes: folders added
    /// are watched, folders removed are dropped. A watcher that is not running is left
    /// off -- starting one is a separate decision (ADR-0072). Spawns a task; call it
    /// once, inside the runtime.
    pub fn follow_config_paths(&self) {
        let Ok(mut changes) = config::subscribe() else {
            return;
        };
        let this = self.clone();
        tokio::spawn(async move {
            while changes.changed().await.is_ok() {
                let paths = changes.borrow_and_update().paths.clone();
                this.sync_watcher_paths(&paths).await;
            }
        });
    }

    /// Brings a running watcher's folders in line with `paths`. Does nothing when no
    /// watcher is running.
    pub async fn sync_watcher_paths(&self, paths: &[String]) {
        let mut guard = self.watcher.lock().await;
        let Some(watcher) = guard.as_mut() else {
            return;
        };
        let wanted: std::collections::HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let stale: Vec<PathBuf> = watcher
            .get_watch_paths()
            .iter()
            .filter(|p| !wanted.contains(*p))
            .cloned()
            .collect();
        for path in stale {
            if let Err(e) = watcher.remove_watch_path(&path) {
                tracing::warn!("Failed to stop watching {}: {}", path.display(), e);
            }
        }
        for path in wanted {
            if let Err(e) = watcher.add_watch_path(path.clone()) {
                tracing::warn!("Failed to add watch path {}: {}", path.display(), e);
            }
        }
    }

    /// The folders the running watcher has a handle on; empty when none is running.
    pub async fn watched_paths(&self) -> Vec<PathBuf> {
        match self.watcher.lock().await.as_ref() {
            Some(watcher) => watcher.get_watch_paths().iter().cloned().collect(),
            None => Vec::new(),
        }
    }

    pub async fn stop_watcher(&self) {
        let mut watcher_guard = self.watcher.lock().await;
        let mut events_guard = self.watcher_events.lock().await;
        *watcher_guard = None;
        *events_guard = None;
    }

    /// Takes ownership of the active watcher's event receiver (if any) and
    /// starts forwarding events in the background: `FileEvent::Deleted` marks
    /// the matching project deleted in the database, then every event (after
    /// its DB side effect, if any) is handed to `on_event`. Returns `false` if
    /// no watcher is active -- there is nothing to stream.
    pub async fn start_watcher_event_stream<F>(&self, on_event: F) -> bool
    where
        F: Fn(WatcherEvent) + Send + Sync + 'static,
    {
        let mut events_guard = self.watcher_events.lock().await;
        let Some(event_receiver) = events_guard.take() else {
            return false;
        };
        drop(events_guard);

        let db_clone = Arc::clone(&self.db);
        // A blocking thread: `recv()` waits for as long as the stream is open, and in a
        // task that would take a runtime worker with it.
        tokio::task::spawn_blocking(move || {
            while let Ok(file_event) = event_receiver.recv() {
                if let FileEvent::Deleted(path) = &file_event {
                    let mut db = db_clone.blocking_lock();
                    match db.get_project_by_path(&path.to_string_lossy()) {
                        Ok(Some(project)) => {
                            if let Err(e) = db.mark_project_deleted(&project.id) {
                                tracing::warn!("Failed to mark project as deleted: {}", e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!("Error looking up project for deleted file: {}", e)
                        }
                    }
                }

                let watcher_event = match file_event {
                    FileEvent::Created(path) => WatcherEvent {
                        event_type: WatcherEventType::Created,
                        path: path.to_string_lossy().to_string(),
                        new_path: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    },
                    FileEvent::Modified(path) => WatcherEvent {
                        event_type: WatcherEventType::Modified,
                        path: path.to_string_lossy().to_string(),
                        new_path: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    },
                    FileEvent::Deleted(path) => WatcherEvent {
                        event_type: WatcherEventType::Deleted,
                        path: path.to_string_lossy().to_string(),
                        new_path: None,
                        timestamp: chrono::Utc::now().timestamp(),
                    },
                    FileEvent::Renamed { from, to } => WatcherEvent {
                        event_type: WatcherEventType::Renamed,
                        path: from.to_string_lossy().to_string(),
                        new_path: Some(to.to_string_lossy().to_string()),
                        timestamp: chrono::Utc::now().timestamp(),
                    },
                };

                on_event(watcher_event);
            }
        });

        true
    }

    /// Gathers every statistic.
    ///
    /// Every figure counts the projects in `scope` and what they use (ADR-0045).
    pub async fn get_statistics(&self, scope: ProjectScope) -> Result<Statistics, DatabaseError> {
        let today = chrono::Utc::now().date_naive();
        let mut db = self.db.lock().await;

        let c = db.get_library_counts(scope)?;
        let total_projects = match scope {
            ProjectScope::Active => c.projects_active,
            ProjectScope::All => c.projects_active + c.projects_archived,
        };
        let total_plugins = c.plugins_installed + c.plugins_missing + c.plugins_not_scanned;
        let total_samples = c.samples_present + c.samples_missing;
        let total_collections = c.collections_with_projects + c.collections_empty;
        let total_tags = c.tags_in_use + c.tags_unused;
        let total_tasks = c.tasks_completed + c.tasks_pending;

        let top_plugins = db
            .get_top_plugins(10, scope)?
            .into_iter()
            .map(|(name, vendor, count)| PluginStatistic {
                name,
                vendor,
                usage_count: count,
            })
            .collect();

        let top_vendors = db
            .get_top_vendors(10, scope)?
            .into_iter()
            .map(|(vendor, plugin_count, usage_count)| VendorStatistic {
                vendor,
                plugin_count,
                usage_count,
            })
            .collect();

        let tempo_distribution = db
            .get_tempo_distribution(scope)?
            .into_iter()
            .map(|(tempo, count)| TempoStatistic { tempo, count })
            .collect();

        let key_distribution = db
            .get_key_distribution(scope)?
            .into_iter()
            .map(|(key, count)| KeyStatistic { key, count })
            .collect();

        let time_signature_distribution = db
            .get_time_signature_distribution(scope)?
            .into_iter()
            .map(|(numerator, denominator, count)| TimeSignatureStatistic {
                numerator,
                denominator,
                count,
            })
            .collect();

        let projects_per_year = db
            .get_projects_per_year(scope)?
            .into_iter()
            .map(|(year, count)| YearStatistic { year, count })
            .collect();

        let projects_per_month: Vec<MonthStatistic> = db
            .get_projects_per_month(12, today, scope)?
            .into_iter()
            .map(|(year, month, count)| MonthStatistic { year, month, count })
            .collect();

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

        let (average_project_duration_seconds, projects_under_40_seconds, longest_project_id) =
            db.get_duration_analytics(scope)?;

        let longest_project = match longest_project_id {
            Some(project_id) => db.get_project_by_id_any_status(&project_id).ok().flatten(),
            None => None,
        };

        let (average_plugins_per_project, average_samples_per_project) =
            db.get_complexity_metrics(scope)?;

        let most_complex_projects_raw = db.get_most_complex_projects(5, scope)?;
        let mut most_complex_projects = Vec::new();
        for (project_id, plugin_count, sample_count, complexity_score) in most_complex_projects_raw
        {
            if let Ok(Some(project)) = db.get_project_by_id_any_status(&project_id) {
                most_complex_projects.push(ProjectComplexity {
                    project,
                    plugin_count,
                    sample_count,
                    complexity_score,
                });
            }
        }

        let top_samples = db
            .get_top_samples(10, scope)?
            .into_iter()
            .map(|(name, path, usage_count)| SampleStatistic {
                name,
                path,
                usage_count,
            })
            .collect();

        let top_tags = db
            .get_top_tags(10, scope)?
            .into_iter()
            .map(|(name, usage_count)| TagStatistic { name, usage_count })
            .collect();

        let (completed_tasks, pending_tasks, task_completion_rate) =
            db.get_task_statistics(scope)?;

        let recent_activity = db
            .get_recent_activity(30, today, scope)?
            .into_iter()
            .map(
                |(year, month, day, projects_created, projects_modified)| ActivityTrend {
                    year,
                    month,
                    day,
                    projects_created,
                    projects_modified,
                },
            )
            .collect();

        let ableton_versions = db
            .get_ableton_version_stats(scope)?
            .into_iter()
            .map(|(version, count)| VersionStatistic { version, count })
            .collect();

        let (average_projects_per_collection, largest_collection_id) =
            db.get_collection_analytics(scope)?;

        let largest_collection = if let Some(collection_id) = largest_collection_id {
            match db.get_collection_by_id(&collection_id, scope) {
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
                    let (total_duration_seconds, project_count) = db
                        .get_collection_statistics(&id, scope)
                        .unwrap_or((None, 0));
                    Some(CollectionDetail {
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

        let task_completion_trends = db
            .get_task_completion_trends(12, today, scope)?
            .into_iter()
            .map(
                |(year, month, completed_tasks, total_tasks, completion_rate)| {
                    TaskCompletionTrend {
                        year,
                        month,
                        completed_tasks,
                        total_tasks,
                        completion_rate,
                    }
                },
            )
            .collect();

        Ok(Statistics {
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
            most_complex_projects,
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
            counts: c,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> SystemService {
        let db = ProjectDatabase::new(PathBuf::from(":memory:")).unwrap();
        SystemService::new(
            Arc::new(Mutex::new(db)),
            Arc::new(Mutex::new(ScanStatus::Unknown)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Instant::now(),
        )
    }

    /// Progress updates once wrote the status from tasks spawned per update, which
    /// nothing ordered against the final write. One that ran late put the scan back to
    /// `parsing`, and every later scan was refused until a restart.
    ///
    /// A current-thread runtime makes the old interleaving certain: the spawned writes
    /// queue behind the scan and all run after its final status.
    #[tokio::test]
    async fn a_late_progress_update_cannot_overwrite_the_final_status() {
        let system = test_service();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let started = system
            .start_scan_with(
                move |response| {
                    let _ = tx.send(response);
                },
                |mut progress| {
                    for i in 0..100 {
                        progress(i, 100, i as f32 / 100.0, "Parsing".to_string(), "parsing");
                    }
                    Ok(())
                },
            )
            .await;
        assert!(started);

        while rx.recv().await.is_some() {}
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }

        assert_eq!(*system.scan_status.lock().await, ScanStatus::Completed);
        assert!(
            system.start_scan_with(|_| {}, |_| Ok(())).await,
            "a new scan can start"
        );
    }

    /// The watcher stream once looped on a blocking `recv()` inside an async task, so
    /// while a client streamed events, one runtime worker did nothing else. On a
    /// current-thread runtime that is all of them: no other task ran again.
    #[test]
    fn a_watcher_event_stream_leaves_the_runtime_free() {
        let (event_tx, event_rx) = std::sync::mpsc::channel::<FileEvent>();
        let db = ProjectDatabase::new(PathBuf::from(":memory:")).unwrap();
        let system = SystemService::new(
            Arc::new(Mutex::new(db)),
            Arc::new(Mutex::new(ScanStatus::Unknown)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(Some(event_rx))),
            Instant::now(),
        );

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let runtime = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                assert!(system.start_watcher_event_stream(|_| {}).await);
                // Give the stream its turn, then see whether anything else gets one.
                tokio::task::yield_now().await;
                tokio::spawn(async {}).await.unwrap();
                let _ = done_tx.send(());
            });
        });

        let other_task_ran = done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .is_ok();
        // Ends the stream either way, so the runtime thread can finish.
        drop(event_tx);
        runtime.join().unwrap();
        assert!(other_task_ran, "the stream blocked the runtime");
    }

    /// Adding projects once parsed them inline, so the runtime worker ran nothing else
    /// until every file was gunzipped and parsed. Here, a task spawned first has to get
    /// a turn before each call returns.
    #[tokio::test]
    async fn adding_projects_parses_them_off_the_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not really.als");
        std::fs::write(&path, b"not gzip").unwrap();
        let system = test_service();

        for multiple in [false, true] {
            let returned = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let returned_seen = Arc::clone(&returned);
            let other =
                tokio::spawn(
                    async move { !returned_seen.load(std::sync::atomic::Ordering::SeqCst) },
                );

            if multiple {
                let (added, failed) = system
                    .add_multiple_projects(vec![path.to_string_lossy().into_owned()])
                    .await;
                assert!(added.is_empty() && failed.len() == 1);
            } else {
                assert!(system.add_single_project(&path).await.is_err());
            }
            returned.store(true, std::sync::atomic::Ordering::SeqCst);

            let ran_during_the_call = other.await.unwrap();
            assert!(
                ran_during_the_call,
                "parsing held the runtime (multiple: {multiple})"
            );
        }
    }

    /// Adding a file that is already a project updates that project, under its id
    /// (ADR-0052). Both calls read the project back by the id the parse minted, which a
    /// known path no longer gets.
    #[tokio::test]
    async fn adding_a_known_project_again_returns_it_under_its_id() {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Song.als");
        let mut gz = GzEncoder::new(
            std::fs::File::create(&path).unwrap(),
            Compression::default(),
        );
        gz.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12049" Creator="Ableton Live 12.0">
    <LiveSet>
        <Tempo>
            <LomId Value="0" />
            <Manual Value="120.0" />
        </Tempo>
        <EnumEvent Value="201" />
    </LiveSet>
</Ableton>"#,
        )
        .unwrap();
        gz.finish().unwrap();
        let system = test_service();

        let first = system.add_single_project(&path).await.expect("first add");
        let again = system.add_single_project(&path).await.expect("second add");
        assert_eq!(again.id, first.id);

        let (added, failed) = system
            .add_multiple_projects(vec![path.to_string_lossy().into_owned()])
            .await;
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(added[0].1.id, first.id);
    }
}
