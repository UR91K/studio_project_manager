use std::collections::HashSet;
use std::{fs};
use std::f32::NAN;
use std::path::{PathBuf};
use std::str::FromStr;
use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Local};
use colored::*;

use env_logger::Builder;

use log::{debug, error, info};
use log::LevelFilter;
use quick_xml::events::Event;
use quick_xml::Reader;
use toml::value::Time;
use custom_types::{AbletonVersion,
                   Id,
                   KeySignature,
                   Plugin,
                   Sample,
                   TimeSignature,
};
use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::errors::{LiveSetError, XmlParseError};
use crate::helpers::{decompress_gzip_file, load_version, find_all_plugins, format_file_size, load_file_hash, load_file_name, load_file_timestamps, parse_sample_paths, validate_ableton_file, load_time_signature, get_most_recent_db_file, StringResultExt, find_tags, find_attribute};

mod custom_types;
mod errors;
mod helpers;
mod ableton_db;
mod config;

#[allow(dead_code)]
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
    tempo: Option<f64>,
    time_signature: TimeSignature,
    estimated_duration: Option<chrono::Duration>,
    furthest_bar: Option<f64>,

    plugins: HashSet<Plugin>,
    samples: HashSet<Sample>,
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
        validate_ableton_file(&file_path)?;
        
        let file_name:String = load_file_name(&file_path)?;
        let (modified_time, created_time) = load_file_timestamps(&file_path)?;
        let file_hash:String = load_file_hash(&file_path)?;
        let xml_data:Vec<u8> = decompress_gzip_file(&file_path)?;
        let ableton_version:AbletonVersion = load_version(&xml_data)?;
        let time_signature:TimeSignature = load_time_signature(&xml_data)?;
        
        let mut live_set = LiveSet {
            id: Id::default(),
            file_path,
            file_name,
            xml_data,
            file_hash,
            created_time,
            modified_time,
            last_scan_timestamp: Local::now(),
            ableton_version,
            key_signature: None,
            tempo: None,
            time_signature,
            estimated_duration: None,
            furthest_bar: None,
            plugins: HashSet::new(),
            samples: HashSet::new(),
        };

        let samples = live_set.load_samples()?;

        let plugins = live_set.load_plugins()?;
        
        live_set.update_furthest_bar()?;
        live_set.update_tempo()?;
        
        Ok(Self {
            samples,
            plugins,
            ..live_set
        })
    }

    pub fn load_plugins(&mut self) -> Result<HashSet<Plugin>, LiveSetError> {
        Ok(find_all_plugins(&self.xml_data)?.into_iter().collect())
    }
    
    #[allow(dead_code)]
    pub fn rescan_plugins(&mut self) -> Result<(), LiveSetError> {
        let config = CONFIG.as_ref().map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        let ableton_db = AbletonDatabase::new(
            get_most_recent_db_file(&PathBuf::from(db_dir))
                .map_err(LiveSetError::DatabaseError)?
        ).map_err(LiveSetError::DatabaseError)?;
        
        let mut updated_plugins = HashSet::new();
        
        for plugin in self.plugins.iter() {
            let mut updated_plugin = plugin.clone();
            updated_plugin.rescan(&ableton_db)
                .map_err(|e| LiveSetError::DatabaseError(e))?;
            updated_plugins.insert(updated_plugin);
        }
        
        self.plugins = updated_plugins;

        Ok(())
    }
    
    pub fn load_samples(&self) -> Result<HashSet<Sample>, LiveSetError> {
        #[cfg(debug_assertions)]
        let start_time = Instant::now();

        let sample_paths = parse_sample_paths(&self.xml_data, self.ableton_version.major)?;

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

    pub fn update_furthest_bar(&mut self) -> Result<(), LiveSetError> {
        let mut reader = Reader::from_reader(&self.xml_data[..]);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut largest_current_end_value = f64::NAN;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref event)) | Ok(Event::Start(ref event)) => {
                    let name = event.name().to_string_result()?;

                    if name == "CurrentEnd" {
                        for attr in event.attributes().flatten() {
                            if attr.key.as_ref().to_string_result()? == "Value" {
                                if let Ok(value_str) = std::str::from_utf8(&attr.value) {
                                    if let Ok(value) = f64::from_str(value_str) {
                                        largest_current_end_value = if largest_current_end_value.is_nan() {
                                            value
                                        } else {
                                            largest_current_end_value.max(value)
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e))),
                _ => (),
            }
            buf.clear();
        }

        let beats_per_bar = self.time_signature.numerator as f64;
        let furthest_bar = if largest_current_end_value.is_nan() {
            0.0
        } else {
            largest_current_end_value / beats_per_bar
        };
        
        self.furthest_bar = Some(furthest_bar);
        Ok(())
    }

    pub fn update_tempo(&mut self) -> Result<(), LiveSetError> {

        if self.ableton_version.major < 8 ||
            (self.ableton_version.major == 8 && self.ableton_version.minor < 2) {
            return Ok(());
        }

        let previous_tempo:Option<f64> = self.tempo;

        let tempo_value:f64 = if self.ableton_version.major >= 10 ||
            (self.ableton_version.major == 9 && self.ableton_version.minor >= 7) {
            self.find_post_10_tempo()?
        } else {
            self.find_pre_10_tempo()?
        };

        let new_tempo:f64 = ((tempo_value * 1_000_000.0) / 1_000_000.0).round();

        if Some(new_tempo) != previous_tempo {
            self.tempo = Some(new_tempo);
            info_fn!("update_tempo", "{} ({:?}): updated tempo from {:?} to {}", 
                  self.file_name, 
                  self.id, 
                  previous_tempo, 
                  new_tempo);
        }

        Ok(())
    }

    fn find_post_10_tempo(&self) -> Result<f64, LiveSetError> {
        let mut reader = Reader::from_reader(&self.xml_data[..]);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let mut in_tempo = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref().to_string_result()? == "Tempo" {
                        in_tempo = true;
                    }
                }
                Ok(Event::Empty(ref e)) if in_tempo => {
                    if e.name().as_ref() == b"Manual" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref().to_string_result()? == "Value" {
                                return attr.value.as_ref()
                                    .to_str_result()
                                    .and_then(|s| s.parse::<f64>().map_err(|_| XmlParseError::InvalidStructure))
                                    .map_err(LiveSetError::XmlError);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) if in_tempo => {
                    if e.name().as_ref().to_string_result()? == "Tempo" {
                        in_tempo = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e))),
                _ => (),
            }
            buf.clear();
        }

        Err(LiveSetError::XmlError(XmlParseError::EventNotFound("Tempo".to_string())))
    }

    fn find_pre_10_tempo(&self) -> Result<f64, LiveSetError> {
        let search_queries = &["FloatEvent"];
        let target_depth: u8 = 0;
        let float_event_tags = find_tags(&self.xml_data, search_queries, target_depth)?;

        if let Some(float_event_list) = float_event_tags.get("FloatEvent") {
            for tags in float_event_list {
                if !tags.is_empty() {
                    if let Ok(value_str) = find_attribute(&tags[..], "FloatEvent", "Value") {
                        return value_str.parse::<f64>()
                            .map_err(|_| LiveSetError::XmlError(XmlParseError::InvalidStructure));
                    }
                }
            }
        }

        Err(LiveSetError::XmlError(XmlParseError::EventNotFound("FloatEvent".to_string())))
    }
    
    
    //TODO: Add duration estimation (based on furthest bar and tempo)
    //TODO: Add key signature finding

    #[allow(dead_code)]
    pub fn reload_if_changed(&mut self) -> Result<bool, LiveSetError> {
        let current_hash = load_file_hash(&self.file_path)?;

        if current_hash != self.file_hash {
            let file_name = load_file_name(&self.file_path)?;
            let xml_data = decompress_gzip_file(&self.file_path)?;
            let (modified_time, created_time) = load_file_timestamps(&self.file_path)?;
            let ableton_version = load_version(&xml_data)?;
            let time_signature = load_time_signature(&xml_data)?;
            
            self.file_name = file_name;
            self.xml_data = xml_data;
            self.file_hash = current_hash;
            self.modified_time = modified_time;
            self.created_time = created_time;
            self.last_scan_timestamp = Local::now();
            self.ableton_version = ableton_version;
            self.time_signature = time_signature;
            
            self.samples = self.load_samples()?;
            self.plugins = self.load_plugins()?;

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn main() {
    Builder::new()
        .filter_level(LevelFilter::Trace)
        .init();

    let mut paths: Vec<PathBuf> = Vec::new();
    // TEST DATA:
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