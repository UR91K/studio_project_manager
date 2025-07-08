//! Studio Project Manager Library
//! 
//! This library provides functionality for scanning, parsing, and managing 
//! Ableton Live project files.

pub mod ableton_db;
pub mod config;
pub mod database;
pub mod error;
pub mod grpc;
pub mod live_set;
pub mod models;
pub mod scan;
pub mod utils;
pub mod watcher;

// Re-export commonly used items for easier imports in tests
pub use config::CONFIG;
pub use database::LiveSetDatabase;
pub use live_set::LiveSet;
pub use models::*;
pub use utils::decompress_gzip_file;

// Re-export functions from main that are used by other modules
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::mpsc::RecvTimeoutError;
use log::{info, debug, error};
use crate::database::batch::BatchInsertManager;
use crate::scan::parallel::ParallelParser;
use crate::error::LiveSetError;
use crate::live_set::LiveSetPreprocessed;
use crate::scan::project_scanner::ProjectPathScanner;

pub fn process_projects() -> Result<(), LiveSetError> {
    // Implementation moved from main.rs
    debug!("Starting process_projects");
    
    // Get paths from config
    let config = CONFIG.as_ref().map_err(|e| LiveSetError::ConfigError(e.clone()))?;
    debug!("Using database path from config: {}", config.database_path);
    debug!("Using project paths from config: {:?}", config.paths);
    
    // Initialize database early to use for filtering
    debug!("Initializing database at {}", config.database_path);
    let mut db = LiveSetDatabase::new(PathBuf::from(&config.database_path))?;
    
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
        return Ok(());
    }

    // Preprocess and filter projects
    let preprocessed = preprocess_projects(found_projects)?;
    let projects_to_parse = filter_unchanged_projects(preprocessed, &db)?;
    
    if projects_to_parse.is_empty() {
        info!("No projects need updating");
        return Ok(());
    }

    let total_projects = projects_to_parse.len();
    info!("Found {} projects that need parsing", total_projects);

    // Create parallel parser
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
    
    let mut successful_live_sets = Vec::new();
    let mut completed_count = 0;
    
    // Collect results from parser
    while completed_count < total_projects {
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => {
                completed_count += 1;
                info!("Progress: {}/{} projects processed", completed_count, total_projects);
                
                match result {
                    Ok((path, live_set)) => {
                        debug!("Successfully parsed: {}", path.display());
                        successful_live_sets.push(live_set);
                    }
                    Err((path, error)) => {
                        error!("Failed to parse {}: {:?}", path.display(), error);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                debug!("Timeout waiting for parser results, but continuing...");
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                debug!("Parser channel disconnected, finishing");
                break;
            }
        }
    }

    // Batch insert successful results
    if !successful_live_sets.is_empty() {
        info!("Inserting {} successfully parsed projects into database", successful_live_sets.len());
        let mut batch_manager = BatchInsertManager::new(&mut db.conn, successful_live_sets.into());
        let stats = batch_manager.execute()?;
        info!("Database insert stats: {:?}", stats);
    }

    Ok(())
}

pub fn process_projects_with_progress<F>(mut progress_callback: F) -> Result<(), LiveSetError> 
where 
    F: FnMut(u32, u32, f32, String, &str) + Send + 'static,
{
    debug!("Starting process_projects_with_progress");
    
    progress_callback(0, 0, 0.0, "Starting scan...".to_string(), "starting");
    
    // Get paths from config
    let config = CONFIG.as_ref().map_err(|e| LiveSetError::ConfigError(e.clone()))?;
    debug!("Using database path from config: {}", config.database_path);
    debug!("Using project paths from config: {:?}", config.paths);
    
    // Initialize database early to use for filtering
    debug!("Initializing database at {}", config.database_path);
    let mut db = LiveSetDatabase::new(PathBuf::from(&config.database_path))?;
    
    progress_callback(0, 0, 0.0, "Discovering projects...".to_string(), "discovering");
    
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
        progress_callback(1, 1, 1.0, "No projects found".to_string(), "completed");
        return Ok(());
    }

    let total_found = found_projects.len();
    progress_callback(0, total_found as u32, 0.0, format!("Found {} projects, preprocessing...", total_found), "preprocessing");

    // Preprocess and filter projects
    let preprocessed = preprocess_projects(found_projects)?;
    let projects_to_parse = filter_unchanged_projects(preprocessed, &db)?;
    
    if projects_to_parse.is_empty() {
        info!("No projects need updating");
        progress_callback(total_found as u32, total_found as u32, 1.0, "All projects up to date".to_string(), "completed");
        return Ok(());
    }

    let total_projects = projects_to_parse.len();
    info!("Found {} projects that need parsing", total_projects);
    progress_callback(0, total_projects as u32, 0.0, format!("Parsing {} projects...", total_projects), "parsing");

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
                let progress = completed_count as f32 / total_projects as f32;
                
                match result {
                    Ok((path, live_set)) => {
                        let filename = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        
                        debug!("Successfully parsed: {}", path.display());
                        
                        // Send detailed progress update with file name
                        progress_callback(
                            completed_count as u32, 
                            total_projects as u32, 
                            progress,
                            format!("Parsed {} ({}/{})", filename, completed_count, total_projects),
                            "parsing"
                        );
                        
                        successful_live_sets.push(live_set);
                    }
                    Err((path, error)) => {
                        let filename = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        
                        error!("Failed to parse {}: {:?}", path.display(), error);
                        
                        // Send progress update even for failed files
                        progress_callback(
                            completed_count as u32, 
                            total_projects as u32, 
                            progress,
                            format!("✗ Failed to parse {} ({}/{})", filename, completed_count, total_projects),
                            "parsing"
                        );
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                debug!("Timeout waiting for parser results, but continuing...");
                // Send heartbeat progress update during timeout
                progress_callback(
                    completed_count as u32, 
                    total_projects as u32, 
                    completed_count as f32 / total_projects as f32,
                    format!("Parsing in progress... ({}/{})", completed_count, total_projects),
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
        progress_callback(total_projects as u32, total_projects as u32, 1.0, "No projects needed updates".to_string(), "completed");
        return Ok(());
    }

    progress_callback(completed_count as u32, total_projects as u32, 0.9, "Saving to database...".to_string(), "inserting");

    // Batch insert the successfully parsed projects
    let num_live_sets = successful_live_sets.len();
    info!("Inserting {} projects into database", num_live_sets);
    let live_sets = std::sync::Arc::new(successful_live_sets);
    let mut batch_manager = BatchInsertManager::new(&mut db.conn, live_sets);
    let stats = batch_manager.execute()?;
    
    info!(
        "Batch insert complete: {} projects, {} plugins, {} samples",
        stats.projects_inserted,
        stats.plugins_inserted,
        stats.samples_inserted
    );

    progress_callback(total_projects as u32, total_projects as u32, 1.0, format!("Successfully processed {} projects", stats.projects_inserted), "completed");
    info!("Successfully processed {} projects", num_live_sets);
    Ok(())
}

fn preprocess_projects(paths: HashSet<PathBuf>) -> Result<Vec<LiveSetPreprocessed>, LiveSetError> {
    debug!("Preprocessing {} projects", paths.len());
    let mut preprocessed = Vec::with_capacity(paths.len());
    
    for path in paths {
        match LiveSetPreprocessed::new(path.clone()) {
            Ok(metadata) => {
                debug!("Successfully preprocessed: {}", metadata.name);
                preprocessed.push(metadata);
            }
            Err(e) => {
                error!("Failed to preprocess {}: {}", path.display(), e);
                continue;
            }
        }
    }
    
    debug!("Successfully preprocessed {} projects", preprocessed.len());
    Ok(preprocessed)
}

fn filter_unchanged_projects(
    preprocessed: Vec<LiveSetPreprocessed>, 
    db: &LiveSetDatabase
) -> Result<Vec<PathBuf>, LiveSetError> {
    let total_count = preprocessed.len();
    debug!("Filtering {} preprocessed projects", total_count);
    let mut to_parse = Vec::new();
    
    for project in preprocessed.into_iter() {
        match db.get_last_scanned_time(&project.path)? {
            Some(last_scanned) => {
                if project.modified_time > last_scanned {
                    debug!("Project needs update: {}", project.name);
                    to_parse.push(project.path);
                } else {
                    debug!("Project unchanged: {}", project.name);
                }
            }
            None => {
                debug!("New project found: {}", project.name);
                to_parse.push(project.path);
            }
        }
    }
    
    info!("Found {} projects that need parsing out of {} total", to_parse.len(), total_count);
    Ok(to_parse)
} 