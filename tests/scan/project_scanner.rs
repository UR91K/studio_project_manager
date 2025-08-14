//! Project scanner tests

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use seula::scan::project_scanner::ProjectPathScanner;
use tempfile::TempDir;

use crate::common::setup;

fn create_test_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    File::create(&path).unwrap();
    path
}

#[test]
fn test_basic_file_detection() {
    setup("error");
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
    setup("error");
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
    setup("error");
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
    setup("error");
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();

    create_test_file(temp_dir1.path(), "project1.als");
    create_test_file(temp_dir2.path(), "project2.als");
    create_test_file(temp_dir2.path(), "project2 [2023-10-15 123456].als");

    let parser = ProjectPathScanner::new().unwrap();
    let paths = parser
        .scan_directories(&[
            temp_dir1.path().to_path_buf(),
            temp_dir2.path().to_path_buf(),
        ])
        .unwrap();

    assert_eq!(paths.len(), 2);
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == "project1.als"));
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == "project2.als"));
}
