//! # Ableton Live Set Parser
//!
//! This module contains the core XML parser for Ableton Live Set (`.als`) files.
//! It's responsible for extracting all metadata from project files including plugins,
//! samples, tempo, time signature, key signatures, and other musical properties.
//!
//! ## Overview
//!
//! The parser is built around a state machine that processes XML events from the
//! compressed Live Set files. It handles multiple Ableton Live versions (9-12) with
//! version-specific parsing logic for different data formats.
//!
//! ## Key Components
//!
//! - [`Parser`]: Main state machine that processes XML events
//! - [`ParserState`]: State enumeration for tracking current parsing context
//! - [`ParseOptions`]: Configuration for which data to extract
//! - [`ParseResult`]: Final output containing all extracted metadata
//!
//! ## Parsing Process
//!
//! 1. **Version Detection**: Identify Ableton Live version from XML header
//! 2. **State Machine**: Process XML events and maintain parsing state
//! 3. **Data Extraction**: Extract specific data based on current state
//! 4. **Result Finalization**: Convert raw data into structured models
//!
//! ## Supported Data Types
//!
//! - **Musical Properties**: Tempo, time signature, key signature
//! - **Plugins**: VST2/VST3 instruments and effects with installation status
//! - **Samples**: Audio file references with presence validation
//! - **Project Structure**: Track end times for duration calculation
//!
//! ## Version Compatibility
//!
//! The parser handles version-specific differences:
//! - **< v11**: Encoded sample paths, limited key detection
//! - **>= v11**: Direct sample paths, full key signature support
//!
//! ## Performance Considerations
//!
//! - Streaming XML parsing for large files
//! - Configurable feature extraction via [`ParseOptions`]
//! - Memory-efficient state tracking
//! - Early termination for specific data extraction

#[allow(unused_imports)]
use log::{debug, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::LiveSetError;
use crate::models::{
    AbletonVersion, KeySignature, Plugin, PluginInfo, Sample, Scale, TimeSignature, Tonic,
};
use crate::utils::plugins::get_most_recent_db_file;
use crate::utils::plugins::LineTrackingBuffer;
use crate::utils::{EventExt, StringResultExt};
#[allow(unused_imports)]
use crate::{trace_fn, warn_fn};

/// Sample path encoding type based on Ableton Live version.
///
/// Ableton Live changed how sample paths are stored in project files between versions.
/// This enum tracks which format we're currently processing.
///
/// # Version Compatibility
///
/// - **Direct**: Used in Ableton Live 11+ where paths are stored as plain text
/// - **Encoded**: Used in Ableton Live <11 where paths are hex-encoded UTF-16
#[derive(Debug, Clone, PartialEq)]
pub enum PathType {
    /// Direct path storage (version >= 11) - paths stored as plain text
    Direct,
    /// Encoded path storage (version < 11) - paths stored as hex-encoded UTF-16
    Encoded,
}

/// Parser state machine states for tracking current parsing context.
///
/// The parser uses a state machine to track which type of XML element it's currently
/// processing. This allows context-aware parsing where the same XML tag names can
/// have different meanings depending on the current parsing state.
///
/// # State Categories
///
/// ## Sample Parsing States
/// - [`Root`]: Default state, not inside any specific data structure
/// - [`InSampleRef`]: Processing a sample reference with version info
/// - [`InFileRef`]: Inside a file reference for a sample
/// - [`InData`]: Reading encoded sample path data (pre-v11)
/// - [`InPath`]: Reading direct sample path data (v11+)
///
/// ## Plugin Parsing States
/// - [`InSourceContext`]: Inside a plugin source context
/// - [`InValue`]: Reading plugin context values
/// - [`InBranchSourceContext`]: Processing plugin branch information
/// - [`InPluginDesc`]: Inside a plugin description with device ID
/// - [`InVst3PluginInfo`]: Reading VST3 plugin metadata
/// - [`InVstPluginInfo`]: Reading VST2 plugin metadata
///
/// ## Musical Property States
/// - [`InTempo`]: Processing tempo information
/// - [`InTempoManual`]: Reading manual tempo values
/// - [`InTimeSignature`]: Processing time signature data
/// - [`InMidiClip`]: Inside a MIDI clip (for key detection)
/// - [`InScaleInformation`]: Reading musical scale information
///
/// [`Root`]: ParserState::Root
/// [`InSampleRef`]: ParserState::InSampleRef
/// [`InFileRef`]: ParserState::InFileRef
/// [`InData`]: ParserState::InData
/// [`InPath`]: ParserState::InPath
/// [`InSourceContext`]: ParserState::InSourceContext
/// [`InValue`]: ParserState::InValue
/// [`InBranchSourceContext`]: ParserState::InBranchSourceContext
/// [`InPluginDesc`]: ParserState::InPluginDesc
/// [`InVst3PluginInfo`]: ParserState::InVst3PluginInfo
/// [`InVstPluginInfo`]: ParserState::InVstPluginInfo
/// [`InTempo`]: ParserState::InTempo
/// [`InTempoManual`]: ParserState::InTempoManual
/// [`InTimeSignature`]: ParserState::InTimeSignature
/// [`InMidiClip`]: ParserState::InMidiClip
/// [`InScaleInformation`]: ParserState::InScaleInformation
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ParserState {
    /// Default state - not inside any specific data structure
    Root,

    // Sample parsing states
    /// Processing a sample reference with version-specific logic
    InSampleRef { version: u32 },
    /// Inside a file reference element for a sample
    InFileRef,
    /// Reading encoded sample path data (pre-v11 format)
    InData { current_data: String },
    /// Reading direct sample path data (v11+ format)
    InPath { path_type: PathType },

    // Plugin states
    /// Inside a plugin source context element
    InSourceContext,
    /// Reading plugin context values
    InValue,
    /// Processing plugin branch source context
    InBranchSourceContext,
    /// Inside a plugin description with associated device ID
    InPluginDesc { device_id: String },
    /// Reading VST3 plugin metadata
    InVst3PluginInfo,
    /// Reading VST2 plugin metadata
    InVstPluginInfo,

    // Tempo states
    /// Processing tempo information with version context
    InTempo { version: u32 },
    /// Reading manual tempo values
    InTempoManual,

    // Time signature state
    /// Processing time signature data
    InTimeSignature,

    // Key parsing states
    /// Inside a MIDI clip (used for key signature detection)
    InMidiClip,
    /// Reading musical scale information within a MIDI clip
    InScaleInformation,
}

/// Configuration options for controlling which data to extract during parsing.
///
/// This struct allows fine-grained control over which elements of the Ableton Live
/// project should be parsed and extracted. This is useful for performance optimization
/// when only specific data is needed, or for compatibility with different Live versions.
///
/// # Default Behavior
///
/// By default, all parsing options are enabled to extract comprehensive project metadata.
/// Individual options can be disabled to improve parsing performance when that data isn't needed.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::scan::parser::ParseOptions;
///
/// // Parse only basic musical properties
/// let basic_options = ParseOptions {
///     parse_plugins: false,
///     parse_samples: false,
///     parse_tempo: true,
///     parse_time_signature: true,
///     parse_key: true,
///     ..Default::default()
/// };
///
/// // Parse everything (default)
/// let full_options = ParseOptions::default();
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParseOptions {
    /// Extract plugin information (instruments and effects)
    pub parse_plugins: bool,
    /// Extract sample file references and paths
    pub parse_samples: bool,
    /// Extract project tempo information
    pub parse_tempo: bool,
    /// Extract time signature information
    pub parse_time_signature: bool,
    /// Extract MIDI clip data (required for key detection)
    pub parse_midi: bool,
    /// Extract audio clip data
    pub parse_audio: bool,
    /// Extract automation data
    pub parse_automation: bool,
    /// Extract return track information
    pub parse_return_tracks: bool,
    /// Extract master track information
    pub parse_master_track: bool,
    /// Calculate estimated project duration
    pub estimate_duration: bool,
    /// Calculate the furthest bar position in the project
    pub calculate_furthest_bar: bool,
    /// Extract key signature information (requires Ableton Live 11+)
    pub parse_key: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            parse_plugins: true,
            parse_samples: true,
            parse_tempo: true,
            parse_time_signature: true,
            parse_midi: true,
            parse_audio: true,
            parse_automation: true,
            parse_return_tracks: true,
            parse_master_track: true,
            estimate_duration: true,
            calculate_furthest_bar: true,
            parse_key: true,
        }
    }
}

/// Complete parsing results containing all extracted project metadata.
///
/// This struct holds all the data extracted from an Ableton Live project file.
/// It represents the final output of the parsing process and contains structured
/// data that can be stored in the database or used by other parts of the application.
///
/// # Data Categories
///
/// ## Required Properties
/// - [`version`]: Ableton Live version used to create the project
/// - [`tempo`]: Project tempo in BPM (beats per minute)
/// - [`time_signature`]: Musical time signature (e.g., 4/4, 3/4)
///
/// ## Optional Properties
/// - [`furthest_bar`]: Calculated project length in bars
/// - [`key_signature`]: Musical key signature (Live 11+ only)
///
/// ## Collections
/// - [`samples`]: Set of audio samples referenced in the project
/// - [`plugins`]: Set of plugins used in the project with installation status
///
/// [`version`]: ParseResult::version
/// [`tempo`]: ParseResult::tempo
/// [`time_signature`]: ParseResult::time_signature
/// [`furthest_bar`]: ParseResult::furthest_bar
/// [`key_signature`]: ParseResult::key_signature
/// [`samples`]: ParseResult::samples
/// [`plugins`]: ParseResult::plugins
#[derive(Default)]
#[allow(dead_code)]
pub struct ParseResult {
    /// Ableton Live version that created this project
    pub version: AbletonVersion,
    /// Set of audio samples referenced in the project
    pub samples: HashSet<Sample>,
    /// Set of plugins used in the project with installation status
    pub plugins: HashSet<Plugin>,
    /// Project tempo in beats per minute (BPM)
    pub tempo: f64,
    /// Musical time signature of the project
    pub time_signature: TimeSignature,
    /// Calculated furthest bar position (project length)
    pub furthest_bar: Option<f64>,
    /// Musical key signature (available in Live 11+ only)
    pub key_signature: Option<KeySignature>,
}

/// High-performance XML parser for Ableton Live Set files.
///
/// The parser is built around a state machine that processes XML events from compressed
/// Live Set files. It maintains extensive state to handle the complex, nested structure
/// of Ableton's XML format and extract meaningful data from various contexts.
///
/// # Architecture
///
/// The parser operates as a streaming XML processor with:
/// - **State Machine**: Tracks current parsing context via [`ParserState`]
/// - **Version Awareness**: Handles format differences between Live versions
/// - **Configurable Extraction**: Selective data extraction via [`ParseOptions`]
/// - **Memory Efficiency**: Streaming processing without loading entire XML into memory
///
/// # State Management
///
/// The parser maintains several categories of state:
///
/// ## Core Parser State
/// - Current parsing state and XML depth
/// - Ableton Live version for format compatibility
/// - Configuration options for data extraction
///
/// ## Sample Processing State
/// - Collected sample paths
/// - Current sample being processed
/// - Path encoding type (direct vs encoded)
///
/// ## Plugin Processing State
/// - Current plugin context information
/// - Plugin metadata collection
/// - Plugin processing flags
///
/// ## Musical Property State
/// - Tempo and timing information
/// - Key signature frequency analysis
/// - Time signature data
///
/// # Usage
///
/// ```rust,ignore
/// use studio_project_manager::scan::parser::{Parser, ParseOptions};
///
/// let xml_data = b"<Ableton>...</Ableton>";
/// let options = ParseOptions::default();
/// let mut parser = Parser::new(xml_data, options)?;
/// let result = parser.parse(xml_data)?;
///
/// println!("Project tempo: {}", result.tempo);
/// println!("Found {} plugins", result.plugins.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[allow(dead_code)]
pub struct Parser {
    // Core parser state
    /// Current state in the parsing state machine
    pub state: ParserState,
    /// Current XML nesting depth for context tracking
    pub depth: i32,
    /// Detected Ableton Live version for format compatibility
    pub ableton_version: AbletonVersion,
    /// Configuration options controlling what data to extract
    pub options: ParseOptions,
    /// Line tracking for error reporting and debugging
    line_tracker: LineTrackingBuffer,

    // Sample parsing state
    /// Collected sample file paths discovered during parsing
    pub sample_paths: HashSet<PathBuf>,
    /// Raw sample data being accumulated (for encoded paths)
    pub current_sample_data: Option<String>,
    /// Current file reference being processed
    pub current_file_ref: Option<PathBuf>,
    /// Path encoding type for current sample (direct vs encoded)
    pub current_path_type: Option<PathType>,

    // Plugin parsing state
    /// Current plugin branch information (device ID)
    pub current_branch_info: Option<String>,
    /// Collected plugin metadata keyed by device identifier
    pub plugin_info_tags: HashMap<String, PluginInfo>,
    /// Flag indicating if we're inside a plugin source context
    pub in_source_context: bool,
    /// Flag to prevent duplicate plugin info processing
    pub plugin_info_processed: bool,

    // Tempo and timing state
    /// Thread-safe collection of device identifiers
    pub dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>>,
    /// Current project tempo in BPM
    pub current_tempo: f64,
    /// Current project time signature
    pub current_time_signature: TimeSignature,
    /// Collected end times for duration calculation
    pub current_end_times: Vec<f64>,

    // Key signature parsing state
    /// Frequency count of detected key signatures
    pub key_frequencies: HashMap<KeySignature, usize>,
    /// Current scale information being processed
    current_scale_info: Option<(Tonic, Scale)>,
    /// Flag indicating if current clip is in a detected key
    current_clip_in_key: bool,
}

#[allow(dead_code)]
impl Parser {
    /// Creates a new parser instance with version detection and compatibility handling.
    ///
    /// This constructor performs initial version detection from the XML data and
    /// automatically adjusts parsing options based on version compatibility.
    /// For example, key signature detection is only available in Ableton Live 11+.
    ///
    /// # Arguments
    ///
    /// * `xml_data` - Raw XML data from the decompressed .als file
    /// * `options` - Parsing configuration options (will be modified for compatibility)
    ///
    /// # Returns
    ///
    /// Returns a new [`Parser`] instance ready to process the XML data, or an error
    /// if version detection fails or the version is unsupported.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - XML version information cannot be found or parsed
    /// - Ableton Live version is unsupported (< 9 or > 12)
    /// - XML format is corrupted or invalid
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use studio_project_manager::scan::parser::{Parser, ParseOptions};
    ///
    /// let xml_data = b"<Ableton MinorVersion=\"11.2_11215\">...</Ableton>";
    /// let options = ParseOptions::default();
    ///
    /// let parser = Parser::new(xml_data, options)?;
    /// println!("Detected version: {}", parser.ableton_version);
    /// # Ok::<(), studio_project_manager::error::LiveSetError>(())
    /// ```
    ///
    /// # Version Compatibility
    ///
    /// The parser automatically adjusts options based on detected version:
    /// - **< v11**: Disables key signature parsing (not supported)
    /// - **>= v11**: Full feature support including key signatures
    pub fn new(xml_data: &[u8], mut options: ParseOptions) -> Result<Self, LiveSetError> {
        // First, detect and validate the version
        let version = Self::detect_version(xml_data)?;

        // Disable features not supported in older versions
        if version.major < 11 {
            options.parse_key = false; // Key detection only available in v11+
            warn_fn!(
                "parser",
                "Key detection not supported in version {}",
                version
            );
            // Add other version-specific feature flags here
        }

        Ok(Self {
            state: ParserState::Root,
            depth: 0,
            ableton_version: version,
            options,
            line_tracker: LineTrackingBuffer::new(xml_data.to_vec()),

            // Initialize sample parsing state
            sample_paths: HashSet::new(),
            current_sample_data: None,
            current_file_ref: None,
            current_path_type: None,

            // Initialize plugin parsing state
            current_branch_info: None,
            plugin_info_tags: HashMap::new(),
            in_source_context: false,
            plugin_info_processed: false,

            // Initialize other state
            dev_identifiers: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            current_tempo: 0.0,
            current_time_signature: TimeSignature::default(),
            current_end_times: Vec::new(),

            // Initialize key parsing state
            key_frequencies: HashMap::new(),
            current_scale_info: None,
            current_clip_in_key: false,
        })
    }

    /// Detects the Ableton Live version from XML header information.
    ///
    /// This method performs a fast scan of the XML header to extract version information
    /// without parsing the entire file. It reads the `MinorVersion` and `SchemaChangeCount`
    /// attributes from the root `<Ableton>` element to determine the exact Live version.
    ///
    /// # Version Format
    ///
    /// Ableton Live stores version information in the format: `"major.minor_patch"`
    /// - Example: `"11.2_11215"` represents Live 11.2 patch 11215
    /// - Beta versions are indicated by `SchemaChangeCount="beta"`
    ///
    /// # Arguments
    ///
    /// * `xml_data` - Raw XML data to scan for version information
    ///
    /// # Returns
    ///
    /// Returns an [`AbletonVersion`] struct with parsed version components,
    /// or an error if version information cannot be found or parsed.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - No `<Ableton>` root element is found
    /// - `MinorVersion` attribute is missing or malformed
    /// - Version format doesn't match expected pattern
    /// - Version numbers cannot be parsed as integers
    /// - Major version is outside supported range (9-12)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use studio_project_manager::scan::parser::Parser;
    ///
    /// let xml_data = br#"<Ableton MinorVersion="11.2_11215" SchemaChangeCount="3">..."#;
    /// let version = Parser::detect_version(xml_data)?;
    ///
    /// assert_eq!(version.major, 11);
    /// assert_eq!(version.minor, 2);
    /// assert_eq!(version.patch, 11215);
    /// assert_eq!(version.beta, false);
    /// # Ok::<(), studio_project_manager::error::LiveSetError>(())
    /// ```
    pub fn detect_version(xml_data: &[u8]) -> Result<AbletonVersion, LiveSetError> {
        let mut reader = Reader::from_reader(xml_data);
        reader.config_mut().trim_text(true);
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

    /// Parses the complete XML data and extracts all configured project metadata.
    ///
    /// This is the main parsing method that processes the entire XML structure using
    /// a streaming parser. It maintains state throughout the parsing process and
    /// delegates specific XML events to specialized handler methods.
    ///
    /// # Parsing Process
    ///
    /// 1. **Stream Processing**: Uses quick-xml to stream through XML events
    /// 2. **State Management**: Maintains parsing state and XML depth tracking
    /// 3. **Event Handling**: Delegates start, end, and text events to handlers
    /// 4. **Result Finalization**: Converts accumulated state into final result
    ///
    /// # Arguments
    ///
    /// * `xml_data` - Complete XML data from the decompressed .als file
    ///
    /// # Returns
    ///
    /// Returns a [`ParseResult`] containing all extracted project metadata,
    /// or an error if parsing fails.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - XML is malformed or cannot be parsed
    /// - Required project properties (tempo, time signature) are invalid
    /// - Plugin database queries fail during finalization
    /// - Memory allocation fails during processing
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use studio_project_manager::scan::parser::{Parser, ParseOptions};
    ///
    /// let xml_data = include_bytes!("project.als");
    /// let options = ParseOptions::default();
    /// let mut parser = Parser::new(xml_data, options)?;
    /// 
    /// let result = parser.parse(xml_data)?;
    /// println!("Project: {} BPM, {}/{} time signature", 
    ///          result.tempo, 
    ///          result.time_signature.numerator,
    ///          result.time_signature.denominator);
    /// println!("Found {} plugins and {} samples", 
    ///          result.plugins.len(), 
    ///          result.samples.len());
    /// # Ok::<(), studio_project_manager::error::LiveSetError>(())
    /// ```
    ///
    /// # Performance Notes
    ///
    /// - Uses streaming XML parsing to handle large files efficiently
    /// - Memory usage scales with number of plugins/samples, not file size
    /// - Processing time is roughly linear with XML file size
    /// - Configure [`ParseOptions`] to skip unnecessary data extraction
    pub fn parse(&mut self, xml_data: &[u8]) -> Result<ParseResult, LiveSetError> {
        let mut reader = Reader::from_reader(xml_data);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut byte_pos; // Will be set in the loop
        let result = ParseResult::default();

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
                    warn_fn!("parse", "Error at line {}: {:?}", line, e);
                    return Err(LiveSetError::from(e));
                }
                _ => {}
            }
            buf.clear();
        }

        // Convert collected data into final result
        self.finalize_result(result)
    }

    /// Converts accumulated parser state into the final structured result.
    ///
    /// This method performs the final processing step that transforms the raw data
    /// collected during XML parsing into structured model objects. It handles
    /// validation, plugin installation detection, and key signature analysis.
    ///
    /// # Processing Steps
    ///
    /// 1. **Version Assignment**: Sets the detected Ableton Live version
    /// 2. **Property Validation**: Validates tempo and time signature values
    /// 3. **Duration Calculation**: Computes project length from end times
    /// 4. **Sample Processing**: Converts file paths to Sample objects
    /// 5. **Plugin Resolution**: Queries Ableton database for plugin installation status
    /// 6. **Key Analysis**: Determines most frequent key signature (Live 11+)
    ///
    /// # Arguments
    ///
    /// * `result` - Pre-initialized ParseResult to populate with data
    ///
    /// # Returns
    ///
    /// Returns a complete [`ParseResult`] with all extracted and processed data,
    /// or an error if validation or processing fails.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - Tempo is outside valid range (10-999 BPM)
    /// - Time signature is invalid (numerator 1-99, denominator power of 2)
    /// - Configuration cannot be loaded for plugin database access
    /// - Ableton plugin database cannot be opened or queried
    ///
    /// # Plugin Installation Detection
    ///
    /// For each plugin found in the project:
    /// - Queries Ableton's plugin database using the device identifier
    /// - If found: Populates full metadata and marks as installed
    /// - If not found: Creates basic plugin record marked as not installed
    ///
    /// # Key Signature Analysis
    ///
    /// When key parsing is enabled (Live 11+):
    /// - Analyzes frequency of detected key signatures across MIDI clips
    /// - Selects the most frequently occurring key as the project key
    /// - Falls back to empty key signature if none detected
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use studio_project_manager::scan::parser::{Parser, ParseResult, ParseOptions};
    ///
    /// let mut parser = Parser::new(b"<xml>", ParseOptions::default())?;
    /// // ... parsing happens here ...
    /// 
    /// let result = parser.finalize_result(ParseResult::default())?;
    /// println!("Final result: {} BPM", result.tempo);
    /// # Ok::<(), studio_project_manager::error::LiveSetError>(())
    /// ```
    pub fn finalize_result(
        &self,
        mut result: ParseResult,
    ) -> Result<ParseResult, LiveSetError> {
        // Set the version
        result.version = self.ableton_version;

        // Validate and set tempo (required for a valid project)
        if self.current_tempo < 10.0 || self.current_tempo > 999.0 {
            return Err(LiveSetError::InvalidProject(format!(
                "Invalid tempo value: {}",
                self.current_tempo
            )));
        }
        result.tempo = self.current_tempo;

        // Validate and set time signature (required for a valid project)
        if self.current_time_signature.is_valid() {
            result.time_signature = self.current_time_signature.clone();
        } else {
            return Err(LiveSetError::InvalidProject(format!(
                "Invalid time signature: {}/{}",
                self.current_time_signature.numerator, self.current_time_signature.denominator
            )));
        }

        // Calculate furthest bar if requested and we have end times
        if self.options.calculate_furthest_bar && !self.current_end_times.is_empty() {
            let beats_per_bar = result.time_signature.numerator as f64;
            let max_end_time = self
                .current_end_times
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            result.furthest_bar = Some(max_end_time / beats_per_bar);

            trace_fn!(
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
        let db_path =
            get_most_recent_db_file(&PathBuf::from(db_dir)).map_err(LiveSetError::DatabaseError)?;
        let ableton_db = AbletonDatabase::new(db_path).map_err(LiveSetError::DatabaseError)?;

        for (dev_identifier, info) in &self.plugin_info_tags {
            let db_plugin = ableton_db
                .get_plugin_by_dev_identifier(dev_identifier)
                .map_err(LiveSetError::DatabaseError)?;

            let plugin = match db_plugin {
                Some(db_plugin) => {
                    trace_fn!(
                        "finalize_result",
                        "Found plugin {} {} on system, flagging as installed",
                        db_plugin.vendor.as_deref().unwrap_or("Unknown").purple(),
                        db_plugin.name.green()
                    );
                    Plugin {
                        id: Uuid::new_v4(),
                        plugin_id: Some(db_plugin.plugin_id),
                        module_id: db_plugin.module_id,
                        dev_identifier: db_plugin.dev_identifier.clone(),
                        name: db_plugin.name.clone(),
                        vendor: db_plugin.vendor.clone(),
                        version: db_plugin.version.clone(),
                        sdk_version: db_plugin.sdk_version.clone(),
                        flags: db_plugin.flags,
                        scanstate: db_plugin.parsestate,
                        enabled: db_plugin.enabled,
                        plugin_format: info.plugin_format,
                        installed: true,
                    }
                }
                None => {
                    trace_fn!(
                        "finalize_result",
                        "Plugin not found in database: {:?}",
                        info
                    );
                    Plugin {
                        id: Uuid::new_v4(),
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
        if self.options.parse_key {
            // Find the most frequent key signature
            let most_frequent_key = self
                .key_frequencies
                .iter()
                .max_by_key(|&(_, count)| count)
                .map(|(key, count)| {
                    trace_fn!(
                        "finalize_result",
                        "Found most frequent key signature: {} (count: {})",
                        key,
                        count
                    );
                    key.clone()
                })
                .unwrap_or_else(|| {
                    trace_fn!("finalize_result", "No key signatures found, using default");
                    KeySignature::default()
                });

            result.key_signature = Some(most_frequent_key);
        } else {
            result.key_signature = None;
        }

        Ok(result)
    }
    


    /// Handles XML start/empty element events with context-aware processing.
    ///
    /// This is the core event handler that processes XML start and empty elements.
    /// It uses the current parser state to determine how to interpret each XML tag
    /// and what data to extract. The method handles complex nested structures and
    /// maintains state transitions throughout the parsing process.
    ///
    /// # Key Processing Areas
    ///
    /// ## Sample Processing
    /// - **SampleRef**: Initiates sample processing with version awareness
    /// - **FileRef**: Handles file reference contexts
    /// - **Data/Path**: Extracts sample paths (encoded vs direct format)
    ///
    /// ## Plugin Processing
    /// - **SourceContext**: Plugin context initialization
    /// - **BranchSourceContext**: Plugin branch analysis with lookahead
    /// - **PluginDesc**: Plugin description processing
    /// - **Vst3PluginInfo/VstPluginInfo**: Plugin metadata extraction
    ///
    /// ## Musical Properties
    /// - **Tempo/Manual**: Tempo extraction and validation
    /// - **EnumEvent**: Time signature processing
    /// - **CurrentEnd**: End time collection for duration calculation
    /// - **MidiClip/ScaleInformation**: Key signature detection (Live 11+)
    ///
    /// # Arguments
    ///
    /// * `event` - XML start or empty element event
    /// * `reader` - XML reader for lookahead operations
    /// * `byte_pos` - Current byte position for line tracking
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the event was processed successfully, or an error
    /// if parsing fails or required attributes are missing.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - Required XML attributes are missing or malformed
    /// - State transitions are invalid
    /// - XML lookahead operations fail
    /// - Numeric parsing fails for tempo/time signature values
    ///
    /// # State Machine Behavior
    ///
    /// The method implements a complex state machine where the same XML tag
    /// can have different meanings based on current state:
    /// - `<Name>` in plugin context → plugin name
    /// - `<Name>` in scale context → scale name
    /// - `<Manual>` in tempo context → tempo value
    ///
    /// # Performance Considerations
    ///
    /// - Uses selective processing based on [`ParseOptions`]
    /// - Implements lookahead for complex plugin detection
    /// - Maintains efficient state transitions
    /// - Skips irrelevant XML subtrees when possible
    pub fn handle_start_event<R: BufRead>(
        &mut self,
        event: &quick_xml::events::BytesStart,
        reader: &mut Reader<R>,
        byte_pos: &mut u64,
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
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering SampleRef at depth {}, version: {}",
                    line,
                    self.depth,
                    self.ableton_version.major
                );
                self.state = ParserState::InSampleRef {
                    version: self.ableton_version.major,
                };
            }
            "FileRef" if matches!(self.state, ParserState::InSampleRef { .. }) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering FileRef at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InFileRef;
            }
            "Data"
                if matches!(self.state, ParserState::InFileRef)
                    && self.ableton_version.major < 11 =>
            {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Found Data tag for old format sample at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InData {
                    current_data: String::new(),
                };
                self.current_path_type = Some(PathType::Encoded);
            }
            "Path"
                if matches!(self.state, ParserState::InFileRef)
                    && self.ableton_version.major >= 11 =>
            {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Found Path tag for new format sample at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InPath {
                    path_type: PathType::Direct,
                };
                self.current_path_type = Some(PathType::Direct);

                // Extract the path value from the Value attribute
                if let Some(value) = event.try_get_attribute("Value")? {
                    let path_str = value.unescape_value()?.to_string();
                    trace_fn!(
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
                if !matches!(self.state, ParserState::InPluginDesc { .. }) {
                    self.state = ParserState::InSourceContext;
                }
            }
            "Value" if matches!(self.state, ParserState::InSourceContext) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering Value tag inside SourceContext at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InValue;
            }
            "BranchSourceContext" if matches!(self.state, ParserState::InValue) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Found BranchSourceContext at depth {}, looking for device ID",
                    line,
                    self.depth
                );
                self.state = ParserState::InBranchSourceContext;

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
                                    trace_fn!(
                                        "handle_start_event",
                                        "[{}] Found BrowserContentPath at depth {}",
                                        line,
                                        self.depth
                                    );
                                    found_browser_content_path = true;
                                }
                                "BranchDeviceId" => {
                                    if let Some(id) = event.get_value_as_string_result()? {
                                        trace_fn!(
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
                                trace_fn!(
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
                                trace_fn!(
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
                            trace_fn!(
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
                    trace_fn!(
                        "handle_start_event",
                        "[{}] Entering PluginDesc at depth {} for device: {}",
                        line,
                        self.depth,
                        device_id
                    );
                    self.plugin_info_processed = false; // Reset the flag for new PluginDesc
                    self.state = ParserState::InPluginDesc {
                        device_id: device_id.clone(),
                    };
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
                if let ParserState::InPluginDesc { device_id } = &self.state {
                    if self.plugin_info_processed {
                        trace_fn!(
                            "handle_start_event",
                            "[{}] Ignoring subsequent plugin info tag at depth {}: {} for device: {} (already processed)",
                            line,
                            self.depth,
                            name,
                            device_id
                        );
                    } else {
                        trace_fn!(
                            "handle_start_event",
                            "[{}] Found plugin info tag at depth {}: {} for device: {}",
                            line,
                            self.depth,
                            name,
                            device_id
                        );
                        self.state = if name.as_str() == "Vst3PluginInfo" {
                            ParserState::InVst3PluginInfo
                        } else {
                            ParserState::InVstPluginInfo
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
                        ParserState::InVst3PluginInfo | ParserState::InVstPluginInfo => {
                            if !self.plugin_info_processed {
                                if let Some(device_id) = &self.current_branch_info {
                                    if let Some(plugin_format) =
                                        crate::utils::plugins::parse_plugin_format(device_id)
                                    {
                                        trace_fn!(
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
                                        trace_fn!(
                                            "handle_start_event",
                                            "[{}] Adding plugin info at depth {}: {:?}",
                                            line,
                                            self.depth,
                                            plugin_info
                                        );
                                        self.plugin_info_tags
                                            .insert(device_id.clone(), plugin_info);
                                        self.plugin_info_processed = true;
                                    }
                                }
                            } else {
                                trace_fn!(
                                    "handle_start_event",
                                    "[{}] Ignoring plugin name at depth {} (already processed): {}",
                                    line,
                                    self.depth,
                                    value
                                );
                            }
                        }
                        ParserState::InScaleInformation => {
                            trace_fn!(
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
                if !self.options.parse_time_signature {
                    return Ok(());
                }

                // Get the Value attribute
                if let Some(value) = event.try_get_attribute("Value")? {
                    let value_str = value.unescape_value()?.to_string();
                    trace_fn!(
                        "handle_start_event",
                        "[{}] Found EnumEvent with value: {}",
                        line,
                        value_str
                    );

                    // Parse the encoded time signature
                    match crate::utils::time_signature::parse_encoded_time_signature(&value_str) {
                        Ok(encoded_value) => match TimeSignature::from_encoded(encoded_value) {
                            Ok(time_sig) => {
                                trace_fn!(
                                    "handle_start_event",
                                    "[{}] Successfully decoded time signature: {}/{}",
                                    line,
                                    time_sig.numerator,
                                    time_sig.denominator
                                );
                                self.current_time_signature = time_sig;
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
                        },
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
                            trace_fn!(
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
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering Tempo tag at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InTempo {
                    version: self.ableton_version.major,
                };
            }
            "Manual" if matches!(self.state, ParserState::InTempo { .. }) => {
                // Get the Value attribute for the tempo
                if let Some(value) = event.try_get_attribute("Value")? {
                    let value_str = value.unescape_value()?.to_string();
                    match value_str.parse::<f64>() {
                        Ok(tempo) if tempo >= 10.0 && tempo <= 999.0 => {
                            trace_fn!(
                                "handle_start_event",
                                "[{}] Found valid tempo value: {}",
                                line,
                                tempo
                            );
                            self.current_tempo = tempo;
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
                self.state = ParserState::InTempoManual;
            }
            "MidiClip" => {
                if self.options.parse_key {
                    trace_fn!(
                        "handle_start_event",
                        "[{}] Entering MidiClip at depth {}",
                        line,
                        self.depth
                    );
                    self.state = ParserState::InMidiClip;
                    self.current_clip_in_key = false; // Reset for new clip
                    self.current_scale_info = None; // Reset scale info
                }
            }
            "ScaleInformation" if matches!(self.state, ParserState::InMidiClip) => {
                trace_fn!(
                    "handle_start_event",
                    "[{}] Entering ScaleInformation at depth {}",
                    line,
                    self.depth
                );
                self.state = ParserState::InScaleInformation;
            }
            "RootNote" if matches!(self.state, ParserState::InScaleInformation) => {
                if let Some(value) = event.try_get_attribute("Value")? {
                    if let Ok(root_note) = value.unescape_value()?.parse::<i32>() {
                        trace_fn!(
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
            "IsInKey" if matches!(self.state, ParserState::InMidiClip) => {
                if let Some(value) = event.try_get_attribute("Value")? {
                    let is_in_key = value.unescape_value()?.as_ref() == "true";
                    trace_fn!(
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

    /// Handles XML end element events and manages state transitions.
    ///
    /// This method processes XML closing tags and manages the parser's state machine
    /// transitions. It's responsible for finalizing data collection for completed
    /// elements and returning to appropriate parent states.
    ///
    /// # Key Responsibilities
    ///
    /// - **State Restoration**: Returns to appropriate parent states after element processing
    /// - **Data Finalization**: Completes data collection for finished elements
    /// - **Context Cleanup**: Resets temporary state variables
    /// - **Sample Registration**: Adds completed sample paths to the collection
    ///
    /// # State Transitions
    ///
    /// The method handles complex state transitions such as:
    /// - Exiting nested plugin contexts back to source contexts
    /// - Finalizing sample path processing and adding to collection
    /// - Completing tempo and time signature processing
    /// - Cleaning up plugin processing flags and temporary data
    ///
    /// # Arguments
    ///
    /// * `event` - XML end element event containing the closing tag name
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the state transition completed successfully,
    /// or an error if data processing fails.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - Sample path decoding fails (for encoded paths)
    /// - State transitions are inconsistent
    /// - Required data is missing when finalizing elements
    pub fn handle_end_event(
        &mut self,
        event: &quick_xml::events::BytesEnd,
    ) -> Result<(), LiveSetError> {
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
                trace_fn!(
                    "handle_end_event",
                    "Exiting SampleRef at depth {}, resetting state",
                    self.depth
                );
                // If we have a current file reference, add it to our sample paths
                if let Some(path) = self.current_file_ref.take() {
                    trace_fn!("handle_end_event", "Adding sample path: {:?}", path);
                    self.sample_paths.insert(path);
                }
                self.current_path_type = None;
                self.in_source_context = false; // Reset in_source_context when exiting SampleRef
                self.current_branch_info = None; // Reset branch info to ensure next plugin is processed correctly
                self.plugin_info_processed = false; // Reset plugin info processed flag
                self.state = ParserState::Root;
            }
            "FileRef" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting FileRef at depth {}",
                    self.depth
                );
                if let ParserState::InSampleRef { version } = self.state {
                    self.state = ParserState::InSampleRef { version };
                } else {
                    self.state = ParserState::Root;
                }
            }
            "Data" => {
                if let ParserState::InData { ref current_data } = self.state {
                    trace_fn!(
                        "handle_end_event",
                        "Processing encoded path data of length {}",
                        current_data.len()
                    );
                    match crate::utils::samples::decode_sample_path(current_data) {
                        Ok(path) => {
                            trace_fn!(
                                "handle_end_event",
                                "Successfully decoded sample path: {:?}",
                                path
                            );
                            self.current_file_ref = Some(path);
                        }
                        Err(e) => {
                            warn_fn!("handle_end_event", "Failed to decode sample path: {:?}", e);
                        }
                    }
                    // After processing Data tag, return to InFileRef state
                    self.state = ParserState::InFileRef;
                }
            }
            "Path" => {
                if let ParserState::InPath { .. } = self.state {
                    self.state = ParserState::InFileRef;
                }
            }
            "SourceContext" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting SourceContext at depth {}, resetting state",
                    self.depth
                );
                self.in_source_context = false;
                if !matches!(self.state, ParserState::InPluginDesc { .. }) {
                    self.state = ParserState::Root;
                }
            }
            "Value" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting Value at depth {}, returning to SourceContext state",
                    self.depth
                );
                if !matches!(self.state, ParserState::InPluginDesc { .. }) {
                    self.state = ParserState::InSourceContext;
                }
            }
            "BranchSourceContext" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting BranchSourceContext at depth {}, returning to Value state",
                    self.depth
                );
                if !matches!(self.state, ParserState::InPluginDesc { .. }) {
                    self.state = ParserState::InValue;
                }
            }
            "PluginDesc" => {
                // Clear the current branch info and plugin info processed flag
                trace_fn!(
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
                    ParserState::InSourceContext
                } else {
                    trace_fn!(
                        "handle_end_event",
                        "Returning to Root state at depth {}",
                        self.depth
                    );
                    ParserState::Root
                };
            }
            "Vst3PluginInfo" | "VstPluginInfo" => {
                if let Some(device_id) = &self.current_branch_info {
                    trace_fn!(
                        "handle_end_event",
                        "Exiting plugin info tag at depth {}, returning to PluginDesc state for device: {}",
                        self.depth,
                        device_id
                    );
                    self.state = ParserState::InPluginDesc {
                        device_id: device_id.clone(),
                    };
                } else {
                    trace_fn!(
                        "handle_end_event",
                        "Exiting plugin info tag at depth {} but no current device ID",
                        self.depth
                    );
                    self.state = ParserState::Root;
                }
            }
            "Tempo" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting Tempo tag at depth {}, resetting state",
                    self.depth
                );
                self.state = ParserState::Root;
            }
            "Manual" if matches!(self.state, ParserState::InTempoManual) => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting Manual tag at depth {}, returning to Tempo state",
                    self.depth
                );
                self.state = ParserState::InTempo {
                    version: self.ableton_version.major,
                };
            }
            "MidiClip" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting MidiClip at depth {}, resetting state",
                    self.depth
                );
                if matches!(self.state, ParserState::InMidiClip) {
                    self.state = ParserState::Root;
                    self.current_clip_in_key = false;
                    self.current_scale_info = None;
                }
            }
            "ScaleInformation" => {
                trace_fn!(
                    "handle_end_event",
                    "Exiting ScaleInformation at depth {}, returning to MidiClip state",
                    self.depth
                );
                if matches!(self.state, ParserState::InScaleInformation) {
                    self.state = ParserState::InMidiClip;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handles XML text content events for data accumulation.
    ///
    /// This method processes XML text content that appears between opening and
    /// closing tags. It's primarily used for accumulating encoded sample path
    /// data that may be split across multiple text events in older Live versions.
    ///
    /// # Processing Behavior
    ///
    /// - **Selective Processing**: Only processes text when in specific states
    /// - **Data Accumulation**: Appends text to current data buffer
    /// - **Encoding Handling**: Properly unescapes XML entities
    ///
    /// # State-Specific Behavior
    ///
    /// Currently only processes text content when in [`ParserState::InData`] state,
    /// which occurs when reading encoded sample paths in Ableton Live versions < 11.
    /// The text content is accumulated and later decoded as hex-encoded UTF-16.
    ///
    /// # Arguments
    ///
    /// * `event` - XML text event containing the text content
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if text processing succeeds, or an error if
    /// XML unescaping fails.
    ///
    /// # Errors
    ///
    /// Returns [`LiveSetError`] if:
    /// - XML entity unescaping fails
    /// - Text content contains invalid characters
    ///
    /// # Examples
    ///
    /// For encoded sample paths in pre-v11 Live files:
    /// ```xml
    /// <Data>
    ///   48006500780020004400610074006100200032003000310038002D00...
    /// </Data>
    /// ```
    /// 
    /// The hex-encoded text is accumulated and later decoded to a file path.
    pub fn handle_text_event(
        &mut self,
        event: &quick_xml::events::BytesText,
    ) -> Result<(), LiveSetError> {
        if let ParserState::InData {
            ref mut current_data,
        } = self.state
        {
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
