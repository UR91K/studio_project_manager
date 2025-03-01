use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use crate::live_set::LiveSet;

// Main application state
#[derive(Debug, Clone)]
pub enum AppState {
    Loading,
    Ready {
        projects: Vec<LiveSet>,
        search_results: Vec<LiveSet>,
        selected_project_id: Option<Uuid>,
    },
    Error(String),
}

impl Default for AppState {
    fn default() -> Self {
        Self::Loading
    }
}

// UI state for tracking UI-specific state
#[derive(Debug, Clone)]
pub struct UiState {
    pub search_query: String,
    pub status: StatusInfo,
}

// Status information for the status bar
#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub message: String,
    pub progress: Option<f32>, // 0.0 to 1.0 for progress bar
}

impl Default for StatusInfo {
    fn default() -> Self {
        Self {
            message: "Ready".to_string(),
            progress: None,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            status: StatusInfo::default(),
        }
    }
}

// Simple Collection struct for UI representation
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl Collection {
    pub fn new(id: String, name: String, description: Option<String>) -> Self {
        Self {
            id,
            name,
            description,
        }
    }
} 