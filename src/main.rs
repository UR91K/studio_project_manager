mod ableton_db;
mod config;
mod error;
mod live_set;
pub mod database;
mod live_set_db_test;
mod models;
pub mod scan;
mod test_utils;
mod utils;
mod watcher;
pub mod grpc;
use std::collections::HashSet;
use std::path::PathBuf;
use log::{info, debug, error, warn};
use std::time::Duration;
// use std::sync::Mutex;
// use once_cell::sync::Lazy;

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::database::batch::BatchInsertManager;
use crate::scan::parallel::ParallelParser;
use crate::scan::project_scanner::ProjectPathScanner;
use crate::error::LiveSetError;
use crate::live_set::LiveSetPreprocessed;

// // Define a global progress callback
// type ProgressCallback = Box<dyn Fn(f32) + Send + 'static>;
// static PROGRESS_CALLBACK: Lazy<Mutex<Option<ProgressCallback>>> = Lazy::new(|| Mutex::new(None));

// // Function to set the progress callback
// pub fn set_progress_callback<F>(callback: F)
// where
//     F: Fn(f32) + Send + 'static,
// {
//     let mut progress_callback = PROGRESS_CALLBACK.lock().unwrap();
//     *progress_callback = Some(Box::new(callback));
// }

// // Function to clear the progress callback
// pub fn clear_progress_callback() {
//     let mut progress_callback = PROGRESS_CALLBACK.lock().unwrap();
//     *progress_callback = None;
// }

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
                // Continue with other files even if one fails
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
                    debug!(
                        "Project needs update: {} (last scanned: {}, modified: {})",
                        project.name,
                        last_scanned,
                        project.modified_time
                    );
                    to_parse.push(project.path);
                } else {
                    debug!(
                        "Project unchanged: {} (last scanned: {}, modified: {})",
                        project.name,
                        last_scanned,
                        project.modified_time
                    );
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

pub fn process_projects() -> Result<(), LiveSetError> {
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
    for project in &projects_to_parse {
        debug!("Will parse project: {}", project.display());
    }

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
                info!("Progress: {}/{} projects processed", completed_count, total_projects);
                
                // Progress updates are now handled by gRPC streaming
                // in the ScanDirectories endpoint
                
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
            Err(_) => {
                // If we timeout waiting for results, but haven't received all expected results
                // wait a bit longer unless it's been too long
                if completed_count < total_projects {
                    warn!(
                        "Timeout while waiting for results. Processed {}/{} projects. Continuing to wait...",
                        completed_count, total_projects
                    );
                    continue;
                }
                break;
            }
        }
    }
    
    info!("Processing complete. Successfully parsed {} out of {} projects", 
          successful_live_sets.len(), total_projects);

    // Insert all successful live sets into database
    if !successful_live_sets.is_empty() {
        debug!("Inserting {} live sets into database", successful_live_sets.len());
        let live_sets = std::sync::Arc::new(successful_live_sets);
        let mut batch_manager = BatchInsertManager::new(&mut db.conn, live_sets);
        let stats = batch_manager.execute()?;
        
        info!(
            "Batch insert complete: {} projects, {} plugins, {} samples",
            stats.projects_inserted,
            stats.plugins_inserted,
            stats.samples_inserted
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use std::env;

    static INIT: Once = Once::new();

    fn setup() {
        let _ = INIT.call_once(|| {
            let _ = env::set_var("RUST_LOG", "debug");
            if let Err(_) = env_logger::try_init() {
                // Logger already initialized, that's fine
            }
        });
    }

    #[test]
    fn test_process_projects_integration() {
        setup();

        // Get expected project paths from config
        let config = CONFIG.as_ref().expect("Failed to load config");
        let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
        
        // Scan configured paths to know what we expect to find
        let mut expected_projects = HashSet::new();
        for path in &config.paths {
            let path = PathBuf::from(path);
            if path.exists() {
                let projects = scanner.scan_directory(&path).expect("Failed to scan directory");
                expected_projects.extend(projects);
            }
        }
        
        assert!(!expected_projects.is_empty(), "No projects found in configured paths");
        let expected_project_names: HashSet<String> = expected_projects
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        // Run process_projects
        process_projects().expect("process_projects failed");

        // Open database and verify contents
        let db = LiveSetDatabase::new(PathBuf::from(&config.database_path))
            .expect("Failed to open database");

        // Get actual project names from database
        let mut stmt = db.conn.prepare("SELECT name FROM projects").expect("Failed to prepare query");
        let project_names: HashSet<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("Failed to execute query")
            .map(|r| r.expect("Failed to get project name"))
            .collect();

        // Verify all expected projects were processed
        for expected_name in &expected_project_names {
            assert!(
                project_names.contains(expected_name),
                "Project '{}' not found in database", expected_name
            );
        }

        // Get some statistics
        let project_count: i64 = db.conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("Failed to count projects");
        let plugin_count: i64 = db.conn
            .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
            .expect("Failed to count plugins");
        let sample_count: i64 = db.conn
            .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))
            .expect("Failed to count samples");

        println!("\nDatabase Statistics:");
        println!("Projects found: {}", project_count);
        println!("Plugins found: {}", plugin_count);
        println!("Samples found: {}", sample_count);
        println!("\nProjects processed:");
        for name in &project_names {
            println!("- {}", name);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    info!("Starting Studio Project Manager gRPC Server");
    
    // Create the gRPC server
    let server = grpc::server::StudioProjectManagerServer::new().await?;
    
    // Set up the gRPC service
    let addr = "127.0.0.1:50051".parse()?;
    info!("gRPC server listening on {}", addr);
    
    // Start the server
    tonic::transport::Server::builder()
        .add_service(grpc::proto::studio_project_manager_server::StudioProjectManagerServer::new(server))
        .serve(addr)
        .await?;
    
    Ok(())
}
