use tauri::State;
use log::{info, error};
use crate::commands::state::AppState;
use crate::process_projects;
use std::sync::Arc;

#[tauri::command]
pub async fn start_scan(state: State<'_, AppState>) -> Result<(), String> {
    info!("Received scan request");
    
    // Check if already scanning
    let mut is_scanning = state.is_scanning.lock().map_err(|e| e.to_string())?;
    if *is_scanning {
        info!("Scan already in progress, ignoring request");
        return Err("Scan already in progress".to_string());
    }
    
    // Set scanning flag
    *is_scanning = true;
    info!("Starting new scan");
    
    // Clone Arc for the async task
    let is_scanning_clone = Arc::clone(&state.is_scanning);
    
    // Run process_projects in a separate thread
    tokio::spawn(async move {
        info!("Starting process_projects in background");
        let scan_result = process_projects();
        
        // Always reset scanning flag, even if there was an error
        if let Ok(mut is_scanning) = is_scanning_clone.lock().map_err(|e| {
            error!("Failed to lock scanning state: {}", e);
            e.to_string()
        }) {
            *is_scanning = false;
        } else {
            error!("Critical error: Failed to reset scanning state");
        }
        
        // Log the scan result
        match scan_result {
            Ok(_) => info!("Scan completed successfully"),
            Err(e) => error!("Scan failed: {}", e),
        }
        
        info!("Scan state reset");
    });
    
    info!("Scan initiated");
    Ok(())
} 