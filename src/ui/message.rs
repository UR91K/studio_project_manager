use uuid::Uuid;
use crate::error::{DatabaseError, LiveSetError};
use crate::live_set::LiveSet;
use crate::database::LiveSetDatabase;

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
} 