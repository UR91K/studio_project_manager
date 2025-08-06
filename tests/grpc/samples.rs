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