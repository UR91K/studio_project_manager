//! Parallel scanning tests

use colored::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use studio_project_manager::scan::project_scanner::ProjectPathScanner;
use studio_project_manager::scan::ParallelParser;
use studio_project_manager::{AbletonVersion, LiveSet, CONFIG};
use tempfile::tempdir;

use crate::common::setup;

#[derive(Default)]
struct ProjectStats {
    version_counts: HashMap<AbletonVersion, usize>,
    unique_plugins: HashSet<(String, String, String)>, // (name, dev_identifier, format)
    total_plugin_instances: usize,
    total_samples: usize,
    oldest_project: Option<(PathBuf, chrono::DateTime<chrono::Local>)>,
    newest_project: Option<(PathBuf, chrono::DateTime<chrono::Local>)>,
    successful_parses: usize,
    failed_parses: usize,
}

impl ProjectStats {
    fn add_project(&mut self, path: PathBuf, live_set: &LiveSet) {
        self.successful_parses += 1;

        // Update version count
        *self
            .version_counts
            .entry(live_set.ableton_version)
            .or_insert(0) += 1;

        // Update plugin counts
        self.total_plugin_instances += live_set.plugins.len();
        for plugin in &live_set.plugins {
            self.unique_plugins.insert((
                plugin.name.clone(),
                plugin.dev_identifier.clone(),
                plugin.plugin_format.to_string(),
            ));
        }

        // Update sample count
        self.total_samples += live_set.samples.len();

        // Update project dates
        let created = live_set.created_time;
        match &self.oldest_project {
            None => self.oldest_project = Some((path.clone(), created)),
            Some((_, oldest)) if created < *oldest => {
                self.oldest_project = Some((path.clone(), created))
            }
            _ => {}
        }

        match &self.newest_project {
            None => self.newest_project = Some((path.clone(), created)),
            Some((_, newest)) if created > *newest => self.newest_project = Some((path, created)),
            _ => {}
        }
    }

    fn format_plugin_columns(plugins: &[(String, String, String)]) -> Vec<String> {
        let mut formatted = Vec::new();
        let mut max_name_len = 0;
        let mut max_format_len = 0;

        // Find maximum lengths for alignment
        for (name, _, format) in plugins {
            max_name_len = max_name_len.max(name.len());
            max_format_len = max_format_len.max(format.len());
        }

        // Format each plugin with padding
        for (name, dev_id, format) in plugins {
            let format_color = match format.as_str() {
                "VST2 Instrument" => "VST2 Instrument".bright_green(),
                "VST2 Effect" => "VST2 Effect".bright_blue(),
                "VST3 Instrument" => "VST3 Instrument".bright_yellow(),
                "VST3 Effect" => "VST3 Effect".bright_cyan(),
                _ => format.normal(),
            };

            let line = format!(
                "{:<width$} {} {:<format_width$} {}",
                name.bright_white(),
                "│".dimmed(),
                format_color,
                format!("[{}]", dev_id).dimmed(),
                width = max_name_len,
                format_width = max_format_len
            );
            formatted.push(line);
        }

        formatted
    }

    fn print_summary(&self) {
        println!(
            "\n{}",
            "=== Project Analysis Summary ===".bright_white().bold()
        );

        println!("\n{}:", "Processing Statistics".yellow());
        println!(
            "  - Successfully Parsed: {}",
            self.successful_parses.to_string().green()
        );
        println!(
            "  - Failed to Parse: {}",
            self.failed_parses.to_string().red()
        );
        println!(
            "  - Total Projects: {}",
            (self.successful_parses + self.failed_parses)
                .to_string()
                .cyan()
        );

        println!("\n{}:", "Content Statistics".yellow());
        println!(
            "  - Unique Plugins: {}",
            self.unique_plugins.len().to_string().cyan()
        );
        println!(
            "  - Total Plugin Instances: {}",
            self.total_plugin_instances.to_string().cyan()
        );
        println!(
            "  - Average Plugin Instances per Project: {:.1}",
            (self.total_plugin_instances as f64 / self.successful_parses as f64)
                .to_string()
                .cyan()
        );
        println!(
            "  - Total Samples Used: {}",
            self.total_samples.to_string().cyan()
        );
        println!(
            "  - Average Samples per Project: {:.1}",
            (self.total_samples as f64 / self.successful_parses as f64)
                .to_string()
                .cyan()
        );

        println!("\n{}:", "Plugin List".yellow());
        let mut plugins: Vec<(String, String, String)> = self
            .unique_plugins
            .iter()
            .map(|(name, dev_id, format)| (name.clone(), dev_id.clone(), format.clone()))
            .collect();
        plugins.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by plugin name

        // Format plugins into columns
        let formatted_plugins = Self::format_plugin_columns(&plugins);

        // Calculate number of columns based on terminal width
        if let Some((width, _)) = terminal_size::terminal_size() {
            let max_line_length = formatted_plugins
                .iter()
                .map(|line| line.len())
                .max()
                .unwrap_or(0);

            let num_columns = (width.0 as usize / max_line_length).max(1);
            let num_rows = (formatted_plugins.len() + num_columns - 1) / num_columns;

            // Print in columns
            for row in 0..num_rows {
                for col in 0..num_columns {
                    let idx = row + (col * num_rows);
                    if idx < formatted_plugins.len() {
                        print!(
                            "{:<width$}",
                            formatted_plugins[idx],
                            width = max_line_length + 2
                        );
                    }
                }
                println!();
            }
        } else {
            // Fallback to single column if terminal size can't be determined
            for line in formatted_plugins {
                println!("  {}", line);
            }
        }

        println!("\n{}:", "Version Distribution".yellow());
        let mut versions: Vec<_> = self.version_counts.iter().collect();
        versions.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
        for (version, count) in versions {
            println!(
                "  - Live {}.{}.{}{}: {} projects",
                version.major,
                version.minor,
                version.patch,
                if version.beta { " beta" } else { "" },
                count
            );
        }

        println!("\n{}:", "Project Timeline".yellow());
        if let Some((path, date)) = &self.oldest_project {
            println!(
                "  - Oldest: {} ({})",
                path.file_name().unwrap().to_string_lossy().bright_red(),
                date.format("%Y-%m-%d %H:%M:%S").to_string().dimmed()
            );
        }
        if let Some((path, date)) = &self.newest_project {
            println!(
                "  - Newest: {} ({})",
                path.file_name().unwrap().to_string_lossy().bright_green(),
                date.format("%Y-%m-%d %H:%M:%S").to_string().dimmed()
            );
        }
        println!(
            "\n{}",
            "===============================".bright_white().bold()
        );
    }
}

#[test]
fn test_parallel_parser() {
    setup("error");
    let dir = tempdir().unwrap();
    let test_file = dir.path().join("test.als");
    let mut file = File::create(&test_file).unwrap();
    write!(file, "test data").unwrap();

    let parser = ParallelParser::new(2);
    parser.submit_paths(vec![test_file.clone()]).unwrap();

    // Get first result
    let result = parser.get_results_receiver().recv().unwrap();
    assert!(result.is_err()); // Should be error since not valid .als file

    let (path, _) = result.unwrap_err();
    assert_eq!(path, test_file);
}

#[test]
fn test_integrated_scanning_and_parsing() {
    setup("error");
    // Create a scanner
    let scanner = ProjectPathScanner::new().unwrap();

    // Get paths from config
    let config = CONFIG.as_ref().expect("Failed to load config");
    let mut found_projects = HashSet::new();

    // Scan all configured directories
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            let projects = scanner.scan_directory(&path).unwrap();
            found_projects.extend(projects);
        }
    }

    // Skip test if no projects found
    if found_projects.is_empty() {
        println!("No Ableton projects found in configured paths, skipping test");
        return;
    }

    // Create parallel parser with number of threads based on project count
    let thread_count = (found_projects.len() / 2).max(1).min(4);
    let parser = ParallelParser::new(thread_count);

    // Submit all found projects for parsing
    parser
        .submit_paths(found_projects.into_iter().collect())
        .unwrap();

    // Collect results with timeout
    let receiver = parser.get_results_receiver();
    let mut stats = ProjectStats::default();

    while let Ok(result) = receiver.recv_timeout(Duration::from_secs(5)) {
        match result {
            Ok((path, live_set)) => {
                stats.add_project(path, &live_set);
            }
            Err((path, error)) => {
                println!("Failed to parse {}: {:?}", path.display(), error);
                stats.failed_parses += 1;
            }
        }
    }

    // Print the summary
    stats.print_summary();

    // Assert that we processed at least one file
    assert!(
        stats.successful_parses + stats.failed_parses > 0,
        "No files were processed"
    );
}
