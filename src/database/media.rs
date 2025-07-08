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
    
    /// List all media files with optional pagination
    pub fn list_media_files(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<MediaFile>, DatabaseError> {
        let mut query = "SELECT id, original_filename, file_extension, media_type, file_size_bytes, mime_type, uploaded_at, checksum FROM media_files ORDER BY uploaded_at DESC".to_string();
        
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }
        
        let mut stmt = self.conn.prepare(&query)?;
        let media_files = stmt.query_map([], |row| {
            self.row_to_media_file(row)
        })?;
        
        let mut result = Vec::new();
        for media_file in media_files {
            result.push(media_file?);
        }
        
        Ok(result)
    }
    
    /// Get media files by type with optional pagination
    pub fn get_media_files_by_type(&self, media_type: &str, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<MediaFile>, DatabaseError> {
        let mut query = "SELECT id, original_filename, file_extension, media_type, file_size_bytes, mime_type, uploaded_at, checksum FROM media_files WHERE media_type = ? ORDER BY uploaded_at DESC".to_string();
        
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }
        
        let mut stmt = self.conn.prepare(&query)?;
        let media_files = stmt.query_map([media_type], |row| {
            self.row_to_media_file(row)
        })?;
        
        let mut result = Vec::new();
        for media_file in media_files {
            result.push(media_file?);
        }
        
        Ok(result)
    }
    
    /// Get orphaned media files (files not referenced by any project or collection)
    pub fn get_orphaned_media_files(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<MediaFile>, DatabaseError> {
        let mut query = r#"
            SELECT id, original_filename, file_extension, media_type, file_size_bytes, mime_type, uploaded_at, checksum
            FROM media_files 
            WHERE id NOT IN (
                SELECT DISTINCT audio_file_id FROM projects WHERE audio_file_id IS NOT NULL
                UNION
                SELECT DISTINCT cover_art_id FROM collections WHERE cover_art_id IS NOT NULL
            )
            ORDER BY uploaded_at DESC
        "#.to_string();
        
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }
        
        let mut stmt = self.conn.prepare(&query)?;
        let media_files = stmt.query_map([], |row| {
            self.row_to_media_file(row)
        })?;
        
        let mut result = Vec::new();
        for media_file in media_files {
            result.push(media_file?);
        }
        
        Ok(result)
    }
    
    /// Get media statistics
    pub fn get_media_statistics(&self) -> Result<(i32, i64, i32, i32, i32, i64), DatabaseError> {
        // Get total files and size
        let mut stmt = self.conn.prepare("SELECT COUNT(*), COALESCE(SUM(file_size_bytes), 0) FROM media_files")?;
        let (total_files, total_size): (i32, i64) = stmt.query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        
        // Get cover art count
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM media_files WHERE media_type = 'cover_art'")?;
        let cover_art_count: i32 = stmt.query_row([], |row| row.get(0))?;
        
        // Get audio file count
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM media_files WHERE media_type = 'audio_file'")?;
        let audio_file_count: i32 = stmt.query_row([], |row| row.get(0))?;
        
        // Get orphaned files count and size
        let mut stmt = self.conn.prepare(r#"
            SELECT COUNT(*), COALESCE(SUM(file_size_bytes), 0)
            FROM media_files 
            WHERE id NOT IN (
                SELECT DISTINCT audio_file_id FROM projects WHERE audio_file_id IS NOT NULL
                UNION
                SELECT DISTINCT cover_art_id FROM collections WHERE cover_art_id IS NOT NULL
            )
        "#)?;
        let (orphaned_count, orphaned_size): (i32, i64) = stmt.query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        
        Ok((total_files, total_size, cover_art_count, audio_file_count, orphaned_count, orphaned_size))
    }
    
    /// Get count of media files (for pagination)
    pub fn get_media_files_count(&self) -> Result<i32, DatabaseError> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM media_files")?;
        let count: i32 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }
    
    /// Get count of media files by type (for pagination)
    pub fn get_media_files_count_by_type(&self, media_type: &str) -> Result<i32, DatabaseError> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM media_files WHERE media_type = ?")?;
        let count: i32 = stmt.query_row([media_type], |row| row.get(0))?;
        Ok(count)
    }
    
    /// Get count of orphaned media files (for pagination)
    pub fn get_orphaned_media_files_count(&self) -> Result<i32, DatabaseError> {
        let mut stmt = self.conn.prepare(r#"
            SELECT COUNT(*) 
            FROM media_files 
            WHERE id NOT IN (
                SELECT DISTINCT audio_file_id FROM projects WHERE audio_file_id IS NOT NULL
                UNION
                SELECT DISTINCT cover_art_id FROM collections WHERE cover_art_id IS NOT NULL
            )
        "#)?;
        let count: i32 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
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
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MediaFileStats {
    pub cover_art_count: u32,
    pub cover_art_total_size_bytes: u64,
    pub audio_file_count: u32,
    pub audio_file_total_size_bytes: u64,
}

#[allow(dead_code)]
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