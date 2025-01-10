use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::io::BufRead;
#[allow(unused_imports)]
use log::{debug, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;
use colored::Colorize;

use crate::error::LiveSetError;
use crate::models::{AbletonVersion, Id, Plugin, PluginInfo, Sample, TimeSignature, KeySignature, Scale, Tonic};
use crate::utils::plugins::LineTrackingBuffer;
use crate::utils::{StringResultExt, EventExt};
use crate::config::CONFIG;
use crate::ableton_db::AbletonDatabase;
use crate::utils::plugins::get_most_recent_db_file;
#[allow(unused_imports)]
use crate::{debug_fn, trace_fn, warn_fn};

#[derive(Debug, Clone, PartialEq)]
pub enum PathType {
    Direct,    // For version >= 11
    Encoded,   // For version < 11
}

/// Represents what type of data we're currently scanning
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ScannerState {
    Root,
    
    // Sample scanning states
    InSampleRef {
        version: u32,
    },
    InFileRef,
    InData {
        current_data: String,
    },
    InPath {
        path_type: PathType,
    },
    
    // Plugin states
    InSourceContext,
    InValue,
    InBranchSourceContext,
    InPluginDesc {
        device_id: String,
    },
    InVst3PluginInfo,
    InVstPluginInfo,
    
    // Tempo states
    InTempo {
        version: u32,
    },
    InTempoManual,
    
    // Time signature state
    InTimeSignature,

    // Key scanning states
    InMidiClip,
    InScaleInformation,
}

/// Configuration for what should be scanned
#[derive(Debug, Clone)]
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
    pub scan_key: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
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
            scan_key: true,
        }
    }
}

/// Holds the results of the scanning process
#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct ScanResult {
    pub(crate) version: AbletonVersion,
    pub(crate) samples: HashSet<Sample>,
    pub(crate) plugins: HashSet<Plugin>,
    pub(crate) tempo: f64,
    pub(crate) time_signature: TimeSignature,
    pub(crate) furthest_bar: Option<f64>,
    pub(crate) key_signature: KeySignature,
}

/// The main scanner that processes the XML data
#[allow(dead_code)]
pub struct Scanner {
    // Core scanner state
    pub(crate) state: ScannerState,
    pub(crate) depth: i32,
    pub(crate) ableton_version: AbletonVersion,
    pub(crate) options: ScanOptions,
    pub(crate) line_tracker: LineTrackingBuffer,
    
    // Sample scanning state
    pub(crate) sample_paths: HashSet<PathBuf>,
    pub(crate) current_sample_data: Option<String>,
    pub(crate) current_file_ref: Option<PathBuf>,  // Tracks the current file reference being processed
    pub(crate) current_path_type: Option<PathType>, // Tracks whether we're processing a direct or encoded path
    
    // Plugin scanning state
    pub(crate) current_branch_info: Option<String>,
    pub(crate) plugin_info_tags: HashMap<String, PluginInfo>,
    pub(crate) in_source_context: bool,
    pub(crate) plugin_info_processed: bool,
    
    // Tempo and timing state
    pub(crate) dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>>,
    pub(crate) current_tempo: Option<f64>,
    pub(crate) current_time_signature: Option<TimeSignature>,
    pub(crate) current_end_times: Vec<f64>,

    // Initialize key scanning state
    pub(crate) key_frequencies: HashMap<KeySignature, usize>,
    current_scale_info: Option<(Tonic, Scale)>,
    current_clip_in_key: bool,
}

#[allow(dead_code)]
impl Scanner {
    pub fn new(xml_data: &[u8], options: ScanOptions) -> Result<Self, LiveSetError> {
        // First, detect and validate the version
        let version = Self::detect_version(xml_data)?;
        
        Ok(Self {
            state: ScannerState::Root,
            depth: 0,
            ableton_version: version,
            options,
            line_tracker: LineTrackingBuffer::new(xml_data.to_vec()),
            
            // Initialize sample scanning state
            sample_paths: HashSet::new(),
            current_sample_data: None,
            current_file_ref: None,
            current_path_type: None,
            
            // Initialize plugin scanning state
            current_branch_info: None,
            plugin_info_tags: HashMap::new(),
            in_source_context: false,
            plugin_info_processed: false,
            
            // Initialize other state
            dev_identifiers: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            current_tempo: None,
            current_time_signature: None,
            current_end_times: Vec::new(),

            // Initialize key scanning state
            key_frequencies: HashMap::new(),
            current_scale_info: None,
            current_clip_in_key: false,
        })
    }

    /// Detects the Ableton Live version from the XML data
    fn detect_version(xml_data: &[u8]) -> Result<AbletonVersion, LiveSetError> {
        let mut reader = Reader::from_reader(xml_data);
        reader.trim_text(true);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"Ableton" {
                        // Get MinorVersion attribute which contains the actual version info
                        let version_str = e
                            .try_get_attribute("MinorVersion")?
                            .ok_or(LiveSetError::MissingVersion)?
                            .unescape_value()?;

                        // Parse version components from MinorVersion (format: "12.0_12049")
                        let parts: Vec<&str> = version_str.split('_').collect();
                        if parts.len() != 2 {
                            return Err(LiveSetError::InvalidVersion(version_str.to_string()));
                        }

                        let version_parts: Vec<&str> = parts[0].split('.').collect();
                        if version_parts.len() != 2 {
                            return Err(LiveSetError::InvalidVersion(version_str.to_string()));
                        }

                        // Parse major, minor, and patch versions
                        let major: u32 = version_parts[0]
                            .parse()
                            .map_err(|_| LiveSetError::InvalidVersion(version_str.to_string()))?;
                        let minor: u32 = version_parts[1]
                            .parse()
                            .map_err(|_| LiveSetError::InvalidVersion(version_str.to_string()))?;
                        let patch: u32 = parts[1]
                            .parse()
                            .map_err(|_| LiveSetError::InvalidVersion(version_str.to_string()))?;

                        // Get beta status from SchemaChangeCount
                        let beta = e
                            .try_get_attribute("SchemaChangeCount")
                            .ok()
                            .flatten()
                            .map(|attr| attr.unescape_value())
                            .transpose()?
                            .map(|v| v == "beta")
                            .unwrap_or(false);

                        // Validate major version
                        if major < 9 || major > 12 {
                            return Err(LiveSetError::UnsupportedVersion(major));
                        }

                        return Ok(AbletonVersion {
                            major,
                            minor,
                            patch,
                            beta,
                        });
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(LiveSetError::from(e)),
                _ => {}
            }
            buf.clear();
        }
        Err(LiveSetError::MissingVersion)
    }

    /// Main scanning function that processes the XML data
    pub(crate) fn scan(&mut self, xml_data: &[u8]) -> Result<ScanResult, LiveSetError> {
        let mut reader = Reader::from_reader(xml_data);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let mut byte_pos;  // Will be set in the loop
        let result = ScanResult::default();

        // Skip the version tag since we've already processed it
        let mut skip_first = true;

        #[allow(unused_variables)]
        loop {
            byte_pos = reader.buffer_position();
            let line = self.line_tracker.get_line_number(byte_pos);

            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) => {
                    if skip_first && event.name().as_ref() == b"Ableton" {
                        skip_first = false;
                        continue;
                    }
                    self.depth += 1;
                    self.handle_start_event(event, &mut reader, &mut byte_pos)?;
                }

                Ok(Event::Empty(ref event)) => {
                    if skip_first && event.name().as_ref() == b"Ableton" {
                        skip_first = false;
                        continue;
                    }
                    self.handle_start_event(event, &mut reader, &mut byte_pos)?;
                }

                Ok(Event::Text(ref event)) => {
                    self.handle_text_event(event)?;
                }
                Ok(Event::End(ref event)) => {
                    self.handle_end_event(event)?;
                    self.depth -= 1;
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn_fn!(
                        "scan",
                        "Error at line {}: {:?}",
                        line,
                        e
                    );
                    return Err(LiveSetError::from(e));
                }
                _ => {}
            }
            buf.clear();
        }

        // Convert collected data into final result
        self.finalize_result(result)
    }

    /// Converts the scanner's state into the final ScanResult
    #[cfg(test)]
    pub(crate) fn finalize_result(&self, mut result: ScanResult) -> Result<ScanResult, LiveSetError> {
        // Set the version
        result.version = self.ableton_version;

        // Validate and set tempo (required for a valid project)
        match self.current_tempo {
            Some(tempo) if tempo > 0.0 => {
                result.tempo = tempo;
            }
            Some(tempo) => {
                return Err(LiveSetError::InvalidProject(
                    format!("Invalid tempo value: {}", tempo)
                ));
            }
            None => {
                return Err(LiveSetError::InvalidProject(
                    "No tempo found in project".into()
                ));
            }
        }

        // Validate and set time signature (required for a valid project)
        match &self.current_time_signature {
            Some(time_sig) if time_sig.numerator > 0 && time_sig.denominator > 0 => {
                result.time_signature = time_sig.clone();
            }
            Some(time_sig) => {
                return Err(LiveSetError::InvalidProject(
                    format!("Invalid time signature: {}/{}", time_sig.numerator, time_sig.denominator)
                ));
            }
            None => {
                return Err(LiveSetError::InvalidProject(
                    "No time signature found in project".into()
                ));
            }
        }

        // Calculate furthest bar if requested and we have end times
        if self.options.calculate_furthest_bar && !self.current_end_times.is_empty() {
            // We can now use result.time_signature directly since it's guaranteed to be valid
            let beats_per_bar = result.time_signature.numerator as f64;
            let max_end_time = self.current_end_times.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            result.furthest_bar = Some(max_end_time / beats_per_bar);
            
            debug_fn!(
                "finalize_result",
                "Calculated furthest bar: {} (max end time: {}, beats per bar: {})",
                result.furthest_bar.unwrap(),
                max_end_time,
                beats_per_bar
            );
        }

        // Convert sample paths to Sample structs
        for path in &self.sample_paths {
            result.samples.insert(Sample::new(
                Id::default(),
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path.clone(),
            ));
        }

        // Convert plugin info tags to Plugin instances
        let config = CONFIG
            .as_ref()
            .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        let db_path = get_most_recent_db_file(&PathBuf::from(db_dir))
            .map_err(LiveSetError::DatabaseError)?;
        let ableton_db = AbletonDatabase::new(db_path)
            .map_err(LiveSetError::DatabaseError)?;

        for (dev_identifier, info) in &self.plugin_info_tags {
            let db_plugin = ableton_db
                .get_plugin_by_dev_identifier(dev_identifier)
                .map_err(LiveSetError::DatabaseError)?;

            let plugin = match db_plugin {
                Some(db_plugin) => {
                    debug_fn!(
                        "finalize_result",
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
                    debug_fn!(
                        "finalize_result",
                        "Plugin not found in database: {:?}",
                        info
                    );
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
            result.plugins.insert(plugin);
        }

        // Handle key signature if requested
        if self.options.scan_key {
            if self.ableton_version.major < 11 {
                debug_fn!(
                    "finalize_result",
                    "Version {} is less than 11, defaulting to Empty key signature",
                    self.ableton_version
                );
                result.key_signature = KeySignature::default();
            } else {
                // Find the most frequent key signature
                let most_frequent_key = self.key_frequencies
                    .iter()
                    .max_by_key(|&(_, count)| count)
                    .map(|(key, count)| {
                        debug_fn!(
                            "finalize_result",
                            "Found most frequent key signature: {} (count: {})",
                            key,
                            count
                        );
                        key.clone()
                    })
                    .unwrap_or_else(|| {
                        debug_fn!(
                            "finalize_result",
                            "No key signatures found, using default"
                        );
                        KeySignature::default()
                    });
                
                result.key_signature = most_frequent_key;
            }
        } else {
            result.key_signature = KeySignature::default();
        }

        Ok(result)
    }

    /// Converts the scanner's state into the final ScanResult
    #[cfg(not(test))]
    fn finalize_result(&self, mut result: ScanResult) -> Result<ScanResult, LiveSetError> {
        // Set the version
        result.version = self.ableton_version;

        // Validate and set tempo (required for a valid project)
        match self.current_tempo {
            Some(tempo) if tempo > 0.0 => {
                result.tempo = tempo;
            }
            Some(tempo) => {
                return Err(LiveSetError::InvalidProject(
                    format!("Invalid tempo value: {}", tempo)
                ));
            }
            None => {
                return Err(LiveSetError::InvalidProject(
                    "No tempo found in project".into()
                ));
            }
        }

        // Validate and set time signature (required for a valid project)
        match &self.current_time_signature {
            Some(time_sig) if time_sig.numerator > 0 && time_sig.denominator > 0 => {
                result.time_signature = time_sig.clone();
            }
            Some(time_sig) => {
                return Err(LiveSetError::InvalidProject(
                    format!("Invalid time signature: {}/{}", time_sig.numerator, time_sig.denominator)
                ));
            }
            None => {
                return Err(LiveSetError::InvalidProject(
                    "No time signature found in project".into()
                ));
            }
        }

        // Calculate furthest bar if requested and we have end times
        if self.options.calculate_furthest_bar && !self.current_end_times.is_empty() {
            // We can now use result.time_signature directly since it's guaranteed to be valid
            let beats_per_bar = result.time_signature.numerator as f64;
            let max_end_time = self.current_end_times.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            result.furthest_bar = Some(max_end_time / beats_per_bar);
            
            debug_fn!(
                "finalize_result",
                "Calculated furthest bar: {} (max end time: {}, beats per bar: {})",
                result.furthest_bar.unwrap(),
                max_end_time,
                beats_per_bar
            );
        }

        // Convert sample paths to Sample structs
        for path in &self.sample_paths {
            result.samples.insert(Sample::new(
                Id::default(),
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path.clone(),
            ));
        }

        // Convert plugin info tags to Plugin instances
        let config = CONFIG
            .as_ref()
            .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        let db_path = get_most_recent_db_file(&PathBuf::from(db_dir))
            .map_err(LiveSetError::DatabaseError)?;
        let ableton_db = AbletonDatabase::new(db_path)
            .map_err(LiveSetError::DatabaseError)?;

        for (dev_identifier, info) in &self.plugin_info_tags {
            let db_plugin = ableton_db
                .get_plugin_by_dev_identifier(dev_identifier)
                .map_err(LiveSetError::DatabaseError)?;

            let plugin = match db_plugin {
                Some(db_plugin) => {
                    debug_fn!(
                        "finalize_result",
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
                    debug_fn!(
                        "finalize_result",
                        "Plugin not found in database: {:?}",
                        info
                    );
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
            result.plugins.insert(plugin);
        }

        // Handle key signature if requested
        if self.options.scan_key {
            if self.ableton_version.major < 11 {
                debug_fn!(
                    "finalize_result",
                    "Version {} is less than 11, defaulting to Empty key signature",
                    self.ableton_version
                );
                result.key_signature = KeySignature::default();
            } else {
                // Find the most frequent key signature
                let most_frequent_key = self.key_frequencies
                    .iter()
                    .max_by_key(|&(_, count)| count)
                    .map(|(key, count)| {
                        debug_fn!(
                            "finalize_result",
                            "Found most frequent key signature: {} (count: {})",
                            key,
                            count
                        );
                        key.clone()
                    })
                    .unwrap_or_else(|| {
                        debug_fn!(
                            "finalize_result",
                            "No key signatures found, using default"
                        );
                        KeySignature::default()
                    });
                
                result.key_signature = most_frequent_key;
            }
        } else {
            result.key_signature = KeySignature::default();
        }

        Ok(result)
    }

    pub(crate) fn handle_start_event<R: BufRead>(
        &mut self,
        event: &quick_xml::events::BytesStart,
        reader: &mut Reader<R>,
        byte_pos: &mut usize,
    ) -> Result<(), LiveSetError> {
        let name = event.name().to_string_result()?;
        let line = self.line_tracker.get_line_number(*byte_pos);
        
        trace_fn!(
            "handle_start_event",
            "[{}] Processing tag: {}, state: {:?}, depth: {}",
            line,
            name,
            self.state,
            self.depth
        );
        
        match name.as_str() {
            "SampleRef" => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Entering SampleRef at depth {}, version: {}",
                    line,
                    self.depth,
                    self.ableton_version.major
                );
                self.state = ScannerState::InSampleRef {
                    version: self.ableton_version.major,
                };
            }
            "FileRef" if matches!(self.state, ScannerState::InSampleRef { .. }) => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Entering FileRef at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InFileRef;
            }
            "Data" if matches!(self.state, ScannerState::InFileRef) && self.ableton_version.major < 11 => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Found Data tag for old format sample at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InData {
                    current_data: String::new(),
                };
                self.current_path_type = Some(PathType::Encoded);
            }
            "Path" if matches!(self.state, ScannerState::InFileRef) && self.ableton_version.major >= 11 => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Found Path tag for new format sample at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InPath {
                    path_type: PathType::Direct,
                };
                self.current_path_type = Some(PathType::Direct);

                // Extract the path value from the Value attribute
                if let Some(value) = event.try_get_attribute("Value")? {
                    let path_str = value.unescape_value()?.to_string();
                    debug_fn!(
                        "handle_start_event",
                        "[{}] Found sample path: {}",
                        line,
                        path_str
                    );
                    self.current_file_ref = Some(PathBuf::from(path_str));
                }
            }
            "SourceContext" => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering SourceContext at depth {}",
                    line,
                    self.depth
                );
                self.in_source_context = true;
                if !matches!(self.state, ScannerState::InPluginDesc { .. }) {
                    self.state = ScannerState::InSourceContext;
                }
            }
            "Value" if matches!(self.state, ScannerState::InSourceContext) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering Value tag inside SourceContext at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InValue;
            }
            "BranchSourceContext" if matches!(self.state, ScannerState::InValue) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Found BranchSourceContext at depth {}, looking for device ID",
                    line,
                    self.depth
                );
                self.state = ScannerState::InBranchSourceContext;
                
                // Look ahead for BrowserContentPath and BranchDeviceId
                let mut buf = Vec::new();
                let mut found_browser_content_path = false;
                let mut device_id = None;
                let mut found_nested_plugin_desc = false;
                let start_depth = self.depth;

                loop {
                    *byte_pos = reader.buffer_position();
                    let line = self.line_tracker.get_line_number(*byte_pos);

                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Empty(ref event)) => {
                            let tag_name = event.name().to_string_result()?;
                            match tag_name.as_str() {
                                "BrowserContentPath" => {
                                    debug_fn!(
                                        "handle_start_event",
                                        "[{}] Found BrowserContentPath at depth {}",
                                        line,
                                        self.depth
                                    );
                                    found_browser_content_path = true;
                                }
                                "BranchDeviceId" => {
                                    if let Some(id) = event.get_value_as_string_result()? {
                                        debug_fn!(
                                            "handle_start_event",
                                            "[{}] Found device ID at depth {}: {}",
                                            line,
                                            self.depth,
                                            id
                                        );
                                        device_id = Some(id);
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok(Event::Start(ref e)) => {
                            let tag_name = e.name().to_string_result()?;
                            // Only consider PluginDesc nested if it's at a deeper depth
                            if tag_name == "PluginDesc" && self.depth > start_depth {
                                debug_fn!(
                                    "handle_start_event",
                                    "[{}] Found nested PluginDesc at depth {}, ignoring device ID",
                                    line,
                                    self.depth
                                );
                                found_nested_plugin_desc = true;
                                // Skip this tag and its contents
                                let mut nested_depth = 1;
                                while nested_depth > 0 {
                                    match reader.read_event_into(&mut buf) {
                                        Ok(Event::Start(_)) => nested_depth += 1,
                                        Ok(Event::End(_)) => nested_depth -= 1,
                                        Ok(Event::Eof) => break,
                                        Err(e) => return Err(LiveSetError::from(e)),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Ok(Event::End(ref e)) => {
                            let end_name = e.name().to_string_result()?;
                            if end_name == "BranchSourceContext" && self.depth <= start_depth {
                                debug_fn!(
                                    "handle_start_event",
                                    "[{}] Exiting BranchSourceContext look-ahead at depth {}",
                                    line,
                                    self.depth
                                );
                                break;
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(e) => return Err(LiveSetError::from(e)),
                        _ => {}
                    }
                    buf.clear();
                }

                // Store device ID if we found a browser content path and it's a valid plugin
                // and we didn't find a nested PluginDesc
                if found_browser_content_path && !found_nested_plugin_desc {
                    if let Some(id) = device_id {
                        if id.starts_with("device:vst:") || id.starts_with("device:vst3:") {
                            debug_fn!(
                                "handle_start_event",
                                "[{}] Storing valid plugin device ID at depth {}: {}",
                                line,
                                self.depth,
                                id
                            );
                            self.current_branch_info = Some(id);
                        } else {
                            trace_fn!(
                                "handle_start_event",
                                "[{}] Ignoring non-plugin device ID at depth {}: {}",
                                line,
                                self.depth,
                                id
                            );
                        }
                    }
                }
            }
            "PluginDesc" => {
                if let Some(device_id) = &self.current_branch_info {
                    debug_fn!(
                        "handle_start_event",
                        "[{}] Entering PluginDesc at depth {} for device: {}",
                        line,
                        self.depth,
                        device_id
                    );
                    self.plugin_info_processed = false;  // Reset the flag for new PluginDesc
                    self.state = ScannerState::InPluginDesc { device_id: device_id.clone() };
                } else {
                    trace_fn!(
                        "handle_start_event",
                        "[{}] Found PluginDesc at depth {} but no current device ID",
                        line,
                        self.depth
                    );
                }
            }
            "Vst3PluginInfo" | "VstPluginInfo" => {
                if let ScannerState::InPluginDesc { device_id } = &self.state {
                    if self.plugin_info_processed {
                        debug_fn!(
                            "handle_start_event",
                            "[{}] Ignoring subsequent plugin info tag at depth {}: {} for device: {} (already processed)",
                            line,
                            self.depth,
                            name,
                            device_id
                        );
                    } else {
                        debug_fn!(
                            "handle_start_event",
                            "[{}] Found plugin info tag at depth {}: {} for device: {}",
                            line,
                            self.depth,
                            name,
                            device_id
                        );
                        self.state = if name.as_str() == "Vst3PluginInfo" {
                            ScannerState::InVst3PluginInfo
                        } else {
                            ScannerState::InVstPluginInfo
                        };
                    }
                } else {
                    trace_fn!(
                        "handle_start_event",
                        "[{}] Found plugin info tag at depth {} but not in PluginDesc state: {:?}",
                        line,
                        self.depth,
                        self.state
                    );
                }
            }
            "Name" | "PlugName" => {
                if let Some(value) = event.get_value_as_string_result()? {
                    match self.state {
                        ScannerState::InVst3PluginInfo | ScannerState::InVstPluginInfo => {
                            if !self.plugin_info_processed {
                                if let Some(device_id) = &self.current_branch_info {
                                    if let Some(plugin_format) = crate::utils::plugins::parse_plugin_format(device_id) {
                                        debug_fn!(
                                            "handle_start_event",
                                            "[{}] Found plugin name at depth {}: {} for device: {}",
                                            line,
                                            self.depth,
                                            value,
                                            device_id
                                        );
                                        let plugin_info = PluginInfo {
                                            name: value,
                                            dev_identifier: device_id.clone(),
                                            plugin_format,
                                        };
                                        debug_fn!(
                                            "handle_start_event",
                                            "[{}] Adding plugin info at depth {}: {:?}",
                                            line,
                                            self.depth,
                                            plugin_info
                                        );
                                        self.plugin_info_tags.insert(device_id.clone(), plugin_info);
                                        self.plugin_info_processed = true;
                                    }
                                }
                            } else {
                                debug_fn!(
                                    "handle_start_event",
                                    "[{}] Ignoring plugin name at depth {} (already processed): {}",
                                    line,
                                    self.depth,
                                    value
                                );
                            }
                        }
                        ScannerState::InScaleInformation => {
                            debug_fn!(
                                "handle_start_event",
                                "[{}] Found scale name: {}",
                                line,
                                value
                            );
                            let scale = match value.as_ref() {
                                "Major" => Scale::Major,
                                "Minor" => Scale::Minor,
                                // Add other scale mappings as needed
                                _ => Scale::Empty,
                            };
                            if let Some((tonic, _)) = self.current_scale_info.as_ref() {
                                self.current_scale_info = Some((tonic.clone(), scale));
                            } else {
                                self.current_scale_info = Some((Tonic::Empty, scale));
                            }
                        }
                        _ => {
                            trace_fn!(
                                "handle_start_event",
                                "[{}] Found name tag at depth {} but not in correct state: {:?}",
                                line,
                                self.depth,
                                self.state
                            );
                        }
                    }
                }
            }
            "EnumEvent" => {
                // Only process if we're looking for time signatures
                if !self.options.scan_time_signature {
                    return Ok(());
                }

                // Get the Value attribute
                if let Some(value) = event.try_get_attribute("Value")? {
                    let value_str = value.unescape_value()?.to_string();
                    debug_fn!(
                        "handle_start_event",
                        "[{}] Found EnumEvent with value: {}",
                        line,
                        value_str
                    );

                    // Parse the encoded time signature
                    match crate::utils::time_signature::parse_encoded_time_signature(&value_str) {
                        Ok(encoded_value) => {
                            match TimeSignature::from_encoded(encoded_value) {
                                Ok(time_sig) => {
                                    debug_fn!(
                                        "handle_start_event",
                                        "[{}] Successfully decoded time signature: {}/{}",
                                        line,
                                        time_sig.numerator,
                                        time_sig.denominator
                                    );
                                    self.current_time_signature = Some(time_sig);
                                }
                                Err(e) => {
                                    warn_fn!(
                                        "handle_start_event",
                                        "[{}] Failed to decode time signature from value {}: {:?}",
                                        line,
                                        encoded_value,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn_fn!(
                                "handle_start_event",
                                "[{}] Failed to parse encoded time signature from '{}': {:?}",
                                line,
                                value_str,
                                e
                            );
                        }
                    }
                }
            }
            "CurrentEnd" => {
                // Only process if we're calculating furthest bar
                if !self.options.calculate_furthest_bar {
                    return Ok(());
                }

                // Get the Value attribute
                if let Some(value) = event.try_get_attribute("Value")? {
                    let value_str = value.unescape_value()?.to_string();
                    match value_str.parse::<f64>() {
                        Ok(end_time) => {
                            debug_fn!(
                                "handle_start_event",
                                "[{}] Found CurrentEnd with value: {}",
                                line,
                                end_time
                            );
                            self.current_end_times.push(end_time);
                        }
                        Err(e) => {
                            warn_fn!(
                                "handle_start_event",
                                "[{}] Failed to parse CurrentEnd value '{}': {:?}",
                                line,
                                value_str,
                                e
                            );
                        }
                    }
                }
            }
            "Tempo" => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Entering Tempo tag at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InTempo {
                    version: self.ableton_version.major,
                };
            }
            "Manual" if matches!(self.state, ScannerState::InTempo { .. }) => {
                // Get the Value attribute for the tempo
                if let Some(value) = event.try_get_attribute("Value")? {
                    let value_str = value.unescape_value()?.to_string();
                    match value_str.parse::<f64>() {
                        Ok(tempo) if tempo > 0.0 => {
                            debug_fn!(
                                "handle_start_event",
                                "[{}] Found valid tempo value: {}",
                                line,
                                tempo
                            );
                            self.current_tempo = Some(tempo);
                        }
                        Ok(_) => {
                            warn_fn!(
                                "handle_start_event",
                                "[{}] Invalid tempo value (must be positive): {}",
                                line,
                                value_str
                            );
                        }
                        Err(e) => {
                            warn_fn!(
                                "handle_start_event",
                                "[{}] Failed to parse tempo value '{}': {:?}",
                                line,
                                value_str,
                                e
                            );
                        }
                    }
                }
                self.state = ScannerState::InTempoManual;
            }
            "MidiClip" => {
                if self.options.scan_key {
                    debug_fn!(
                        "handle_start_event",
                        "[{}] Entering MidiClip at depth {}",
                        line,
                        self.depth
                    );
                    self.state = ScannerState::InMidiClip;
                    self.current_clip_in_key = false;  // Reset for new clip
                    self.current_scale_info = None;    // Reset scale info
                }
            }
            "ScaleInformation" if matches!(self.state, ScannerState::InMidiClip) => {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Entering ScaleInformation at depth {}",
                    line,
                    self.depth
                );
                self.state = ScannerState::InScaleInformation;
            }
            "RootNote" if matches!(self.state, ScannerState::InScaleInformation) => {
                if let Some(value) = event.try_get_attribute("Value")? {
                    if let Ok(root_note) = value.unescape_value()?.parse::<i32>() {
                        debug_fn!(
                            "handle_start_event",
                            "[{}] Found root note: {}",
                            line,
                            root_note
                        );
                        let tonic = Tonic::from_midi_note(root_note);
                        if let Some((_, scale)) = self.current_scale_info.as_ref() {
                            self.current_scale_info = Some((tonic, scale.clone()));
                        } else {
                            self.current_scale_info = Some((tonic, Scale::Empty));
                        }
                    }
                }
            }
            "IsInKey" if matches!(self.state, ScannerState::InMidiClip) => {
                if let Some(value) = event.try_get_attribute("Value")? {
                    let is_in_key = value.unescape_value()?.as_ref() == "true";
                    debug_fn!(
                        "handle_start_event",
                        "[{}] Found IsInKey: {}",
                        line,
                        is_in_key
                    );
                    self.current_clip_in_key = is_in_key;
                    
                    // If clip is in key and we have scale info, add to frequencies
                    if is_in_key {
                        if let Some((tonic, scale)) = self.current_scale_info.take() {
                            let key_sig = KeySignature { tonic, scale };
                            *self.key_frequencies.entry(key_sig).or_insert(0) += 1;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_end_event(&mut self, event: &quick_xml::events::BytesEnd) -> Result<(), LiveSetError> {
        let name = event.name().to_string_result()?;
        
        trace_fn!(
            "handle_end_event",
            "Exiting tag: {}, current state: {:?}, depth: {}",
            name,
            self.state,
            self.depth
        );
        
        match name.as_str() {
            "SampleRef" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting SampleRef at depth {}, resetting state",
                    self.depth
                );
                // If we have a current file reference, add it to our sample paths
                if let Some(path) = self.current_file_ref.take() {
                    debug_fn!(
                        "handle_end_event",
                        "Adding sample path: {:?}",
                        path
                    );
                    self.sample_paths.insert(path);
                }
                self.current_path_type = None;
                self.in_source_context = false;  // Reset in_source_context when exiting SampleRef
                self.current_branch_info = None; // Reset branch info to ensure next plugin is processed correctly
                self.plugin_info_processed = false; // Reset plugin info processed flag
                self.state = ScannerState::Root;
            }
            "FileRef" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting FileRef at depth {}",
                    self.depth
                );
                if let ScannerState::InSampleRef { version } = self.state {
                    self.state = ScannerState::InSampleRef { version };
                } else {
                    self.state = ScannerState::Root;
                }
            }
            "Data" => {
                if let ScannerState::InData { ref current_data } = self.state {
                    debug_fn!(
                        "handle_end_event",
                        "Processing encoded path data of length {}",
                        current_data.len()
                    );
                    match crate::utils::samples::decode_sample_path(current_data) {
                        Ok(path) => {
                            debug_fn!(
                                "handle_end_event",
                                "Successfully decoded sample path: {:?}",
                                path
                            );
                            self.current_file_ref = Some(path);
                        }
                        Err(e) => {
                            warn_fn!(
                                "handle_end_event",
                                "Failed to decode sample path: {:?}",
                                e
                            );
                        }
                    }
                    // After processing Data tag, return to InFileRef state
                    self.state = ScannerState::InFileRef;
                }
            }
            "Path" => {
                if let ScannerState::InPath { .. } = self.state {
                    self.state = ScannerState::InFileRef;
                }
            }
            "SourceContext" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting SourceContext at depth {}, resetting state",
                    self.depth
                );
                self.in_source_context = false;
                if !matches!(self.state, ScannerState::InPluginDesc { .. }) {
                    self.state = ScannerState::Root;
                }
            }
            "Value" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting Value at depth {}, returning to SourceContext state",
                    self.depth
                );
                if !matches!(self.state, ScannerState::InPluginDesc { .. }) {
                    self.state = ScannerState::InSourceContext;
                }
            }
            "BranchSourceContext" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting BranchSourceContext at depth {}, returning to Value state",
                    self.depth
                );
                if !matches!(self.state, ScannerState::InPluginDesc { .. }) {
                    self.state = ScannerState::InValue;
                }
            }
            "PluginDesc" => {
                // Clear the current branch info and plugin info processed flag
                debug_fn!(
                    "handle_end_event",
                    "Exiting PluginDesc at depth {}, clearing device ID: {:?}",
                    self.depth,
                    self.current_branch_info
                );
                self.current_branch_info = None;
                self.plugin_info_processed = false;
                self.state = if self.in_source_context {
                    trace_fn!(
                        "handle_end_event",
                        "Returning to SourceContext state at depth {}",
                        self.depth
                    );
                    ScannerState::InSourceContext
                } else {
                    trace_fn!(
                        "handle_end_event",
                        "Returning to Root state at depth {}",
                        self.depth
                    );
                    ScannerState::Root
                };
            }
            "Vst3PluginInfo" | "VstPluginInfo" => {
                if let Some(device_id) = &self.current_branch_info {
                    debug_fn!(
                        "handle_end_event",
                        "Exiting plugin info tag at depth {}, returning to PluginDesc state for device: {}",
                        self.depth,
                        device_id
                    );
                    self.state = ScannerState::InPluginDesc { device_id: device_id.clone() };
                } else {
                    trace_fn!(
                        "handle_end_event",
                        "Exiting plugin info tag at depth {} but no current device ID",
                        self.depth
                    );
                    self.state = ScannerState::Root;
                }
            }
            "Tempo" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting Tempo tag at depth {}, resetting state",
                    self.depth
                );
                self.state = ScannerState::Root;
            }
            "Manual" if matches!(self.state, ScannerState::InTempoManual) => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting Manual tag at depth {}, returning to Tempo state",
                    self.depth
                );
                self.state = ScannerState::InTempo {
                    version: self.ableton_version.major,
                };
            }
            "MidiClip" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting MidiClip at depth {}, resetting state",
                    self.depth
                );
                if matches!(self.state, ScannerState::InMidiClip) {
                    self.state = ScannerState::Root;
                    self.current_clip_in_key = false;
                    self.current_scale_info = None;
                }
            }
            "ScaleInformation" => {
                debug_fn!(
                    "handle_end_event",
                    "Exiting ScaleInformation at depth {}, returning to MidiClip state",
                    self.depth
                );
                if matches!(self.state, ScannerState::InScaleInformation) {
                    self.state = ScannerState::InMidiClip;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_text_event(&mut self, event: &quick_xml::events::BytesText) -> Result<(), LiveSetError> {
        if let ScannerState::InData { ref mut current_data } = self.state {
            let text = event.unescape().map_err(LiveSetError::from)?;
            trace_fn!(
                "handle_text_event",
                "Appending data: {} (current length: {})",
                text,
                current_data.len()
            );
            current_data.push_str(&text);
        }
        Ok(())
    }
}