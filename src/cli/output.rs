use crate::cli::OutputFormat;
use crate::cli::CliError;
use colored::Colorize;
use comfy_table::{Table, presets};
use serde::Serialize;

/// Output formatter for CLI results
pub struct OutputFormatter {
    format: OutputFormat,
    no_color: bool,
}

impl OutputFormatter {
    pub fn new(format: OutputFormat, no_color: bool) -> Self {
        Self { format, no_color }
    }

    /// Format and print data
    pub fn print<T: Serialize + TableDisplay>(&self, data: &T) -> Result<(), CliError> {
        match self.format {
            OutputFormat::Table => self.print_table(data),
            OutputFormat::Json => self.print_json(data),
            OutputFormat::Csv => self.print_csv(data),
        }
    }

    /// Print a message with appropriate formatting
    pub fn print_message(&self, message: &str, message_type: MessageType) {
        let formatted_message = match message_type {
            MessageType::Info => message.blue(),
            MessageType::Success => message.green(),
            MessageType::Warning => message.yellow(),
            MessageType::Error => message.red(),
        };

        if self.no_color {
            println!("{}", message);
        } else {
            println!("{}", formatted_message);
        }
    }

    /// Create a new table with consistent styling
    pub fn create_table(&self) -> Table {
        let mut table = Table::new();
        table.load_preset(presets::UTF8_FULL);

        if !self.no_color {
            table.set_header(vec!["Column1", "Column2"]); // Placeholder
        }

        table
    }

    fn print_table<T: Serialize + TableDisplay>(&self, data: &T) -> Result<(), CliError> {
        let table = data.to_table();
        println!("{}", table);
        Ok(())
    }

    fn print_json<T: Serialize>(&self, data: &T) -> Result<(), CliError> {
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| -> CliError { e.into() })?;
        println!("{}", json);
        Ok(())
    }

    fn print_csv<T: Serialize + TableDisplay>(&self, data: &T) -> Result<(), CliError> {
        let mut writer = csv::Writer::from_writer(std::io::stdout());
        data.to_csv(&mut writer)?;
        writer.flush()?;
        Ok(())
    }
}

/// Message type for colored output
pub enum MessageType {
    Info,
    Success,
    Warning,
    Error,
}

/// Trait for types that can be displayed as tables
pub trait TableDisplay {
    fn to_table(&self) -> Table;
    fn to_csv<W: std::io::Write>(&self, writer: &mut csv::Writer<W>) -> Result<(), CliError>;
}

/// Helper macro to create table rows
#[macro_export]
macro_rules! table_row {
    ($table:expr, $($cell:expr),* $(,)?) => {
        $table.add_row(vec![$($cell.to_string()),*]);
    };
}

/// Helper macro to create colored table cells
#[macro_export]
macro_rules! colored_cell {
    ($value:expr, $color:ident) => {
        if $crate::cli::output::should_use_color() {
            $value.to_string().$color().to_string()
        } else {
            $value.to_string()
        }
    };
}

/// Check if colors should be used
pub fn should_use_color() -> bool {
    !std::env::var("NO_COLOR").is_ok() && colored::control::SHOULD_COLORIZE.should_colorize()
}
