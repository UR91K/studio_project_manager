//! Sample service tests

use crate::grpc::*;

#[tokio::test]
async fn test_get_sample_success() {
    let server = create_test_server().await;
    let db = server.db();
    
    // First, create a test sample in the database
    let sample_id = uuid::Uuid::new_v4().to_string();
    let sample_name = "Test Sample";
    let sample_path = "/path/to/test/sample.wav";
    
    // Insert test sample directly into database
    {
        let db_lock = db.lock().await;
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample_id, sample_name, sample_path, true],
        ).unwrap();
    }
    
    // Test the GetSample endpoint
    let request = Request::new(GetSampleRequest {
        sample_id: sample_id.to_string(),
    });
    let response = server.get_sample(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return the sample
    assert!(response.sample.is_some());
    let sample = response.sample.unwrap();
    assert_eq!(sample.id, sample_id);
    assert_eq!(sample.name, sample_name);
    assert_eq!(sample.path, sample_path);
    assert_eq!(sample.is_present, true);
}

#[tokio::test]
async fn test_get_sample_not_found() {
    let server = create_test_server().await;
    
    // Test with a non-existent sample ID
    let request = Request::new(GetSampleRequest {
        sample_id: "non-existent-sample".to_string(),
    });
    let response = server.get_sample(request).await;
    
    // Should return NotFound error
    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
    assert!(status.message().contains("Sample not found"));
}

#[tokio::test]
async fn test_refresh_sample_presence_status() {
    let server = create_test_server().await;
    
    // Test the new sample refresh endpoint
    let request = Request::new(RefreshSamplePresenceStatusRequest {});
    let response = server.refresh_sample_presence_status(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return success
    assert!(response.success);
    assert!(response.error_message.is_none());
    
    // Should have checked some samples (even if 0 in test database)
    assert!(response.total_samples_checked >= 0);
    assert!(response.samples_now_present >= 0);
    assert!(response.samples_now_missing >= 0);
    assert!(response.samples_unchanged >= 0);
    
    // Total should add up
    assert_eq!(
        response.total_samples_checked,
        response.samples_now_present + response.samples_now_missing + response.samples_unchanged
    );
}

#[tokio::test]
async fn test_get_all_samples_with_filters() {
    crate::common::setup("trace");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test samples with different properties
    let sample1_id = uuid::Uuid::new_v4().to_string();
    let sample2_id = uuid::Uuid::new_v4().to_string();
    let sample3_id = uuid::Uuid::new_v4().to_string();
    let project_id = uuid::Uuid::new_v4().to_string();
    
    // Insert test samples and project
    {
        let db_lock = db.lock().await;
        
        // Insert project
        db_lock.conn.execute(
            "INSERT INTO projects (id, name, path, hash, created_at, modified_at, last_parsed_at, tempo, time_signature_numerator, time_signature_denominator, ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                project_id,
                "Test Project",
                "/path/to/test/project.als",
                "test_hash",
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                120.0,
                4,
                4,
                11,
                0,
                0,
                false
            ],
        ).unwrap();
        
        // Insert samples
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample1_id, "kick.wav", "/samples/kick.wav", true],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample2_id, "snare.mp3", "/samples/snare.mp3", false],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample3_id, "hihat.aiff", "/samples/hihat.aiff", true],
        ).unwrap();
        
        // Link sample1 to project (usage count = 1)
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project_id, sample1_id],
        ).unwrap();
    }
    
    // Test present_only filter
    let request = Request::new(GetAllSamplesRequest {
        present_only: Some(true),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 2); // Only kick.wav and hihat.aiff should be present
    assert!(samples.iter().all(|s| s.is_present));
    
    // Test missing_only filter
    let request = Request::new(GetAllSamplesRequest {
        missing_only: Some(true),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 1); // Only snare.mp3 should be missing
    assert!(samples.iter().all(|s| !s.is_present));
    
    // Test extension_filter
    let request = Request::new(GetAllSamplesRequest {
        extension_filter: Some("wav".to_string()),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 1); // Only kick.wav should match
    assert!(samples[0].path.ends_with(".wav"));
    
    // Test min_usage_count filter
    let request = Request::new(GetAllSamplesRequest {
        min_usage_count: Some(1),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 1); // Only kick.wav has usage count >= 1
    assert_eq!(samples[0].id, sample1_id);
    
    // Test max_usage_count filter
    let request = Request::new(GetAllSamplesRequest {
        max_usage_count: Some(0),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 2); // snare.mp3 and hihat.aiff have usage count = 0
    assert!(!samples.iter().any(|s| s.id == sample1_id));
    
    // Test combined filters
    let request = Request::new(GetAllSamplesRequest {
        present_only: Some(true),
        extension_filter: Some("aiff".to_string()),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 1); // Only hihat.aiff should match both filters
    assert!(samples[0].path.ends_with(".aiff"));
    assert!(samples[0].is_present);
}

#[tokio::test]
async fn test_get_all_samples_sorting() {
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test samples
    let sample1_id = uuid::Uuid::new_v4().to_string();
    let sample2_id = uuid::Uuid::new_v4().to_string();
    let sample3_id = uuid::Uuid::new_v4().to_string();
    let project_id = uuid::Uuid::new_v4().to_string();
    
    // Insert test data
    {
        let db_lock = db.lock().await;
        
        // Insert project
        db_lock.conn.execute(
            "INSERT INTO projects (id, name, path, hash, created_at, modified_at, last_parsed_at, tempo, time_signature_numerator, time_signature_denominator, ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                project_id,
                "Test Project",
                "/path/to/test/project.als",
                "test_hash",
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                120.0,
                4,
                4,
                11,
                0,
                0,
                false
            ],
        ).unwrap();
        
        // Insert samples with different names
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample1_id, "zebra.wav", "/samples/zebra.wav", true],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample2_id, "alpha.wav", "/samples/alpha.wav", true],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            rusqlite::params![sample3_id, "beta.wav", "/samples/beta.wav", true],
        ).unwrap();
        
        // Link samples to project with different usage counts
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project_id, sample1_id],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project_id, sample2_id],
        ).unwrap();
        
        // Create a second project to give sample2 a higher usage count
        let project2_id = uuid::Uuid::new_v4().to_string();
        db_lock.conn.execute(
            "INSERT INTO projects (id, name, path, hash, created_at, modified_at, last_parsed_at, tempo, time_signature_numerator, time_signature_denominator, ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                project2_id,
                "Test Project 2",
                "/path/to/test/project2.als",
                "test_hash2",
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
                120.0,
                4,
                4,
                11,
                0,
                0,
                false
            ],
        ).unwrap();
        
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project2_id, sample2_id], // sample2 used in second project
        ).unwrap();
    }
    
    // Test sorting by name ASC
    let request = Request::new(GetAllSamplesRequest {
        sort_by: Some("name".to_string()),
        sort_desc: Some(false),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].name, "alpha.wav");
    assert_eq!(samples[1].name, "beta.wav");
    assert_eq!(samples[2].name, "zebra.wav");
    
    // Test sorting by name DESC
    let request = Request::new(GetAllSamplesRequest {
        sort_by: Some("name".to_string()),
        sort_desc: Some(true),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].name, "zebra.wav");
    assert_eq!(samples[1].name, "beta.wav");
    assert_eq!(samples[2].name, "alpha.wav");
    
    // Test sorting by usage_count DESC
    let request = Request::new(GetAllSamplesRequest {
        sort_by: Some("usage_count".to_string()),
        sort_desc: Some(true),
        ..Default::default()
    });
    let response = server.get_all_samples(request).await.unwrap();
    let samples = response.into_inner().samples;
    assert_eq!(samples.len(), 3);
    // sample2 should have highest usage count (2), then sample1 (1), then sample3 (0)
    assert_eq!(samples[0].id, sample2_id);
    assert_eq!(samples[1].id, sample1_id);
    assert_eq!(samples[2].id, sample3_id);
} 