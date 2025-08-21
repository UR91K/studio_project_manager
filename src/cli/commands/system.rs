use crate::cli::commands::{CliCommand, CliContext};
use crate::cli::{SystemCommands, WatchAction};
use crate::cli::CliError;
use colored::Colorize;
use comfy_table::Table;
use std::path::PathBuf;

#[async_trait::async_trait]
impl CliCommand for SystemCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<(), CliError> {
        match self {
            SystemCommands::Info => self.show_info(ctx).await,
            SystemCommands::Stats => self.show_stats(ctx).await,
            SystemCommands::Export { format, output } => self.export_data(ctx, format, output).await,
            SystemCommands::Watch { action } => self.handle_watch(ctx, action).await,
            SystemCommands::ScanStatus => self.show_scan_status(ctx).await,
        }
    }
}

impl SystemCommands {
    async fn show_info(&self, ctx: &CliContext) -> Result<(), CliError> {
        println!("{}", "System Information".bold().underline());

        let mut table = Table::new();
        table
            .set_header(vec!["Property", "Value"])
            .load_preset(comfy_table::presets::UTF8_FULL);

        // Add configuration info
        table.add_row(vec![
            "Database Path",
            &ctx.config.database_path.clone().unwrap_or_else(|| "<default>".to_string()),
        ]);

        table.add_row(vec![
            "Project Paths",
            &ctx.config.paths.len().to_string(),
        ]);

        table.add_row(vec![
            "gRPC Port",
            &ctx.config.grpc_port.to_string(),
        ]);

        table.add_row(vec![
            "Log Level",
            &ctx.config.log_level,
        ]);

        println!("{}", table);
        Ok(())
    }

    async fn show_stats(&self, ctx: &CliContext) -> Result<(), CliError> {
        println!("{}", "System Statistics".bold().underline());

        let db = ctx.db.lock().await;

        let mut table = Table::new();
        table
            .set_header(vec!["Statistic", "Value"])
            .load_preset(comfy_table::presets::UTF8_FULL);

        let (projects, _plugins, samples, collections, tags, tasks) =
            db.get_basic_counts().unwrap_or((0, 0, 0, 0, 0, 0));
        table.add_row(vec!["Total Projects", &projects.to_string()]);
        table.add_row(vec!["Total Samples", &samples.to_string()]);
        table.add_row(vec!["Total Collections", &collections.to_string()]);
        table.add_row(vec!["Total Tags", &tags.to_string()]);
        table.add_row(vec!["Total Tasks", &tasks.to_string()]);

        // Add more statistics as needed
        table.add_row(vec![
            "Database Path",
            &ctx.config.database_path.clone().unwrap_or_else(|| "<default>".to_string()),
        ]);

        println!("{}", table);
        Ok(())
    }

    async fn export_data(&self, ctx: &CliContext, format: &str, output: &PathBuf) -> Result<(), CliError> {
        println!("{}", format!("Exporting system data to {} format...", format).bold());

        // For now, just export basic statistics
        let db = ctx.db.lock().await;
        let (projects, _plugins, samples, _collections, _tags, _tasks) =
            db.get_basic_counts().unwrap_or((0, 0, 0, 0, 0, 0));

        let data = format!("Projects: {}, Samples: {}", projects, samples);

        std::fs::write(output, &data)?;

        println!("{}", format!("Data exported to: {}", output.display()).green());
        Ok(())
    }

    async fn handle_watch(&self, _ctx: &CliContext, action: &WatchAction) -> Result<(), CliError> {
        match action {
            WatchAction::Start => {
                println!("{}", "Starting file watcher...".bold());
                println!("{}", "File watcher functionality not yet implemented in CLI mode".yellow());
            }
            WatchAction::Stop => {
                println!("{}", "Stopping file watcher...".bold());
                println!("{}", "File watcher functionality not yet implemented in CLI mode".yellow());
            }
        }
        Ok(())
    }

    async fn show_scan_status(&self, _ctx: &CliContext) -> Result<(), CliError> {
        println!("{}", "Scan Status".bold().underline());
        println!("{}", "Scanning functionality not yet implemented in CLI mode".yellow());
        Ok(())
    }
}
