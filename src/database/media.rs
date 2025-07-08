use super::core::LiveSetDatabase;
use super::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::media::{MediaFile, MediaType};
use rusqlite::{params, OptionalExtension, Row};
use chrono::DateTime;
use log::{debug, info, warn};

impl LiveSetDatabase {
    /// Insert a new media file record into the database
    pub fn insert_media_file(&mut self, media_file: &MediaFile) -> Result<(), DatabaseError> {
        debug!("Inserting media file: {} ({})", media_file.original_filename, media_file.id);
        
        self.conn.execute(
            "INSERT INTO media_files (
                id, original_filename, file_extension, media_type, file_size_bytes,
                mime_type, uploaded_at, checksum
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                media_file.id,
                media_file.original_filename,
                media_file.file_extension,
                media_file.media_type.as_str(),
                media_file.file_size_bytes as i64,
                media_file.mime_type,
                SqlDateTime::from_utc(media_file.uploaded_at),
                media_file.checksum,
            ],
        )?;
        
        info!("Successfully inserted media file: {}", media_file.id);
        Ok(())
    }
    
    /// Retrieve a media file by its ID
    pub fn get_media_file(&self, file_id: &str) -> Result<Option<MediaFile>, DatabaseError> {
        debug!("Retrieving media file: {}", file_id);
        
        let media_file = self.conn.query_row(
            "SELECT id, original_filename, file_extension, media_type, file_size_bytes,
                    mime_type, uploaded_at, checksum
             FROM media_files 
             WHERE id = ?",
            params![file_id],
            |row| self.row_to_media_file(row),
        ).optional()?;
        
        if let Some(ref file) = media_file {
            debug!("Found media file: {} ({})", file.original_filename, file.id);
        } else {
            debug!("Media file not found: {}", file_id);
        }
        
        Ok(media_file)
    }
    
    /// Delete a media file from the database
    pub fn delete_media_file(&mut self, file_id: &str) -> Result<(), DatabaseError> {
        debug!("Deleting media file: {}", file_id);
        
        let rows_affected = self.conn.execute(
            "DELETE FROM media_files WHERE id = ?",
            params![file_id],
        )?;
        
        if rows_affected > 0 {
            info!("Successfully deleted media file: {}", file_id);
        } else {
            warn!("No media file found to delete: {}", file_id);
        }
        
        Ok(())
    }
    
    /// Update collection cover art
    pub fn update_collection_cover_art(&mut self, collection_id: &str, cover_art_id: Option<&str>) -> Result<(), DatabaseError> {
        debug!("Updating collection {} cover art to: {:?}", collection_id, cover_art_id);
        
        let rows_affected = self.conn.execute(
            "UPDATE collections SET cover_art_id = ? WHERE id = ?",
            params![cover_art_id, collection_id],
        )?;
        
        if rows_affected > 0 {
            info!("Successfully updated collection cover art: {}", collection_id);
        } else {
            warn!("Collection not found: {}", collection_id);
            return Err(DatabaseError::NotFound(format!("Collection with ID {} not found", collection_id)));
        }
        
        Ok(())
    }
    
    /// Update project audio file
    pub fn update_project_audio_file(&mut self, project_id: &str, audio_file_id: Option<&str>) -> Result<(), DatabaseError> {
        debug!("Updating project {} audio file to: {:?}", project_id, audio_file_id);
        
        let rows_affected = self.conn.execute(
            "UPDATE projects SET audio_file_id = ? WHERE id = ?",
            params![audio_file_id, project_id],
        )?;
        
        if rows_affected > 0 {
            info!("Successfully updated project audio file: {}", project_id);
        } else {
            warn!("Project not found: {}", project_id);
            return Err(DatabaseError::NotFound(format!("Project with ID {} not found", project_id)));
        }
        
        Ok(())
    }
    
    /// Get all orphaned media files (files not referenced by any collection or project)
    pub fn get_orphaned_media_files(&self) -> Result<Vec<MediaFile>, DatabaseError> {
        debug!("Finding orphaned media files");
        
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.original_filename, m.file_extension, m.media_type, m.file_size_bytes,
                    m.mime_type, m.uploaded_at, m.checksum
             FROM media_files m
             LEFT JOIN collections c ON c.cover_art_id = m.id
             LEFT JOIN projects p ON p.audio_file_id = m.id
             WHERE c.id IS NULL AND p.id IS NULL"
        )?;
        
        let orphaned_files = stmt.query_map([], |row| {
            self.row_to_media_file(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        info!("Found {} orphaned media files", orphaned_files.len());
        Ok(orphaned_files)
    }
    
    /// Get all media files of a specific type
    pub fn get_media_files_by_type(&self, media_type: &MediaType) -> Result<Vec<MediaFile>, DatabaseError> {
        debug!("Getting media files of type: {}", media_type.as_str());
        
        let mut stmt = self.conn.prepare(
            "SELECT id, original_filename, file_extension, media_type, file_size_bytes,
                    mime_type, uploaded_at, checksum
             FROM media_files 
             WHERE media_type = ?
             ORDER BY uploaded_at DESC"
        )?;
        
        let media_files = stmt.query_map([media_type.as_str()], |row| {
            self.row_to_media_file(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        debug!("Found {} media files of type {}", media_files.len(), media_type.as_str());
        Ok(media_files)
    }
    
    /// Get the cover art for a collection
    pub fn get_collection_cover_art(&self, collection_id: &str) -> Result<Option<MediaFile>, DatabaseError> {
        debug!("Getting cover art for collection: {}", collection_id);
        
        let media_file = self.conn.query_row(
            "SELECT m.id, m.original_filename, m.file_extension, m.media_type, m.file_size_bytes,
                    m.mime_type, m.uploaded_at, m.checksum
             FROM media_files m
             JOIN collections c ON c.cover_art_id = m.id
             WHERE c.id = ?",
            params![collection_id],
            |row| self.row_to_media_file(row),
        ).optional()?;
        
        if let Some(ref file) = media_file {
            debug!("Found cover art for collection {}: {}", collection_id, file.original_filename);
        } else {
            debug!("No cover art found for collection: {}", collection_id);
        }
        
        Ok(media_file)
    }
    
    /// Get the audio file for a project
    pub fn get_project_audio_file(&self, project_id: &str) -> Result<Option<MediaFile>, DatabaseError> {
        debug!("Getting audio file for project: {}", project_id);
        
        let media_file = self.conn.query_row(
            "SELECT m.id, m.original_filename, m.file_extension, m.media_type, m.file_size_bytes,
                    m.mime_type, m.uploaded_at, m.checksum
             FROM media_files m
             JOIN projects p ON p.audio_file_id = m.id
             WHERE p.id = ?",
            params![project_id],
            |row| self.row_to_media_file(row),
        ).optional()?;
        
        if let Some(ref file) = media_file {
            debug!("Found audio file for project {}: {}", project_id, file.original_filename);
        } else {
            debug!("No audio file found for project: {}", project_id);
        }
        
        Ok(media_file)
    }
    
    /// Get media file usage statistics
    pub fn get_media_file_stats(&self) -> Result<MediaFileStats, DatabaseError> {
        debug!("Getting media file statistics");
        
        let mut stmt = self.conn.prepare(
            "SELECT 
                media_type,
                COUNT(*) as count,
                SUM(file_size_bytes) as total_size
             FROM media_files 
             GROUP BY media_type"
        )?;
        
        let mut cover_art_count = 0;
        let mut cover_art_size = 0i64;
        let mut audio_file_count = 0;
        let mut audio_file_size = 0i64;
        
        let rows = stmt.query_map([], |row| {
            let media_type: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let total_size: i64 = row.get(2)?;
            Ok((media_type, count, total_size))
        })?;
        
        for row in rows {
            let (media_type, count, total_size) = row?;
            match media_type.as_str() {
                "cover_art" => {
                    cover_art_count = count;
                    cover_art_size = total_size;
                }
                "audio_file" => {
                    audio_file_count = count;
                    audio_file_size = total_size;
                }
                _ => {}
            }
        }
        
        let stats = MediaFileStats {
            cover_art_count: cover_art_count as u32,
            cover_art_total_size_bytes: cover_art_size as u64,
            audio_file_count: audio_file_count as u32,
            audio_file_total_size_bytes: audio_file_size as u64,
        };
        
        debug!("Media file stats: {:?}", stats);
        Ok(stats)
    }
    
    /// Convert a database row to a MediaFile
    fn row_to_media_file(&self, row: &Row) -> Result<MediaFile, rusqlite::Error> {
        let media_type_str: String = row.get("media_type")?;
        let media_type = MediaType::from_str(&media_type_str)
            .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid media type".into()))?;
        
        let uploaded_at_timestamp: i64 = row.get("uploaded_at")?;
        let uploaded_at = DateTime::from_timestamp(uploaded_at_timestamp, 0)
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?;
        
        Ok(MediaFile {
            id: row.get("id")?,
            original_filename: row.get("original_filename")?,
            file_extension: row.get("file_extension")?,
            media_type,
            file_size_bytes: row.get::<_, i64>("file_size_bytes")? as u64,
            mime_type: row.get("mime_type")?,
            uploaded_at,
            checksum: row.get("checksum")?,
        })
    }
}

/// Statistics about media files in the database
#[derive(Debug, Clone)]
pub struct MediaFileStats {
    pub cover_art_count: u32,
    pub cover_art_total_size_bytes: u64,
    pub audio_file_count: u32,
    pub audio_file_total_size_bytes: u64,
}

impl MediaFileStats {
    pub fn total_files(&self) -> u32 {
        self.cover_art_count + self.audio_file_count
    }
    
    pub fn total_size_bytes(&self) -> u64 {
        self.cover_art_total_size_bytes + self.audio_file_total_size_bytes
    }
    
    pub fn total_size_mb(&self) -> f64 {
        self.total_size_bytes() as f64 / (1024.0 * 1024.0)
    }
} 