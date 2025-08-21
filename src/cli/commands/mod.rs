#![allow(unused_imports)]
pub mod project;
pub mod sample;
pub mod collection;
pub mod tag;
pub mod task;
pub mod system;
pub mod config;
pub mod search;
pub mod scan;

use crate::grpc::handlers::*;
use crate::config::CONFIG;
use crate::database::LiveSetDatabase;
use crate::cli::CliError;
use std::sync::Arc;
use tokio::sync::Mutex;

/// CLI command execution context
pub struct CliContext {
    pub db: Arc<Mutex<LiveSetDatabase>>,
    pub config: &'static crate::config::Config,
    pub output_format: crate::cli::OutputFormat,
    pub no_color: bool,
}

impl CliContext {
    pub async fn new(output_format: crate::cli::OutputFormat, no_color: bool) -> Result<Self, CliError> {
        let config = CONFIG.as_ref()?;
        let db_path = std::path::PathBuf::from(
            config.database_path.clone().expect("Database path must be set by config initialization"),
        );
        let db = Arc::new(Mutex::new(LiveSetDatabase::new(db_path)?));

        Ok(Self {
            db,
            config,
            output_format,
            no_color,
        })
    }
}

/// Trait for CLI command execution
#[async_trait::async_trait]
pub trait CliCommand {
    async fn execute(&self, ctx: &CliContext) -> Result<(), CliError>;
}

/// Execute a CLI command with proper error handling
pub async fn execute_command(command: &impl CliCommand, output_format: crate::cli::OutputFormat, no_color: bool) -> Result<(), CliError> {
    let ctx = CliContext::new(output_format, no_color).await?;
    command.execute(&ctx).await
}

/// Helper to create database connection for CLI commands
pub async fn create_db_connection() -> Result<Arc<Mutex<LiveSetDatabase>>, CliError> {
    let config = CONFIG.as_ref()?;
    let db_path = std::path::PathBuf::from(
        config.database_path.clone().expect("Database path must be set by config initialization"),
    );
    let db = LiveSetDatabase::new(db_path)?;
    let db = Arc::new(Mutex::new(db));
    Ok(db)
}

pub use project::*;
pub use sample::*;
pub use collection::*;
pub use tag::*;
pub use task::*;
pub use system::*;
pub use config::*;
pub use search::*;
pub use scan::*;
