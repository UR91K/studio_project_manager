use chrono::{DateTime, Duration, Local};
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::LiveSetError;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use crate::scan::{ParseOptions, Parser};
use crate::utils::metadata::{load_file_hash, load_file_name, load_file_timestamps};
use crate::utils::plugins::get_most_recent_db_file;
use crate::utils::{decompress_gzip_file, validate_ableton_file};

#[derive(Debug)]
pub struct LiveSetPreprocessed {
    pub path: PathBuf,
    pub name: String,
    pub file_hash: String,
    pub created_time: DateTime<Local>,
    pub modified_time: DateTime<Local>,
}

impl LiveSetPreprocessed {
    pub fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        validate_ableton_file(&file_path)?;

        let name = load_file_name(&file_path)?;
        let (modified_time, created_time) = load_file_timestamps(&file_path)?;
        let file_hash = load_file_hash(&file_path)?;

        Ok(Self {
            path: file_path,
            name,
            file_hash,
            created_time,
            modified_time,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct LiveSet {
    pub is_active: bool,

    pub id: Uuid,
    pub file_path: PathBuf,
    pub name: String,
    pub file_hash: String,
    pub created_time: DateTime<Local>,
    pub modified_time: DateTime<Local>,
    pub last_parsed_timestamp: DateTime<Local>,

    pub ableton_version: AbletonVersion,

    pub key_signature: Option<KeySignature>,
    pub tempo: f64,
    pub time_signature: TimeSignature,
    pub furthest_bar: Option<f64>,
    pub plugins: HashSet<Plugin>,
    pub samples: HashSet<Sample>,
    pub tags: HashSet<String>,

    pub estimated_duration: Option<chrono::Duration>,
}

impl LiveSet {
    pub fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        let preprocessed = LiveSetPreprocessed::new(file_path)?;
        Self::from_preprocessed(preprocessed)
    }

    pub fn from_preprocessed(preprocessed: LiveSetPreprocessed) -> Result<Self, LiveSetError> {
        // Scope the xml_data to this block so it's dropped after parsing
        let parse_result = {
            let xml_data = decompress_gzip_file(&preprocessed.path)?;
            let parser_options = ParseOptions::default();
            let mut parser = Parser::new(&xml_data, parser_options)?;
            parser.set_current_file(&preprocessed.name);
            parser.parse(&xml_data)?
            // xml_data is dropped here when the block ends
        };

        let mut live_set = LiveSet {
            is_active: true,
            id: Uuid::new_v4(),
            file_path: preprocessed.path,
            name: preprocessed.name,
            file_hash: preprocessed.file_hash,
            created_time: preprocessed.created_time,
            modified_time: preprocessed.modified_time,
            last_parsed_timestamp: Local::now(),

            ableton_version: parse_result.version,
            key_signature: parse_result.key_signature,
            tempo: parse_result.tempo,
            time_signature: parse_result.time_signature,
            furthest_bar: parse_result.furthest_bar,
            plugins: parse_result.plugins,
            samples: parse_result.samples,
            tags: HashSet::new(),

            estimated_duration: None,
        };

        live_set.calculate_duration()?;

        Ok(live_set)
    }

    pub fn calculate_duration(&mut self) -> Result<(), LiveSetError> {
        if let (tempo, Some(furthest_bar)) = (self.tempo, self.furthest_bar) {
            let beats_per_second = tempo / 60.0;
            let total_seconds = furthest_bar * 4.0 / beats_per_second;
            self.estimated_duration = Some(Duration::seconds(total_seconds as i64));
        }
        Ok(())
    }

    /// Add a sample to this LiveSet
    pub fn add_sample(&mut self, sample: Sample) {
        self.samples.insert(sample);
    }

    /// Add a plugin to this LiveSet
    pub fn add_plugin(&mut self, plugin: Plugin) {
        self.plugins.insert(plugin);
    }

    #[allow(dead_code)]
    pub fn reparse_plugins(&mut self) -> Result<(), LiveSetError> {
        let config = CONFIG
            .as_ref()
            .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        let ableton_db = AbletonDatabase::new(
            get_most_recent_db_file(&PathBuf::from(db_dir)).map_err(LiveSetError::DatabaseError)?,
        )
        .map_err(LiveSetError::DatabaseError)?;

        let mut updated_plugins = HashSet::new();

        for plugin in self.plugins.iter() {
            let mut updated_plugin = plugin.clone();
            updated_plugin
                .reparse(&ableton_db)
                .map_err(|e| LiveSetError::DatabaseError(e))?;
            updated_plugins.insert(updated_plugin);
        }

        self.plugins = updated_plugins;

        Ok(())
    }

    pub fn debug_log_info(&self) {
        println!("{}", "\n=== Live Set Information ===".bold().blue());

        // Basic Information
        println!(
            "Name: {}\nPath: {}\nID: {}",
            self.name.bold().purple(),
            self.file_path.display().to_string().cyan(),
            self.id.to_string().bright_black()
        );

        // File Information
        println!("{}", "\nFile Details:".bold().yellow());
        println!(
            "Created: {}\nModified: {}\nLast Parsed: {}\nHash: {}",
            self.created_time
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .cyan(),
            self.modified_time
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .cyan(),
            self.last_parsed_timestamp
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .cyan(),
            self.file_hash.bright_black()
        );

        // Version Information
        println!("{}", "\nAbleton Version:".bold().yellow());
        println!(
            "Version: {}.{}.{} {}",
            self.ableton_version.major.to_string().cyan(),
            self.ableton_version.minor.to_string().cyan(),
            self.ableton_version.patch.to_string().cyan(),
            if self.ableton_version.beta {
                "(Beta)".yellow()
            } else {
                "".normal()
            }
        );

        // Musical Properties
        println!("{}", "\nMusical Properties:".bold().yellow());
        println!(
            "Tempo: {} BPM\nTime Signature: {}/{}\nKey: {}",
            self.tempo.to_string().cyan(),
            self.time_signature.numerator.to_string().cyan(),
            self.time_signature.denominator.to_string().cyan(),
            self.key_signature
                .as_ref()
                .map(|k| format!("{:?} {:?}", k.tonic, k.scale).cyan().to_string())
                .unwrap_or_else(|| "Not specified".bright_black().to_string())
        );

        // Duration Information
        if let Some(duration) = self.estimated_duration {
            println!(
                "Duration: {}m {}s (~ {} bars)",
                duration.num_minutes().to_string().cyan(),
                (duration.num_seconds() % 60).to_string().cyan(),
                self.furthest_bar
                    .map(|b| format!("{:.1}", b).cyan().to_string())
                    .unwrap_or_else(|| "Unknown".bright_black().to_string())
            );
        }

        // Content Summary
        println!("{}", "\nContent Summary:".bold().yellow());
        println!(
            "Plugins: {}\nSamples: {}\nTags: {}",
            self.plugins.len().to_string().green(),
            self.samples.len().to_string().green(),
            if self.tags.is_empty() {
                "None".bright_black().to_string()
            } else {
                self.tags
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
                    .cyan()
                    .to_string()
            }
        );

        // Plugin Details
        if !self.plugins.is_empty() {
            println!("{}", "\nPlugins:".bold().yellow());
            for plugin in &self.plugins {
                println!(
                    "{} {} ({})",
                    if plugin.installed {
                        "✓".green()
                    } else {
                        "✗".red()
                    },
                    plugin.name.cyan(),
                    format!("{:?}", plugin.plugin_format).bright_black()
                );
            }
        }

        // Sample Details
        if !self.samples.is_empty() {
            println!("{}", "\nSamples:".bold().yellow());
            for sample in &self.samples {
                println!("- {}", sample.name.cyan());
            }
        }

        println!("{}", format!("\n{}\n", "=".repeat(50)).bright_black());
    }
}
