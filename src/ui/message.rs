use uuid::Uuid;
use crate::live_set::LiveSet;

#[derive(Debug, Clone)]
pub enum Message {
    // Initialization
    Initialize,
    DatabaseLoaded(Result<(), String>),
    ProjectsLoaded(Result<Vec<LiveSet>, String>),
    
    // Navigation
    ViewAllProjects,
    
    // Project list
    ProjectSelected(Option<Uuid>),
    
    // Search
    SearchQueryChanged(String),
    SearchPerformed(Result<Vec<LiveSet>, String>),
    
    // Scanning
    ScanFoldersClicked,
    ScanCompleted(Result<(), String>),
    ScanProgress(f32), // Progress value between 0.0 and 1.0
    
    // Status
    UpdateStatus(String),
    UpdateStatusWithProgress(String, f32),
    ClearStatus,
} 