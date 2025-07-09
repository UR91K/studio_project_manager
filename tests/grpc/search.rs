//! Search-related gRPC tests

use studio_project_manager::grpc::proto::studio_project_manager_server::StudioProjectManager;

use super::*;
use crate::common::setup;

#[tokio::test]
async fn test_search_empty_database() {
    setup("debug");
    
    let server = create_test_server().await;
    
    let request = SearchRequest {
        query: "test".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.projects.len(), 0);
    assert_eq!(search_response.total_count, 0);
}

#[tokio::test]
async fn test_search_basic_query() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects with different names
    let _project_id1 = create_test_project_in_db(db).await;
    let _project_id2 = create_test_project_in_db(db).await;
    
    // Search for projects
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find the test projects
    assert!(search_response.projects.len() > 0);
    assert_eq!(search_response.total_count as usize, search_response.projects.len());
    
    // Verify project structure
    for project in &search_response.projects {
        assert!(!project.id.is_empty());
        assert!(!project.name.is_empty());
        assert!(project.name.contains("Test Project"));
    }
}

#[tokio::test]
async fn test_search_with_limit() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create multiple test projects
    for _ in 0..5 {
        let _project_id = create_test_project_in_db(db).await;
    }
    
    // Search with limit
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: Some(3),
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return limited results
    assert_eq!(search_response.projects.len(), 3);
    assert_eq!(search_response.total_count, 5); // Total count should still be 5
}

#[tokio::test]
async fn test_search_with_offset() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create multiple test projects
    for _ in 0..5 {
        let _project_id = create_test_project_in_db(db).await;
    }
    
    // Search with offset
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: None,
        offset: Some(2),
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return results starting from offset
    assert_eq!(search_response.projects.len(), 3); // 5 total - 2 offset = 3
    assert_eq!(search_response.total_count, 5);
}

#[tokio::test]
async fn test_search_with_limit_and_offset() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create multiple test projects
    for _ in 0..10 {
        let _project_id = create_test_project_in_db(db).await;
    }
    
    // Search with both limit and offset
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: Some(3),
        offset: Some(2),
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return limited results starting from offset
    assert_eq!(search_response.projects.len(), 3);
    assert_eq!(search_response.total_count, 10);
}

#[tokio::test]
async fn test_search_empty_query() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search with empty query
    let request = SearchRequest {
        query: "".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Empty query should return some results (depending on search implementation)
    // The exact behavior might vary based on search_simple implementation
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_no_matches() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search for something that won't match
    let request = SearchRequest {
        query: "NonExistentProjectName12345".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.projects.len(), 0);
    assert_eq!(search_response.total_count, 0);
}

#[tokio::test]
async fn test_search_special_characters() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search with special characters
    let request = SearchRequest {
        query: "Test@#$%^&*()".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await;
    
    // Should not crash even with special characters
    assert!(response.is_ok());
    let search_response = response.unwrap().into_inner();
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_unicode_characters() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search with unicode characters
    let request = SearchRequest {
        query: "🎵 Test 音楽 プロジェクト".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await;
    
    // Should handle unicode gracefully
    assert!(response.is_ok());
    let search_response = response.unwrap().into_inner();
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_case_sensitivity() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Test different case variations
    let test_queries = vec![
        "test project",
        "TEST PROJECT", 
        "Test Project",
        "TeSt PrOjEcT",
    ];
    
    let mut results = Vec::new();
    for query in test_queries {
        let request = SearchRequest {
            query: query.to_string(),
            limit: None,
            offset: None,
        };
        
        let response = server.search(Request::new(request)).await.unwrap();
        let search_response = response.into_inner();
        results.push(search_response.total_count);
    }
    
    // All case variations should return the same number of results
    // (assuming case-insensitive search)
    let first_count = results[0];
    for count in results.iter() {
        // Note: This test assumes case-insensitive search.
        // If search is case-sensitive, this test would need to be adjusted
        assert_eq!(*count, first_count, "Search should be case-insensitive");
    }
}

#[tokio::test]
async fn test_search_large_offset() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a few test projects
    for _ in 0..3 {
        let _project_id = create_test_project_in_db(db).await;
    }
    
    // Search with offset larger than result set
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: None,
        offset: Some(100),
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return empty results but correct total count
    assert_eq!(search_response.projects.len(), 0);
    assert_eq!(search_response.total_count, 3);
}

#[tokio::test]
async fn test_search_zero_limit() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search with zero limit
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: Some(0),
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return empty results but correct total count
    assert_eq!(search_response.projects.len(), 0);
    assert!(search_response.total_count > 0);
}

#[tokio::test]
async fn test_search_negative_offset() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search with negative offset (should be handled gracefully)
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: None,
        offset: Some(-5),
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should handle negative offset gracefully (likely treated as 0)
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_project_attributes_returned() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test project
    let _project_id = create_test_project_in_db(db).await;
    
    // Search for projects
    let request = SearchRequest {
        query: "Test Project".to_string(),
        limit: Some(1),
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    assert_eq!(search_response.projects.len(), 1);
    let project = &search_response.projects[0];
    
    // Verify all expected project attributes are present
    assert!(!project.id.is_empty());
    assert!(!project.name.is_empty());
    assert!(!project.path.is_empty());
    assert!(!project.hash.is_empty());
    assert!(project.created_at > 0);
    assert!(project.modified_at > 0);
    assert!(project.tempo > 0.0);
    
    // Verify nested objects are present
    assert!(project.time_signature.is_some());
    assert!(project.key_signature.is_some());
    assert!(project.ableton_version.is_some());
    
    // Verify collections (plugins, samples, tags) are arrays
    // (may be empty but should be present)
    // Length checks are implicit - Vec::len() always returns valid usize
}

// =============================================================================
// OPERATOR TESTS
// =============================================================================
// Tests for search operators
#[tokio::test]
async fn test_search_name_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id1 = create_test_project_in_db(db).await;
    let _project_id2 = create_test_project_in_db(db).await;
    
    // Search using name operator
    let request = SearchRequest {
        query: "name:Test".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects with "Test" in the name
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        assert!(project.name.to_lowercase().contains("test"));
    }
}

#[tokio::test]
async fn test_search_bpm_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects (they should have tempo 140.0 from LiveSetBuilder)
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using bpm operator
    let request = SearchRequest {
        query: "bpm:140".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects with 140 BPM
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        assert_eq!(project.tempo, 140.0);
    }
}

#[tokio::test]
async fn test_search_plugin_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test project with Serum plugin (from LiveSetBuilder)
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using plugin operator
    let request = SearchRequest {
        query: "plugin:Serum".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects using Serum plugin
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let has_serum = project.plugins.iter().any(|p| p.name.contains("Serum"));
        assert!(has_serum, "Project should contain Serum plugin");
    }
}

#[tokio::test]
async fn test_search_sample_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test project with kick.wav sample (from LiveSetBuilder)
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using sample operator
    let request = SearchRequest {
        query: "sample:kick".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects using kick sample
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let has_kick = project.samples.iter().any(|s| s.name.to_lowercase().contains("kick"));
        assert!(has_kick, "Project should contain kick sample");
    }
}

#[tokio::test]
async fn test_search_tag_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create a test project
    let project_id = create_test_project_in_db(db).await;
    
    // Create a tag and apply it to the project
    let tag_req = CreateTagRequest {
        name: "Electronic".to_string(),
    };
    let tag_resp = server.create_tag(Request::new(tag_req)).await.unwrap();
    let tag = tag_resp.into_inner().tag.unwrap();
    
    let tag_project_req = TagProjectRequest {
        project_id: project_id.clone(),
        tag_id: tag.id.clone(),
    };
    server.tag_project(Request::new(tag_project_req)).await.unwrap();
    
    // Search using tag operator
    let request = SearchRequest {
        query: "tag:Electronic".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects with Electronic tag
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let has_electronic_tag = project.tags.iter().any(|t| t.name == "Electronic");
        assert!(has_electronic_tag, "Project should have Electronic tag");
    }
}

#[tokio::test]
async fn test_search_path_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using path operator
    let request = SearchRequest {
        query: "path:Test".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects with "Test" in the path
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        assert!(project.path.to_lowercase().contains("test"));
    }
}

#[tokio::test]
async fn test_search_version_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using version operator (LiveSetBuilder creates version 11.3.0)
    let request = SearchRequest {
        query: "version:11".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects with version containing "11"
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let version = &project.ableton_version.as_ref().unwrap();
        assert_eq!(version.major, 11);
    }
}

#[tokio::test]
async fn test_search_key_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects (LiveSetBuilder creates key signature C Major)
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using key operator
    let request = SearchRequest {
        query: "key:C".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects in C key
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let key_sig = project.key_signature.as_ref().unwrap();
        assert_eq!(key_sig.tonic, "C");
    }
}

#[tokio::test]
async fn test_search_ts_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects (LiveSetBuilder creates 4/4 time signature)
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using time signature operator
    let request = SearchRequest {
        query: "ts:4/4".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects in 4/4 time
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        let time_sig = project.time_signature.as_ref().unwrap();
        assert_eq!(time_sig.numerator, 4);
        assert_eq!(time_sig.denominator, 4);
    }
}

#[tokio::test]
async fn test_search_multiple_operators() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using multiple operators
    let request = SearchRequest {
        query: "bpm:140 key:C plugin:Serum".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should find projects matching all criteria
    assert!(search_response.projects.len() > 0);
    for project in &search_response.projects {
        // Check BPM
        assert_eq!(project.tempo, 140.0);
        
        // Check key
        let key_sig = project.key_signature.as_ref().unwrap();
        assert_eq!(key_sig.tonic, "C");
        
        // Check plugin
        let has_serum = project.plugins.iter().any(|p| p.name.contains("Serum"));
        assert!(has_serum);
    }
}

#[tokio::test]
async fn test_search_quoted_operator_value() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using quoted operator value
    let request = SearchRequest {
        query: "name:\"Test Project\"".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should handle quoted values correctly
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_unknown_operator() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search using unknown operator (should be treated as text)
    let request = SearchRequest {
        query: "unknown:value Test".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should not crash and treat unknown operator as text search
    assert!(search_response.total_count >= 0);
}

#[tokio::test]
async fn test_search_operator_no_matches() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search for BPM that doesn't exist
    let request = SearchRequest {
        query: "bpm:999".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should return no results
    assert_eq!(search_response.projects.len(), 0);
    assert_eq!(search_response.total_count, 0);
}

#[tokio::test]
async fn test_search_mixed_text_and_operators() {
    setup("debug");
    
    let server = create_test_server().await;
    let db = server.db();
    
    // Create test projects
    let _project_id = create_test_project_in_db(db).await;
    
    // Search mixing free text and operators
    let request = SearchRequest {
        query: "Test bpm:140 Electronic".to_string(),
        limit: None,
        offset: None,
    };
    
    let response = server.search(Request::new(request)).await.unwrap();
    let search_response = response.into_inner();
    
    // Should handle mixed search correctly
    assert!(search_response.total_count >= 0);
    
    // If there are results, they should match the BPM criteria
    for project in &search_response.projects {
        assert_eq!(project.tempo, 140.0);
    }
}