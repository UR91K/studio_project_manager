// Storage operations for media files
// This module can be extended for advanced features like:
// - Thumbnail generation for images
// - Audio metadata extraction
// - File compression/optimization
// - Batch operations

use super::MediaError;
use std::path::Path;
use std::fs;

pub struct StorageOperations;

impl StorageOperations {
    /// Verify file integrity by checking its checksum
    pub fn verify_file_integrity(file_path: &Path, expected_checksum: &str) -> Result<bool, MediaError> {
        if !file_path.exists() {
            return Err(MediaError::FileNotFound(file_path.to_string_lossy().to_string()));
        }
        
        let file_data = fs::read(file_path)?;
        let actual_checksum = calculate_checksum(&file_data);
        
        Ok(actual_checksum == expected_checksum)
    }
    
    /// Get file size in bytes
    pub fn get_file_size(file_path: &Path) -> Result<u64, MediaError> {
        let metadata = fs::metadata(file_path)?;
        Ok(metadata.len())
    }
    
    /// Check if file exists
    pub fn file_exists(file_path: &Path) -> bool {
        file_path.exists()
    }
}

fn calculate_checksum(file_data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(file_data);
    format!("{:x}", hasher.finalize())
}

// TODO: Future enhancements can include:
// - Image thumbnail generation
// - File format conversion
// - Metadata extraction