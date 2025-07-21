use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use regex::Regex;
use walkdir::WalkDir;

use crate::error::{LiveSetError, PatternError};

/// Scanner for finding Ableton Live project files in directories
pub struct ProjectPathScanner {
    /// Regex pattern for identifying backup files
    backup_pattern: Regex,
}

impl ProjectPathScanner {
    pub fn new() -> Result<Self, LiveSetError> {
        // This pattern matches Ableton's backup format: [YYYY-MM-DD HHMMSS]
        let backup_pattern =
            Regex::new(r"\[\d{4}-\d{2}-\d{2}\s\d{6}]").map_err(PatternError::InvalidRegex)?;

        Ok(Self { backup_pattern })
    }

    /// Scan a directory for Ableton Live project files
    pub fn scan_directory(&self, dir: &Path) -> Result<Vec<PathBuf>, LiveSetError> {
        let mut project_paths = HashSet::new();

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Check if it's an .als file
            if let Some(ext) = path.extension() {
                if ext == "als" {
                    let path_str = path.to_string_lossy();

                    // Skip if it's a backup file
                    if !self.backup_pattern.is_match(&path_str) {
                        project_paths.insert(path.to_path_buf());
                    }
                }
            }
        }

        Ok(project_paths.into_iter().collect())
    }

    /// Scan multiple directories for Ableton Live project files
    pub fn scan_directories(&self, dirs: &[PathBuf]) -> Result<Vec<PathBuf>, LiveSetError> {
        let mut all_paths = HashSet::new();

        for dir in dirs {
            let paths = self.scan_directory(dir)?;
            all_paths.extend(paths);
        }

        Ok(all_paths.into_iter().collect())
    }
}
