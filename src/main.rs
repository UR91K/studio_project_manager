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
use std::collections::HashSet;
use std::path::PathBuf;
use log::{info, debug, error, warn};
use std::time::Duration;

use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::database::batch::BatchInsertManager;
use crate::scan::parallel::ParallelParser;
use crate::scan::project_scanner::ProjectPathScanner;
use crate::error::LiveSetError;

pub fn process_projects() -> Result<(), LiveSetError> {
    debug!("Starting process_projects");
    
    // Get paths from config
    let config = CONFIG.as_ref().map_err(|e| LiveSetError::ConfigError(e.clone()))?;
    debug!("Using database path from config: {}", config.database_path);
    debug!("Using project paths from config: {:?}", config.paths);
    
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

    let total_projects = found_projects.len();
    info!("Found {} projects to process", total_projects);
    for project in &found_projects {
        debug!("Found project: {}", project.display());
    }

    // Create parallel parser with number of threads based on project count
    let thread_count = (total_projects / 2).max(1).min(4);
    debug!("Creating parallel parser with {} threads", thread_count);
    let parser = ParallelParser::new(thread_count);
    
    // TODO: filter out projects that have not been modified since last scan

    // Submit all found projects for parsing
    debug!("Submitting {} projects to parser", total_projects);
    let receiver = {
        let receiver = parser.get_results_receiver();
        parser.submit_paths(found_projects.into_iter().collect())?;
        receiver
    };
    // Parser is dropped here, which will close the work channel
    
    // Initialize database and keep the connection alive
    debug!("Initializing database at {}", config.database_path);
    let mut db = LiveSetDatabase::new(PathBuf::from(&config.database_path))?;
    let mut successful_live_sets = Vec::new();
    
    // Collect results from parser with progress tracking
    debug!("Starting to collect parser results");
    let mut completed_count = 0;
    
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

fn main() {
    process_projects().unwrap();
}
