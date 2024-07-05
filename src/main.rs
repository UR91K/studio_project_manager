use std::collections::{HashMap, HashSet};
use std::{fs, io};
use std::fs::File;
use std::io::{Cursor, Error, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, Utc};
use colored::*;
use crc32fast::Hasher;
use elementtree::Element;
use env_logger::Builder;
use flate2::read::GzDecoder;
use log::{debug, error, info, trace};
use log::LevelFilter;
use quick_xml::events::Event;
use quick_xml::Reader;
use thiserror::Error;

use custom_types::{AbletonVersion,
                   Id,
                   KeySignature,
                   Plugin,
                   PluginFormat,
                   Sample,
                   Scale,
                   TimeSignature,
                   Tonic,
                   XmlTag,
};

use crate::errors::{LiveSetError, TimeSignatureError, XmlParseError};
use crate::helpers::{extract_gzipped_data,
                     extract_version,
                     find_all_plugins,
                     find_attribute,
                     find_empty_event,
                     find_tags,
                     format_file_size,
                     get_file_hash,
                     get_file_name,
                     get_file_timestamps,
                     parse_encoded_value,
                     parse_xml_data,
                     validate_time_signature,
                     find_all_samples,
};

mod custom_types;
mod errors;
mod helpers;

#[derive(Debug)]
struct LiveSet {
    id: Id,

    file_path: PathBuf,
    file_name: String,
    xml_data: Vec<u8>,
    file_hash: String,
    created_time: DateTime<Local>,
    modified_time: DateTime<Local>,
    last_scan_timestamp: DateTime<Local>,

    ableton_version: AbletonVersion,
    key_signature: Option<KeySignature>,
    tempo: Option<f32>,
    time_signature: Option<TimeSignature>,
    estimated_duration: Option<chrono::Duration>,
    furthest_bar: Option<u32>,

    vst2_plugin_names: Option<HashSet<String>>,
    vst3_plugin_names: Option<HashSet<String>>,
    sample_paths: Option<HashSet<Sample>>,
}

impl LiveSet {
    /// Creates a new `LiveSet` instance from the given file path.
    ///
    /// This function performs several initialization steps:
    /// 1. Extracts the file name from the path
    /// 2. Validates that the file exists and has the correct extension
    /// 3. Retrieves file timestamps (creation and modification times)
    /// 4. Generates a hash of the file contents
    /// 5. Extracts and decompresses the XML data from the Ableton Live Set file
    /// 6. Extracts the Ableton version information from the XML data
    ///
    /// # Arguments
    ///
    /// * `file_path` - A `PathBuf` representing the path to the Ableton Live Set file
    ///
    /// # Returns
    ///
    /// * `Result<Self, String>` - A `Result` containing either the new `LiveSet` instance or an error message
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * The file does not exist or is not accessible
    /// * The file is not a valid Ableton Live Set file (doesn't have .als extension)
    /// * File metadata cannot be retrieved
    /// * XML data cannot be extracted or decompressed
    /// * Ableton version information cannot be parsed from the XML data
    fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        let file_name = get_file_name(&file_path)?;
        let path = Path::new(&file_path);

        if !path.exists() {
            return Err(LiveSetError::InvalidLiveSetFile {
                path: file_path.clone(),
                source: Box::new(Error::new(io::ErrorKind::NotFound, "File does not exist")),
            });
        }

        if !path.is_file() {
            return Err(LiveSetError::InvalidLiveSetFile {
                path: file_path.clone(),
                source: Box::new(Error::new(io::ErrorKind::InvalidInput, "Path is not a file")),
            });
        }

        if path.extension().unwrap_or_default() != "als" {
            return Err(LiveSetError::InvalidLiveSetFile {
                path: file_path.clone(),
                source: Box::new(Error::new(io::ErrorKind::InvalidInput, "File is not an Ableton Live Set (.als) file")),
            });
        }

        let (modified_time, created_time) = get_file_timestamps(&file_path)?;
        let file_hash = get_file_hash(&file_path)?;
        let xml_data = extract_gzipped_data(&file_path)?;
        let last_scan_timestamp = Local::now();
        let ableton_version = extract_version(&xml_data)?;

        let live_set = LiveSet {
            id: Id::default(),
            file_path,
            file_name,
            xml_data: xml_data.clone(),
            file_hash,
            created_time,
            modified_time,
            last_scan_timestamp,
            ableton_version,
            key_signature: None,
            tempo: None,
            time_signature: None,
            estimated_duration: None,
            furthest_bar: None,
            vst2_plugin_names: None,
            vst3_plugin_names: None,
            sample_paths: None,
        };

        let samples = live_set.find_samples()?;
        let (vst2, vst3) = live_set.find_plugins()?;
        let time_signature = live_set.update_time_signature()?;

        Ok(Self {
            sample_paths: Some(samples),
            vst2_plugin_names: Some(vst2),
            vst3_plugin_names: Some(vst3),
            time_signature: Some(time_signature),
            ..live_set
        })
    }

    /// Loads and decompresses the raw XML data from the Ableton Live Set file.
    ///
    /// This function re-reads the file from disk, decompresses its contents,
    /// and stores the resulting XML data in the `xml_data` field of the `LiveSet` instance.
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - A `Result` indicating success or containing an error message
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * The file does not exist or is not accessible
    /// * The file is not a valid Ableton Live Set file (doesn't have .als extension)
    /// * The file contents cannot be decompressed
    fn load_raw_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.file_path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{:?}: is either inaccessible or not a valid Ableton Live Set file", self.file_path));
        }

        let decompressed_data = extract_gzipped_data(&path).map_err(|err| err.to_string())?;

        self.xml_data = decompressed_data;

        Ok(())
    }

    /// Finds and stores all plugin names used in the Ableton Live Set.
    ///
    /// This function parses the XML data to extract names of VST2 and VST3 plugins
    /// used in the project. The plugin names are stored in `vst2_plugin_names` and
    /// `vst3_plugin_names` fields of the `LiveSet` instance.
    ///
    /// # Returns
    ///
    /// * `Result<(), LiveSetError>` - A `Result` indicating success or containing a `LiveSetError`
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * The XML data cannot be parsed
    /// * Plugin information cannot be extracted from the XML data
    ///
    /// # Performance
    ///
    /// This function measures and logs its execution time for performance monitoring.
    pub fn find_plugins(&self) -> Result<(HashSet<String>, HashSet<String>), LiveSetError> {
        #[cfg(debug_assertions)]
        let start_time = Instant::now();

        let plugin_names = find_all_plugins(&self.xml_data)?;

        let mut vst2_plugin_names = HashSet::new();
        let mut vst3_plugin_names = HashSet::new();

        if let Some(vst2_names) = plugin_names.get("VstPluginInfo") {
            vst2_plugin_names.extend(vst2_names.iter().cloned());
        }

        if let Some(vst3_names) = plugin_names.get("Vst3PluginInfo") {
            vst3_plugin_names.extend(vst3_names.iter().cloned());
        }

        #[cfg(debug_assertions)]
        debug!("{}: found {} VST2 Plugins and {} VST3 Plugins in {:.2} ms",
            self.file_name.bold().purple(),
            vst2_plugin_names.len(),
            vst3_plugin_names.len(),
            start_time.elapsed().as_secs_f64() * 1000.0
        );

        Ok((vst2_plugin_names, vst3_plugin_names))
    }

    pub fn find_samples(&self) -> Result<HashSet<Sample>, LiveSetError> {
        #[cfg(debug_assertions)]
        let start_time = Instant::now();

        let sample_paths = find_all_samples(&self.xml_data, self.ableton_version.major)?;

        let mut all_samples = HashSet::new();
        for (_, paths) in sample_paths {
            for path in paths {
                let sample = Sample::new(
                    Id::default(), //TODO: generate unique IDs
                    path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    path,
                );
                all_samples.insert(sample);
            }
        }

        #[cfg(debug_assertions)]
        debug!("{}: found {} sample(s) in {:.2} ms",
            self.file_name.bold().purple(),
            all_samples.len(),
            start_time.elapsed().as_secs_f64() * 1000.0
        );

        info!("{}: found {} sample(s)",
            self.file_name.bold().purple(),
            all_samples.len()
        );

        Ok(all_samples)
    }

    fn update_time_signature(&self) -> Result<TimeSignature, LiveSetError> {
        debug!("Updating time signature");
        trace!("XML data: {:?}", std::str::from_utf8(&self.xml_data));

        let search_query = "EnumEvent";

        let event_attributes = find_empty_event(&self.xml_data, search_query)
            .map_err(|e| match e {
                XmlParseError::EventNotFound(_) => LiveSetError::EnumEventNotFound,
                _ => LiveSetError::XmlError(e),
            })?;

        debug!("Found time signature enum event");
        trace!("Attributes: {:?}", event_attributes);

        let value_attribute = event_attributes
            .get("Value")
            .ok_or(LiveSetError::ValueAttributeNotFound)?;

        debug!("Found 'Value' attribute");
        trace!("Value: {}", value_attribute);

        let encoded_value = parse_encoded_value(value_attribute)?;
        debug!("Parsed encoded value: {}", encoded_value);

        let time_signature = TimeSignature::from_encoded(encoded_value)
            .map_err(LiveSetError::TimeSignatureError)?;

        debug!("Decoded time signature: {:?}", time_signature);

        info!(
            "Time signature updated: {}/{}",
            time_signature.numerator,
            time_signature.denominator
        );

        Ok(time_signature)
    }

    //TODO: Add furthest bar finding
    //TODO: Add tempo finding
    //TODO: Add duration estimation (based on furthest bar and tempo)
    //TODO: Add key signature finding

    pub fn rescan_project(&mut self) -> Result<(), String> {
        self.load_raw_xml_data()?;

        match self.find_samples() {
            Ok(samples) => self.sample_paths = Some(samples),
            Err(e) => return Err(format!("Failed to rescan samples: {}", e)),
        }

        if let Err(e) = self.find_plugins() {
            return Err(format!("Failed to rescan plugins: {}", e));
        }

        if let Err(e) = self.update_time_signature() {
            return Err(format!("Failed to update time signature: {}", e));
        }

        //TODO: ddd other rescanning steps

        Ok(())
    }
}

fn print_first_and_last_32_bytes_as_text(data: &[u8]) {
    let total_bytes = data.len();

    println!("Total bytes: {}", total_bytes);

    println!("First 32 bytes as text:");
    let first_32 = &data[..32.min(total_bytes)];
    print_bytes_as_text(first_32);

    println!("Last 32 bytes as text:");
    let last_32 = &data[total_bytes.saturating_sub(32)..];
    print_bytes_as_text(last_32);
}

fn print_bytes_as_text(bytes: &[u8]) {
    match std::str::from_utf8(bytes) {
        Ok(text) => println!("{}", text),
        Err(err) => println!("Invalid UTF-8: {}", err),
    }
}

fn main() {
    Builder::new()
        .filter_level(LevelFilter::Debug)
        .init();

    let mut paths: Vec<PathBuf> = Vec::new();
    /// TEST DATA:
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2.als")); // max size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\Beats Project\a lot on my mind 130 Live11.als")); // mean size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\rust mastering\dp tekno 19 master Project\dp tekno 19 master.als")); // mode size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\Beats Project\SET 120.als")); // median size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\tape\white tape b Project\white tape b.als")); // min size
    for path in &paths {
        let start_time = Instant::now();
        let live_set_result = LiveSet::new(path.to_path_buf());
        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;
        let mut file_size: u64 = 0;
        let mut formatted_size: String = String::new();
        if let Ok(metadata) = fs::metadata(&path) {
            file_size = metadata.len();
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