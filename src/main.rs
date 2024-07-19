use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use colored::*;
use env_logger::Builder;
use log::error;
use log::LevelFilter;

use crate::live_set::LiveSet;

use crate::utils::format_file_size;

mod ableton_db;
mod config;
mod error;
mod models;

mod live_set;
mod utils;

fn main() {
    Builder::new().filter_level(LevelFilter::Trace).init();

    let mut paths: Vec<PathBuf> = Vec::new();
    // TEST DATA:
    paths.push(PathBuf::from(
        r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2.als",
    )); // max size
    paths.push(PathBuf::from(
        r"C:\Users\judee\Documents\Projects\Beats\Beats Project\a lot on my mind 130 Live11.als",
    )); // mean size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\rust mastering\dp tekno 19 master Project\dp tekno 19 master.als")); // mode size
    paths.push(PathBuf::from(
        r"C:\Users\judee\Documents\Projects\Beats\Beats Project\SET 120.als",
    )); // median size
    paths.push(PathBuf::from(
        r"C:\Users\judee\Documents\Projects\tape\white tape b Project\white tape b.als",
    )); // min size
    for path in &paths {
        let start_time = Instant::now();
        let live_set_result = LiveSet::new(path.to_path_buf());
        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;
        let mut formatted_size: String = String::new();
        if let Ok(metadata) = fs::metadata(&path) {
            let file_size = metadata.len();
            formatted_size = format_file_size(file_size);
        }

        match live_set_result {
            Ok(_) => {
                println!(
                    "{} ({}) Loaded in {:.2} ms",
                    path.file_name().unwrap().to_string_lossy().bold().purple(),
                    formatted_size,
                    duration_ms
                );

                // Print the first and last 32 bytes of the XML data as text
                // let xml_data = live_set.xml_data;
                // println!("First and last 32 bytes of XML data:");
                // print_first_and_last_32_bytes_as_text(xml_data.as_slice());
            }
            Err(err) => error!("Error: {}", err),
        }
    }
}
