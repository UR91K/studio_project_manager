// /src/live_set.rs
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Duration, Local};
use colored::Colorize;
use log::{debug, error, info};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{AttributeError, LiveSetError, XmlParseError};
use crate::{debug_fn, error_fn, info_fn, trace_fn, warn_fn};
use crate::models::{AbletonVersion, Id, KeySignature, Plugin, PluginInfo, Sample, TimeSignature};
use crate::utils::metadata::{load_file_hash, load_file_name, load_file_timestamps};
use crate::utils::plugins::{find_all_plugins, get_most_recent_db_file, LineTrackingBuffer, parse_plugin_format};
use crate::utils::samples::parse_sample_paths;
use crate::utils::tempo::{find_post_10_tempo, find_pre_10_tempo};
use crate::utils::time_signature::load_time_signature;
use crate::utils::version::load_version;
use crate::utils::{decompress_gzip_file, validate_ableton_file, StringResultExt, format_duration, EventExt};
use crate::utils::xml_parsing::get_value_as_string_result;

// Define a new struct to hold the scanned data
#[allow(dead_code)]
pub struct LiveSetData {
    ableton_version: AbletonVersion,
    time_signature: TimeSignature,
    plugins: HashSet<Plugin>,
    samples: HashSet<Sample>,
    tempo: Option<f64>,
    furthest_bar: Option<f64>,
    // Add other fields as needed
}

#[allow(dead_code)]
impl LiveSetData {
    // Constructor with required fields
    fn new(ableton_version: AbletonVersion, time_signature: TimeSignature) -> Self {
        Self {
            ableton_version,
            time_signature,
            plugins: HashSet::new(),
            samples: HashSet::new(),
            tempo: None,
            furthest_bar: None,
        }
    }
}

#[allow(dead_code)]
pub struct ScanOptions {
    pub scan_plugins: bool,
    pub scan_samples: bool,
    pub scan_tempo: bool,
    pub scan_time_signature: bool,
    pub scan_midi: bool,
    pub scan_audio: bool,
    pub scan_automation: bool,
    pub scan_return_tracks: bool,
    pub scan_master_track: bool,
    pub estimate_duration: bool,
    pub calculate_furthest_bar: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            scan_plugins: true,
            scan_samples: true,
            scan_tempo: true,
            scan_time_signature: true,
            scan_midi: true,
            scan_audio: true,
            scan_automation: true,
            scan_return_tracks: true,
            scan_master_track: true,
            estimate_duration: true,
            calculate_furthest_bar: true,
        }
    }
}

#[allow(dead_code)]
impl ScanOptions {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn plugins_only() -> Self {
        ScanOptions {
            scan_plugins: true,
            ..Default::default()
        }
    }

    pub fn samples_only() -> Self {
        ScanOptions {
            scan_samples: true,
            ..Default::default()
        }
    }
}

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
    #[allow(dead_code, unused_variables)]
    pub fn scan(&mut self, options: &ScanOptions) -> Result<(), LiveSetError> {
        let mut reader = Reader::from_reader(&self.xml_data[..]);
        reader.trim_text(true);
        let mut buf = Vec::new();

        let mut in_source_context = false;
        let mut current_branch_info: Option<String> = None;
        let mut plugin_infos: HashMap<String, PluginInfo> = HashMap::new();
        let dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>> = Arc::new(parking_lot::RwLock::new(HashMap::new()));

        let mut line_tracker = LineTrackingBuffer::new(self.xml_data.clone());

        loop {
            let mut byte_pos = reader.buffer_position();
            let line = line_tracker.get_line_number(byte_pos);
            
            
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    match e.name().to_str_result()? {
                        //PLUGIN
                        "SourceContext" if options.scan_plugins => {
                            in_source_context = true;
                        },
                        "BranchSourceContext" if in_source_context && options.scan_plugins => {
                            current_branch_info = self.handle_branch_source_context(
                                &mut reader,
                                &mut byte_pos,
                                &mut line_tracker,
                                &dev_identifiers,
                            )?;
                        },
                        "PluginDesc" if in_source_context && options.scan_plugins => {
                            if let Some(device_id) = current_branch_info.take() {
                                if let Some(plugin_info) = self.handle_plugin_desc(
                                    &mut reader,
                                    device_id.clone(),
                                    &mut byte_pos,
                                    &mut line_tracker,
                                )? {
                                    plugin_infos.insert(device_id, plugin_info);
                                }
                            }
                        },
                        
                        //
                        _ => {}
                    }
                },
                Ok(Event::Empty(ref e)) => {
                    match e.name().to_string_result()? {
                        _ => {}
                    }
                },
                Ok(Event::End(ref e)) => {
                    match e.name().to_str_result()? {
                        //PLUGIN
                        "SourceContext" => {
                            in_source_context = false;
                        },
                        
                        
                        _ => {}
                    }
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e))),
                _ => {},
            }
            buf.clear();
        }

        if options.scan_plugins {
            self.process_plugin_infos(plugin_infos)?;
        }
        
        if options.estimate_duration {
            self.estimate_duration()?;
        }

        if options.calculate_furthest_bar {
            self.calculate_furthest_bar()?;
        }

        Ok(())
    }
    #[allow(dead_code)]
    fn handle_branch_source_context(
        &self,
        reader: &mut Reader<&[u8]>,
        byte_pos: &mut usize,
        line_tracker: &mut LineTrackingBuffer,
        dev_identifiers: &Arc<parking_lot::RwLock<HashMap<String, ()>>>,
    ) -> Result<Option<String>, LiveSetError> {
        // Adapt the logic from parse_branch_source_context
        let mut line = line_tracker.get_line_number(*byte_pos);
        trace_fn!(
            "handle_branch_source_context",
            "[{}] Starting function",
            line.to_string().yellow()
        );

        let mut buf = Vec::new();
        let mut browser_content_path = false;
        let mut branch_device_id = None;
        let mut read_count = 0;

        loop {
            *byte_pos = reader.buffer_position();
            line = line_tracker.get_line_number(*byte_pos);

            if read_count > 2 && !browser_content_path {
                trace_fn!(
                    "handle_branch_source_context",
                    "[{}] No BrowserContentPath found within first 2 reads; returning early",
                    line.to_string().yellow()
                );
                return Ok(None);
            }

            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref event)) => {
                    read_count += 1;

                    let name = event.name().to_string_result()
                        .map_err(|e| LiveSetError::XmlError(e))?;
                    match name.as_str() {
                        "BranchDeviceId" => {
                            if let Some(device_id) = event.get_value_as_string_result()
                                .map_err(|e| LiveSetError::XmlError(e))? {
                                let mut identifiers = dev_identifiers.write();
                                if identifiers.contains_key(&device_id) {
                                    trace_fn!(
                                        "handle_branch_source_context",
                                        "[{}] Duplicate device ID found: {}",
                                        line.to_string().yellow(),
                                        device_id
                                    );
                                    return Ok(None);
                                } else {
                                    identifiers.insert(device_id.clone(), ());
                                    branch_device_id = Some(device_id);
                                }
                            }
                            break;
                        }

                        "BrowserContentPath" => {
                            trace_fn!(
                                "handle_branch_source_context",
                                "[{}] Found BrowserContentPath",
                                line.to_string().yellow()
                            );
                            browser_content_path = true;
                        }

                        _ => {}
                    }
                }

                Ok(Event::End(_)) => {
                    break;
                }

                Ok(Event::Eof) => {
                    warn_fn!(
                        "handle_branch_source_context",
                        "[{}] EOF reached",
                        line.to_string().yellow()
                    );
                    return Err(LiveSetError::XmlError(XmlParseError::InvalidStructure));
                }

                Err(e) => {
                    error_fn!(
                        "handle_branch_source_context",
                        "[{}] Error parsing XML: {:?}",
                        line,
                        e
                    );
                    return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e)));
                }

                _ => (),
            }
        }

        *byte_pos = reader.buffer_position();
        line_tracker.update_position(*byte_pos);

        if !browser_content_path {
            trace_fn!("handle_branch_source_context", "Missing BrowserContentPath");
            return Ok(None);
        }

        let Some(device_id) = branch_device_id else {
            return Ok(None);
        };

        if !device_id.starts_with("device:vst:") && !device_id.starts_with("device:vst3:") {
            trace_fn!("handle_branch_source_context", "Not a VST/VST3 plugin");
            return Ok(None);
        }

        let vst_type = if device_id.contains("vst3:") {
            "VST 3"
        } else if device_id.contains("vst:") {
            "VST 2"
        } else {
            ""
        };

        trace_fn!(
            "handle_branch_source_context",
            "[{}] Found valid {} plugin info",
            line.to_string().yellow(),
            vst_type,
        );
        Ok(Some(device_id))
    }


    #[allow(dead_code, unused_variables)]
    fn handle_plugin_desc(
        &self,
        reader: &mut Reader<&[u8]>,
        device_id: String,
        byte_pos: &mut usize,
        line_tracker: &mut LineTrackingBuffer,
    ) -> Result<Option<PluginInfo>, LiveSetError> {
        let mut line = line_tracker.get_line_number(*byte_pos);
        trace_fn!(
            "handle_plugin_desc", 
            "[{}] Starting function", 
            line.to_string().yellow()
        );

        let mut buf = Vec::new();
        let plugin_name;
        let mut depth = 0;

        loop {
            *byte_pos = reader.buffer_position();
            line = line_tracker.get_line_number(*byte_pos);

            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref _event)) => {
                    depth += 1;
                }

                Ok(Event::Empty(ref event)) => {
                    let name = event.name().to_string_result()
                        .map_err(|e| LiveSetError::XmlError(e))?;

                    if (name == "PlugName" || name == "Name") && depth == 1 {
                        if let Some(value) = get_value_as_string_result(event)
                            .map_err(|e| LiveSetError::XmlError(e))? {
                            plugin_name = Some(value);

                            trace_fn!(
                                "handle_plugin_desc",
                                "[{}] Found plugin name: {}",
                                line.to_string().yellow(),
                                plugin_name.as_deref().unwrap_or("None"),
                            );
                            break;
                        }
                    }
                }

                Ok(Event::End(ref _event)) => {
                    depth -= 1;
                }

                Ok(Event::Eof) => {
                    error_fn!(
                        "handle_plugin_desc",
                        "Found unexpected end of file while parsing"
                    );
                    return Err(LiveSetError::XmlError(XmlParseError::InvalidStructure));
                }

                Err(e) => {
                    return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e)));
                }

                _ => (),
            }
        }

        *byte_pos = reader.buffer_position();
        line_tracker.update_position(*byte_pos);

        if let Some(name) = plugin_name {
            let plugin_format = parse_plugin_format(&device_id)
                .ok_or_else(|| LiveSetError::XmlError(XmlParseError::UnknownPluginFormat(device_id.clone())))?;

            let plugin_info = Some(PluginInfo {
                name,
                dev_identifier: device_id,
                plugin_format,
            });
            trace_fn!(
                "handle_plugin_desc",
                "[{}] Successfully collected plugin info: {}",
                line.to_string().yellow(),
                plugin_info.as_ref().map_or("No plugin info available".to_string(), |info| info.to_string())
            );

            Ok(plugin_info)
        } else {
            warn_fn!("handle_plugin_desc", "Missing plugin name");
            Ok(None)
        }
    }
    #[allow(dead_code)]
    fn process_plugin_infos(&mut self, plugin_infos: HashMap<String, PluginInfo>) -> Result<(), LiveSetError> {
        debug_fn!("process_plugin_infos", "{}", "Starting function".bold().purple());
        trace_fn!(
            "process_plugin_infos",
            "Processing {} plugin infos",
            plugin_infos.len()
        );

        let config = CONFIG
            .as_ref()
            .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        trace_fn!("process_plugin_infos", "Database directory: {:?}", db_dir);

        let db_path = get_most_recent_db_file(&PathBuf::from(db_dir))
            .map_err(|e| LiveSetError::DatabaseError(e))?;
        trace_fn!("process_plugin_infos", "Using database file: {:?}", db_path);

        let ableton_db = AbletonDatabase::new(db_path)
            .map_err(|e| LiveSetError::DatabaseError(e))?;

        let mut unique_plugins: HashMap<String, Plugin> = HashMap::new();

        for (dev_identifier, info) in plugin_infos.iter() {
            trace_fn!(
                "process_plugin_infos",
                "Processing plugin info: {:?}",
                dev_identifier
            );
            let db_plugin = ableton_db.get_plugin_by_dev_identifier(dev_identifier)
                .map_err(|e| LiveSetError::DatabaseError(e))?;
            let plugin = match db_plugin {
                Some(db_plugin) => {
                    debug_fn!(
                        "process_plugin_infos",
                        "Found plugin {} {} on system, flagging as installed",
                        db_plugin.vendor.as_deref().unwrap_or("Unknown").purple(),
                        db_plugin.name.green()
                    );
                    Plugin {
                        plugin_id: Some(db_plugin.plugin_id),
                        module_id: db_plugin.module_id,
                        dev_identifier: db_plugin.dev_identifier.clone(),
                        name: db_plugin.name.clone(),
                        vendor: db_plugin.vendor.clone(),
                        version: db_plugin.version.clone(),
                        sdk_version: db_plugin.sdk_version.clone(),
                        flags: db_plugin.flags,
                        scanstate: db_plugin.scanstate,
                        enabled: db_plugin.enabled,
                        plugin_format: info.plugin_format,
                        installed: true,
                    }
                }
                None => {
                    debug!("Plugin not found in database: {:?}", info);
                    Plugin {
                        plugin_id: None,
                        module_id: None,
                        dev_identifier: dev_identifier.clone(),
                        name: info.name.clone(),
                        vendor: None,
                        version: None,
                        sdk_version: None,
                        flags: None,
                        scanstate: None,
                        enabled: None,
                        plugin_format: info.plugin_format,
                        installed: false,
                    }
                }
            };
            unique_plugins
                .entry(plugin.dev_identifier.clone())
                .and_modify(|existing| {
                    if existing.plugin_id.is_none() && plugin.plugin_id.is_some() {
                        *existing = plugin.clone();
                    }
                })
                .or_insert(plugin);
        }

        debug!("Found {} plugins", unique_plugins.len());
        
        self.plugins = unique_plugins.into_values().collect();

        Ok(())
    }
    #[allow(dead_code)]
    fn estimate_duration(&mut self) -> Result<(), LiveSetError> {
        // Implementation for estimating duration
        unimplemented!()
    }
    #[allow(dead_code)]
    fn calculate_furthest_bar(&mut self) -> Result<(), LiveSetError> {
        // Implementation for calculating furthest bar
        unimplemented!()
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

