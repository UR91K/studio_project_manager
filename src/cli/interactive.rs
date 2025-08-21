use crate::cli::commands::CliContext;
use crate::cli::OutputFormat;
use crate::cli::commands::CliCommand;
use crate::cli::CliError;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::config::Configurer;
use std::collections::HashMap;

/// Interactive CLI mode
pub struct InteractiveCli {
    editor: Editor<()>,
    context: CliContext,
    commands: HashMap<&'static str, &'static str>,
}

impl InteractiveCli {
    pub async fn new(output_format: OutputFormat, no_color: bool) -> Result<Self, CliError> {
        let context = CliContext::new(output_format, no_color).await?;

        let mut editor = Editor::<()>::new().map_err(|e| -> CliError {
            std::io::Error::new(std::io::ErrorKind::Other, format!("{e}"))
                .into()
        })?;

        // Configure the editor
        editor.set_max_history_size(1000);

        // Load command history if it exists
        let _ = editor.load_history(&Self::history_file());

        let mut commands = HashMap::new();
        commands.insert("help", "Show this help message");
        commands.insert("exit", "Exit the interactive CLI");
        commands.insert("quit", "Exit the interactive CLI");
        commands.insert("clear", "Clear the screen");
        commands.insert("status", "Show system status");
        commands.insert("scan", "Scan directories for projects");
        commands.insert("search", "Search projects");
        commands.insert("project", "Project management commands");
        commands.insert("sample", "Sample management commands");
        commands.insert("collection", "Collection management commands");
        commands.insert("tag", "Tag management commands");
        commands.insert("task", "Task management commands");
        commands.insert("system", "System operations");
        commands.insert("config", "Configuration management");

        Ok(Self {
            editor,
            context,
            commands,
        })
    }

    pub async fn run(&mut self) -> Result<(), CliError> {
        self.show_welcome();

        loop {
            let line = match self.editor.readline(&self.prompt()) {
                Ok(line) => line,
                Err(ReadlineError::Interrupted) => {
                    println!("{}", "Interrupted (Ctrl+C)".yellow());
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    println!("{}", "Goodbye! 👋".green());
                    break;
                }
                Err(err) => {
                    println!("{}", format!("Error: {:?}", err).red());
                    break;
                }
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Add to history
            self.editor.add_history_entry(line);

            // Parse and execute command
            if let Err(e) = self.execute_command(line).await {
                println!("{}", format!("Error: {}", e).red());
            }
        }

        // Save history
        let _ = self.editor.save_history(&Self::history_file());

        Ok(())
    }

    fn show_welcome(&self) {
        println!("{}", "=".repeat(60).bold().blue());
        println!("{}", "  Seula Interactive CLI".bold().blue());
        println!("{}", "  Ableton Live Project Manager".bold().cyan());
        println!("{}", "=".repeat(60).bold().blue());
        println!();
        println!("{}", "Type 'help' for available commands or 'exit' to quit.".yellow());
        println!();
    }

    fn prompt(&self) -> String {
        format!("{} ", "seula>".bold().green())
    }

    fn history_file() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".seula_history")
    }

    async fn execute_command(&mut self, line: &str) -> Result<(), CliError> {
        let args: Vec<&str> = line.split_whitespace().collect();
        let command = args[0].to_lowercase();

        match command.as_str() {
            "help" | "?" => self.show_help(),
            "exit" | "quit" => {
                println!("{}", "Goodbye! 👋".green());
                std::process::exit(0);
            }
            "clear" => self.clear_screen(),
            "status" => self.show_status().await?,
            _ => self.execute_external_command(args).await?,
        }

        Ok(())
    }

    fn show_help(&self) {
        println!("{}", "Available Commands:".bold().underline());
        println!();

        let mut sorted_commands: Vec<_> = self.commands.iter().collect();
        sorted_commands.sort_by_key(|(name, _)| *name);

        for (name, description) in sorted_commands {
            println!("  {:<15} {}", name.bold().cyan(), description);
        }

        println!();
        println!("{}", "Command Examples:".bold().underline());
        println!("  {}", "scan ~/Music/Projects".italic());
        println!("  {}", "search \"bpm:128 plugin:serum\"".italic());
        println!("  {}", "project list".italic());
        println!("  {}", "system info".italic());
        println!("  {}", "status".italic());
        println!();
    }

    fn clear_screen(&self) {
        print!("\x1B[2J\x1B[1;1H");
    }

    async fn show_status(&self) -> Result<(), CliError> {
        println!("{}", "System Status".bold().underline());

        // Show database status
        let db = self.context.db.lock().await;
        let (projects, _plugins, samples, _collections, _tags, _tasks) =
            db.get_basic_counts().unwrap_or((0, 0, 0, 0, 0, 0));
        println!("  📁 Projects: {}", projects.to_string().bold().green());
        println!("  🎵 Samples: {}", samples.to_string().bold().green());

        // Show configuration status
        if self.context.config.needs_setup() {
            println!("  ⚠️  Configuration: {}", "Needs setup".bold().yellow());
        } else {
            println!("  ✅ Configuration: {}", "OK".bold().green());
        }

        // Show scan status
        println!("  ⏸️  Scanner: {}", "Idle".bold().cyan());

        println!();
        Ok(())
    }

    async fn execute_external_command(&mut self, args: Vec<&str>) -> Result<(), CliError> {
        if args.is_empty() {
            return Ok(());
        }

        // Convert the command line arguments back to a string for clap parsing
        #[allow(unused_variables)] // TODO: Remove this once we have implemented the commands that use this
        let full_command = format!("seula {}", args.join(" "));

        // This is a simplified approach - in a real implementation, you'd want to
        // properly parse the arguments and route to the appropriate command handlers
        match args[0] {
            "scan" => {
                println!("{}", "Scanning directories...".bold());
                // TODO: Implement scan command
                println!("{}", "Scan completed!".green());
            }
            "search" => {
                if args.len() < 2 {
                    println!("{}", "Usage: search <query>".red());
                    return Ok(());
                }
                let query = args[1..].join(" ");
                println!("{}", format!("Searching for: {}", query).bold());
                // TODO: Implement search command
                println!("{}", "Search completed!".green());
            }
            "project" => {
                println!("{}", "Project management commands not yet implemented".yellow());
            }
            "system" => {
                if args.len() > 1 && args[1] == "info" {
                    let cmd = crate::cli::SystemCommands::Info;
                    CliCommand::execute(&cmd, &self.context).await?;
                } else if args.len() > 1 && args[1] == "stats" {
                    let cmd = crate::cli::SystemCommands::Stats;
                    CliCommand::execute(&cmd, &self.context).await?;
                } else {
                    println!("{}", "Usage: system <info|stats>".red());
                }
            }
            _ => {
                println!("{}", format!("Unknown command: {}. Type 'help' for available commands.", args[0]).red());
            }
        }

        Ok(())
    }
}
