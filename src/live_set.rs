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
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) file_hash: String,
    pub(crate) created_time: DateTime<Local>,
    pub(crate) modified_time: DateTime<Local>,
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
    pub(crate) is_active: bool,

    pub(crate) id: Uuid,
    pub(crate) file_path: PathBuf,
    pub(crate) name: String,
    pub(crate) file_hash: String,
    pub(crate) created_time: DateTime<Local>,
    pub(crate) modified_time: DateTime<Local>,
    pub(crate) last_parsed_timestamp: DateTime<Local>,

    pub(crate) ableton_version: AbletonVersion,

    pub(crate) key_signature: Option<KeySignature>,
    pub(crate) tempo: f64,
    pub(crate) time_signature: TimeSignature,
    pub(crate) furthest_bar: Option<f64>,
    pub(crate) plugins: HashSet<Plugin>,
    pub(crate) samples: HashSet<Sample>,
    pub(crate) tags: HashSet<String>,

    pub(crate) estimated_duration: Option<chrono::Duration>,
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

    pub fn log_info(&self) {
        println!(
            "{}",
            "\n=== Live Set Information ===".bold().blue()
        );

        // Basic Information
        println!(
            "Name: {}\nPath: {}\nID: {}",
            self.name.bold().purple(),
            self.file_path.display().to_string().cyan(),
            self.id.to_string().bright_black()
        );

        // File Information
        println!(
            "{}",
            "\nFile Details:".bold().yellow()
        );
        println!(
            "Created: {}\nModified: {}\nLast Parsed: {}\nHash: {}",
            self.created_time.format("%Y-%m-%d %H:%M:%S").to_string().cyan(),
            self.modified_time.format("%Y-%m-%d %H:%M:%S").to_string().cyan(),
            self.last_parsed_timestamp.format("%Y-%m-%d %H:%M:%S").to_string().cyan(),
            self.file_hash.bright_black()
        );

        // Version Information
        println!(
            "{}",
            "\nAbleton Version:".bold().yellow()
        );
        println!(
            "Version: {}.{}.{} {}",
            self.ableton_version.major.to_string().cyan(),
            self.ableton_version.minor.to_string().cyan(),
            self.ableton_version.patch.to_string().cyan(),
            if self.ableton_version.beta { "(Beta)".yellow() } else { "".normal() }
        );

        // Musical Properties
        println!(
            "{}",
            "\nMusical Properties:".bold().yellow()
        );
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
        println!(
            "{}",
            "\nContent Summary:".bold().yellow()
        );
        println!(
            "Plugins: {}\nSamples: {}\nTags: {}",
            self.plugins.len().to_string().green(),
            self.samples.len().to_string().green(),
            if self.tags.is_empty() {
                "None".bright_black().to_string()
            } else {
                self.tags.iter().cloned().collect::<Vec<_>>().join(", ").cyan().to_string()
            }
        );

        // Plugin Details
        if !self.plugins.is_empty() {
            println!(
                "{}",
                "\nPlugins:".bold().yellow()
            );
            for plugin in &self.plugins {
                println!(
                    "{} {} ({})",
                    if plugin.installed { "✓".green() } else { "✗".red() },
                    plugin.name.cyan(),
                    format!("{:?}", plugin.plugin_format).bright_black()
                );
            }
        }

        // Sample Details
        if !self.samples.is_empty() {
            println!(
                "{}",
                "\nSamples:".bold().yellow()
            );
            for sample in &self.samples {
                println!(
                    "- {}",
                    sample.name.cyan()
                );
            }
        }

        println!(
            "{}",
            format!("\n{}\n", "=".repeat(50)).bright_black()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Instant;
    use crate::test_utils::setup;

    #[test]
    fn test_load_real_project() {
        setup("debug");

        let project_path = Path::new(
            r"C:\Users\judee\Documents\Projects\band with joel\Forkspan Project\Forkspan.als",
        );
        let live_set = LiveSet::new(project_path.to_path_buf()).expect("Failed to load project");

        // Basic project validation
        assert!(!live_set.name.is_empty());
        assert!(live_set.created_time < live_set.modified_time);
        assert!(!live_set.file_hash.is_empty());

        // Version check
        assert!(live_set.ableton_version.major >= 9);
        assert!(live_set.ableton_version.beta == false);

        // Musical properties
        assert!(live_set.tempo > 0.0);
        assert!(live_set.time_signature.is_valid());
        assert!(live_set.furthest_bar.is_some());

        if let Some(duration) = live_set.estimated_duration {
            assert!(duration.num_seconds() > 0);
        }

        // Content checks
        assert!(
            !live_set.plugins.is_empty(),
            "Project should contain at least one plugin"
        );
        assert!(
            !live_set.samples.is_empty(),
            "Project should contain at least one sample"
        );

        // Log project info for manual verification
        live_set.log_info();
    }

    #[test]
    fn test_parse_performance() {
        setup("error");
        
        let project_paths = [
            (
                r"C:\Users\judee\Documents\Projects\band with joel\Forkspan Project\Forkspan.als",
                "small",
            ),
            (
                r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2 re mix.als",
                "medium",
            ),
            (
                r"C:\Users\judee\Documents\Projects\band with joel\green tea Project\green tea.als",
                "large",
            ),
            (
                r"C:\Users\judee\Documents\Projects\Beats\Beats Project\SET 120.als",
                "median",
            ),
            (
                r"C:\Users\judee\Documents\Projects\tape\white tape b Project\white tape b.als",
                "min",
            ),
            (
                r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2.als",
                "another large",
            ),
            (
                r"C:\Users\judee\Documents\Projects\Beats\Beats Project\a lot on my mind 130 Live11.als",
                "mean",
            ),
            (
                r"C:\Users\judee\Documents\Projects\rust mastering\dp tekno 19 master Project\dp tekno 19 master.als",
                "mode",
            ),
            (
                r"C:\Users\judee\Documents\Projects\test_projects_dir\duplicated plugins test Project\duplicated plugins test.als",
                "min",
            ),
        ];

        let mut total_size = 0.0;
        let mut total_time = 0.0;

        for (path, size) in project_paths.iter() {
            let path = Path::new(path);
            println!(
                "\n{}",
                format!(
                    "=== Testing {} project: {} ===",
                    size,
                    path.file_name().unwrap().to_string_lossy()
                )
                .bold()
                .blue()
            );

            // Get XML size before creating LiveSet
            let xml_data = decompress_gzip_file(&path.to_path_buf()).expect("Failed to decompress file");
            let xml_size_mb = xml_data.len() as f64 / 1_000_000.0;
            total_size += xml_size_mb;
            
            // Drop xml_data before creating LiveSet
            drop(xml_data);

            let start = Instant::now();
            let live_set = LiveSet::new(path.to_path_buf()).expect("Failed to load project");
            let duration = start.elapsed();
            let duration_secs = duration.as_secs_f64();
            total_time += duration_secs;

            println!("\n{}", "Parse Performance:".yellow().bold());
            println!(
                "  - {}: {}",
                "Parse time".bright_black(),
                format!("{:.2?}", duration).green()
            );
            println!(
                "  - {}: {:.2} MB",
                "XML data size".bright_black(),
                xml_size_mb
            );
            println!(
                "  - {}: {:.2} MB/s",
                "Throughput".bright_black(),
                xml_size_mb / duration_secs
            );

            println!("\n{}", "File Info:".yellow().bold());
            println!(
                "  - {}: {}",
                "Name".bright_black(),
                live_set.name.cyan()
            );
            println!(
                "  - {}: {}",
                "Created".bright_black(),
                live_set.created_time.format("%Y-%m-%d %H:%M:%S")
            );
            println!(
                "  - {}: {}",
                "Modified".bright_black(),
                live_set.modified_time.format("%Y-%m-%d %H:%M:%S")
            );
            println!(
                "  - {}: {}",
                "Hash".bright_black(),
                live_set.file_hash.bright_black()
            );

            println!("\n{}", "Ableton Version:".yellow().bold());
            println!(
                "  - {}: {}",
                "Major".bright_black(),
                live_set.ableton_version.major
            );
            println!(
                "  - {}: {}",
                "Minor".bright_black(),
                live_set.ableton_version.minor
            );
            println!(
                "  - {}: {}",
                "Patch".bright_black(),
                live_set.ableton_version.patch
            );
            println!(
                "  - {}: {}",
                "Beta".bright_black(),
                live_set.ableton_version.beta
            );

            println!("\n{}", "Musical Properties:".yellow().bold());
            println!(
                "  - {}: {} BPM",
                "Tempo".bright_black(),
                live_set.tempo.to_string().cyan()
            );
            println!(
                "  - {}: {}/{}",
                "Time Signature".bright_black(),
                live_set.time_signature.numerator.to_string().cyan(),
                live_set.time_signature.denominator.to_string().cyan()
            );
            if let Some(key) = &live_set.key_signature {
                println!(
                    "  - {}: {:?} {:?}",
                    "Key".bright_black(),
                    key.tonic,
                    key.scale
                );
            }
            if let Some(bars) = live_set.furthest_bar {
                println!("  - {}: {:.1} bars", "Length".bright_black(), bars);
            }
            if let Some(duration) = live_set.estimated_duration {
                println!(
                    "  - {}: {}m {}s",
                    "Duration".bright_black(),
                    duration.num_minutes().to_string().cyan(),
                    (duration.num_seconds() % 60).to_string().cyan()
                );
            }

            println!("\n{}", "Content Summary:".yellow().bold());
            println!(
                "  - {}: {}",
                "Total Plugins".bright_black(),
                live_set.plugins.len().to_string().green()
            );
            println!(
                "  - {}: {}",
                "Total Samples".bright_black(),
                live_set.samples.len().to_string().green()
            );

            if !live_set.plugins.is_empty() {
                println!("\n{}", "Plugins:".yellow().bold());
                for plugin in &live_set.plugins {
                    let status = if plugin.installed {
                        "✓".green()
                    } else {
                        "✗".red()
                    };
                    println!(
                        "  {} {} ({})",
                        status,
                        plugin.name.cyan(),
                        format!("{:?}", plugin.plugin_format).bright_black()
                    );
                }
            }

            if !live_set.samples.is_empty() {
                println!("\n{}", "Samples:".yellow().bold());
                for sample in &live_set.samples {
                    println!("  - {}", sample.name.cyan());
                }
            }

            println!("\n{}", "=".repeat(50).bright_black());
        }

        println!("\n{}", "=== Overall Performance ===".bold().blue());
        println!("  - {}: {:.2} MB", "Total size".bright_black(), total_size);
        println!(
            "  - {}: {:.2?}",
            "Total time".bright_black(),
            std::time::Duration::from_secs_f64(total_time)
        );
        println!(
            "  - {}: {:.2} MB/s",
            "Average throughput".bright_black(),
            total_size / total_time
        );
        println!("{}", "=".repeat(50).bright_black());
    }
}
