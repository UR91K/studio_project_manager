use std::path::{Path, PathBuf};
use regex::Regex;
use walkdir::WalkDir;
use std::collections::HashSet;
use crate::error::{LiveSetError, PatternError};

/// Scanner for finding Ableton Live project files in directories
pub struct ProjectPathScanner {
    /// Regex pattern for identifying backup files
    backup_pattern: Regex,
}

impl ProjectPathScanner {
    pub fn new() -> Result<Self, LiveSetError> {
        // This pattern matches Ableton's backup format: [YYYY-MM-DD HHMMSS]
        let backup_pattern = Regex::new(r"\[\d{4}-\d{2}-\d{2}\s\d{6}]")
            .map_err(PatternError::InvalidRegex)?;
            
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        File::create(&path).unwrap();
        path
    }

    #[test]
    fn test_basic_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test files
        create_test_file(temp_dir.path(), "project1.als");
        create_test_file(temp_dir.path(), "project2.als");
        create_test_file(temp_dir.path(), "not_a_project.txt");
        
        let scanner = ProjectPathScanner::new().unwrap();
        let paths = scanner.scan_directory(temp_dir.path()).unwrap();
        
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().all(|p| p.extension().unwrap() == "als"));
    }

    #[test]
    fn test_backup_file_exclusion() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test files
        create_test_file(temp_dir.path(), "project.als");
        create_test_file(temp_dir.path(), "project [2023-10-15 123456].als");
        create_test_file(temp_dir.path(), "another [2023-11-20 154321].als");
        
        let scanner = ProjectPathScanner::new().unwrap();
        let paths = scanner.scan_directory(temp_dir.path()).unwrap();
        
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].file_name().unwrap(), "project.als");
    }

    #[test]
    fn test_nested_directory_scanning() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create nested directory structure
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        
        create_test_file(temp_dir.path(), "root.als");
        create_test_file(&sub_dir, "nested.als");
        create_test_file(&sub_dir, "nested [2023-10-15 123456].als");
        
        let scanner = ProjectPathScanner::new().unwrap();
        let paths = scanner.scan_directory(temp_dir.path()).unwrap();
        
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p.file_name().unwrap() == "root.als"));
        assert!(paths.iter().any(|p| p.file_name().unwrap() == "nested.als"));
    }

    #[test]
    fn test_multiple_directory_scanning() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        
        create_test_file(temp_dir1.path(), "project1.als");
        create_test_file(temp_dir2.path(), "project2.als");
        create_test_file(temp_dir2.path(), "project2 [2023-10-15 123456].als");
        
        let scanner = ProjectPathScanner::new().unwrap();
        let paths = scanner.scan_directories(&[
            temp_dir1.path().to_path_buf(),
            temp_dir2.path().to_path_buf(),
        ]).unwrap();
        
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p.file_name().unwrap() == "project1.als"));
        assert!(paths.iter().any(|p| p.file_name().unwrap() == "project2.als"));
    }
} 