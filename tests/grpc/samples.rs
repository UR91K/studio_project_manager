//! Sample service tests

use crate::grpc::*;
use crate::grpc::server_setup::{setup_test_server, create_test_project, create_test_sample, add_sample_to_project, create_sample_struct};

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
async fn test_get_sample_analytics() {
    crate::common::setup("trace");

    let server = create_test_server().await;
    let db = server.db();
    
    // Create test samples with different properties
    let sample1_id = uuid::Uuid::new_v4().to_string();
    let sample2_id = uuid::Uuid::new_v4().to_string();
    let sample3_id = uuid::Uuid::new_v4().to_string();
    
    // Insert test samples and projects
    {
        let db_lock = db.lock().await;
        
        // Insert samples with different usage patterns
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
        
        // Create multiple projects to simulate high usage for sample1
        for i in 0..5 {
            let project_id = uuid::Uuid::new_v4().to_string();
            db_lock.conn.execute(
                "INSERT INTO projects (id, name, path, hash, created_at, modified_at, last_parsed_at, tempo, time_signature_numerator, time_signature_denominator, ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    project_id,
                    format!("Test Project {}", i),
                    format!("/path/to/test/project{}.als", i),
                    format!("test_hash_{}", i),
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
            
            // Link sample1 to this project (high usage)
            db_lock.conn.execute(
                "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
                rusqlite::params![project_id, sample1_id],
            ).unwrap();
        }
        
        // Create one more project for sample3 (moderate usage)
        let project_id = uuid::Uuid::new_v4().to_string();
        db_lock.conn.execute(
            "INSERT INTO projects (id, name, path, hash, created_at, modified_at, last_parsed_at, tempo, time_signature_numerator, time_signature_denominator, ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                project_id,
                "Test Project for Sample3",
                "/path/to/test/project_sample3.als",
                "test_hash_sample3",
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
        
        // Link sample3 to project once (moderate usage)
        db_lock.conn.execute(
            "INSERT INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            rusqlite::params![project_id, sample3_id],
        ).unwrap();
    }
    
    // Test the GetSampleAnalytics endpoint
    let request = Request::new(GetSampleAnalyticsRequest {});
    let response = server.get_sample_analytics(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return analytics
    assert!(response.analytics.is_some());
    let analytics = response.analytics.unwrap();
    
    // Check usage distribution
    assert_eq!(analytics.most_used_samples_count, 1); // sample1 (5+ usages)
    assert_eq!(analytics.moderately_used_samples_count, 0); // no samples with 2-4 usages
    assert_eq!(analytics.rarely_used_samples_count, 1); // sample3 (1 usage)
    assert_eq!(analytics.unused_samples_count, 1); // sample2 (0 usages)
    
    // Check extensions
    assert!(analytics.extensions.contains_key("wav"));
    assert!(analytics.extensions.contains_key("mp3"));
    assert!(analytics.extensions.contains_key("aiff"));
    
    // Check presence percentages (accounting for rounding)
    assert_eq!(analytics.present_samples_percentage, 66); // 2 out of 3 samples present (66.67% rounded down)
    assert_eq!(analytics.missing_samples_percentage, 33); // 1 out of 3 samples missing (33.33% rounded down)
    
    // Check storage usage
    assert!(analytics.total_storage_bytes > 0);
    assert!(analytics.present_storage_bytes > 0);
    assert!(analytics.missing_storage_bytes > 0);
    
    // Check top used samples
    assert_eq!(analytics.top_used_samples.len(), 3);
    assert_eq!(analytics.top_used_samples[0].usage_count, 5); // sample1 should be first
    assert_eq!(analytics.top_used_samples[0].sample_id, sample1_id);
    
    // Check recently added samples (should be 0 for now since we don't track creation dates)
    assert_eq!(analytics.recently_added_samples, 0);
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

#[tokio::test]
async fn test_get_sample_extensions() {
    let (server, _db) = setup_test_server().await;

    // Create test projects with samples of different extensions
    let project1_id = create_test_project(&server, "Test Project 1", "/path/to/project1.als").await;
    let project2_id = create_test_project(&server, "Test Project 2", "/path/to/project2.als").await;

    // Add samples with different extensions
    let sample1_id = create_test_sample(&server, "test_wav.wav", "/path/to/test_wav.wav", true).await;
    let sample2_id = create_test_sample(&server, "test_aiff.aiff", "/path/to/test_aiff.aiff", true).await;
    let sample3_id = create_test_sample(&server, "test_mp3.mp3", "/path/to/test_mp3.mp3", false).await;
    let sample4_id = create_test_sample(&server, "test_flac.flac", "/path/to/test_flac.flac", true).await;

    // Add samples to projects
    add_sample_to_project(&server, &project1_id, &sample1_id).await;
    add_sample_to_project(&server, &project1_id, &sample2_id).await;
    add_sample_to_project(&server, &project2_id, &sample1_id).await; // wav sample used in 2 projects
    add_sample_to_project(&server, &project2_id, &sample3_id).await;
    add_sample_to_project(&server, &project2_id, &sample4_id).await;

    // Test GetSampleExtensions
    let request = Request::new(GetSampleExtensionsRequest {});
    let response = server.get_sample_extensions(request).await.unwrap();
    let extensions = response.into_inner().extensions;

    // Should have extensions for wav, aiff, mp3, flac
    assert!(extensions.contains_key("wav"));
    assert!(extensions.contains_key("aiff"));
    assert!(extensions.contains_key("mp3"));
    assert!(extensions.contains_key("flac"));

    // Check wav extension stats
    let wav_stats = extensions.get("wav").unwrap();
    assert_eq!(wav_stats.count, 1); // 1 wav sample
    assert_eq!(wav_stats.present_count, 1); // 1 present
    assert_eq!(wav_stats.missing_count, 0); // 0 missing
    assert_eq!(wav_stats.average_usage_count, 2.0); // used in 2 projects

    // Check mp3 extension stats
    let mp3_stats = extensions.get("mp3").unwrap();
    assert_eq!(mp3_stats.count, 1); // 1 mp3 sample
    assert_eq!(mp3_stats.present_count, 0); // 0 present
    assert_eq!(mp3_stats.missing_count, 1); // 1 missing
    assert_eq!(mp3_stats.average_usage_count, 1.0); // used in 1 project

    // Check that total_size_bytes is reasonable (estimated sizes)
    assert!(wav_stats.total_size_bytes > 0);
    assert!(mp3_stats.total_size_bytes > 0);
}

#[tokio::test]
async fn test_liveset_add_sample_method() {
    use studio_project_manager::live_set::LiveSet;
    use std::path::PathBuf;
    use std::collections::HashSet;
    use uuid::Uuid;
    use chrono::Local;
    
    // Create a minimal LiveSet for testing
    let mut live_set = LiveSet {
        is_active: true,
        id: Uuid::new_v4(),
        file_path: PathBuf::from("/test/path.als"),
        name: "Test Project".to_string(),
        file_hash: "test_hash".to_string(),
        created_time: Local::now(),
        modified_time: Local::now(),
        last_parsed_timestamp: Local::now(),
        ableton_version: studio_project_manager::models::AbletonVersion {
            major: 11,
            minor: 0,
            patch: 0,
            beta: false,
        },
        key_signature: None,
        tempo: 120.0,
        time_signature: studio_project_manager::models::TimeSignature {
            numerator: 4,
            denominator: 4,
        },
        furthest_bar: None,
        plugins: HashSet::new(),
        samples: HashSet::new(),
        tags: HashSet::new(),
        estimated_duration: None,
    };

    // Initially no samples
    assert_eq!(live_set.samples.len(), 0);

    // Create test samples using the helper function
    let sample1 = create_sample_struct("kick.wav", "/samples/kick.wav", true);
    let sample2 = create_sample_struct("snare.wav", "/samples/snare.wav", false);

    // Add samples using the new method
    live_set.add_sample(sample1.clone());
    live_set.add_sample(sample2.clone());

    // Verify samples were added
    assert_eq!(live_set.samples.len(), 2);
    assert!(live_set.samples.contains(&sample1));
    assert!(live_set.samples.contains(&sample2));

    // Test that adding the same sample again doesn't duplicate (HashSet behavior)
    live_set.add_sample(sample1.clone());
    assert_eq!(live_set.samples.len(), 2);
} 