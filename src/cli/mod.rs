pub mod commands;
pub mod interactive;
pub mod output;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A lightweight error type for CLI commands
pub type CliError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Seula - Interactive CLI for Ableton Live Project Management
#[derive(Parser)]
#[command(
    name = "seula",
    about = "A high-performance CLI for indexing and searching Ableton Live projects",
    version,
    long_about = r#"
Seula - Ableton Live Project Manager

A comprehensive CLI tool for managing Ableton Live projects with powerful search,
scanning, and organization capabilities.

MODES:
- Interactive mode: seula --cli (default when no subcommands provided)
- Direct commands: seula <COMMAND> [SUBCOMMAND] [ARGS...]

EXAMPLES:
  seula --cli                    # Start interactive mode
  seula scan ~/Music/Projects    # Scan specific directory
  seula project list             # List all projects
  seula search "bpm:128 plugin:serum"  # Search with filters
  seula system info              # Show system information
"#
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Run in interactive CLI mode
    #[arg(long)]
    pub cli: bool,

    /// Output format (table, json, csv)
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Config file path
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    /// Human-readable table format
    Table,
    /// JSON format
    Json,
    /// CSV format
    Csv,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan directories for Ableton Live projects
    Scan {
        /// Directories to scan (uses config paths if not specified)
        paths: Vec<PathBuf>,

        /// Force rescan of all projects (ignore timestamps)
        #[arg(long)]
        force: bool,
    },

    /// Search projects using full-text search
    Search {
        /// Search query with optional operators (plugin:, bpm:, key:, etc.)
        query: String,

        /// Limit number of results
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Project management commands
    Project {
        #[command(subcommand)]
        subcommand: ProjectCommands,
    },

    /// Sample management commands
    Sample {
        #[command(subcommand)]
        subcommand: SampleCommands,
    },

    /// Collection management commands
    Collection {
        #[command(subcommand)]
        subcommand: CollectionCommands,
    },

    /// Tag management commands
    Tag {
        #[command(subcommand)]
        subcommand: TagCommands,
    },

    /// Task management commands
    Task {
        #[command(subcommand)]
        subcommand: TaskCommands,
    },

    /// System operations and information
    System {
        #[command(subcommand)]
        subcommand: SystemCommands,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommands,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// List all projects
    List {
        /// Show deleted projects
        #[arg(long)]
        deleted: bool,

        /// Limit number of results
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Show detailed information about a specific project
    Show {
        /// Project ID
        id: String,
    },

    /// Update project information
    Update {
        /// Project ID
        id: String,

        /// New project name
        #[arg(long)]
        name: Option<String>,

        /// New project notes
        #[arg(long)]
        notes: Option<String>,
    },

    /// Mark project as deleted
    Delete {
        /// Project ID
        id: String,
    },

    /// Restore deleted project
    Restore {
        /// Project ID
        id: String,
    },

    /// Rescan a specific project
    Rescan {
        /// Project ID
        id: String,
    },

    /// Show project statistics
    Stats,
}

#[derive(Subcommand)]
pub enum SampleCommands {
    /// List all samples
    List {
        /// Limit number of results
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Offset for pagination
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Search samples
    Search {
        /// Search query
        query: String,

        /// Limit number of results
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Show sample statistics
    Stats,

    /// Check sample presence on filesystem
    CheckPresence,
}

#[derive(Subcommand)]
pub enum CollectionCommands {
    /// List all collections
    List,

    /// Show collection details
    Show {
        /// Collection ID
        id: String,
    },

    /// Create a new collection
    Create {
        /// Collection name
        name: String,

        /// Collection description
        #[arg(long)]
        description: Option<String>,
    },

    /// Add project to collection
    Add {
        /// Collection ID
        collection_id: String,

        /// Project ID
        project_id: String,
    },

    /// Remove project from collection
    Remove {
        /// Collection ID
        collection_id: String,

        /// Project ID
        project_id: String,
    },
}

#[derive(Subcommand)]
pub enum TagCommands {
    /// List all tags
    List,

    /// Create a new tag
    Create {
        /// Tag name
        name: String,

        /// Tag color (hex format)
        #[arg(long)]
        color: Option<String>,
    },

    /// Assign tag to project
    Assign {
        /// Project ID
        project_id: String,

        /// Tag ID
        tag_id: String,
    },

    /// Remove tag from project
    Remove {
        /// Project ID
        project_id: String,

        /// Tag ID
        tag_id: String,
    },

    /// Search projects by tag
    Search {
        /// Tag name or ID
        tag: String,
    },
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List tasks (all or for specific project)
    List {
        /// Project ID (optional - shows all tasks if not specified)
        project_id: Option<String>,

        /// Show completed tasks
        #[arg(long)]
        completed: bool,
    },

    /// Create a new task
    Create {
        /// Project ID
        project_id: String,

        /// Task description
        description: String,

        /// Task priority (1-5, where 5 is highest)
        #[arg(long, default_value = "3")]
        priority: u8,
    },

    /// Complete a task
    Complete {
        /// Task ID
        id: String,
    },

    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
}

#[derive(Subcommand)]
pub enum SystemCommands {
    /// Show system information
    Info,

    /// Show system statistics
    Stats,

    /// Export system data
    Export {
        /// Export format (json, csv)
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file path
        #[arg(long)]
        output: PathBuf,
    },

    /// Start/stop file watcher
    Watch {
        /// Action to perform (start or stop)
        action: WatchAction,
    },

    /// Show scan status
    ScanStatus,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum WatchAction {
    Start,
    Stop,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Validate configuration
    Validate,

    /// Edit configuration
    Edit,
}

pub use commands::*;
pub use interactive::*;
pub use output::*;
