use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::database::LiveSetDatabase;
use super::super::media::*;
use super::super::collections::*;
use super::super::common::*;
use crate::media::{MediaStorageManager, MediaType};

#[derive(Clone)]
pub struct MediaHandler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
    pub media_storage: Arc<MediaStorageManager>,
}

impl MediaHandler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>, media_storage: Arc<MediaStorageManager>) -> Self {
        Self { db, media_storage }
    }
    // Media Management - Streaming implementations
    pub async fn upload_cover_art(
        &self,
        request: Request<tonic::Streaming<UploadCoverArtRequest>>,
    ) -> Result<Response<UploadCoverArtResponse>, Status> {
        debug!("UploadCoverArt streaming request received");

        let mut stream = request.into_inner();
        let mut collection_id: Option<String> = None;
        let mut filename: Option<String> = None;
        let mut data_chunks: Vec<u8> = Vec::new();

        // Process the streaming request
        while let Some(chunk_result) = stream.message().await? {
            let chunk = chunk_result;

            if let Some(data) = chunk.data {
                match data {
                    upload_cover_art_request::Data::CollectionId(id) => {
                        collection_id = Some(id);
                    }
                    upload_cover_art_request::Data::Filename(name) => {
                        filename = Some(name);
                    }
                    upload_cover_art_request::Data::Chunk(bytes) => {
                        data_chunks.extend(bytes);
                    }
                }
            }
        }

        // Validate we have all required data
        let collection_id =
            collection_id.ok_or_else(|| Status::invalid_argument("Collection ID is required"))?;

        // we dont seem to actually need a file name here, but ill leave it for now
        let filename = filename.ok_or_else(|| Status::invalid_argument("Filename is required"))?;

        if data_chunks.is_empty() {
            return Err(Status::invalid_argument("No file data received"));
        }

        // Store the file using MediaStorageManager
        let media_file =
            match self
                .media_storage
                .store_file(&data_chunks, &filename, MediaType::CoverArt)
            {
                Ok(file) => file,
                Err(e) => {
                    error!("Failed to store cover art file: {:?}", e);
                    return Ok(Response::new(UploadCoverArtResponse {
                        media_file_id: String::new(),
                        success: false,
                        error_message: Some(format!("Failed to store file: {}", e)),
                    }));
                }
            };

        // Store the media file metadata in the database
        let mut db = self.db.lock().await;
        if let Err(e) = db.insert_media_file(&media_file) {
            error!("Failed to insert media file into database: {:?}", e);
            // Clean up the stored file
            if let Err(cleanup_err) = self.media_storage.delete_file(
                &media_file.id,
                &media_file.file_extension,
                &media_file.media_type,
            ) {
                error!(
                    "Failed to cleanup stored file after database error: {:?}",
                    cleanup_err
                );
            }
            return Ok(Response::new(UploadCoverArtResponse {
                media_file_id: String::new(),
                success: false,
                error_message: Some(format!("Failed to store metadata: {}", e)),
            }));
        }

        // Optionally set as collection cover art if collection_id was provided
        if let Err(e) = db.update_collection_cover_art(&collection_id, Some(&media_file.id)) {
            warn!("Failed to set collection cover art: {:?}", e);
            // Don't fail the upload, just log the warning
        }

        info!(
            "Successfully uploaded cover art: {} bytes for collection {}",
            data_chunks.len(),
            collection_id
        );

        let response = UploadCoverArtResponse {
            media_file_id: media_file.id,
            success: true,
            error_message: None,
        };

        Ok(Response::new(response))
    }

    pub async fn upload_audio_file(
        &self,
        request: Request<tonic::Streaming<UploadAudioFileRequest>>,
    ) -> Result<Response<UploadAudioFileResponse>, Status> {
        debug!("UploadAudioFile streaming request received");

        let mut stream = request.into_inner();
        let mut project_id: Option<String> = None;
        let mut filename: Option<String> = None;
        let mut data_chunks: Vec<u8> = Vec::new();

        // Process the streaming request
        while let Some(chunk_result) = stream.message().await? {
            let chunk = chunk_result;

            if let Some(data) = chunk.data {
                match data {
                    upload_audio_file_request::Data::ProjectId(id) => {
                        project_id = Some(id);
                    }
                    upload_audio_file_request::Data::Filename(name) => {
                        filename = Some(name);
                    }
                    upload_audio_file_request::Data::Chunk(bytes) => {
                        data_chunks.extend(bytes);
                    }
                }
            }
        }

        // Validate we have all required data
        let project_id =
            project_id.ok_or_else(|| Status::invalid_argument("Project ID is required"))?;

        let filename = filename.ok_or_else(|| Status::invalid_argument("Filename is required"))?;

        if data_chunks.is_empty() {
            return Err(Status::invalid_argument("No file data received"));
        }

        // Store the file using MediaStorageManager
        let media_file =
            match self
                .media_storage
                .store_file(&data_chunks, &filename, MediaType::AudioFile)
            {
                Ok(file) => file,
                Err(e) => {
                    error!("Failed to store audio file: {:?}", e);
                    return Ok(Response::new(UploadAudioFileResponse {
                        media_file_id: String::new(),
                        success: false,
                        error_message: Some(format!("Failed to store file: {}", e)),
                    }));
                }
            };

        // Store the media file metadata in the database
        let mut db = self.db.lock().await;
        if let Err(e) = db.insert_media_file(&media_file) {
            error!("Failed to insert media file into database: {:?}", e);
            // Clean up the stored file
            if let Err(cleanup_err) = self.media_storage.delete_file(
                &media_file.id,
                &media_file.file_extension,
                &media_file.media_type,
            ) {
                error!(
                    "Failed to cleanup stored file after database error: {:?}",
                    cleanup_err
                );
            }
            return Ok(Response::new(UploadAudioFileResponse {
                media_file_id: String::new(),
                success: false,
                error_message: Some(format!("Failed to store metadata: {}", e)),
            }));
        }

        // Optionally set as project audio file if project_id was provided
        if let Err(e) = db.update_project_audio_file(&project_id, Some(&media_file.id)) {
            warn!("Failed to set project audio file: {:?}", e);
            // Don't fail the upload, just log the warning
        }

        info!(
            "Successfully uploaded audio file: {} bytes for project {}",
            data_chunks.len(),
            project_id
        );

        let response = UploadAudioFileResponse {
            media_file_id: media_file.id,
            success: true,
            error_message: None,
        };

        Ok(Response::new(response))
    }

    pub async fn download_media(
        &self,
        request: Request<DownloadMediaRequest>,
    ) -> Result<Response<ReceiverStream<Result<DownloadMediaResponse, Status>>>, Status> {
        debug!("DownloadMedia request: {:?}", request);

        let req = request.into_inner();
        let db = self.db.lock().await;

        // Get media file metadata
        let media_file = match db.get_media_file(&req.media_file_id) {
            Ok(Some(file)) => file,
            Ok(None) => {
                return Err(Status::not_found("Media file not found"));
            }
            Err(e) => {
                error!("Failed to get media file: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Clone values needed for later use
        let file_id = media_file.id.clone();
        let file_extension = media_file.file_extension.clone();
        let media_type = media_file.media_type.clone();

        let (tx, rx) = mpsc::channel(100);

        // Convert our MediaFile to protobuf MediaFile
        let proto_media_file = MediaFile {
            id: media_file.id,
            original_filename: media_file.original_filename,
            file_extension: media_file.file_extension,
            media_type: media_file.media_type.as_str().to_string(),
            file_size_bytes: media_file.file_size_bytes as i64,
            mime_type: media_file.mime_type,
            uploaded_at: media_file.uploaded_at.timestamp(),
            checksum: media_file.checksum,
        };

        // Send metadata first
        let metadata_response = DownloadMediaResponse {
            data: Some(download_media_response::Data::Metadata(proto_media_file)),
        };

        if tx.send(Ok(metadata_response)).await.is_err() {
            return Err(Status::internal("Failed to send metadata"));
        }

        // Get the file path and stream the actual file data
        let file_path =
            match self
                .media_storage
                .get_file_path(&file_id, &file_extension, &media_type)
            {
                Ok(path) => path,
                Err(e) => {
                    error!("Failed to get file path: {:?}", e);
                    return Err(Status::internal(format!("Failed to get file path: {}", e)));
                }
            };

        // Read and stream the file in chunks
        match tokio::fs::read(&file_path).await {
            Ok(file_data) => {
                // Stream the file in chunks (e.g., 64KB chunks)
                const CHUNK_SIZE: usize = 64 * 1024;
                for chunk in file_data.chunks(CHUNK_SIZE) {
                    let chunk_response = DownloadMediaResponse {
                        data: Some(download_media_response::Data::Chunk(chunk.to_vec())),
                    };

                    if tx.send(Ok(chunk_response)).await.is_err() {
                        return Err(Status::internal("Failed to send file chunk"));
                    }
                }
            }
            Err(e) => {
                error!("Failed to read file: {:?}", e);
                return Err(Status::internal(format!("Failed to read file: {}", e)));
            }
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    pub async fn delete_media(
        &self,
        request: Request<DeleteMediaRequest>,
    ) -> Result<Response<DeleteMediaResponse>, Status> {
        debug!("DeleteMedia request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // First check if the media file exists and get its info
        match db.get_media_file(&req.media_file_id) {
            Ok(Some(media_file)) => {
                // Clone the values we need for later use
                let file_id = media_file.id.clone();
                let file_extension = media_file.file_extension.clone();
                let media_type = media_file.media_type.clone();

                // Delete from database first
                match db.delete_media_file(&req.media_file_id) {
                    Ok(()) => {
                        // Also delete physical file from storage
                        if let Err(e) =
                            self.media_storage
                                .delete_file(&file_id, &file_extension, &media_type)
                        {
                            warn!("Failed to delete physical file from storage: {:?}", e);
                            // Don't fail the operation if physical file deletion fails
                        }

                        info!("Successfully deleted media file: {}", req.media_file_id);
                        let response = DeleteMediaResponse {
                            success: true,
                            error_message: None,
                        };
                        Ok(Response::new(response))
                    }
                    Err(e) => {
                        error!("Failed to delete media file from database: {:?}", e);
                        let response = DeleteMediaResponse {
                            success: false,
                            error_message: Some(format!("Database error: {}", e)),
                        };
                        Ok(Response::new(response))
                    }
                }
            }
            Ok(None) => {
                let response = DeleteMediaResponse {
                    success: false,
                    error_message: Some("Media file not found".to_string()),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to check media file existence: {:?}", e);
                let response = DeleteMediaResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn set_collection_cover_art(
        &self,
        request: Request<SetCollectionCoverArtRequest>,
    ) -> Result<Response<SetCollectionCoverArtResponse>, Status> {
        debug!("SetCollectionCoverArt request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_collection_cover_art(&req.collection_id, Some(&req.media_file_id)) {
            Ok(()) => {
                let response = SetCollectionCoverArtResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to set collection cover art: {:?}", e);
                let response = SetCollectionCoverArtResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn remove_collection_cover_art(
        &self,
        request: Request<RemoveCollectionCoverArtRequest>,
    ) -> Result<Response<RemoveCollectionCoverArtResponse>, Status> {
        debug!("RemoveCollectionCoverArt request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_collection_cover_art(&req.collection_id, None) {
            Ok(()) => {
                let response = RemoveCollectionCoverArtResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove collection cover art: {:?}", e);
                let response = RemoveCollectionCoverArtResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn set_project_audio_file(
        &self,
        request: Request<SetProjectAudioFileRequest>,
    ) -> Result<Response<SetProjectAudioFileResponse>, Status> {
        debug!("SetProjectAudioFile request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_project_audio_file(&req.project_id, Some(&req.media_file_id)) {
            Ok(()) => {
                let response = SetProjectAudioFileResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to set project audio file: {:?}", e);
                let response = SetProjectAudioFileResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }

    pub async fn remove_project_audio_file(
        &self,
        request: Request<RemoveProjectAudioFileRequest>,
    ) -> Result<Response<RemoveProjectAudioFileResponse>, Status> {
        debug!("RemoveProjectAudioFile request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.update_project_audio_file(&req.project_id, None) {
            Ok(()) => {
                let response = RemoveProjectAudioFileResponse {
                    success: true,
                    error_message: None,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to remove project audio file: {:?}", e);
                let response = RemoveProjectAudioFileResponse {
                    success: false,
                    error_message: Some(format!("Database error: {}", e)),
                };
                Ok(Response::new(response))
            }
        }
    }

    /// List all media files with optional pagination
    pub async fn list_media_files(
        &self,
        request: Request<ListMediaFilesRequest>,
    ) -> Result<Response<ListMediaFilesResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;

        // Get media files
        let media_files = match db.list_media_files(req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to list media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Get total count
        let total_count = match db.get_media_files_count() {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get media files count: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Convert to proto format
        let proto_files = media_files
            .into_iter()
            .map(|file| MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            })
            .collect();

        Ok(Response::new(ListMediaFilesResponse {
            media_files: proto_files,
            total_count,
        }))
    }

    /// Get media files by type
    pub async fn get_media_files_by_type(
        &self,
        request: Request<GetMediaFilesByTypeRequest>,
    ) -> Result<Response<GetMediaFilesByTypeResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;

        // Get media files by type
        let media_files = match db.get_media_files_by_type(&req.media_type, req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get media files by type: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Get total count for this type
        let total_count = match db.get_media_files_count_by_type(&req.media_type) {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get media files count by type: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Convert to proto format
        let proto_files = media_files
            .into_iter()
            .map(|file| MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            })
            .collect();

        Ok(Response::new(GetMediaFilesByTypeResponse {
            media_files: proto_files,
            total_count,
        }))
    }

    /// Get orphaned media files
    pub async fn get_orphaned_media_files(
        &self,
        request: Request<GetOrphanedMediaFilesRequest>,
    ) -> Result<Response<GetOrphanedMediaFilesResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().await;

        // Get orphaned media files
        let orphaned_files = match db.get_orphaned_media_files(req.limit, req.offset) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get orphaned media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Get total count of orphaned files
        let total_count = match db.get_orphaned_media_files_count() {
            Ok(count) => count,
            Err(e) => {
                error!("Failed to get orphaned media files count: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Convert to proto format
        let proto_files = orphaned_files
            .into_iter()
            .map(|file| MediaFile {
                id: file.id,
                original_filename: file.original_filename,
                file_extension: file.file_extension,
                media_type: file.media_type.as_str().to_string(),
                file_size_bytes: file.file_size_bytes as i64,
                mime_type: file.mime_type,
                uploaded_at: file.uploaded_at.timestamp(),
                checksum: file.checksum,
            })
            .collect();

        Ok(Response::new(GetOrphanedMediaFilesResponse {
            orphaned_files: proto_files,
            total_count,
        }))
    }

    /// Get media statistics
    pub async fn get_media_statistics(
        &self,
        _request: Request<GetMediaStatisticsRequest>,
    ) -> Result<Response<GetMediaStatisticsResponse>, Status> {
        let db = self.db.lock().await;

        let (
            total_files,
            total_size,
            cover_art_count,
            audio_file_count,
            orphaned_count,
            orphaned_size,
        ) = match db.get_media_statistics() {
            Ok(stats) => stats,
            Err(e) => {
                error!("Failed to get media statistics: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        // Create a map of files by type
        let mut files_by_type = std::collections::HashMap::new();
        files_by_type.insert("cover_art".to_string(), cover_art_count);
        files_by_type.insert("audio_file".to_string(), audio_file_count);

        Ok(Response::new(GetMediaStatisticsResponse {
            total_files,
            total_size_bytes: total_size,
            cover_art_count,
            audio_file_count,
            orphaned_files_count: orphaned_count,
            orphaned_files_size_bytes: orphaned_size,
            files_by_type,
        }))
    }

    /// Cleanup orphaned media files
    pub async fn cleanup_orphaned_media(
        &self,
        request: Request<CleanupOrphanedMediaRequest>,
    ) -> Result<Response<CleanupOrphanedMediaResponse>, Status> {
        let req = request.into_inner();
        let mut db = self.db.lock().await;

        // Get orphaned files first
        let orphaned_files = match db.get_orphaned_media_files(None, None) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to get orphaned media files: {:?}", e);
                return Err(Status::internal(format!("Database error: {}", e)));
            }
        };

        let mut deleted_file_ids = Vec::new();
        let mut bytes_freed = 0i64;

        if !req.dry_run {
            // Actually delete the files
            for file in &orphaned_files {
                // Delete from storage
                if let Err(e) =
                    self.media_storage
                        .delete_file(&file.id, &file.file_extension, &file.media_type)
                {
                    warn!("Failed to delete physical file from storage: {:?}", e);
                    // Continue with database deletion even if physical file deletion fails
                }

                // Delete from database
                if let Err(e) = db.delete_media_file(&file.id) {
                    error!("Failed to delete media file from database: {:?}", e);
                    continue;
                }

                deleted_file_ids.push(file.id.clone());
                bytes_freed += file.file_size_bytes as i64;
            }
        } else {
            // Dry run - just calculate what would be deleted
            for file in &orphaned_files {
                deleted_file_ids.push(file.id.clone());
                bytes_freed += file.file_size_bytes as i64;
            }
        }

        Ok(Response::new(CleanupOrphanedMediaResponse {
            files_cleaned: deleted_file_ids.len() as i32,
            bytes_freed,
            deleted_file_ids,
            success: true,
            error_message: None,
        }))
    }
}
