use log::info;
use studio_project_manager::grpc;
use studio_project_manager::config::CONFIG;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    info!("Starting Studio Project Manager gRPC Server");
    
    // Load configuration
    let config = CONFIG.as_ref().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;
    
    // Create the gRPC server
    let server = grpc::server::StudioProjectManagerServer::new().await?;
    
    // Set up the gRPC service
    let addr = format!("127.0.0.1:{}", config.grpc_port).parse()?;
    info!("gRPC server listening on {}", addr);
    
    // Start the server
    tonic::transport::Server::builder()
        .add_service(grpc::proto::studio_project_manager_server::StudioProjectManagerServer::new(server))
        .serve(addr)
        .await?;
    
    Ok(())
}
