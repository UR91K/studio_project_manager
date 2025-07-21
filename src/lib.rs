//! # Studio Project Manager Library
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
//! - **gRPC API**: Remote access and integration capabilities
//! - **Media management**: Handle cover art and audio files
//! - **Real-time watching**: Monitor file system changes
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use studio_project_manager::process_projects;
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
//! - [`grpc`]: gRPC server and API handlers
//! - [`models`]: Core data structures and types
//! - [`media`]: Media file storage and management
//! - [`watcher`]: File system monitoring
//!
//! ## Configuration
//!
//! The library uses a `config.toml` file for configuration. See [`config`] module for details.

pub mod ableton_db;
pub mod config;
pub mod database;
pub mod error;
pub mod grpc;
pub mod live_set;
pub mod media;
pub mod models;
pub mod scan;
pub mod tray;
pub mod utils;
pub mod watcher;
#[cfg(windows)]
pub mod windows_paths;

// Re-export commonly used items for easier imports

/// Global configuration instance loaded from `config.toml`.
///
/// This provides access to all configuration settings including project paths,
/// database location, gRPC port, and other runtime options.
///
/// # Examples
///
/// ```rust,ignore
/// use studio_project_manager::CONFIG;
///
/// let config = CONFIG.as_ref().expect("Config should be loaded");
/// println!("gRPC port: {}", config.grpc_port);
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
/// use studio_project_manager::LiveSetDatabase;
/// use std::path::PathBuf;
///
/// let db = LiveSetDatabase::new(PathBuf::from("projects.db"))
///     .expect("Failed to create database");
/// ```
pub use database::LiveSetDatabase;

/// Represents a parsed Ableton Live project with all extracted metadata.
///
/// This is the primary data structure containing all information extracted
/// from an Ableton Live Set (`.als`) file, including musical properties,
/// plugins, samples, and file metadata.
///
/// # Examples
///
/// ```rust,ignore
/// use studio_project_manager::LiveSet;
/// use std::path::PathBuf;
///
/// let project_path = PathBuf::from("project.als");
/// let live_set = LiveSet::new(project_path).expect("Failed to parse project");
/// println!("Project tempo: {}", live_set.tempo);
/// ```
pub use live_set::LiveSet;

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
use crate::live_set::LiveSetPreprocessed;
use crate::scan::parallel::ParallelParser;
use crate::scan::project_scanner::ProjectPathScanner;
use log::{debug, error, info, trace};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

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
/// use studio_project_manager::process_projects;
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
/// use studio_project_manager::process_projects_with_progress;
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
pub fn process_projects_with_progress<F>(
    mut progress_callback: Option<F>,
) -> Result<(), LiveSetError>
where
    F: FnMut(u32, u32, f32, String, &str) + Send + 'static,
{
    debug!("Starting process_projects_with_progress");

    // Helper macro to call progress callback if provided
    macro_rules! progress {
        ($completed:expr, $total:expr, $progress:expr, $message:expr, $phase:expr) => {
            if let Some(ref mut callback) = progress_callback {
                callback($completed, $total, $progress, $message, $phase);
            }
        };
    }

    progress!(0, 0, 0.0, "Starting scan...".to_string(), "starting");

    // Get paths from config
    let config = CONFIG
        .as_ref()
        .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
    let database_path = config
        .database_path
        .as_ref()
        .expect("Database path should be set by config initialization");
    debug!("Using database path from config: {}", database_path);
    debug!("Using project paths from config: {:?}", config.paths);

    // Initialize database early to use for filtering
    debug!("Initializing database at {}", database_path);
    let mut db = LiveSetDatabase::new(PathBuf::from(database_path))?;

    progress!(
        0,
        0,
        0.0,
        "Discovering projects...".to_string(),
        "discovering"
    );

    let scanner = ProjectPathScanner::new()?;
    let mut found_projects = HashSet::new();

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
    let projects_to_parse = filter_unchanged_projects(preprocessed, &db)?;

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
    // Parser is dropped here, which will close the work channel

    let mut successful_live_sets = Vec::new();

    // Collect results from parser with progress tracking
    debug!("Starting to collect parser results");
    let mut completed_count = 0;

    while completed_count < total_projects {
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => {
                completed_count += 1;
                let progress_value = completed_count as f32 / total_projects as f32;

                match result {
                    Ok((path, live_set)) => {
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

                        successful_live_sets.push(live_set);
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

    if successful_live_sets.is_empty() {
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
    let num_live_sets = successful_live_sets.len();
    info!("Inserting {} projects into database", num_live_sets);
    let live_sets = std::sync::Arc::new(successful_live_sets);
    let mut batch_manager = BatchInsertManager::new(&mut db.conn, live_sets);
    let stats = batch_manager.execute()?;

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
    info!("Successfully processed {} projects", num_live_sets);
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
/// Returns a vector of [`LiveSetPreprocessed`] objects containing basic metadata
/// for each successfully processed project.
///
/// # Errors
///
/// Returns [`LiveSetError`] if the preprocessing operation fails critically.
/// Individual project preprocessing failures are logged but don't stop the overall process.
fn preprocess_projects(paths: HashSet<PathBuf>) -> Result<Vec<LiveSetPreprocessed>, LiveSetError> {
    debug!("Preprocessing {} projects", paths.len());
    let mut preprocessed = Vec::with_capacity(paths.len());

    for path in paths {
        match LiveSetPreprocessed::new(path.clone()) {
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
    preprocessed: Vec<LiveSetPreprocessed>,
    db: &LiveSetDatabase,
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
