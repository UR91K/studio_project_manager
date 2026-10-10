use clap::Parser;
use seula::cli::{Cli, Commands};
use seula::config::CONFIG;
use seula::database::ProjectDatabase;
use seula::http;
use seula::media::{MediaConfig, MediaStorageManager};
use seula::services::ScanStatus;
use seula::services::{Services, SystemService};
use seula::tray;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parsed before CONFIG is first touched, because `--config` has to be in place by
    // the time the lazy static loads.
    let cli = Cli::parse();

    if let Some(path) = &cli.config {
        // `find_config_file` quietly falls through when SEULA_CONFIG names a missing
        // file. That suits an ambient env var, but an explicit flag that is silently
        // ignored is how this flag went unnoticed, so a bad path is an error here.
        if !path.is_file() {
            eprintln!("Config file not found: {}", path.display());
            std::process::exit(2);
        }
        std::env::set_var("SEULA_CONFIG", path);
    }

    let config = CONFIG.as_ref().inspect_err(|&e| {
        eprintln!("Failed to load configuration: {}", e);
    })?;

    // Only the daemon writes files; the CLI commands print to the terminal (ADR-0071).
    let daemon = cli.command.is_none() && !cli.cli;
    let _log_guard = seula::logging::init(&config.log_level, daemon);

    match &cli.command {
        Some(command) => run_direct_command(command, cli.format, cli.no_color).await,
        None => {
            if cli.cli {
                run_interactive_cli(cli.format, cli.no_color).await
            } else if cli.server {
                run_server_mode().await
            } else {
                run_tray_mode().await
            }
        }
    }
}

/// The state the HTTP router runs on, built once so everything shares the same database
/// connection and the same in-memory `SystemService` state (scan status, watcher handle)
/// rather than opening its own -- see ADR-0024.
struct SharedState {
    services: Services,
    system_service: SystemService,
}

fn build_shared_state() -> Result<SharedState, Box<dyn std::error::Error>> {
    let config = CONFIG
        .as_ref()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    let database_path = config
        .database_path
        .as_ref()
        .expect("Database path should be set by config initialization");
    let db_path = PathBuf::from(database_path);
    let db = ProjectDatabase::new(db_path)
        .map_err(|e| format!("Failed to initialize database: {}", e))?;
    let db = Arc::new(Mutex::new(db));

    let media_config = MediaConfig::from(config);
    let media_storage = Arc::new(MediaStorageManager::new(
        PathBuf::from(&config.media_storage_dir),
        media_config,
    )?);

    let scan_status = Arc::new(Mutex::new(ScanStatus::Unknown));
    let scan_progress = Arc::new(Mutex::new(None));
    let watcher = Arc::new(Mutex::new(None));
    let watcher_events = Arc::new(Mutex::new(None));
    let start_time = Instant::now();
    let services = Services::new(Arc::clone(&db), Arc::clone(&media_storage));
    let system_service = SystemService::new(
        Arc::clone(&db),
        scan_status,
        scan_progress,
        watcher,
        watcher_events,
        start_time,
    );

    Ok(SharedState {
        services,
        system_service,
    })
}

async fn run_server_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Seula (server-only mode)");

    let state = build_shared_state()?;
    start_http_server(state.services, state.system_service).await
}

async fn run_tray_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Seula (tray mode)");

    let state = build_shared_state()?;

    // The HTTP server runs in a background task while the tray owns this thread.
    let http_handle = tokio::spawn(async move {
        if let Err(e) = start_http_server(state.services, state.system_service).await {
            tracing::error!("HTTP server error: {}", e);
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
            tracing::error!("Tray app error: {}", e);
        }
        Err(e) => {
            tracing::error!("Failed to run tray app: {}", e);
        }
    }

    // If we get here, the user quit the tray, so stop the server
    http_handle.abort();

    Ok(())
}

async fn start_http_server(
    services: Services,
    system_service: SystemService,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = CONFIG.as_ref().inspect_err(|&e| {
        eprintln!("Failed to load configuration: {}", e);
    })?;

    let state = http::state::AppState::new(services, system_service);
    let router = http::server::build_router(state);

    let addr = format!("127.0.0.1:{}", config.http_port()).parse::<std::net::SocketAddr>()?;
    info!("HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

async fn run_interactive_cli(
    format: seula::cli::OutputFormat,
    no_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Seula Interactive CLI");

    let mut interactive = seula::cli::InteractiveCli::new(format, no_color)
        .await
        .map_err(|e| e as Box<dyn std::error::Error>)?;
    interactive
        .run()
        .await
        .map_err(|e| e as Box<dyn std::error::Error>)?;

    Ok(())
}

async fn run_direct_command(
    command: &Commands,
    format: seula::cli::OutputFormat,
    no_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Executing direct CLI command");

    use seula::cli::commands::{execute_command, ScanCommand, SearchCommand};

    let result: Result<(), Box<dyn std::error::Error>> = match command {
        Commands::Scan { paths, force } => {
            let scan_cmd = ScanCommand {
                paths: paths.clone(),
                force: *force,
            };
            execute_command(&scan_cmd, format, no_color)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Search {
            query,
            limit,
            offset,
        } => {
            let search_cmd = SearchCommand {
                query: query.clone(),
                limit: *limit,
                offset: *offset,
            };
            execute_command(&search_cmd, format, no_color)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Project { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Sample { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Collection { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Tag { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Task { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Plugin { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::System { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
        Commands::Config { subcommand } => {
            use seula::cli::commands::CliCommand;
            let ctx = seula::cli::commands::CliContext::new(format, no_color)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e })?;
            subcommand
                .execute(&ctx)
                .await
                .map_err(|e| e as Box<dyn std::error::Error>)
        }
    };

    result
}
