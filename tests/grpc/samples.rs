//! Sample service tests

use crate::grpc::*;

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