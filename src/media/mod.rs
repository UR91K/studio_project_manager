use crate::config::Config;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Sha256, Digest};
use uuid::Uuid;
use log::{debug, info};

pub mod storage;
pub mod validation;
pub mod error;

pub use error::MediaError;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    CoverArt,
    AudioFile,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::CoverArt => "cover_art",
            MediaType::AudioFile => "audio_file",
        }
    }
    
    pub fn from_str(s: &str) -> Result<Self, MediaError> {
        match s {
            "cover_art" => Ok(MediaType::CoverArt),
            "audio_file" => Ok(MediaType::AudioFile),
            _ => Err(MediaError::InvalidMediaType(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaFile {
    pub id: String,
    pub original_filename: String,
    pub file_extension: String,
    pub media_type: MediaType,
    pub file_size_bytes: u64,
    pub mime_type: String,
    pub uploaded_at: DateTime<Utc>,
    pub checksum: String,
}

impl MediaFile {
    pub fn new(
        original_filename: String,
        file_extension: String,
        media_type: MediaType,
        file_size_bytes: u64,
        mime_type: String,
        checksum: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            original_filename,
            file_extension,
            media_type,
            file_size_bytes,
            mime_type,
            uploaded_at: Utc::now(),
            checksum,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub max_cover_art_size_mb: u32,
    pub max_audio_file_size_mb: u32,
    pub allowed_image_formats: Vec<String>,
    pub allowed_audio_formats: Vec<String>,
}

impl From<&Config> for MediaConfig {
    fn from(config: &Config) -> Self {
        Self {
            max_cover_art_size_mb: config.max_cover_art_size_mb,
            max_audio_file_size_mb: config.max_audio_file_size_mb,
            allowed_image_formats: config.allowed_image_formats.clone(),
            allowed_audio_formats: config.allowed_audio_formats.clone(),
        }
    }
}

#[derive(Debug)]
pub struct MediaStorageManager {
    storage_dir: PathBuf,
    config: MediaConfig,
}

impl MediaStorageManager {
    pub fn new(storage_dir: PathBuf, config: MediaConfig) -> Result<Self, MediaError> {
        let manager = Self {
            storage_dir,
            config,
        };
        
        manager.ensure_directories_exist()?;
        Ok(manager)
    }
    
    fn ensure_directories_exist(&self) -> Result<(), MediaError> {
        let cover_art_dir = self.storage_dir.join("cover_art");
        let audio_files_dir = self.storage_dir.join("audio_files");
        
        fs::create_dir_all(&cover_art_dir)
            .map_err(|e| MediaError::IoError(format!("Failed to create cover art directory: {}", e)))?;
        
        fs::create_dir_all(&audio_files_dir)
            .map_err(|e| MediaError::IoError(format!("Failed to create audio files directory: {}", e)))?;
        
        debug!("Created media storage directories at: {}", self.storage_dir.display());
        Ok(())
    }
    
    pub fn store_file(&self, file_data: &[u8], original_filename: &str, media_type: MediaType) -> Result<MediaFile, MediaError> {
        debug!("Storing {} file: {}", media_type.as_str(), original_filename);
        
        // Extract file extension
        let file_extension = Path::new(original_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Validate file
        self.validate_file(file_data, &file_extension, &media_type)?;
        
        // Calculate checksum
        let checksum = self.calculate_checksum(file_data);
        
        // Determine MIME type
        let mime_type = self.get_mime_type(&file_extension, &media_type)?;
        
        // Create media file metadata
        let media_file = MediaFile::new(
            original_filename.to_string(),
            file_extension,
            media_type.clone(),
            file_data.len() as u64,
            mime_type,
            checksum,
        );
        
        // Store physical file
        let storage_path = self.get_storage_path(&media_file.id, &media_file.file_extension, &media_type)?;
        fs::write(&storage_path, file_data)
            .map_err(|e| MediaError::IoError(format!("Failed to write file: {}", e)))?;
        
        info!("Successfully stored media file: {} -> {}", original_filename, storage_path.display());
        Ok(media_file)
    }
    
    pub fn get_file_path(&self, file_id: &str, file_extension: &str, media_type: &MediaType) -> Result<PathBuf, MediaError> {
        self.get_storage_path(file_id, file_extension, media_type)
    }
    
    pub fn delete_file(&self, file_id: &str, file_extension: &str, media_type: &MediaType) -> Result<(), MediaError> {
        let file_path = self.get_storage_path(file_id, file_extension, media_type)?;
        
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| MediaError::IoError(format!("Failed to delete file: {}", e)))?;
            info!("Deleted media file: {}", file_path.display());
        }
        
        Ok(())
    }
    
    fn validate_file(&self, file_data: &[u8], file_extension: &str, media_type: &MediaType) -> Result<(), MediaError> {
        // Check file size
        let file_size_mb = file_data.len() as f64 / (1024.0 * 1024.0);
        let max_size = match media_type {
            MediaType::CoverArt => self.config.max_cover_art_size_mb as f64,
            MediaType::AudioFile => self.config.max_audio_file_size_mb as f64,
        };
        
        if file_size_mb > max_size {
            return Err(MediaError::FileTooLarge {
                actual_size_mb: file_size_mb,
                max_size_mb: max_size,
            });
        }
        
        // Check file extension
        let allowed_formats = match media_type {
            MediaType::CoverArt => &self.config.allowed_image_formats,
            MediaType::AudioFile => &self.config.allowed_audio_formats,
        };
        
        if !allowed_formats.contains(&file_extension.to_string()) {
            return Err(MediaError::UnsupportedFormat {
                format: file_extension.to_string(),
                allowed_formats: allowed_formats.clone(),
            });
        }
        
        // TODO: Add magic number validation for file type verification
        
        Ok(())
    }
    
    fn calculate_checksum(&self, file_data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_data);
        format!("{:x}", hasher.finalize())
    }
    
    fn get_mime_type(&self, file_extension: &str, media_type: &MediaType) -> Result<String, MediaError> {
        match media_type {
            MediaType::CoverArt => match file_extension {
                "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
                "png" => Ok("image/png".to_string()),
                "webp" => Ok("image/webp".to_string()),
                _ => Err(MediaError::UnsupportedFormat {
                    format: file_extension.to_string(),
                    allowed_formats: self.config.allowed_image_formats.clone(),
                }),
            },
            MediaType::AudioFile => match file_extension {
                "mp3" => Ok("audio/mpeg".to_string()),
                "wav" => Ok("audio/wav".to_string()),
                "m4a" => Ok("audio/mp4".to_string()),
                "flac" => Ok("audio/flac".to_string()),
                _ => Err(MediaError::UnsupportedFormat {
                    format: file_extension.to_string(),
                    allowed_formats: self.config.allowed_audio_formats.clone(),
                }),
            },
        }
    }
    
    fn get_storage_path(&self, file_id: &str, file_extension: &str, media_type: &MediaType) -> Result<PathBuf, MediaError> {
        let subdirectory = match media_type {
            MediaType::CoverArt => "cover_art",
            MediaType::AudioFile => "audio_files",
        };
        
        let filename = format!("{}.{}", file_id, file_extension);
        Ok(self.storage_dir.join(subdirectory).join(filename))
    }
}

#[derive(Debug)]
pub struct CleanupStats {
    pub files_deleted: u32,
    pub bytes_freed: u64,
}

impl CleanupStats {
    pub fn new() -> Self {
        Self {
            files_deleted: 0,
            bytes_freed: 0,
        }
    }
    
    pub fn add_file(&mut self, file_size: u64) {
        self.files_deleted += 1;
        self.bytes_freed += file_size;
    }
} 