//! # Seula Library
//!
//! A high-performance library for scanning, parsing, and managing Ableton Live project files.
//! This library provides the core functionality for indexing Ableton Live projects, extracting
//! metadata, and storing it in a searchable database.
//!
//! ## Features
//!
//! - **Fast scanning**: Efficiently discovers Ableton Live projects across multiple directories
//! - **Parallel parsing**: Multi-threaded parsing of `.als` files for maximum performance
//! - **Comprehensive metadata extraction**: Tempo, plugins, samples, key signatures, and more
//! - **SQLite database**: Persistent storage with full-text search capabilities
//! - **HTTP API**: Remote access and integration capabilities
//! - **Media management**: Handle cover art and audio files
//! - **Real-time watching**: Monitor file system changes
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use seula::process_projects;
//!
//! // Process all projects configured in config.toml
//! process_projects().expect("Failed to process projects");
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//! - [`scan`]: Project discovery and parallel parsing
//! - [`database`]: SQLite storage and full-text search
//! - [`models`]: Core data structures and types
//! - [`media`]: Media file storage and management
//! - [`watcher`]: File system monitoring
//!
//! ## Configuration
//!
//! The library uses a `config.toml` file for configuration. See [`config`] module for details.

pub mod cli;
pub mod config;
pub mod database;
pub mod error;
pub mod http;
pub mod logging;
pub mod media;
pub mod models;
pub mod project;
pub mod scan;
pub mod services;
pub mod tray;
pub mod utils;
pub mod watcher;

// Re-export commonly used items for easier imports

/// Global configuration instance loaded from `config.toml`.
///
/// This provides access to all configuration settings including project paths,
/// database location, HTTP port, and other runtime options.
///
/// # Examples
///
/// ```rust,ignore
/// use seula::CONFIG;
///
/// let config = CONFIG.as_ref().expect("Config should be loaded");
/// ```
pub use config::CONFIG;

/// SQLite database interface for managing Ableton Live project data.
///
/// This is the main database abstraction that provides methods for storing,
/// retrieving, and searching project information, tags, collections, and more.
///
/// # Examples
///
/// ```rust,ignore
/// use seula::ProjectDatabase;
/// use std::path::PathBuf;
///
/// let db = ProjectDatabase::new(PathBuf::from("projects.db"))
///     .expect("Failed to create database");
/// ```
pub use database::ProjectDatabase;

/// Represents a parsed Ableton Live project with all extracted metadata.
///
/// This is the primary data structure containing all information extracted
/// from an Ableton Live Set (`.als`) file, including musical properties,
/// plugins, samples, and file metadata.
///
/// # Examples
///
/// ```rust,ignore
/// use seula::Project;
/// use std::path::PathBuf;
///
/// let project_path = PathBuf::from("project.als");
/// let project = Project::new(project_path).expect("Failed to parse project");
/// println!("Project tempo: {}", project.tempo);
/// ```
pub use project::Project;

/// All core data structures and types used throughout the library.
///
/// This includes enums for plugin formats, key signatures, time signatures,
/// and other musical and technical data types.
pub use models::*;

/// Utility function for decompressing gzip files.
///
/// This is commonly used for processing compressed Ableton Live project files.
///
/// # Arguments
///
/// * `path` - Path to the gzip file to decompress
///
/// # Returns
///
/// Returns `Vec<u8>` containing the decompressed data
///
/// # Errors
///
/// Returns an error if the file cannot be read or decompressed
pub use utils::decompress_gzip_file;

// Core processing functions
use crate::database::batch::BatchInsertManager;
use crate::error::LiveSetError;
use crate::project::ProjectPreprocessed;
use crate::scan::parallel::ParallelParser;
use crate::scan::plugins::{scan_files, ScanMode};
use crate::scan::project_scanner::ProjectPathScanner;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};

/// Processes all Ableton Live projects found in configured directories.
///
/// This is the main entry point for scanning and indexing projects. It discovers
/// projects in all configured paths, parses them in parallel, and stores the results
/// in the database. Only projects that are new or have been modified since the last
/// scan will be processed.
///
/// The function uses configuration from `config.toml` to determine which directories
/// to scan and where to store the database.
///
/// # Returns
///
/// Returns `Ok(())` if all projects were processed successfully, or an error if
/// the operation failed.
///
/// # Errors
///
/// Returns [`LiveSetError`] in the following cases:
/// - Configuration file cannot be loaded
/// - Database cannot be initialized
/// - Project directories cannot be accessed
/// - Critical parsing errors occur
///
/// # Examples
///
/// ```rust,ignore
/// use seula::process_projects;
///
/// // Process all projects - this will scan configured directories,
/// // parse any new or modified projects, and update the database
/// match process_projects() {
///     Ok(()) => println!("All projects processed successfully"),
///     Err(e) => eprintln!("Failed to process projects: {}", e),
/// }
/// ```
///
/// # Configuration
///
/// This function requires a valid `config.toml` file with at least:
/// ```toml
/// paths = ["/path/to/projects"]
/// database_path = "projects.db"
/// ```
pub fn process_projects() -> Result<(), LiveSetError> {
    process_projects_with_progress::<fn(u32, u32, f32, String, &str)>(None)
}

/// Processes all Ableton Live projects with progress callback support.
///
/// This function provides the same functionality as [`process_projects`] but allows
/// you to receive progress updates throughout the scanning and parsing process.
/// This is useful for building UIs or monitoring long-running operations.
///
/// # Arguments
///
/// * `progress_callback` - Optional callback function that receives progress updates.
///   The callback receives:
///   - `completed`: Number of items completed
///   - `total`: Total number of items to process
///   - `progress`: Progress as a float between 0.0 and 1.0
///   - `message`: Human-readable status message
///   - `phase`: Current phase ("starting", "discovering", "parsing", "inserting", "completed")
///
/// # Returns
///
/// Returns `Ok(())` if all projects were processed successfully, or an error if
/// the operation failed.
///
/// # Errors
///
/// Returns [`LiveSetError`] in the following cases:
/// - Configuration file cannot be loaded
/// - Database cannot be initialized
/// - Project directories cannot be accessed
/// - Critical parsing errors occur
///
/// # Examples
///
/// ```rust,ignore
/// use seula::process_projects_with_progress;
///
/// // Process with progress callback
/// let result = process_projects_with_progress(Some(|completed, total, progress, message, phase| {
///     println!("[{}] {:.1}% - {} ({}/{})",
///              phase.clone(),
///              progress.clone() * 100.0,
///              message.clone(),
///              completed.clone(),
///              total.clone());
/// }));
///
/// match result {
///     Ok(()) => println!("Processing complete!"),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
///
/// # Progress Phases
///
/// The callback will receive updates during these phases:
/// - `"starting"`: Initial setup and validation
/// - `"discovering"`: Scanning directories for project files
/// - `"preprocessing"`: Extracting basic metadata for filtering
/// - `"parsing"`: Full parsing of project files
/// - `"inserting"`: Saving results to database
/// - `"completed"`: Operation finished successfully
pub fn process_projects_with_progress<F>(progress_callback: Option<F>) -> Result<(), LiveSetError>
where
    F: FnMut(u32, u32, f32, String, &str) + Send + 'static,
{
    let config = CONFIG
        .as_ref()
        .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
    scan_projects(config, open_scan_database(config)?, progress_callback)
}

/// The same scan, reading and writing through `db`: the tray daemon's one connection,
/// shared with the HTTP adapter. A second connection would hold SQLite's
/// write lock through the batch insert, and a write from an adapter would wait out the
/// busy timeout and fail. On the shared handle it waits on the mutex instead.
///
/// `db` is locked only for each database step, never while discovering, parsing or
/// scanning plugins, and never across a progress callback. Call this from a blocking
/// thread: it takes the lock with `blocking_lock`.
pub fn process_projects_into<F>(
    db: &tokio::sync::Mutex<ProjectDatabase>,
    config: &config::Config,
    progress_callback: Option<F>,
) -> Result<(), LiveSetError>
where
    F: FnMut(u32, u32, f32, String, &str) + Send + 'static,
{
    scan_projects(config, ScanDatabase::Shared(db), progress_callback)
}

fn open_scan_database(config: &config::Config) -> Result<ScanDatabase<'static>, LiveSetError> {
    let database_path = config
        .database_path
        .as_ref()
        .expect("Database path should be set by config initialization");
    debug!("Initializing database at {}", database_path);
    Ok(ScanDatabase::Own(ProjectDatabase::new(PathBuf::from(
        database_path,
    ))?))
}

/// Where a scan reads and writes: a connection of its own (the CLI), or the daemon's
/// shared one.
enum ScanDatabase<'a> {
    Own(ProjectDatabase),
    Shared(&'a tokio::sync::Mutex<ProjectDatabase>),
}

impl ScanDatabase<'_> {
    fn lock(&mut self) -> ScanDatabaseGuard<'_> {
        match self {
            ScanDatabase::Own(db) => ScanDatabaseGuard::Own(db),
            ScanDatabase::Shared(db) => ScanDatabaseGuard::Shared(db.blocking_lock()),
        }
    }
}

enum ScanDatabaseGuard<'a> {
    Own(&'a mut ProjectDatabase),
    Shared(tokio::sync::MutexGuard<'a, ProjectDatabase>),
}

impl std::ops::Deref for ScanDatabaseGuard<'_> {
    type Target = ProjectDatabase;
    fn deref(&self) -> &ProjectDatabase {
        match self {
            ScanDatabaseGuard::Own(db) => db,
            ScanDatabaseGuard::Shared(db) => db,
        }
    }
}

impl std::ops::DerefMut for ScanDatabaseGuard<'_> {
    fn deref_mut(&mut self) -> &mut ProjectDatabase {
        match self {
            ScanDatabaseGuard::Own(db) => db,
            ScanDatabaseGuard::Shared(db) => db,
        }
    }
}

fn scan_projects<F>(
    config: &config::Config,
    mut db: ScanDatabase<'_>,
    mut progress_callback: Option<F>,
) -> Result<(), LiveSetError>
where
    F: FnMut(u32, u32, f32, String, &str) + Send + 'static,
{
    debug!("Starting a project scan");

    // Helper macro to call progress callback if provided
    macro_rules! progress {
        ($completed:expr, $total:expr, $progress:expr, $message:expr, $phase:expr) => {
            if let Some(ref mut callback) = progress_callback {
                callback($completed, $total, $progress, $message, $phase);
            }
        };
    }

    progress!(0, 0, 0.0, "Starting scan...".to_string(), "starting");

    debug!("Using project paths from config: {:?}", config.paths);

    let scanner = ProjectPathScanner::new()?;
    let mut found_projects = HashSet::new();

    // Check if configuration is ready for scanning
    if config.needs_setup() {
        info!("Configuration needs setup - no project paths configured");
        progress!(
            1,
            1,
            1.0,
            "Setup required: No project paths configured".to_string(),
            "completed"
        );
        return Ok(());
    }

    // Plugins before projects, on a first run only.
    //
    // The project parse below records plugin *references*, which the database layer
    // resolves against the plugins table. Scanning afterwards would work — the upsert
    // matches on identity either way — but it would leave a first run reporting every
    // plugin as unknown until the user found `seula plugin refresh` on their own.
    // Each database step locks for itself: `db.lock()` in a statement or condition is
    // released at its end.
    if !db.lock().has_scanned_plugins()? {
        info!("No plugin scan has completed yet; scanning installed plugins first");

        let roots: Vec<PathBuf> = config.vst_search_paths.iter().map(PathBuf::from).collect();
        let timeout = Duration::from_secs(config.vst_scan_timeout_secs);

        let scan = {
            // Report each plugin as it is attempted. This takes minutes on a real
            // library, and the plugin worth naming is the one currently being loaded --
            // especially when it is the one that hangs.
            let mut on_plugin = |index: usize, total: usize, path: &Path| {
                if let Some(ref mut callback) = progress_callback {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("plugin");
                    callback(
                        index as u32,
                        total as u32,
                        if total == 0 {
                            0.0
                        } else {
                            index as f32 / total as f32
                        },
                        format!("Scanning {}", name),
                        "scanning_plugins",
                    );
                }
            };
            // Nothing has been scanned, so every file is loaded. The files are still
            // recorded, so the next scan for changes can skip them (ADR-0067).
            scan_files(
                &roots,
                timeout,
                ScanMode::All,
                |_| Default::default(),
                &mut on_plugin,
            )
        };

        match scan {
            Ok(scan) => {
                let report = &scan.report;
                // A truncated scan has not really looked everywhere, so it neither
                // sweeps nor counts as having run.
                let full_scan = !report.budget_exhausted;
                let persisted = db
                    .lock()
                    .persist_plugin_scan_of(report, &scan.files, full_scan);
                match persisted {
                    Ok(persisted) => info!(
                        "First-run plugin scan: {} installed, {} missing, {} failed to load",
                        persisted.inserted + persisted.updated,
                        persisted.marked_missing,
                        report.failed()
                    ),
                    Err(e) => warn!("Failed to persist the first-run plugin scan: {:?}", e),
                }
            }
            Err(e) => {
                // Scanning plugins is not why the user ran this. A missing sidecar or
                // an unreadable plugin directory must not stop projects being indexed.
                warn!("Skipping the first-run plugin scan: {}", e);
            }
        }
    }

    progress!(
        0,
        0,
        0.0,
        "Discovering projects...".to_string(),
        "discovering"
    );

    // Scan all configured directories
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            info!("Scanning directory: {}", path.display());
            let projects = scanner.scan_directory(&path)?;
            debug!("Found {} projects in {}", projects.len(), path.display());
            found_projects.extend(projects);
        } else {
            error!("Directory does not exist: {}", path.display());
        }
    }

    if found_projects.is_empty() {
        info!("No Ableton projects found in configured paths");
        progress!(1, 1, 1.0, "No projects found".to_string(), "completed");
        return Ok(());
    }

    let total_found = found_projects.len();
    progress!(
        0,
        total_found as u32,
        0.0,
        format!("Found {} projects, preprocessing...", total_found),
        "preprocessing"
    );

    // Preprocess and filter projects
    let preprocessed = preprocess_projects(found_projects)?;
    let projects_to_parse = filter_unchanged_projects(preprocessed, &db.lock())?;

    if projects_to_parse.is_empty() {
        info!("No projects need updating");
        progress!(
            total_found as u32,
            total_found as u32,
            1.0,
            "All projects up to date".to_string(),
            "completed"
        );
        return Ok(());
    }

    let total_projects = projects_to_parse.len();
    info!("Found {} projects that need parsing", total_projects);
    progress!(
        0,
        total_projects as u32,
        0.0,
        format!("Parsing {} projects...", total_projects),
        "parsing"
    );

    // Create parallel parser with number of threads based on project count
    let thread_count = (total_projects / 2).max(1).min(4);
    debug!("Creating parallel parser with {} threads", thread_count);
    let parser = ParallelParser::new(thread_count);

    // Submit filtered projects for parsing
    debug!("Submitting {} projects to parser", total_projects);
    let receiver = {
        let receiver = parser.get_results_receiver();
        parser.submit_paths(projects_to_parse)?;
        receiver
    };
    // The parser is not dropped here: the receiver borrows it, so its workers live until
    // this function returns. The loop below ends by counting results, not on a closed
    // channel.

    let mut successful_projects = Vec::new();

    // Collect results from parser with progress tracking
    debug!("Starting to collect parser results");
    let mut completed_count = 0;

    while completed_count < total_projects {
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => {
                completed_count += 1;
                let progress_value = completed_count as f32 / total_projects as f32;

                match result {
                    Ok((path, project)) => {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");

                        debug!("Successfully parsed: {}", path.display());

                        // Send detailed progress update with file name
                        progress!(
                            completed_count as u32,
                            total_projects as u32,
                            progress_value,
                            format!(
                                "Parsed {} ({}/{})",
                                filename, completed_count, total_projects
                            ),
                            "parsing"
                        );

                        successful_projects.push(project);
                    }
                    Err((path, error)) => {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");

                        error!("Failed to parse {}: {:?}", path.display(), error);

                        // Send progress update even for failed files
                        progress!(
                            completed_count as u32,
                            total_projects as u32,
                            progress_value,
                            format!(
                                "✗ Failed to parse {} ({}/{})",
                                filename, completed_count, total_projects
                            ),
                            "parsing"
                        );
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                debug!("Timeout waiting for parser results, but continuing...");
                // Send heartbeat progress update during timeout
                progress!(
                    completed_count as u32,
                    total_projects as u32,
                    completed_count as f32 / total_projects as f32,
                    format!(
                        "Parsing in progress... ({}/{})",
                        completed_count, total_projects
                    ),
                    "parsing"
                );
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                debug!("Parser channel disconnected, assuming completion");
                break;
            }
        }
    }

    if successful_projects.is_empty() {
        progress!(
            total_projects as u32,
            total_projects as u32,
            1.0,
            "No projects needed updates".to_string(),
            "completed"
        );
        return Ok(());
    }

    progress!(
        completed_count as u32,
        total_projects as u32,
        0.9,
        "Saving to database...".to_string(),
        "inserting"
    );

    // Batch insert the successfully parsed projects
    let num_projects = successful_projects.len();
    info!("Inserting {} projects into database", num_projects);
    let projects = std::sync::Arc::new(successful_projects);
    let stats = {
        let mut db = db.lock();
        let mut batch_manager = BatchInsertManager::new(&mut db.conn, projects);
        batch_manager.execute()?
    };

    info!(
        "Batch insert complete: {} projects, {} plugins, {} samples",
        stats.projects_inserted, stats.plugins_inserted, stats.samples_inserted
    );

    progress!(
        total_projects as u32,
        total_projects as u32,
        1.0,
        format!(
            "Successfully processed {} projects",
            stats.projects_inserted
        ),
        "completed"
    );
    info!("Successfully processed {} projects", num_projects);
    Ok(())
}

/// Converts a set of project paths into preprocessed metadata objects.
///
/// This function performs lightweight preprocessing on discovered project files,
/// extracting basic metadata like file modification times and names without
/// fully parsing the project content. This allows for efficient filtering
/// of projects that haven't changed since the last scan.
///
/// # Arguments
///
/// * `paths` - Set of filesystem paths to Ableton Live project files
///
/// # Returns
///
/// Returns a vector of [`ProjectPreprocessed`] objects containing basic metadata
/// for each successfully processed project.
///
/// # Errors
///
/// Returns [`LiveSetError`] if the preprocessing operation fails critically.
/// Individual project preprocessing failures are logged but don't stop the overall process.
fn preprocess_projects(paths: HashSet<PathBuf>) -> Result<Vec<ProjectPreprocessed>, LiveSetError> {
    debug!("Preprocessing {} projects", paths.len());
    let mut preprocessed = Vec::with_capacity(paths.len());

    for path in paths {
        match ProjectPreprocessed::new(path.clone()) {
            Ok(metadata) => {
                trace!("Successfully preprocessed: {}", metadata.name);
                preprocessed.push(metadata);
            }
            Err(e) => {
                error!("Failed to preprocess {}: {}", path.display(), e);
                continue;
            }
        }
    }

    trace!("Successfully preprocessed {} projects", preprocessed.len());
    Ok(preprocessed)
}

/// Filters out projects that haven't changed since the last scan.
///
/// This function compares the modification time of each preprocessed project
/// against the last scan time stored in the database. Only projects that are
/// new or have been modified since the last scan are returned for full parsing.
///
/// # Arguments
///
/// * `preprocessed` - Vector of preprocessed project metadata
/// * `db` - Database reference for checking last scan times
///
/// # Returns
///
/// Returns a vector of [`PathBuf`] objects for projects that need to be parsed.
///
/// # Errors
///
/// Returns [`LiveSetError`] if database queries fail during the filtering process.
fn filter_unchanged_projects(
    preprocessed: Vec<ProjectPreprocessed>,
    db: &ProjectDatabase,
) -> Result<Vec<PathBuf>, LiveSetError> {
    let total_count = preprocessed.len();
    debug!("Filtering {} preprocessed projects", total_count);
    let mut to_parse = Vec::new();

    for project in preprocessed.into_iter() {
        match db.get_last_scanned_time(&project.path)? {
            Some(last_scanned) => {
                if project.modified_time > last_scanned {
                    trace!("Project needs update: {}", project.name);
                    to_parse.push(project.path);
                } else {
                    trace!("Project unchanged: {}", project.name);
                }
            }
            None => {
                debug!("New project found: {}", project.name);
                to_parse.push(project.path);
            }
        }
    }

    info!(
        "Found {} projects that need parsing out of {} total",
        to_parse.len(),
        total_count
    );
    Ok(to_parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    /// The daemon's scan once opened a connection of its own and wrote the whole batch
    /// through it, so a write from the HTTP adapter during the insert waited out
    /// SQLite's busy timeout and failed. It has to write through the handle it is given;
    /// an in-memory database makes that checkable, since no other connection can reach
    /// it.
    #[test]
    fn a_scan_writes_through_the_database_it_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let projects = dir.path().join("projects");
        // Empty, so the first-run plugin scan finds nothing instead of this machine's.
        let plugins = dir.path().join("plugins");
        std::fs::create_dir_all(&projects).unwrap();
        std::fs::create_dir_all(&plugins).unwrap();

        let als = projects.join("Song.als");
        let mut gz = GzEncoder::new(std::fs::File::create(&als).unwrap(), Compression::default());
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

        let config: config::Config = toml::from_str(&format!(
            "paths = [{:?}]\ndatabase_path = {:?}\nmedia_storage_dir = {:?}\nvst_search_paths = [{:?}]\n",
            projects.to_string_lossy(),
            dir.path().join("elsewhere.db").to_string_lossy(),
            dir.path().join("media").to_string_lossy(),
            plugins.to_string_lossy(),
        ))
        .unwrap();

        let shared =
            tokio::sync::Mutex::new(ProjectDatabase::new(PathBuf::from(":memory:")).unwrap());
        process_projects_into::<fn(u32, u32, f32, String, &str)>(&shared, &config, None).unwrap();

        let mut db = shared.blocking_lock();
        let project = db.get_project_by_path(&als.to_string_lossy()).unwrap();
        assert!(
            project.is_some(),
            "the scanned project is not in the database the scan was given"
        );
    }
}
