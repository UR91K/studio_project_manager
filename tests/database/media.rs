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
use studio_project_manager::media::{MediaFile, MediaType};

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
        file_extension: filename.split('.').last().unwrap_or("").to_string(),
        media_type,
        file_size_bytes: size,
        mime_type: mime_type.to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "dummy-checksum".to_string(),
    }
}

#[test]
fn test_media_file_crud() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test media file
    let media_file = create_test_media_file(MediaType::CoverArt, "cover.jpg", 1024, "image/jpeg");

    // Test insert
    db.insert_media_file(&media_file).unwrap();

    // Test get
    let retrieved = db.get_media_file(&media_file.id).unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, media_file.id);
    assert_eq!(retrieved.original_filename, "cover.jpg");

    // Test delete
    db.delete_media_file(&media_file.id).unwrap();
    let deleted = db.get_media_file(&media_file.id).unwrap();
    assert!(deleted.is_none());
}

#[test]
fn test_collection_cover_art_management() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = db.create_collection("Test Collection", None, None).unwrap();

    // Create test media file
    let media_file = create_test_media_file(MediaType::CoverArt, "cover.jpg", 1024, "image/jpeg");

    // Insert media file
    db.insert_media_file(&media_file).unwrap();

    // Associate with collection
    db.update_collection_cover_art(&collection_id, Some(&media_file.id))
        .unwrap();

    // Verify association
    let cover_art = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(cover_art.is_some());
    let cover_art = cover_art.unwrap();
    assert_eq!(cover_art.id, media_file.id);

    // Remove association
    db.update_collection_cover_art(&collection_id, None)
        .unwrap();
    let no_cover_art = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(no_cover_art.is_none());
}

#[test]
fn test_project_audio_file_management() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection and project
    let test_project = create_test_live_set();

    db.insert_project(&test_project).unwrap();

    // Create test audio file
    let audio_file = create_test_media_file(MediaType::AudioFile, "demo.mp3", 2048, "audio/mpeg");

    // Insert audio file
    db.insert_media_file(&audio_file).unwrap();

    // Associate with project
    db.update_project_audio_file(&test_project.id.to_string(), Some(&audio_file.id))
        .unwrap();

    // Verify association
    let project_audio = db
        .get_project_audio_file(&test_project.id.to_string())
        .unwrap();
    assert!(project_audio.is_some());
    let project_audio = project_audio.unwrap();
    assert_eq!(project_audio.id, audio_file.id);

    // Remove association
    db.update_project_audio_file(&test_project.id.to_string(), None)
        .unwrap();
    let no_audio = db
        .get_project_audio_file(&test_project.id.to_string())
        .unwrap();
    assert!(no_audio.is_none());
}

#[test]
fn test_media_file_types_separation() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create one of each media type
    let cover_art = create_test_media_file(MediaType::CoverArt, "cover.jpg", 1024, "image/jpeg");
    let audio_file = create_test_media_file(MediaType::AudioFile, "demo.mp3", 2048, "audio/mpeg");

    // Insert both
    db.insert_media_file(&cover_art).unwrap();
    db.insert_media_file(&audio_file).unwrap();

    // Test type-specific queries
    let cover_art_files = db.get_media_files_by_type("cover_art", None, None).unwrap();
    assert_eq!(cover_art_files.len(), 1);
    assert_eq!(cover_art_files[0].media_type, MediaType::CoverArt);

    let audio_files = db
        .get_media_files_by_type("audio_file", None, None)
        .unwrap();
    assert_eq!(audio_files.len(), 1);
    assert_eq!(audio_files[0].media_type, MediaType::AudioFile);
}

#[test]
fn test_media_file_statistics() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test media files
    let cover1 = create_test_media_file(MediaType::CoverArt, "cover1.jpg", 1024, "image/jpeg");
    let cover2 = create_test_media_file(MediaType::CoverArt, "cover2.png", 2048, "image/png");
    let audio1 = create_test_media_file(
        MediaType::AudioFile,
        "audio1.mp3",
        1024 * 1024,
        "audio/mpeg",
    );
    let audio2 = create_test_media_file(
        MediaType::AudioFile,
        "audio2.wav",
        2 * 1024 * 1024,
        "audio/wav",
    );

    // Insert all files
    db.insert_media_file(&cover1).unwrap();
    db.insert_media_file(&cover2).unwrap();
    db.insert_media_file(&audio1).unwrap();
    db.insert_media_file(&audio2).unwrap();

    // Test statistics
    let (total_files, total_size, cover_art_count, audio_file_count, orphaned_count, orphaned_size) =
        db.get_media_statistics().unwrap();
    assert_eq!(total_files, 4);
    assert_eq!(cover_art_count, 2);
    assert_eq!(audio_file_count, 2);
    assert_eq!(total_size, 3072 + 3 * 1024 * 1024);
    assert_eq!(orphaned_count, 4); // All files are orphaned since they're not associated
    assert_eq!(orphaned_size, 3072 + 3 * 1024 * 1024);
}

#[test]
fn test_orphaned_media_files() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection and project
    let collection_id = db.create_collection("Test Collection", None, None).unwrap();
    let test_project = create_test_live_set();
    db.insert_project(&test_project).unwrap();

    // Create orphaned media files
    let orphaned_cover =
        create_test_media_file(MediaType::CoverArt, "orphaned.jpg", 1024, "image/jpeg");
    let orphaned_audio =
        create_test_media_file(MediaType::AudioFile, "orphaned.mp3", 2048, "audio/mpeg");

    // Create used media files
    let used_cover = create_test_media_file(MediaType::CoverArt, "used.jpg", 1024, "image/jpeg");
    let used_audio = create_test_media_file(MediaType::AudioFile, "used.mp3", 2048, "audio/mpeg");

    // Insert all files
    db.insert_media_file(&orphaned_cover).unwrap();
    db.insert_media_file(&orphaned_audio).unwrap();
    db.insert_media_file(&used_cover).unwrap();
    db.insert_media_file(&used_audio).unwrap();

    // Associate only the "used" files
    db.update_collection_cover_art(&collection_id, Some(&used_cover.id))
        .unwrap();
    db.update_project_audio_file(&test_project.id.to_string(), Some(&used_audio.id))
        .unwrap();

    // Find orphaned files
    let orphaned_files = db.get_orphaned_media_files(None, None).unwrap();
    assert_eq!(orphaned_files.len(), 2);

    // Should contain the orphaned files
    let orphaned_ids: Vec<String> = orphaned_files.iter().map(|f| f.id.clone()).collect();
    assert!(orphaned_ids.contains(&orphaned_cover.id));
    assert!(orphaned_ids.contains(&orphaned_audio.id));
}

#[test]
fn test_media_file_database_constraints() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    let media_file = create_test_media_file(MediaType::CoverArt, "test.jpg", 1024, "image/jpeg");

    // Insert the file
    db.insert_media_file(&media_file).unwrap();

    // Try to insert the same file again (should fail due to unique constraint)
    let result = db.insert_media_file(&media_file);
    assert!(result.is_err());
}

#[test]
fn test_media_file_cascade_deletion() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = db.create_collection("Test Collection", None, None).unwrap();

    // Create and assign cover art
    let cover_art = create_test_media_file(MediaType::CoverArt, "cover.jpg", 1024, "image/jpeg");
    db.insert_media_file(&cover_art).unwrap();
    db.update_collection_cover_art(&collection_id, Some(&cover_art.id))
        .unwrap();

    // Verify association exists
    let retrieved = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(retrieved.is_some());

    // Delete the media file
    db.delete_media_file(&cover_art.id).unwrap();

    // Verify the media file is gone
    let deleted_media = db.get_media_file(&cover_art.id).unwrap();
    assert!(deleted_media.is_none());

    // Verify association is cleaned up
    let no_cover_art = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(no_cover_art.is_none());
}

#[test]
fn test_media_file_cleanup_on_association_removal() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create test collection
    let collection_id = db.create_collection("Test Collection", None, None).unwrap();

    // Create and assign cover art
    let cover_art =
        create_test_media_file(MediaType::CoverArt, "test_cover.jpg", 1024, "image/jpeg");
    db.insert_media_file(&cover_art).unwrap();
    db.update_collection_cover_art(&collection_id, Some(&cover_art.id))
        .unwrap();

    // Remove association
    db.update_collection_cover_art(&collection_id, None)
        .unwrap();

    // Verify the association is removed
    let no_cover_art = db.get_collection_cover_art(&collection_id).unwrap();
    assert!(no_cover_art.is_none());

    // Verify the media file still exists (it's not automatically deleted)
    let media_still_exists = db.get_media_file(&cover_art.id).unwrap();
    assert!(media_still_exists.is_some());
}

#[test]
fn test_media_type_conversion() {
    setup("error");

    // Test MediaType enum functionality
    assert_eq!(MediaType::CoverArt.as_str(), "cover_art");
    assert_eq!(MediaType::AudioFile.as_str(), "audio_file");

    // Test conversion from string
    assert_eq!(
        MediaType::from_str("cover_art").unwrap(),
        MediaType::CoverArt
    );
    assert_eq!(
        MediaType::from_str("audio_file").unwrap(),
        MediaType::AudioFile
    );

    // Test invalid conversion
    assert!(MediaType::from_str("invalid").is_err());
}

#[test]
fn test_media_file_validation_edge_cases() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Test with empty filename
    let media_file = MediaFile {
        id: Uuid::new_v4().to_string(),
        original_filename: "".to_string(),
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 1024,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum".to_string(),
    };

    // Should still work with empty filename
    let result = db.insert_media_file(&media_file);
    assert!(result.is_ok());

    // Test with very long filename
    let long_filename = "a".repeat(1000);
    let media_file_long = MediaFile {
        id: Uuid::new_v4().to_string(),
        original_filename: long_filename,
        file_extension: "jpg".to_string(),
        media_type: MediaType::CoverArt,
        file_size_bytes: 1024,
        mime_type: "image/jpeg".to_string(),
        uploaded_at: chrono::Utc::now(),
        checksum: "test-checksum-2".to_string(),
    };

    let result = db.insert_media_file(&media_file_long);
    assert!(result.is_ok());
}

#[test]
fn test_media_file_foreign_key_constraints() {
    setup("error");
    let mut db =
        LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Try to set cover art for non-existent collection
    let result =
        db.update_collection_cover_art("non-existent-collection", Some("non-existent-media"));
    assert!(result.is_err());

    // Try to set audio file for non-existent project
    let result = db.update_project_audio_file("non-existent-project", Some("non-existent-media"));
    assert!(result.is_err());
}
