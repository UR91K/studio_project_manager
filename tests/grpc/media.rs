//! Media-related gRPC tests
//!
//! This module tests the media functionality including:
//! - Streaming upload/download operations
//! - Collection cover art management
//! - Project audio file management
//! - Error handling and validation

use super::*;
use std::collections::VecDeque;
use studio_project_manager::grpc::media::media_service_server::MediaService;
use studio_project_manager::media::{MediaFile, MediaType};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_set_collection_cover_art_success() {
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: None,
        notes: None,
    });
    let collection_response = server.create_collection(create_request).await.unwrap();
    let collection_id = collection_response.into_inner().collection.unwrap().id;

    // Create a test media file in the database
    let media_file = MediaFile {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: "cover.jpg".to_string(),
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 1024,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&media_file).unwrap();
    }

    // Set collection cover art
    let request = Request::new(SetCollectionCoverArtRequest {
        collection_id: collection_id.clone(),
        media_file_id: media_file.id.clone(),
    });

    let response = server.set_collection_cover_art(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.success);
    assert!(inner.error_message.is_none());

    // Verify the association was created
    {
        let db = server.db().lock().await;
        let cover_art = db.get_collection_cover_art(&collection_id).unwrap();
        assert!(cover_art.is_some());
        assert_eq!(cover_art.unwrap().id, media_file.id);
    }
}

#[tokio::test]
async fn test_set_collection_cover_art_nonexistent_collection() {
    let server = create_test_server().await;

    let request = Request::new(SetCollectionCoverArtRequest {
        collection_id: "nonexistent-collection".to_string(),
        media_file_id: "some-media-id".to_string(),
    });

    let response = server.set_collection_cover_art(request).await.unwrap();
    let inner = response.into_inner();

    assert!(!inner.success);
    assert!(inner.error_message.is_some());
}

#[tokio::test]
async fn test_remove_collection_cover_art_success() {
    let server = create_test_server().await;

    // Create a collection
    let create_request = Request::new(CreateCollectionRequest {
        name: "Test Collection".to_string(),
        description: None,
        notes: None,
    });
    let collection_response = server.create_collection(create_request).await.unwrap();
    let collection_id = collection_response.into_inner().collection.unwrap().id;

    // Create and set cover art first
    let media_file = MediaFile {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: "cover.jpg".to_string(),
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 1024,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&media_file).unwrap();
        db.update_collection_cover_art(&collection_id, Some(&media_file.id))
            .unwrap();
    }

    // Remove collection cover art
    let request = Request::new(RemoveCollectionCoverArtRequest {
        collection_id: collection_id.clone(),
    });

    let response = server.remove_collection_cover_art(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.success);
    assert!(inner.error_message.is_none());

    // Verify the association was removed
    {
        let db = server.db().lock().await;
        let cover_art = db.get_collection_cover_art(&collection_id).unwrap();
        assert!(cover_art.is_none());
    }
}

#[tokio::test]
async fn test_set_project_audio_file_success() {
    let server = create_test_server().await;

    // Create a test project
    let project_id = create_test_project_in_db(server.db()).await;

    // Create a test audio file in the database
    let audio_file = MediaFile {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: "demo.mp3".to_string(),
        file_extension: "mp3".to_string(),
        media_type: MediaType::AudioFile,
        file_size_bytes: 2048,
        mime_type: "audio/mpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&audio_file).unwrap();
    }

    // Set project audio file
    let request = Request::new(SetProjectAudioFileRequest {
        project_id: project_id.clone(),
        media_file_id: audio_file.id.clone(),
    });

    let response = server.set_project_audio_file(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.success);
    assert!(inner.error_message.is_none());

    // Verify the association was created
    {
        let db = server.db().lock().await;
        let project_audio = db.get_project_audio_file(&project_id).unwrap();
        assert!(project_audio.is_some());
        assert_eq!(project_audio.unwrap().id, audio_file.id);
    }
}

#[tokio::test]
async fn test_remove_project_audio_file_success() {
    let server = create_test_server().await;

    // Create a test project
    let project_id = create_test_project_in_db(server.db()).await;

    // Create and set audio file first
    let audio_file = MediaFile {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: "demo.mp3".to_string(),
        file_extension: "mp3".to_string(),
        media_type: MediaType::AudioFile,
        file_size_bytes: 2048,
        mime_type: "audio/mpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&audio_file).unwrap();
        db.update_project_audio_file(&project_id, Some(&audio_file.id))
            .unwrap();
    }

    // Remove project audio file
    let request = Request::new(RemoveProjectAudioFileRequest {
        project_id: project_id.clone(),
    });

    let response = server.remove_project_audio_file(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.success);
    assert!(inner.error_message.is_none());

    // Verify the association was removed
    {
        let db = server.db().lock().await;
        let project_audio = db.get_project_audio_file(&project_id).unwrap();
        assert!(project_audio.is_none());
    }
}

#[tokio::test]
async fn test_delete_media_success() {
    let server = create_test_server().await;

    // Create a test media file in the database
    let media_file = MediaFile {
        id: uuid::Uuid::new_v4().to_string(),
        original_filename: "test.jpg".to_string(),
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 1024,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&media_file).unwrap();
    }

    // Delete the media file
    let request = Request::new(DeleteMediaRequest {
        media_file_id: media_file.id.clone(),
    });

    let response = server.delete_media(request).await.unwrap();
    let inner = response.into_inner();

    assert!(inner.success);
    assert!(inner.error_message.is_none());

    // Verify the media file was deleted
    {
        let db = server.db().lock().await;
        let deleted = db.get_media_file(&media_file.id).unwrap();
        assert!(deleted.is_none());
    }
}

#[tokio::test]
async fn test_delete_media_not_found() {
    let server = create_test_server().await;

    let request = Request::new(DeleteMediaRequest {
        media_file_id: "nonexistent-media-id".to_string(),
    });

    let response = server.delete_media(request).await.unwrap();
    let inner = response.into_inner();

    assert!(!inner.success);
    assert!(inner.error_message.is_some());
    assert!(inner.error_message.unwrap().contains("not found"));
}

// TODO: Implement streaming tests - these require proper tonic::Streaming mocks
// #[tokio::test]
// async fn test_upload_cover_art_streaming() {
//     let server = create_test_server().await;
//
//     // Prepare streaming data
//     let collection_id = "test-collection-id".to_string();
//     let filename = "cover.jpg".to_string();
//     let file_data = b"fake image data";
//
//     let requests = vec![
//         UploadCoverArtRequest {
//             data: Some(upload_cover_art_request::Data::CollectionId(collection_id)),
//         },
//         UploadCoverArtRequest {
//             data: Some(upload_cover_art_request::Data::Filename(filename)),
//         },
//         UploadCoverArtRequest {
//             data: Some(upload_cover_art_request::Data::Chunk(file_data.to_vec())),
//         },
//     ];
//
//     let stream = iter(requests.into_iter().map(Ok));
//     let request = Request::new(stream);
//
//     let response = server.upload_cover_art(request).await.unwrap();
//     let inner = response.into_inner();
//
//     assert!(inner.success);
//     assert!(!inner.media_file_id.is_empty());
//     assert!(inner.error_message.is_none());
// }

// #[tokio::test]
// async fn test_upload_cover_art_missing_collection_id() {
//     let server = create_test_server().await;
//
//     // Only send filename and data, no collection ID
//     let requests = vec![
//         UploadCoverArtRequest {
//             data: Some(upload_cover_art_request::Data::Filename("cover.jpg".to_string())),
//         },
//         UploadCoverArtRequest {
//             data: Some(upload_cover_art_request::Data::Chunk(b"data".to_vec())),
//         },
//     ];
//
//     let stream = iter(requests.into_iter().map(Ok));
//     let request = Request::new(stream);
//
//     let result = server.upload_cover_art(request).await;
//     assert!(result.is_err());
//     assert_eq!(result.unwrap_err().code(), Code::InvalidArgument);
// }

// #[tokio::test]
// async fn test_upload_audio_file_streaming() {
//     let server = create_test_server().await;
//
//     // Prepare streaming data
//     let project_id = "test-project-id".to_string();
//     let filename = "demo.mp3".to_string();
//     let file_data = b"fake audio data";
//
//     let requests = vec![
//         UploadAudioFileRequest {
//             data: Some(upload_audio_file_request::Data::ProjectId(project_id)),
//         },
//         UploadAudioFileRequest {
//             data: Some(upload_audio_file_request::Data::Filename(filename)),
//         },
//         UploadAudioFileRequest {
//             data: Some(upload_audio_file_request::Data::Chunk(file_data.to_vec())),
//         },
//     ];
//
//     let stream = iter(requests.into_iter().map(Ok));
//     let request = Request::new(stream);
//
//     let response = server.upload_audio_file(request).await.unwrap();
//     let inner = response.into_inner();
//
//     assert!(inner.success);
//     assert!(!inner.media_file_id.is_empty());
//     assert!(inner.error_message.is_none());
// }

#[tokio::test]
async fn test_download_media_streaming() {
    let server = create_test_server().await;

    // Create a test file using the MediaStorageManager
    let test_data = b"test file content for streaming";
    let filename = "test.jpg";

    let media_file = server
        .media_storage()
        .store_file(test_data, filename, MediaType::CoverArt)
        .unwrap();

    // Insert the media file into the database
    {
        let mut db = server.db().lock().await;
        db.insert_media_file(&media_file).unwrap();
    }

    // Download the media file
    let request = Request::new(DownloadMediaRequest {
        media_file_id: media_file.id.clone(),
    });

    let mut response_stream = server.download_media(request).await.unwrap().into_inner();

    let mut responses: VecDeque<DownloadMediaResponse> = VecDeque::new();
    while let Some(response_result) = response_stream.next().await {
        responses.push_back(response_result.unwrap());
    }

    // Should have at least metadata and one chunk
    assert!(responses.len() >= 2);

    // First response should be metadata
    let first_response = responses.pop_front().unwrap();
    match first_response.data {
        Some(download_media_response::Data::Metadata(metadata)) => {
            assert_eq!(metadata.id, media_file.id);
            assert_eq!(metadata.original_filename, filename);
            assert_eq!(metadata.media_type, "cover_art");
        }
        _ => panic!("First response should be metadata"),
    }

    // Subsequent responses should be chunks
    let mut all_chunks = Vec::new();
    while let Some(response) = responses.pop_front() {
        match response.data {
            Some(download_media_response::Data::Chunk(data)) => {
                all_chunks.extend(data);
            }
            _ => panic!("Response should be chunk data"),
        }
    }

    // Verify the downloaded content matches the original
    assert_eq!(all_chunks, test_data);
}

#[tokio::test]
async fn test_download_media_not_found() {
    let server = create_test_server().await;

    let request = Request::new(DownloadMediaRequest {
        media_file_id: "nonexistent-media-id".to_string(),
    });

    let result = server.download_media(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), Code::NotFound);
}
