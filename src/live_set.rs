// /src/live_set.rs
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use chrono::{DateTime, Duration, Local};
use colored::Colorize;
use log::{debug, error, info};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{AttributeError, LiveSetError, XmlParseError};
use crate::info_fn;
use crate::models::{AbletonVersion, Id, KeySignature, Plugin, Sample, TimeSignature};
use crate::utils::metadata::{load_file_hash, load_file_name, load_file_timestamps};
use crate::utils::plugins::{find_all_plugins, get_most_recent_db_file};
use crate::utils::samples::parse_sample_paths;
use crate::utils::tempo::{find_post_10_tempo, find_pre_10_tempo};
use crate::utils::time_signature::load_time_signature;
use crate::utils::version::load_version;
use crate::utils::{decompress_gzip_file, validate_ableton_file, StringResultExt, format_duration};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LiveSet {
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
    pub fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        validate_ableton_file(&file_path)?;

        let file_name: String = load_file_name(&file_path)?;
        let (modified_time, created_time) = load_file_timestamps(&file_path)?;
        let file_hash: String = load_file_hash(&file_path)?;
        let xml_data: Vec<u8> = decompress_gzip_file(&file_path)?;
        let ableton_version: AbletonVersion = load_version(&xml_data)?;
        let time_signature: TimeSignature = load_time_signature(&xml_data)?;

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
        live_set.calculate_duration()?;
        
        Ok(Self {
            samples,
            plugins,
            ..live_set
        })
    }

    pub fn load_plugins(&mut self) -> Result<HashSet<Plugin>, LiveSetError> {
        Ok(find_all_plugins(&self.xml_data)?.into_values().collect())
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

    pub fn load_samples(&self) -> Result<HashSet<Sample>, LiveSetError> {
        #[cfg(debug_assertions)]
        let start_time = Instant::now();

        let sample_paths = parse_sample_paths(&self.xml_data, self.ableton_version.major)?;

        let mut all_samples = HashSet::new();
        for (_, paths) in sample_paths {
            for path in paths {
                let sample = Sample::new(
                    Id::default(), //TODO: generate unique IDs
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path,
                );
                all_samples.insert(sample);
            }
        }


        info!(
            "{}: Total samples after deduplication: {}",
            self.file_name.bold().purple(),
            all_samples.len(),

        );
        #[cfg(debug_assertions)]
        debug!(
            "Finished collecting samples in {:.2} ms.",
            start_time.elapsed().as_secs_f64() * 1000.0
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
                        largest_current_end_value = event
                            .attributes()
                            .flatten()
                            .find(|attr| {
                                attr.key.as_ref().to_string_result().ok()
                                    == Some("Value".to_string())
                            })
                            .ok_or(LiveSetError::AttributeError(AttributeError::ValueNotFound(
                                "CurrentEnd".to_string(),
                            )))
                            .and_then(|attr| {
                                String::from_utf8(attr.value.to_vec()).map_err(|e| {
                                    LiveSetError::XmlError(XmlParseError::Utf8Error(e.utf8_error()))
                                })
                            })
                            .and_then(|value_str| {
                                value_str.parse::<f64>().map_err(|_e| {
                                    LiveSetError::XmlError(XmlParseError::InvalidStructure)
                                })
                            })
                            .map(|value| {
                                if largest_current_end_value.is_nan() {
                                    value
                                } else {
                                    largest_current_end_value.max(value)
                                }
                            })
                            .unwrap_or(largest_current_end_value);
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
        if self.ableton_version.major < 8
            || (self.ableton_version.major == 8 && self.ableton_version.minor < 2)
        {
            return Ok(());
        }

        let previous_tempo: Option<f64> = self.tempo;

        let tempo_value: f64 = if self.ableton_version.major >= 10
            || (self.ableton_version.major == 9 && self.ableton_version.minor >= 7)
        {
            find_post_10_tempo(&self.xml_data)?
        } else {
            find_pre_10_tempo(&self.xml_data)?
        };

        let new_tempo: f64 = ((tempo_value * 1_000_000.0) / 1_000_000.0).round();

        if Some(new_tempo) != previous_tempo {
            self.tempo = Some(new_tempo);
            info_fn!(
                "update_tempo",
                "{} ({:?}): updated tempo from {:?} to {}",
                self.file_name,
                self.id,
                previous_tempo,
                new_tempo
            );
        }

        Ok(())
    }

    pub fn calculate_duration(&mut self) -> Result<(), LiveSetError> {
        if self.tempo.is_none() || self.furthest_bar.is_none() {
            error!(
                "Unable to calculate duration for '{}' (ID: {:?}): missing tempo or furthest bar",
                self.file_name, self.id
            );
            return Ok(());
        }

        let tempo = self.tempo.unwrap();
        let furthest_bar = self.furthest_bar.unwrap();

        if tempo == 0.0 {
            error!(
                "Unable to calculate duration for '{}' (ID: {:?}): tempo is zero",
                self.file_name, self.id
            );
            return Ok(());
        }

        let beats_per_bar = self.time_signature.numerator as f64;
        let duration_seconds = (furthest_bar * beats_per_bar * 60.0) / tempo;

        // Convert to milliseconds for higher precision
        let duration_ms = (duration_seconds * 1000.0).round() as i64;
        let new_duration = Duration::milliseconds(duration_ms);

        if Some(new_duration) != self.estimated_duration {
            self.estimated_duration = Some(new_duration);
            info!(
                "calculate_duration: {} ({:?}): updated duration to {}",
                self.file_name,
                self.id,
                format_duration(&new_duration)
            );
        }

        Ok(())
    }

    //TODO: Add duration estimation (based on furthest bar and tempo)
    //TODO: Add key signature finding

    //TODO: Add fuzzy search function with levenshtein distance
    //TODO: Create 5NF database and translation system

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



impl LiveSet {
    pub(crate) fn log_info(&self) {
        info!("LiveSet Information:");
        info!("ID: {:?}", self.id);
        info!("File Path: {:?}", self.file_path);
        info!("File Name: {}", self.file_name);
        info!("File Hash: {}", self.file_hash);
        info!("Created Time: {}", self.created_time);
        info!("Modified Time: {}", self.modified_time);
        info!("Last Scan Timestamp: {}", self.last_scan_timestamp);
        info!("Ableton Version: {}", self.ableton_version);
        info!("Key Signature: {:?}", self.key_signature);
        info!("Tempo: {:?} BPM", self.tempo);
        info!(
            "Time Signature: {}/{}",
            self.time_signature.numerator, self.time_signature.denominator
        );
        info!(
            "Estimated Duration: {}",
            self.estimated_duration
                .as_ref()
                .map_or_else(|| "Not calculated".to_string(), format_duration)
        );
        info!("Furthest Bar: {:?}", self.furthest_bar);
        info!("Number of Plugins: {}", self.plugins.len());
        info!("Number of Samples: {}", self.samples.len());

        if !self.plugins.is_empty() {
            info!("Plugins:");
            for plugin in &self.plugins {
                info!(
                    "  - {} ({})",
                    plugin.name,
                    if plugin.installed {
                        "Installed"
                    } else {
                        "Not Installed"
                    }
                );
            }
        }

        if !self.samples.is_empty() {
            info!("Samples:");
            for sample in &self.samples {
                info!(
                    "  - {} ({})",
                    sample.name,
                    if sample.is_present { "Present" } else { "Missing" }
                );
            }
        }
    }
}

