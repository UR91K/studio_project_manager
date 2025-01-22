// /src/utils/metadata.rs

use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use crc32fast::Hasher;

use crate::error::FileError;

pub(crate) fn load_file_timestamps(
    file_path: &PathBuf,
) -> Result<(DateTime<Local>, DateTime<Local>), FileError> {
    let metadata = fs::metadata(file_path).map_err(|e| FileError::MetadataError {
        path: file_path.clone(),
        source: e,
    })?;

    let modified_time = metadata
        .modified()
        .map(DateTime::<Local>::from)
        .map_err(|e| FileError::MetadataError {
            path: file_path.clone(),
            source: e,
        })?;

    let created_time = metadata
        .created()
        .map(DateTime::<Local>::from)
        .unwrap_or_else(|_| Local::now());

    Ok((modified_time, created_time))
}

pub(crate) fn load_file_hash(file_path: &PathBuf) -> Result<String, FileError> {
    let mut file = File::open(file_path).map_err(|e| FileError::HashingError {
        path: file_path.clone(),
        source: e,
    })?;

    let mut hasher = Hasher::new();
    let mut buffer = [0; 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| FileError::HashingError {
                path: file_path.clone(),
                source: e,
            })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    let hash_string = format!("{:08x}", hash);

    Ok(hash_string)
}

pub(crate) fn load_file_name(file_path: &PathBuf) -> Result<String, FileError> {
    if file_path.is_dir() {
        return Err(FileError::NameError("Path is a directory".to_string()));
    }
    
    file_path
        .file_name()
        .ok_or_else(|| FileError::NameError("File name is not present".to_string()))?
        .to_str()
        .ok_or_else(|| FileError::NameError("File name is not valid UTF-8".to_string()))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
