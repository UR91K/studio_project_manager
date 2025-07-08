//! Media database tests
//!
//! This module tests the media file functionality including:
//! - Media file CRUD operations
//! - Cover art management for collections  
//! - Audio file management for projects
//! - Error handling and validation

use std::path::PathBuf;
use uuid::Uuid;

use studio_project_manager::database::LiveSetDatabase;
use studio_project_manager::media::{MediaType, MediaFile};

use crate::common::setup;
use crate::database::core::create_test_live_set;

/// Creates a test MediaFile with proper types
fn create_test_media_file(
    media_type: MediaType,
    filename: &str,
    size: u64,
    mime_type: &str,
) -> MediaFile {
    MediaFile {
        id: Uuid::new_v4().to_string(),
        original_filename: filename.to_string(),
        file_extension: filename.split('.').last().unwrap().to_string(),
        media_type,
        file_size_bytes: size,
        mime_type: mime_type.to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test_checksum".to_string(),
    }
}

#[test]
fn test_media_file_crud() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Test insert
    let media_file = create_test_media_file(
        MediaType::CoverArt,
        "test_cover.jpg",
        1024,
        "image/jpeg"
    );
    
    db.insert_media_file(&media_file).unwrap();

    // Test get by ID
    let retrieved = db.get_media_file(&media_file.id).unwrap().unwrap();
    assert_eq!(retrieved.id, media_file.id);
    assert_eq!(retrieved.original_filename, media_file.original_filename);
    assert_eq!(retrieved.file_size_bytes, media_file.file_size_bytes);
    assert_eq!(retrieved.mime_type, media_file.mime_type);

    // Test get non-existent
    let non_existent = db.get_media_file("non-existent-id").unwrap();
    assert!(non_existent.is_none());

    // Test delete
    db.delete_media_file(&media_file.id).unwrap();
    let after_delete = db.get_media_file(&media_file.id).unwrap();
    assert!(after_delete.is_none());
}

#[test]
fn test_collection_cover_art_management() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = Uuid::new_v4().to_string();
    db.create_collection("Test Collection", None, None).unwrap();

    // Create test media file
    let media_file = create_test_media_file(
        MediaType::CoverArt,
        "cover.jpg",
        2048,
        "image/jpeg"
    );
    
    db.insert_media_file(&media_file).unwrap();

    // Test set cover art
    db.update_collection_cover_art(&collection_id, Some(&media_file.id)).unwrap();
    
    let cover_art = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(cover_art.is_some());
    assert_eq!(cover_art.unwrap().id, media_file.id);

    // Test remove cover art
    db.update_collection_cover_art(&collection_id, None).unwrap();
    let after_removal = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(after_removal.is_none());

    // Test foreign key constraint (should fail for non-existent media)
    let result = db.update_collection_cover_art(&collection_id, Some("non-existent-id"));
    assert!(result.is_err());
}

#[test]
fn test_project_audio_file_management() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test project
    let test_project = create_test_live_set();
    db.insert_project(&test_project).unwrap();

    // Create test audio file
    let audio_file = create_test_media_file(
        MediaType::AudioFile,
        "demo.mp3",
        5 * 1024 * 1024, // 5MB
        "audio/mpeg"
    );
    
    db.insert_media_file(&audio_file).unwrap();

    // Test set audio file
    let project_id = test_project.id.to_string();
    db.update_project_audio_file(&project_id, Some(&audio_file.id)).unwrap();
    
    let project_audio = db.get_project_audio_file(&project_id).unwrap();
    assert!(project_audio.is_some());
    assert_eq!(project_audio.unwrap().id, audio_file.id);

    // Test remove audio file
    db.update_project_audio_file(&project_id, None).unwrap();
    let after_removal = db.get_project_audio_file(&project_id).unwrap();
    assert!(after_removal.is_none());

    // Test foreign key constraint (should fail for non-existent media)
    let result = db.update_project_audio_file(&project_id, Some("non-existent-id"));
    assert!(result.is_err());
}

#[test]
fn test_media_file_types_separation() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create media files of different types
    let cover_art = create_test_media_file(
        MediaType::CoverArt,
        "cover.jpg",
        1024,
        "image/jpeg"
    );
    let audio_file = create_test_media_file(
        MediaType::AudioFile,
        "demo.mp3",
        2048,
        "audio/mpeg"
    );

    db.insert_media_file(&cover_art).unwrap();
    db.insert_media_file(&audio_file).unwrap();

    // Test get by type
    let cover_arts = db.get_media_files_by_type(&MediaType::CoverArt).unwrap();
    let audio_files = db.get_media_files_by_type(&MediaType::AudioFile).unwrap();

    assert_eq!(cover_arts.len(), 1);
    assert_eq!(audio_files.len(), 1);
    assert_eq!(cover_arts[0].id, cover_art.id);
    assert_eq!(audio_files[0].id, audio_file.id);
}

#[test]
fn test_orphaned_media_files() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection and project
    let collection_id = Uuid::new_v4().to_string();
    let test_project = create_test_live_set();
    
    db.create_collection("Test Collection", None, None).unwrap();
    db.insert_project(&test_project).unwrap();

    // Create media files
    let orphaned_cover = create_test_media_file(
        MediaType::CoverArt,
        "orphaned.jpg",
        1024,
        "image/jpeg"
    );
    let used_cover = create_test_media_file(
        MediaType::CoverArt,
        "used.jpg",
        1024,
        "image/jpeg"
    );
    let orphaned_audio = create_test_media_file(
        MediaType::AudioFile,
        "orphaned.mp3",
        2048,
        "audio/mpeg"
    );
    let used_audio = create_test_media_file(
        MediaType::AudioFile,
        "used.mp3",
        2048,
        "audio/mpeg"
    );

    // Insert all media files
    db.insert_media_file(&orphaned_cover).unwrap();
    db.insert_media_file(&used_cover).unwrap();
    db.insert_media_file(&orphaned_audio).unwrap();
    db.insert_media_file(&used_audio).unwrap();

    // Assign some media files to collection/project
    db.update_collection_cover_art(&collection_id, Some(&used_cover.id)).unwrap();
    let project_id = test_project.id.to_string();
    db.update_project_audio_file(&project_id, Some(&used_audio.id)).unwrap();

    // Get orphaned files
    let orphaned = db.get_orphaned_media_files().unwrap();
    assert_eq!(orphaned.len(), 2);
    
    let orphaned_ids: Vec<String> = orphaned.iter().map(|f| f.id.clone()).collect();
    assert!(orphaned_ids.contains(&orphaned_cover.id));
    assert!(orphaned_ids.contains(&orphaned_audio.id));
    assert!(!orphaned_ids.contains(&used_cover.id));
    assert!(!orphaned_ids.contains(&used_audio.id));
}

#[test]
fn test_media_file_statistics() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create various media files
    let files = vec![
        create_test_media_file(MediaType::CoverArt, "cover1.jpg", 1024, "image/jpeg"),
        create_test_media_file(MediaType::CoverArt, "cover2.png", 2048, "image/png"),
        create_test_media_file(MediaType::AudioFile, "audio1.mp3", 1024 * 1024, "audio/mpeg"),
        create_test_media_file(MediaType::AudioFile, "audio2.wav", 2 * 1024 * 1024, "audio/wav"),
    ];

    for file in &files {
        db.insert_media_file(file).unwrap();
    }

    // Test statistics
    let stats = db.get_media_file_stats().unwrap();
    assert_eq!(stats.total_files(), 4);
    assert_eq!(stats.cover_art_count, 2);
    assert_eq!(stats.audio_file_count, 2);
    assert_eq!(stats.total_size_bytes(), 3 * 1024 * 1024 + 1024 + 2048); // ~3MB + small images
    assert_eq!(stats.cover_art_total_size_bytes, 1024 + 2048);
    assert_eq!(stats.audio_file_total_size_bytes, 1024 * 1024 + 2 * 1024 * 1024);
}

#[test]
fn test_media_file_database_constraints() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    let media_file = create_test_media_file(
        MediaType::CoverArt,
        "test.jpg",
        1024,
        "image/jpeg"
    );
    
    db.insert_media_file(&media_file).unwrap();

    // Test unique constraint on ID (should fail)
    let duplicate_id = MediaFile {
        id: media_file.id.clone(), // Same ID
        original_filename: "different.jpg".to_string(),
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 512,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "different_checksum".to_string(),
    };
    
    let result = db.insert_media_file(&duplicate_id);
    assert!(result.is_err());
}

#[test]
fn test_media_file_cleanup_on_association_removal() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = Uuid::new_v4().to_string();
    db.create_collection("Test Collection", None, None).unwrap();

    // Create and assign cover art
    let cover_art = create_test_media_file(
        MediaType::CoverArt,
        "cover.jpg",
        1024,
        "image/jpeg"
    );
    
    db.insert_media_file(&cover_art).unwrap();
    db.update_collection_cover_art(&collection_id, Some(&cover_art.id)).unwrap();

    // Verify association exists
    let retrieved = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(retrieved.is_some());

    // Remove association
    db.update_collection_cover_art(&collection_id, None).unwrap();
    let after_removal = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(after_removal.is_none());

    // Media file should still exist in database (not auto-deleted)
    let media_still_exists = db.get_media_file(&cover_art.id).unwrap();
    assert!(media_still_exists.is_some());
}

#[test]
fn test_media_file_cascade_deletion() {
    setup("debug");
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = Uuid::new_v4().to_string();
    db.create_collection("Test Collection", None, None).unwrap();

    // Create and assign cover art
    let cover_art = create_test_media_file(
        MediaType::CoverArt,
        "cover.jpg",
        1024,
        "image/jpeg"
    );
    
    db.insert_media_file(&cover_art).unwrap();
    db.update_collection_cover_art(&collection_id, Some(&cover_art.id)).unwrap();

    // Delete media file
    db.delete_media_file(&cover_art.id).unwrap();

    // Collection should no longer have cover art (should be set to NULL)
    let after_deletion = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(after_deletion.is_none());

    // Verify media file is gone
    let media_file = db.get_media_file(&cover_art.id).unwrap();
    assert!(media_file.is_none());
}

// TODO: Add integration tests with actual file storage
// TODO: Add tests for concurrent access to media files
// TODO: Add tests for media file validation
// TODO: Add tests for media file streaming operations 