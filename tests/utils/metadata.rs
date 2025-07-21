use std::path::PathBuf;

use studio_project_manager::utils::metadata::load_file_name;

#[test]
fn test_load_file_name() {
    // Test file with extension
    let path = PathBuf::from("C:/Users/jake/Desktop/test_file.txt");
    assert_eq!(load_file_name(&path).unwrap(), "test_file.txt");

    // Test file without extension
    let path = PathBuf::from("C:/Users/jake/Desktop/test_file");
    assert_eq!(load_file_name(&path).unwrap(), "test_file");

    // Test file with multiple extensions
    let path = PathBuf::from("C:/Users/jake/Desktop/test_file.tar.gz");
    assert_eq!(load_file_name(&path).unwrap(), "test_file.tar.gz");

    // Test file with dots in name
    let path = PathBuf::from("C:/Users/jake/Desktop/test.file.name.txt");
    assert_eq!(load_file_name(&path).unwrap(), "test.file.name.txt");
}
