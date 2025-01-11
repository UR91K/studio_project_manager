use std::collections::HashSet;
use std::path::PathBuf;
use chrono::{DateTime, Duration, Local};
use uuid::Uuid;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::LiveSetError;
use crate::{debug_fn, info_fn};
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use crate::utils::metadata::{load_file_hash, load_file_name, load_file_timestamps};
use crate::utils::plugins::get_most_recent_db_file;
use crate::utils::{decompress_gzip_file, validate_ableton_file};
use crate::scan::{Scanner, ScanOptions as ScannerOptions};
use colored::*;

#[allow(dead_code)]
#[derive(Debug)]
pub struct LiveSet {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub file_name: String,
    pub file_hash: String,
    pub created_time: DateTime<Local>,
    pub modified_time: DateTime<Local>,
    pub xml_data: Vec<u8>,
    pub last_scan_timestamp: DateTime<Local>,

    pub ableton_version: AbletonVersion,
    
    pub key_signature: Option<KeySignature>,
    pub tempo: f64,
    pub time_signature: TimeSignature,
    pub furthest_bar: Option<f64>,
    pub plugins: HashSet<Plugin>,
    pub samples: HashSet<Sample>,
    
    pub estimated_duration: Option<chrono::Duration>,
}

impl LiveSet {
    pub fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        validate_ableton_file(&file_path)?;

        let file_name: String = load_file_name(&file_path)?;
        let (modified_time, created_time) = load_file_timestamps(&file_path)?;
        let file_hash: String = load_file_hash(&file_path)?;
        let xml_data: Vec<u8> = decompress_gzip_file(&file_path)?;

        // Use our new scanner to load everything
        let scanner_options = ScannerOptions::default();
        let mut scanner = Scanner::new(&xml_data, scanner_options)?;
        let scan_result = scanner.scan(&xml_data)?;

        let mut live_set = LiveSet {
            id: Uuid::new_v4(),
            file_path,
            file_name,
            file_hash,
            created_time,
            modified_time,
            xml_data,
            last_scan_timestamp: Local::now(),

            ableton_version: scan_result.version,
            key_signature: scan_result.key_signature,
            tempo: scan_result.tempo,
            time_signature: scan_result.time_signature,
            furthest_bar: scan_result.furthest_bar,
            plugins: scan_result.plugins,
            samples: scan_result.samples,

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
    pub fn rescan_plugins(&mut self) -> Result<(), LiveSetError> {
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
                .rescan(&ableton_db)
                .map_err(|e| LiveSetError::DatabaseError(e))?;
            updated_plugins.insert(updated_plugin);
        }

        self.plugins = updated_plugins;

        Ok(())
    }

    pub fn log_info(&self) {
        info_fn!(
            "log_info",
            "{} - {} plugins, {} samples",
            self.file_name.bold().purple(),
            self.plugins.len(),
            self.samples.len()
        );

        if let Some(duration) = self.estimated_duration {
            debug_fn!(
                "log_info",
                "Estimated duration: {}",
                duration.to_string().green()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Once;
    use std::time::Instant;

    static INIT: Once = Once::new();
    fn setup() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "debug");
            env_logger::builder()
                .is_test(true)
                .filter_level(log::LevelFilter::Debug)
                .try_init()
                .expect("Failed to initialize logger");
        });
    }

    fn setup_no_logging() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "error");
            env_logger::builder()
                .is_test(true)
                .filter_level(log::LevelFilter::Error)
                .try_init()
                .expect("Failed to initialize logger");
        });
    }

    #[test]
    fn test_load_real_project() {
        setup();
        
        let project_path = Path::new(r"C:\Users\judee\Documents\Projects\band with joel\Forkspan Project\Forkspan.als");
        let live_set = LiveSet::new(project_path.to_path_buf()).expect("Failed to load project");

        // Basic project validation
        assert!(!live_set.file_name.is_empty());
        assert!(live_set.created_time < live_set.modified_time);
        assert!(!live_set.file_hash.is_empty());
        assert!(!live_set.xml_data.is_empty());

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
        assert!(!live_set.plugins.is_empty(), "Project should contain at least one plugin");
        assert!(!live_set.samples.is_empty(), "Project should contain at least one sample");

        // Log project info for manual verification
        live_set.log_info();
    }

    #[test]
    fn test_scan_performance() {
        setup_no_logging();

        let project_paths = [
            (r"C:\Users\judee\Documents\Projects\band with joel\Forkspan Project\Forkspan.als", "small"),
            (r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2 re mix.als", "medium"),
            (r"C:\Users\judee\Documents\Projects\band with joel\green tea Project\green tea.als", "large"),
            (r"C:\Users\judee\Documents\Projects\Beats\Beats Project\SET 120.als", "median"),
            (r"C:\Users\judee\Documents\Projects\tape\white tape b Project\white tape b.als", "min"),
            (r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2.als", "another large"),
            (r"C:\Users\judee\Documents\Projects\Beats\Beats Project\a lot on my mind 130 Live11.als", "mean"),
            (r"C:\Users\judee\Documents\Projects\rust mastering\dp tekno 19 master Project\dp tekno 19 master.als", "mode"),
            (r"C:\Users\judee\Documents\Projects\test_projects_dir\duplicated plugins test Project\duplicated plugins test.als", "min"),
        ];

        let mut total_size = 0.0;
        let mut total_time = 0.0;

        for (path, size) in project_paths.iter() {
            let path = Path::new(path);
            println!("\n{}", format!("=== Testing {} project: {} ===", size, path.file_name().unwrap().to_string_lossy()).bold().blue());
            
            let start = Instant::now();
            let live_set = LiveSet::new(path.to_path_buf()).expect("Failed to load project");
            let duration = start.elapsed();
            let duration_secs = duration.as_secs_f64();

            let xml_size_mb = live_set.xml_data.len() as f64 / 1_000_000.0;
            total_size += xml_size_mb;
            total_time += duration_secs;

            println!("\n{}", "Scan Performance:".yellow().bold());
            println!("  - {}: {}", "Scan time".bright_black(), format!("{:.2?}", duration).green());
            println!("  - {}: {:.2} MB", "XML data size".bright_black(), xml_size_mb);
            println!("  - {}: {:.2} MB/s", "Throughput".bright_black(), xml_size_mb / duration_secs);

            println!("\n{}", "File Info:".yellow().bold());
            println!("  - {}: {}", "Name".bright_black(), live_set.file_name.cyan());
            println!("  - {}: {}", "Created".bright_black(), live_set.created_time.format("%Y-%m-%d %H:%M:%S"));
            println!("  - {}: {}", "Modified".bright_black(), live_set.modified_time.format("%Y-%m-%d %H:%M:%S"));
            println!("  - {}: {}", "Hash".bright_black(), live_set.file_hash.bright_black());

            println!("\n{}", "Ableton Version:".yellow().bold());
            println!("  - {}: {}", "Major".bright_black(), live_set.ableton_version.major);
            println!("  - {}: {}", "Minor".bright_black(), live_set.ableton_version.minor);
            println!("  - {}: {}", "Patch".bright_black(), live_set.ableton_version.patch);
            println!("  - {}: {}", "Beta".bright_black(), live_set.ableton_version.beta);

            println!("\n{}", "Musical Properties:".yellow().bold());
            println!("  - {}: {} BPM", "Tempo".bright_black(), live_set.tempo.to_string().cyan());
            println!("  - {}: {}/{}", "Time Signature".bright_black(), 
                live_set.time_signature.numerator.to_string().cyan(), 
                live_set.time_signature.denominator.to_string().cyan());
            if let Some(key) = &live_set.key_signature {
                println!("  - {}: {:?} {:?}", "Key".bright_black(), key.tonic, key.scale);
            }
            if let Some(bars) = live_set.furthest_bar {
                println!("  - {}: {:.1} bars", "Length".bright_black(), bars);
            }
            if let Some(duration) = live_set.estimated_duration {
                println!("  - {}: {}m {}s", "Duration".bright_black(), 
                    duration.num_minutes().to_string().cyan(), 
                    (duration.num_seconds() % 60).to_string().cyan());
            }

            println!("\n{}", "Content Summary:".yellow().bold());
            println!("  - {}: {}", "Total Plugins".bright_black(), live_set.plugins.len().to_string().green());
            println!("  - {}: {}", "Total Samples".bright_black(), live_set.samples.len().to_string().green());

            if !live_set.plugins.is_empty() {
                println!("\n{}", "Plugins:".yellow().bold());
                for plugin in &live_set.plugins {
                    let status = if plugin.installed {
                        "✓".green()
                    } else {
                        "✗".red()
                    };
                    println!("  {} {} ({})", 
                        status,
                        plugin.name.cyan(), 
                        format!("{:?}", plugin.plugin_format).bright_black());
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
        println!("  - {}: {:.2?}", "Total time".bright_black(), std::time::Duration::from_secs_f64(total_time));
        println!("  - {}: {:.2} MB/s", "Average throughput".bright_black(), total_size / total_time);
        println!("{}", "=".repeat(50).bright_black());
    }
}