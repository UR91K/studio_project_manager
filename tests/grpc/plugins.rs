//! Plugin service tests

use crate::grpc::*;

#[tokio::test]
async fn test_refresh_plugin_installation_status() {
    let server= create_test_server().await;
    
    // Test the new plugin refresh endpoint
    let request = Request::new(RefreshPluginInstallationStatusRequest {});
    let response = server.refresh_plugin_installation_status(request).await;
    
    assert!(response.is_ok());
    let response = response.unwrap().into_inner();
    
    // Should return success
    assert!(response.success);
    assert!(response.error_message.is_none());
    
    // Should have checked some plugins (even if 0 in test database)
    assert!(response.total_plugins_checked >= 0);
    assert!(response.plugins_now_installed >= 0);
    assert!(response.plugins_now_missing >= 0);
    assert!(response.plugins_unchanged >= 0);
    
    // Total should add up
    assert_eq!(
        response.total_plugins_checked,
        response.plugins_now_installed + response.plugins_now_missing + response.plugins_unchanged
    );
} 