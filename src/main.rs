use log::info;
use std::env;
use studio_project_manager::config::CONFIG;
use studio_project_manager::{grpc, tray};
use studio_project_manager::grpc::projects::project_service_server;
use studio_project_manager::grpc::search::search_service_server;
use studio_project_manager::grpc::collections::collection_service_server;
use studio_project_manager::grpc::tags::tag_service_server;
use studio_project_manager::grpc::tasks::task_service_server;
use studio_project_manager::grpc::media::media_service_server;
use studio_project_manager::grpc::system::system_service_server;
use studio_project_manager::grpc::plugins::plugin_service_server;
use studio_project_manager::grpc::samples::sample_service_server;
use studio_project_manager::grpc::scanning::scanning_service_server;
use studio_project_manager::grpc::watcher::watcher_service_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration first
    let config = CONFIG.as_ref().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    // Initialize logging with configured level
    init_logging(&config.log_level);

    // Check command line arguments
    let args: Vec<String> = env::args().collect();
    let run_as_cli = args.contains(&"--cli".to_string()) || args.contains(&"-c".to_string());
    let run_as_server = args.contains(&"--server".to_string()) || args.contains(&"-s".to_string());

    if run_as_cli {
        // Run as CLI (original behavior)
        run_cli_mode().await
    } else if run_as_server {
        // Run as server-only mode (for TUI clients)
        run_server_mode().await
    } else {
        // Run as tray application (default behavior)
        run_tray_mode().await
    }
}

async fn run_cli_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Studio Project Manager gRPC Server (CLI mode)");
    start_grpc_server().await
}

async fn run_server_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Studio Project Manager gRPC Server (server-only mode)");
    start_grpc_server().await
}

async fn run_tray_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Studio Project Manager gRPC Server (tray mode)");

    // Start the gRPC server in a background task
    let server_handle = tokio::spawn(async {
        if let Err(e) = start_grpc_server().await {
            eprintln!("gRPC server error: {}", e);
        }
    });

    // Create and run the tray app in a blocking task
    let tray_result = tokio::task::spawn_blocking(move || {
        let tray_app = tray::TrayApp::new()?;
        tray_app.run()
    })
    .await;

    match tray_result {
        Ok(Ok(_)) => {
            info!("Tray app exited normally");
        }
        Ok(Err(e)) => {
            eprintln!("Tray app error: {}", e);
        }
        Err(e) => {
            eprintln!("Failed to run tray app: {}", e);
        }
    }

    // If we get here, the user quit the tray, so abort the server
    server_handle.abort();

    Ok(())
}

async fn start_grpc_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = CONFIG.as_ref().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    // Create the gRPC server
    let server = grpc::server::StudioProjectManagerServer::new().await?;

    // Set up the gRPC service
    let addr = format!("127.0.0.1:{}", config.grpc_port).parse()?;
    info!("gRPC server listening on {}", addr);

    // Start the server with all services
    tonic::transport::Server::builder()
        .add_service(project_service_server::ProjectServiceServer::new(server.clone()))
        .add_service(search_service_server::SearchServiceServer::new(server.clone()))
        .add_service(collection_service_server::CollectionServiceServer::new(server.clone()))
        .add_service(tag_service_server::TagServiceServer::new(server.clone()))
        .add_service(task_service_server::TaskServiceServer::new(server.clone()))
        .add_service(media_service_server::MediaServiceServer::new(server.clone()))
        .add_service(system_service_server::SystemServiceServer::new(server.clone()))
        .add_service(plugin_service_server::PluginServiceServer::new(server.clone()))
        .add_service(sample_service_server::SampleServiceServer::new(server.clone()))
        .add_service(scanning_service_server::ScanningServiceServer::new(server.clone()))
        .add_service(watcher_service_server::WatcherServiceServer::new(server))
        .serve(addr)
        .await?;

    Ok(())
}

fn init_logging(log_level: &str) {
    let level = match log_level.to_lowercase().as_str() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => {
            eprintln!("Invalid log level '{}', defaulting to 'info'", log_level);
            log::LevelFilter::Info
        }
    };

    env_logger::Builder::from_default_env()
        .filter_level(level)
        .init();
}
