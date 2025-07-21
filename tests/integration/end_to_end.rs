//! End-to-end integration tests

use crate::common::setup;
use colored::*;
use std::path::Path;
use std::time::Instant;
use studio_project_manager::live_set::LiveSet;
use studio_project_manager::utils::decompress_gzip_file;

// TODO: Consider creating comprehensive end-to-end tests here
// TODO: These would test the complete workflow: scan -> parse -> database -> gRPC
// TODO: Could include tests that start a real gRPC server and test full client workflows

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
        let xml_data =
            decompress_gzip_file(&path.to_path_buf()).expect("Failed to decompress file");
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
        println!("  - {}: {}", "Name".bright_black(), live_set.name.cyan());
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
