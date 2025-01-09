use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::io::BufRead;
#[allow(unused_imports)]
use log::{debug, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::LiveSetError;
use crate::models::{Id, Plugin, PluginInfo, Sample, TimeSignature};
use crate::utils::plugins::LineTrackingBuffer;
use crate::utils::{StringResultExt, EventExt};
#[allow(unused_imports)]
use crate::{debug_fn, trace_fn, warn_fn};

/// Represents what type of data we're currently scanning
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ScannerState {
    Root,
    
    // Sample scanning states
    InSampleRef {
        version: u32,
    },
    InSampleData {
        current_data: String,
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
        }
    }
}

/// Holds the results of the scanning process
#[derive(Default)]
#[allow(dead_code)]
pub struct ScanResult {
    pub samples: HashSet<Sample>,
    pub plugins: HashSet<Plugin>,
    pub tempo: Option<f64>,
    pub time_signature: Option<TimeSignature>,
    pub furthest_bar: Option<f64>,
    // Will add other results as we port them
}

/// The main scanner that processes the XML data
#[allow(dead_code)]
pub struct Scanner {
    // Core scanner state
    state: ScannerState,
    depth: i32,
    ableton_version: u32,
    options: ScanOptions,
    line_tracker: LineTrackingBuffer,
    
    // Sample scanning state
    sample_paths: HashSet<PathBuf>,
    current_sample_data: Option<String>,
    
    // Will add other scanning state as we port them
    // Plugin scanning state
    dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>>,
    
    // Tempo scanning state
    current_tempo: Option<f64>,
    
    // Time signature scanning state
    current_time_signature: Option<TimeSignature>,
    
    // Plugin scanning state
    current_branch_info: Option<String>,  // Tracks current plugin being processed
    plugin_info_tags: HashMap<String, PluginInfo>,  // Collects plugin info during scanning
    in_source_context: bool,  // Tracks if we're in a SourceContext tag
    plugin_info_processed: bool,  // Tracks if we've already processed a plugin info tag in current PluginDesc
}

#[allow(dead_code)]
impl Scanner {
    pub fn new(xml_data: &[u8], version: u32, options: ScanOptions) -> Self {
        Self {
            state: ScannerState::Root,
            depth: 0,
            ableton_version: version,
            options,
            line_tracker: LineTrackingBuffer::new(xml_data.to_vec()),
            
            // Initialize sample scanning state
            sample_paths: HashSet::new(),
            current_sample_data: None,
            
            // Initialize other state
            dev_identifiers: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            current_tempo: None,
            current_time_signature: None,
            
            // Initialize plugin scanning state
            current_branch_info: None,
            plugin_info_tags: HashMap::new(),
            in_source_context: false,
            plugin_info_processed: false,
        }
    }

    /// Main scanning function that processes the XML data
    pub fn scan(&mut self, xml_data: &[u8]) -> Result<ScanResult, LiveSetError> {
        let mut reader = Reader::from_reader(xml_data);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let result = ScanResult::default();

        #[allow(unused_variables)]
        loop {
            let byte_pos = reader.buffer_position();
            let line = self.line_tracker.get_line_number(byte_pos);

            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) => {
                    self.depth += 1;
                    // Will implement handle_start_event next
                }

                Ok(Event::Empty(ref event)) => {
                    // Will implement handle_empty_event next
                }

                Ok(Event::Text(ref event)) => {
                    // Will implement handle_text_event next
                }

                Ok(Event::End(ref event)) => {
                    self.depth -= 1;
                    // Will implement handle_end_event next
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
    fn finalize_result(&self, mut result: ScanResult) -> Result<ScanResult, LiveSetError> {
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

        // Will add other finalizations as we port them

        Ok(result)
    }

    fn handle_start_event<R: BufRead>(
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
                            // Ignore any plugin-related tags inside BranchSourceContext
                            if tag_name == "PluginDesc" {
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
                        _ => {
                            trace_fn!(
                                "handle_start_event",
                                "[{}] Found plugin name at depth {} but not in correct state: {:?}",
                                line,
                                self.depth,
                                self.state
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_end_event(&mut self, event: &quick_xml::events::BytesEnd) -> Result<(), LiveSetError> {
        let name = event.name().to_string_result()?;
        
        trace_fn!(
            "handle_end_event",
            "Exiting tag: {}, current state: {:?}, depth: {}",
            name,
            self.state,
            self.depth
        );
        
        match name.as_str() {
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
            _ => {}
        }
        Ok(())
    }
}

// Add From implementation for quick_xml::Error
impl From<quick_xml::Error> for LiveSetError {
    fn from(err: quick_xml::Error) -> Self {
        LiveSetError::XmlError(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::{BytesStart, BytesEnd, BytesText};
    use crate::models::PluginFormat;

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();
    }

    fn create_test_scanner() -> Scanner {
        init(); // Initialize logger for each test
        Scanner::new(&[], 11, ScanOptions::default())
    }

    fn create_start_event(name: &str) -> BytesStart {
        BytesStart::new(name)
    }

    fn create_end_event(name: &str) -> BytesEnd {
        BytesEnd::new(name)
    }

    fn create_empty_event<'a>(name: &'a str, value: Option<&'a str>) -> BytesStart<'a> {
        let mut event = BytesStart::new(name);
        if let Some(val) = value {
            event.push_attribute(("Value", val));
        }
        event
    }

    fn handle_tag_sequence(scanner: &mut Scanner, reader: &mut Reader<&[u8]>, byte_pos: &mut usize, tag: &str) {
        scanner.handle_start_event(
            &create_start_event(tag),
            reader,
            byte_pos
        ).unwrap();
        scanner.handle_end_event(
            &create_end_event(tag)
        ).unwrap();
    }

    #[test]
    fn test_vst3_audio_fx() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <OriginalFileRef />
                        <BrowserContentPath Value="query:Everything#Pro-Q%203" />
                        <LocalFiltersJson Value="" />
                        <PresetRef />
                        <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="0">
                    <Name Value="Pro-Q 3" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        let mut byte_pos = 0;

        // Enter SourceContext
        scanner.handle_start_event(
            &create_start_event("SourceContext"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter Value
        scanner.handle_start_event(
            &create_start_event("Value"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter BranchSourceContext
        let mut branch_event = create_start_event("BranchSourceContext");
        branch_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &branch_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "OriginalFileRef");
        
        // Add BrowserContentPath
        scanner.handle_start_event(
            &create_empty_event("BrowserContentPath", Some("query:Everything#Pro-Q%203")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle more empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "LocalFiltersJson");
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "PresetRef");

        // Add device ID
        scanner.handle_start_event(
            &create_empty_event(
                "BranchDeviceId",
                Some("device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d")
            ),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit BranchSourceContext and Value
        scanner.handle_end_event(&create_end_event("BranchSourceContext")).unwrap();
        scanner.handle_end_event(&create_end_event("Value")).unwrap();
        scanner.handle_end_event(&create_end_event("SourceContext")).unwrap();

        // Enter PluginDesc
        scanner.handle_start_event(
            &create_start_event("PluginDesc"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter Vst3PluginInfo
        let mut plugin_info_event = create_start_event("Vst3PluginInfo");
        plugin_info_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &plugin_info_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Add plugin name
        scanner.handle_start_event(
            &create_empty_event("Name", Some("Pro-Q 3")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit all tags
        scanner.handle_end_event(&create_end_event("Name")).unwrap();
        scanner.handle_end_event(&create_end_event("Vst3PluginInfo")).unwrap();
        scanner.handle_end_event(&create_end_event("PluginDesc")).unwrap();

        // Verify the plugin was collected correctly
        assert_eq!(scanner.plugin_info_tags.len(), 1);
        let plugin_info = scanner.plugin_info_tags.values().next().unwrap();
        assert_eq!(plugin_info.name, "Pro-Q 3");
        assert_eq!(
            plugin_info.dev_identifier,
            "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d"
        );
        assert_eq!(plugin_info.plugin_format, PluginFormat::VST3AudioFx);
    }

    #[test]
    fn test_vst2_audio_fx() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <OriginalFileRef />
                        <BrowserContentPath Value="view:X-Plugins#Altiverb%207" />
                        <LocalFiltersJson Value="{&quot;local-filters&quot;:{&quot;devtype&quot;:[&quot;audio-fx&quot;],&quot;devarch&quot;:[&quot;plugin-vst&quot;]}}" />
                        <PresetRef />
                        <BranchDeviceId Value="device:vst:audiofx:1096184373?n=Altiverb%207" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <VstPluginInfo Id="0">
                    <PlugName Value="Altiverb 7" />
                </VstPluginInfo>
            </PluginDesc>
        "#);
        let mut byte_pos = 0;

        // Enter SourceContext
        scanner.handle_start_event(
            &create_start_event("SourceContext"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter Value
        scanner.handle_start_event(
            &create_start_event("Value"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter BranchSourceContext
        let mut branch_event = create_start_event("BranchSourceContext");
        branch_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &branch_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "OriginalFileRef");
        
        // Add BrowserContentPath
        scanner.handle_start_event(
            &create_empty_event("BrowserContentPath", Some("view:X-Plugins#Altiverb%207")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle more empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "LocalFiltersJson");
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "PresetRef");

        // Add device ID
        scanner.handle_start_event(
            &create_empty_event(
                "BranchDeviceId",
                Some("device:vst:audiofx:1096184373?n=Altiverb%207")
            ),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit BranchSourceContext and Value
        scanner.handle_end_event(&create_end_event("BranchSourceContext")).unwrap();
        scanner.handle_end_event(&create_end_event("Value")).unwrap();
        scanner.handle_end_event(&create_end_event("SourceContext")).unwrap();

        // Enter PluginDesc
        scanner.handle_start_event(
            &create_start_event("PluginDesc"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter VstPluginInfo
        let mut plugin_info_event = create_start_event("VstPluginInfo");
        plugin_info_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &plugin_info_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Add plugin name
        scanner.handle_start_event(
            &create_empty_event("PlugName", Some("Altiverb 7")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit all tags
        scanner.handle_end_event(&create_end_event("PlugName")).unwrap();
        scanner.handle_end_event(&create_end_event("VstPluginInfo")).unwrap();
        scanner.handle_end_event(&create_end_event("PluginDesc")).unwrap();

        // Verify the plugin was collected correctly
        assert_eq!(scanner.plugin_info_tags.len(), 1);
        let plugin_info = scanner.plugin_info_tags.values().next().unwrap();
        assert_eq!(plugin_info.name, "Altiverb 7");
        assert_eq!(
            plugin_info.dev_identifier,
            "device:vst:audiofx:1096184373?n=Altiverb%207"
        );
        assert_eq!(plugin_info.plugin_format, PluginFormat::VST2AudioFx);
    }

    #[test]
    fn test_vst3_instrument() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <OriginalFileRef />
                        <BrowserContentPath Value="query:Everything#Omnisphere" />
                        <LocalFiltersJson Value="" />
                        <PresetRef />
                        <BranchDeviceId Value="device:vst3:instr:84e8de5f-9255-2222-96fa-e4133c935a18" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="0">
                    <Name Value="Omnisphere" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        let mut byte_pos = 0;

        // Enter SourceContext
        scanner.handle_start_event(
            &create_start_event("SourceContext"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter Value
        scanner.handle_start_event(
            &create_start_event("Value"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter BranchSourceContext
        let mut branch_event = create_start_event("BranchSourceContext");
        branch_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &branch_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "OriginalFileRef");
        
        // Add BrowserContentPath
        scanner.handle_start_event(
            &create_empty_event("BrowserContentPath", Some("query:Everything#Omnisphere")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Handle more empty tags
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "LocalFiltersJson");
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "PresetRef");

        // Add device ID
        scanner.handle_start_event(
            &create_empty_event(
                "BranchDeviceId",
                Some("device:vst3:instr:84e8de5f-9255-2222-96fa-e4133c935a18")
            ),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit BranchSourceContext and Value
        scanner.handle_end_event(&create_end_event("BranchSourceContext")).unwrap();
        scanner.handle_end_event(&create_end_event("Value")).unwrap();
        scanner.handle_end_event(&create_end_event("SourceContext")).unwrap();

        // Enter PluginDesc
        scanner.handle_start_event(
            &create_start_event("PluginDesc"),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Enter Vst3PluginInfo
        let mut plugin_info_event = create_start_event("Vst3PluginInfo");
        plugin_info_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &plugin_info_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Add plugin name
        scanner.handle_start_event(
            &create_empty_event("Name", Some("Omnisphere")),
            &mut reader,
            &mut byte_pos
        ).unwrap();

        // Exit all tags
        scanner.handle_end_event(&create_end_event("Name")).unwrap();
        scanner.handle_end_event(&create_end_event("Vst3PluginInfo")).unwrap();
        scanner.handle_end_event(&create_end_event("PluginDesc")).unwrap();

        // Verify the plugin was collected correctly
        assert_eq!(scanner.plugin_info_tags.len(), 1);
        let plugin_info = scanner.plugin_info_tags.values().next().unwrap();
        assert_eq!(plugin_info.name, "Omnisphere");
        assert_eq!(
            plugin_info.dev_identifier,
            "device:vst3:instr:84e8de5f-9255-2222-96fa-e4133c935a18"
        );
        assert_eq!(plugin_info.plugin_format, PluginFormat::VST3Instrument);
    }

    #[test]
    fn test_interleaved_plugins_and_sample() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <OriginalFileRef />
                        <BrowserContentPath Value="query:Everything#Pro-Q%203" />
                        <LocalFiltersJson Value="" />
                        <PresetRef />
                        <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="0">
                    <Name Value="Pro-Q 3" />
                </Vst3PluginInfo>
            </PluginDesc>
            <SampleRef>
                <FileRef>
                    <RelativePathType Value="1" />
                    <RelativePath Value="../../../../Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                    <Path Value="C:/Users/judee/Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                    <Type Value="1" />
                    <LivePackName Value="" />
                    <LivePackId Value="" />
                    <OriginalFileSize Value="146440" />
                    <OriginalCrc Value="40395" />
                </FileRef>
                <LastModDate Value="1628727109" />
                <SourceContext>
                    <SourceContext Id="0">
                        <OriginalFileRef>
                            <FileRef Id="0">
                                <RelativePathType Value="1" />
                                <RelativePath Value="../../../../Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                                <Path Value="C:/Users/judee/Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                                <Type Value="1" />
                                <LivePackName Value="" />
                                <LivePackId Value="" />
                                <OriginalFileSize Value="146440" />
                                <OriginalCrc Value="40395" />
                            </FileRef>
                        </OriginalFileRef>
                        <BrowserContentPath Value="view:X-Samples#FileId_689899" />
                        <LocalFiltersJson Value="" />
                    </SourceContext>
                </SourceContext>
                <SampleUsageHint Value="0" />
                <DefaultDuration Value="24284" />
                <DefaultSampleRate Value="44100" />
            </SampleRef>
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <OriginalFileRef />
                        <BrowserContentPath Value="view:X-Plugins#Altiverb%207" />
                        <LocalFiltersJson Value="{&quot;local-filters&quot;:{&quot;devtype&quot;:[&quot;audio-fx&quot;],&quot;devarch&quot;:[&quot;plugin-vst&quot;]}}" />
                        <PresetRef />
                        <BranchDeviceId Value="device:vst:audiofx:1096184373?n=Altiverb%207" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <VstPluginInfo Id="0">
                    <PlugName Value="Altiverb 7" />
                </VstPluginInfo>
            </PluginDesc>
        "#);
        let mut byte_pos = 0;

        // Process first plugin (Pro-Q 3)
        scanner.handle_start_event(
            &create_start_event("SourceContext"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_start_event(
            &create_start_event("Value"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        let mut branch_event = create_start_event("BranchSourceContext");
        branch_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &branch_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "OriginalFileRef");
        scanner.handle_start_event(
            &create_empty_event("BrowserContentPath", Some("query:Everything#Pro-Q%203")),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "LocalFiltersJson");
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "PresetRef");
        scanner.handle_start_event(
            &create_empty_event(
                "BranchDeviceId",
                Some("device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d")
            ),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_end_event(&create_end_event("BranchSourceContext")).unwrap();
        scanner.handle_end_event(&create_end_event("Value")).unwrap();
        scanner.handle_end_event(&create_end_event("SourceContext")).unwrap();

        scanner.handle_start_event(
            &create_start_event("PluginDesc"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        let mut plugin_info_event = create_start_event("Vst3PluginInfo");
        plugin_info_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &plugin_info_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_start_event(
            &create_empty_event("Name", Some("Pro-Q 3")),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_end_event(&create_end_event("Name")).unwrap();
        scanner.handle_end_event(&create_end_event("Vst3PluginInfo")).unwrap();
        scanner.handle_end_event(&create_end_event("PluginDesc")).unwrap();

        // Process sample
        scanner.handle_start_event(
            &create_start_event("SampleRef"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        // ... sample processing will be implemented when we add sample handling ...
        scanner.handle_end_event(&create_end_event("SampleRef")).unwrap();

        // Process second plugin (Altiverb 7)
        scanner.handle_start_event(
            &create_start_event("SourceContext"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_start_event(
            &create_start_event("Value"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        let mut branch_event = create_start_event("BranchSourceContext");
        branch_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &branch_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "OriginalFileRef");
        scanner.handle_start_event(
            &create_empty_event("BrowserContentPath", Some("view:X-Plugins#Altiverb%207")),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "LocalFiltersJson");
        handle_tag_sequence(&mut scanner, &mut reader, &mut byte_pos, "PresetRef");
        scanner.handle_start_event(
            &create_empty_event(
                "BranchDeviceId",
                Some("device:vst:audiofx:1096184373?n=Altiverb%207")
            ),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_end_event(&create_end_event("BranchSourceContext")).unwrap();
        scanner.handle_end_event(&create_end_event("Value")).unwrap();
        scanner.handle_end_event(&create_end_event("SourceContext")).unwrap();

        scanner.handle_start_event(
            &create_start_event("PluginDesc"),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        let mut plugin_info_event = create_start_event("VstPluginInfo");
        plugin_info_event.push_attribute(("Id", "0"));
        scanner.handle_start_event(
            &plugin_info_event,
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_start_event(
            &create_empty_event("PlugName", Some("Altiverb 7")),
            &mut reader,
            &mut byte_pos
        ).unwrap();
        scanner.handle_end_event(&create_end_event("PlugName")).unwrap();
        scanner.handle_end_event(&create_end_event("VstPluginInfo")).unwrap();
        scanner.handle_end_event(&create_end_event("PluginDesc")).unwrap();

        // Verify results
        assert_eq!(scanner.plugin_info_tags.len(), 2);
        let plugin_info: Vec<_> = scanner.plugin_info_tags.values().collect();
        
        // Verify Pro-Q 3
        let proq3 = plugin_info.iter().find(|p| p.name == "Pro-Q 3").unwrap();
        assert_eq!(proq3.dev_identifier, "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d");
        assert_eq!(proq3.plugin_format, PluginFormat::VST3AudioFx);

        // Verify Altiverb 7
        let altiverb = plugin_info.iter().find(|p| p.name == "Altiverb 7").unwrap();
        assert_eq!(altiverb.dev_identifier, "device:vst:audiofx:1096184373?n=Altiverb%207");
        assert_eq!(altiverb.plugin_format, PluginFormat::VST2AudioFx);

        // Verify scanner state is clean
        assert_eq!(scanner.state, ScannerState::Root);
        assert_eq!(scanner.in_source_context, false);
        assert_eq!(scanner.current_branch_info, None);
    }

    #[test]
    fn test_malformed_missing_browser_path() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="0">
                        <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="0">
                    <Name Value="Should Not Appear" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect plugin without browser path");
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_missing_device_id() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="1">
                        <BrowserContentPath Value="query:Everything#Missing-Device" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="1">
                    <Name Value="Should Not Appear Either" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect plugin without device ID");
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_multiple_plugin_info() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="2">
                        <BrowserContentPath Value="query:Everything#Valid-Plugin" />
                        <BranchDeviceId Value="device:vst3:audiofx:valid-plugin-id" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="2">
                    <Name Value="Valid Plugin" />
                </Vst3PluginInfo>
                <VstPluginInfo Id="2">
                    <PlugName Value="Should Be Ignored" />
                </VstPluginInfo>
            </PluginDesc>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 1, "Should only collect the first plugin info");
        let plugin_info = scanner.plugin_info_tags.values().next().unwrap();
        assert_eq!(plugin_info.name, "Valid Plugin");
        assert_eq!(plugin_info.dev_identifier, "device:vst3:audiofx:valid-plugin-id");
        assert_eq!(plugin_info.plugin_format, PluginFormat::VST3AudioFx);
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_invalid_device_id() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="3">
                        <BrowserContentPath Value="query:Everything#Invalid-Format" />
                        <BranchDeviceId Value="invalid:format:not-a-plugin" />
                    </BranchSourceContext>
                </Value>
            </SourceContext>
            <PluginDesc>
                <Vst3PluginInfo Id="3">
                    <Name Value="Should Not Appear" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect plugin with invalid device ID");
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_orphaned_plugin_desc() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <PluginDesc>
                <Vst3PluginInfo Id="4">
                    <Name Value="Should Not Appear" />
                </Vst3PluginInfo>
            </PluginDesc>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect orphaned plugin desc");
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_orphaned_plugin_info() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <Vst3PluginInfo Id="5">
                <Name Value="Should Not Appear" />
            </Vst3PluginInfo>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect orphaned plugin info");
        assert_clean_state(&scanner);
    }

    #[test]
    fn test_malformed_nested_plugin_desc() {
        let mut scanner = create_test_scanner();
        let mut reader = Reader::from_str(r#"
            <SourceContext>
                <Value>
                    <BranchSourceContext Id="6">
                        <BrowserContentPath Value="query:Everything#Nested" />
                        <BranchDeviceId Value="device:vst3:audiofx:nested-id" />
                        <PluginDesc>
                            <Vst3PluginInfo Id="6">
                                <Name Value="Should Not Appear" />
                            </Vst3PluginInfo>
                        </PluginDesc>
                    </BranchSourceContext>
                </Value>
            </SourceContext>
        "#);
        process_xml(&mut scanner, &mut reader);

        assert_eq!(scanner.plugin_info_tags.len(), 0, "Should not collect nested plugin desc");
        assert_clean_state(&scanner);
    }

    // Helper function to process XML in tests
    fn process_xml(scanner: &mut Scanner, reader: &mut Reader<&[u8]>) {
        let mut byte_pos = 0;
        loop {
            match reader.read_event_into(&mut Vec::new()) {
                Ok(Event::Start(ref e)) => {
                    scanner.depth += 1;
                    scanner.handle_start_event(e, reader, &mut byte_pos).unwrap();
                }
                Ok(Event::End(ref e)) => {
                    scanner.handle_end_event(e).unwrap();
                    scanner.depth -= 1;
                }
                Ok(Event::Empty(ref e)) => {
                    scanner.handle_start_event(e, reader, &mut byte_pos).unwrap();
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => {}
            }
        }
    }

    // Helper function to verify scanner is in a clean state
    fn assert_clean_state(scanner: &Scanner) {
        assert_eq!(scanner.state, ScannerState::Root, "Scanner should be in Root state");
        assert_eq!(scanner.in_source_context, false, "Scanner should not be in source context");
        assert_eq!(scanner.current_branch_info, None, "Scanner should have no branch info");
    }
}
