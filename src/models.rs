//! # Core Data Models
//!
//! This module contains all the core data structures used throughout the Studio Project Manager.
//! These models represent the various entities found in Ableton Live projects, including projects
//! themselves, plugins, samples, musical metadata, and more.
//!
//! ## Key Types
//!
//! - [`AbletonVersion`]: Represents an Ableton Live version with comparison support
//! - [`Plugin`]: Represents a plugin with installation status and metadata
//! - [`Sample`]: Represents an audio sample with file presence validation
//! - [`KeySignature`]: Musical key information combining tonic and scale
//! - [`TimeSignature`]: Musical time signature with validation
//! - [`PluginFormat`]: Enumeration of supported plugin formats (VST2/VST3)
//!
//! ## Musical Types
//!
//! The module includes comprehensive musical type definitions:
//! - [`Tonic`]: Musical root notes (C, D, E, etc.)
//! - [`Scale`]: Musical scales (Major, Minor, Dorian, etc.)
//!
//! These types support parsing from strings and provide display formatting for UI purposes.

use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::sync::Arc;
use uuid::Uuid;

use once_cell::sync::Lazy;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{DatabaseError, SampleError, TimeSignatureError};
use crate::utils::plugins::get_most_recent_db_file;

/// Unique identifier type for database entities.
///
/// This is a wrapper around `u64` that provides type safety for entity IDs.
/// Currently used internally for database operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u64);

/// Represents an Ableton Live version with semantic version comparison support.
///
/// This struct stores version information for Ableton Live projects, including
/// major, minor, and patch versions, as well as beta status. It implements
/// proper version ordering where non-beta versions are considered greater
/// than beta versions of the same number.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::AbletonVersion;
///
/// let v11_2_0 = AbletonVersion {
///     major: 11,
///     minor: 2,
///     patch: 0,
///     beta: false,
/// };
///
/// let v11_1_0 = AbletonVersion {
///     major: 11,
///     minor: 1,
///     patch: 0,
///     beta: false,
/// };
///
/// assert!(v11_2_0 > v11_1_0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbletonVersion {
    /// Major version number (e.g., 11 for Ableton Live 11)
    pub major: u32,
    /// Minor version number (e.g., 2 for version 11.2.0)
    pub minor: u32,
    /// Patch version number (e.g., 5 for version 11.2.5)
    pub patch: u32,
    /// Whether this is a beta release
    pub beta: bool,
}

impl Default for AbletonVersion {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
            beta: false,
        }
    }
}

impl PartialOrd for AbletonVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Compare major versions first
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Equal => {
                // If major versions are equal, compare minor versions
                match self.minor.cmp(&other.minor) {
                    std::cmp::Ordering::Equal => {
                        // If minor versions are equal, compare patch versions
                        match self.patch.cmp(&other.patch) {
                            std::cmp::Ordering::Equal => {
                                // If all version numbers are equal, non-beta is greater than beta
                                Some((!self.beta).cmp(&(!other.beta)))
                            }
                            ord => Some(ord),
                        }
                    }
                    ord => Some(ord),
                }
            }
            ord => Some(ord),
        }
    }
}

impl Ord for AbletonVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// Musical scales supported by Ableton Live.
///
/// This enum represents all the musical scales that can be detected in Ableton Live projects.
/// It includes common scales like Major and Minor, as well as more exotic scales and modes.
/// The scales are organized into several categories:
///
/// ## Common Scales
/// - [`Scale::Major`]: Major scale
/// - [`Scale::Minor`]: Natural minor scale
/// - [`Scale::HarmonicMinor`]: Harmonic minor scale
/// - [`Scale::MelodicMinor`]: Melodic minor scale
///
/// ## Modes
/// - [`Scale::Dorian`]: Dorian mode
/// - [`Scale::Mixolydian`]: Mixolydian mode
/// - [`Scale::Aeolian`]: Aeolian mode (natural minor)
/// - [`Scale::Phrygian`]: Phrygian mode
/// - [`Scale::Locrian`]: Locrian mode
///
/// ## Pentatonic Scales
/// - [`Scale::MajorPentatonic`]: Major pentatonic scale
/// - [`Scale::MinorPentatonic`]: Minor pentatonic scale
/// - [`Scale::MinorBlues`]: Minor blues scale
///
/// ## Exotic Scales
/// - [`Scale::WholeTone`]: Whole tone scale
/// - [`Scale::HalfWholeDim`]: Half-whole diminished scale
/// - [`Scale::WholeHalfDim`]: Whole-half diminished scale
/// - [`Scale::Hirajoshi`]: Japanese Hirajoshi scale
/// - [`Scale::Iwato`]: Japanese Iwato scale
/// - [`Scale::PelogSelisir`]: Indonesian Pelog Selisir scale
/// - [`Scale::PelogTembung`]: Indonesian Pelog Tembung scale
///
/// ## Messiaen Modes
/// - [`Scale::Messiaen1`] through [`Scale::Messiaen7`]: Messiaen's modes of limited transposition
///
/// The enum supports parsing from strings and display formatting for UI purposes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(dead_code)]
pub enum Scale {
    /// Empty/unset scale
    Empty,
    /// Major scale (Ionian mode)
    Major,
    /// Natural minor scale
    Minor,
    /// Dorian mode
    Dorian,
    /// Mixolydian mode
    Mixolydian,
    /// Aeolian mode (natural minor)
    Aeolian,
    /// Phrygian mode
    Phrygian,
    /// Locrian mode
    Locrian,
    /// Whole tone scale
    WholeTone,
    /// Half-whole diminished scale
    HalfWholeDim,
    /// Whole-half diminished scale
    WholeHalfDim,
    /// Minor blues scale
    MinorBlues,
    /// Minor pentatonic scale
    MinorPentatonic,
    /// Major pentatonic scale
    MajorPentatonic,
    /// Harmonic minor scale
    HarmonicMinor,
    /// Melodic minor scale
    MelodicMinor,
    /// Dorian #4 mode
    Dorian4,
    /// Phrygian dominant scale
    PhrygianDominant,
    /// Lydian dominant scale
    LydianDominant,
    /// Lydian augmented scale
    LydianAugmented,
    /// Harmonic major scale
    HarmonicMajor,
    /// Super locrian scale
    SuperLocrian,
    /// Spanish scale
    BToneSpanish,
    /// Hungarian minor scale
    HungarianMinor,
    /// Japanese Hirajoshi scale
    Hirajoshi,
    /// Japanese Iwato scale
    Iwato,
    /// Indonesian Pelog Selisir scale
    PelogSelisir,
    /// Indonesian Pelog Tembung scale
    PelogTembung,
    /// Messiaen mode 1 (whole tone)
    Messiaen1,
    /// Messiaen mode 2 (octatonic)
    Messiaen2,
    /// Messiaen mode 3
    Messiaen3,
    /// Messiaen mode 4
    Messiaen4,
    /// Messiaen mode 5
    Messiaen5,
    /// Messiaen mode 6
    Messiaen6,
    /// Messiaen mode 7
    Messiaen7,
}

/// Musical tonic (root note) for key signatures.
///
/// This enum represents the twelve chromatic pitches that can serve as the root
/// note of a key signature. It includes both natural notes (C, D, E, F, G, A, B)
/// and their sharp variants.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::Tonic;
///
/// // Create a tonic from MIDI note number
/// let tonic = Tonic::from_midi_note(60); // Middle C
/// assert_eq!(tonic, Tonic::C);
///
/// // Parse from string
/// let tonic: Tonic = "CSharp".parse().unwrap();
/// assert_eq!(tonic, Tonic::CSharp);
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(dead_code)]
pub enum Tonic {
    /// Empty/unset tonic
    Empty,
    /// C natural
    C,
    /// C sharp / D flat
    CSharp,
    /// D natural
    D,
    /// D sharp / E flat
    DSharp,
    /// E natural
    E,
    /// F natural
    F,
    /// F sharp / G flat
    FSharp,
    /// G natural
    G,
    /// G sharp / A flat
    GSharp,
    /// A natural
    A,
    /// A sharp / B flat
    ASharp,
    /// B natural
    B,
}

impl Tonic {
    /// Creates a tonic from a MIDI note number.
    ///
    /// This method converts a MIDI note number to the corresponding tonic by
    /// using modulo 12 arithmetic. MIDI note 60 corresponds to middle C.
    ///
    /// # Arguments
    ///
    /// * `number` - MIDI note number (0-127, but any i32 is accepted)
    ///
    /// # Returns
    ///
    /// The corresponding [`Tonic`] variant
    ///
    /// # Examples
    ///
    /// ```rust
    /// use studio_project_manager::models::Tonic;
    ///
    /// assert_eq!(Tonic::from_midi_note(60), Tonic::C);      // Middle C
    /// assert_eq!(Tonic::from_midi_note(61), Tonic::CSharp); // C#
    /// assert_eq!(Tonic::from_midi_note(72), Tonic::C);      // C an octave higher
    /// ```
    pub fn from_midi_note(number: i32) -> Self {
        match number % 12 {
            0 => Tonic::C,
            1 => Tonic::CSharp,
            2 => Tonic::D,
            3 => Tonic::DSharp,
            4 => Tonic::E,
            5 => Tonic::F,
            6 => Tonic::FSharp,
            7 => Tonic::G,
            8 => Tonic::GSharp,
            9 => Tonic::A,
            10 => Tonic::ASharp,
            11 => Tonic::B,
            _ => unreachable!(),
        }
    }
}

impl FromStr for Tonic {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Empty" => Ok(Tonic::Empty),
            "C" => Ok(Tonic::C),
            "CSharp" => Ok(Tonic::CSharp),
            "D" => Ok(Tonic::D),
            "DSharp" => Ok(Tonic::DSharp),
            "E" => Ok(Tonic::E),
            "F" => Ok(Tonic::F),
            "FSharp" => Ok(Tonic::FSharp),
            "G" => Ok(Tonic::G),
            "GSharp" => Ok(Tonic::GSharp),
            "A" => Ok(Tonic::A),
            "ASharp" => Ok(Tonic::ASharp),
            "B" => Ok(Tonic::B),
            _ => Err(format!("Invalid tonic: {}", s)),
        }
    }
}

impl FromStr for Scale {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Empty" => Ok(Scale::Empty),
            "Major" => Ok(Scale::Major),
            "Minor" => Ok(Scale::Minor),
            "Dorian" => Ok(Scale::Dorian),
            "Mixolydian" => Ok(Scale::Mixolydian),
            "Aeolian" => Ok(Scale::Aeolian),
            "Phrygian" => Ok(Scale::Phrygian),
            "Locrian" => Ok(Scale::Locrian),
            "WholeTone" => Ok(Scale::WholeTone),
            "HalfWholeDim" => Ok(Scale::HalfWholeDim),
            "WholeHalfDim" => Ok(Scale::WholeHalfDim),
            "MinorBlues" => Ok(Scale::MinorBlues),
            "MinorPentatonic" => Ok(Scale::MinorPentatonic),
            "MajorPentatonic" => Ok(Scale::MajorPentatonic),
            "HarmonicMinor" => Ok(Scale::HarmonicMinor),
            "MelodicMinor" => Ok(Scale::MelodicMinor),
            "Dorian4" => Ok(Scale::Dorian4),
            "PhrygianDominant" => Ok(Scale::PhrygianDominant),
            "LydianDominant" => Ok(Scale::LydianDominant),
            "LydianAugmented" => Ok(Scale::LydianAugmented),
            "HarmonicMajor" => Ok(Scale::HarmonicMajor),
            "SuperLocrian" => Ok(Scale::SuperLocrian),
            "BToneSpanish" => Ok(Scale::BToneSpanish),
            "HungarianMinor" => Ok(Scale::HungarianMinor),
            "Hirajoshi" => Ok(Scale::Hirajoshi),
            "Iwato" => Ok(Scale::Iwato),
            "PelogSelisir" => Ok(Scale::PelogSelisir),
            "PelogTembung" => Ok(Scale::PelogTembung),
            "Messiaen1" => Ok(Scale::Messiaen1),
            "Messiaen2" => Ok(Scale::Messiaen2),
            "Messiaen3" => Ok(Scale::Messiaen3),
            "Messiaen4" => Ok(Scale::Messiaen4),
            "Messiaen5" => Ok(Scale::Messiaen5),
            "Messiaen6" => Ok(Scale::Messiaen6),
            "Messiaen7" => Ok(Scale::Messiaen7),
            _ => Err(format!("Invalid scale: {}", s)),
        }
    }
}

impl FromStr for PluginFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "VST2Instrument" | "VST2 Instrument" => Ok(PluginFormat::VST2Instrument),
            "VST2AudioFx" | "VST2 Effect" => Ok(PluginFormat::VST2AudioFx),
            "VST3Instrument" | "VST3 Instrument" => Ok(PluginFormat::VST3Instrument),
            "VST3AudioFx" | "VST3 Effect" => Ok(PluginFormat::VST3AudioFx),
            _ => Err(format!("Invalid plugin format: {}", s)),
        }
    }
}

impl fmt::Display for Tonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for Scale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Musical key signature combining a tonic and scale.
///
/// This struct represents a complete key signature as used in music theory,
/// combining a root note ([`Tonic`]) with a [`Scale`] to define the key of
/// a musical piece or section.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::{KeySignature, Tonic, Scale};
///
/// let c_major = KeySignature {
///     tonic: Tonic::C,
///     scale: Scale::Major,
/// };
///
/// let a_minor = KeySignature {
///     tonic: Tonic::A,
///     scale: Scale::Minor,
/// };
///
/// // Display formatting
/// println!("{}", c_major); // "C Major"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySignature {
    /// The root note of the key
    pub tonic: Tonic,
    /// The scale type of the key
    pub scale: Scale,
}

impl fmt::Display for KeySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {:?}", self.tonic, self.scale)
    }
}

// PLUGINS

/// Plugin format types supported by Ableton Live.
///
/// This enum represents the different plugin formats that can be used in
/// Ableton Live projects. It distinguishes between VST2 and VST3 formats,
/// as well as between instruments and audio effects.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::PluginFormat;
///
/// let format = PluginFormat::VST3Instrument;
/// println!("{}", format); // "VST3 Instrument"
///
/// // Get development type and category
/// let (dev_type, category) = format.to_dev_type_and_category();
/// assert_eq!(dev_type, "vst3");
/// assert_eq!(category, "instr");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginFormat {
    /// VST2 instrument plugin
    VST2Instrument,
    /// VST2 audio effect plugin
    VST2AudioFx,
    /// VST3 instrument plugin
    VST3Instrument,
    /// VST3 audio effect plugin
    VST3AudioFx,
}

impl PluginFormat {
    /// Generates a random plugin format for testing purposes.
    ///
    /// This method is primarily used in testing and development to create
    /// random plugin formats. It selects equally from all four format variants.
    ///
    /// # Returns
    ///
    /// A randomly selected [`PluginFormat`] variant
    pub fn random() -> Self {
        let variants = [
            PluginFormat::VST2Instrument,
            PluginFormat::VST2AudioFx,
            PluginFormat::VST3Instrument,
            PluginFormat::VST3AudioFx,
        ];
        *variants.choose(&mut thread_rng()).unwrap()
    }

    /// Converts the plugin format to development type and category strings.
    ///
    /// This method maps the plugin format to the string representations used
    /// in Ableton's plugin database for development type and category.
    ///
    /// # Returns
    ///
    /// A tuple containing `(dev_type, category)` where:
    /// - `dev_type` is either "vst" or "vst3"
    /// - `category` is either "instr" or "audiofx"
    ///
    /// # Examples
    ///
    /// ```rust
    /// use studio_project_manager::models::PluginFormat;
    ///
    /// let format = PluginFormat::VST3Instrument;
    /// let (dev_type, category) = format.to_dev_type_and_category();
    /// assert_eq!(dev_type, "vst3");
    /// assert_eq!(category, "instr");
    /// ```
    pub fn to_dev_type_and_category(self) -> (&'static str, &'static str) {
        match self {
            PluginFormat::VST2Instrument => ("vst", "instr"),
            PluginFormat::VST2AudioFx => ("vst", "audiofx"),
            PluginFormat::VST3Instrument => ("vst3", "instr"),
            PluginFormat::VST3AudioFx => ("vst3", "audiofx"),
        }
    }
}

impl fmt::Display for PluginFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginFormat::VST2Instrument => write!(f, "VST2 Instrument"),
            PluginFormat::VST2AudioFx => write!(f, "VST2 Effect"),
            PluginFormat::VST3Instrument => write!(f, "VST3 Instrument"),
            PluginFormat::VST3AudioFx => write!(f, "VST3 Effect"),
        }
    }
}

/// Represents a plugin used in an Ableton Live project.
///
/// This struct contains comprehensive information about a plugin, including
/// both metadata extracted from the project file and installation status
/// determined by checking against Ableton's plugin database.
///
/// # Plugin Installation Status
///
/// The [`Plugin::installed`] field indicates whether the plugin is currently
/// installed on the system. This is determined by cross-referencing the
/// plugin's [`Plugin::dev_identifier`] with Ableton's plugin database.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::{Plugin, PluginFormat};
/// use uuid::Uuid;
///
/// let plugin = Plugin::new(
///     "Serum".to_string(),
///     "serum_vst".to_string(),
///     PluginFormat::VST3Instrument,
/// );
///
/// // Check if plugin is installed
/// if plugin.installed {
///     println!("Plugin {} is installed", plugin.name);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Plugin {
    /// Unique identifier for our database
    pub id: Uuid,
    /// Ableton database plugin ID (if found)
    pub plugin_id: Option<i32>,
    /// Ableton database module ID (if found)
    pub module_id: Option<i32>,
    /// Developer identifier used to uniquely identify the plugin
    pub dev_identifier: String,
    /// Human-readable plugin name
    pub name: String,
    /// Plugin vendor/manufacturer
    pub vendor: Option<String>,
    /// Plugin version string
    pub version: Option<String>,
    /// SDK version used to build the plugin
    pub sdk_version: Option<String>,
    /// Plugin-specific flags from Ableton database
    pub flags: Option<i32>,
    /// Scan state from Ableton database
    pub scanstate: Option<i32>,
    /// Whether the plugin is enabled in Ableton
    pub enabled: Option<i32>,
    /// The format/type of this plugin
    pub plugin_format: PluginFormat,
    /// Whether the plugin is currently installed on the system
    pub installed: bool,
}

/// Plugin data with usage statistics for gRPC responses
pub struct GrpcPlugin {
    /// The base plugin data
    pub plugin: Plugin,
    /// Number of times this plugin is used across all projects
    pub usage_count: i32,
    /// Number of unique projects that use this plugin
    pub project_count: i32,
}

#[allow(dead_code)]
impl Plugin {
    /// Creates a new plugin instance with minimal information.
    ///
    /// This constructor creates a plugin with the provided basic information
    /// and sets all optional fields to `None`. The plugin is initially marked
    /// as not installed until [`Plugin::reparse`] is called.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable plugin name
    /// * `dev_identifier` - Unique developer identifier for the plugin
    /// * `plugin_format` - The format/type of the plugin
    ///
    /// # Returns
    ///
    /// A new [`Plugin`] instance with a generated UUID
    ///
    /// # Examples
    ///
    /// ```rust
    /// use studio_project_manager::models::{Plugin, PluginFormat};
    ///
    /// let plugin = Plugin::new(
    ///     "Massive".to_string(),
    ///     "massive_vst".to_string(),
    ///     PluginFormat::VST2Instrument,
    /// );
    ///
    /// assert_eq!(plugin.name, "Massive");
    /// assert_eq!(plugin.installed, false); // Not yet parsed
    /// ```
    pub fn new(name: String, dev_identifier: String, plugin_format: PluginFormat) -> Self {
        Self {
            id: Uuid::new_v4(),
            plugin_id: None,
            module_id: None,
            dev_identifier,
            name,
            vendor: None,
            version: None,
            sdk_version: None,
            flags: None,
            scanstate: None,
            enabled: None,
            plugin_format,
            installed: false,
        }
    }

    /// Updates plugin information by querying Ableton's plugin database.
    ///
    /// This method looks up the plugin in Ableton's database using the
    /// [`Plugin::dev_identifier`] and updates all available fields with
    /// the information found. If the plugin is found, it's marked as installed.
    ///
    /// # Arguments
    ///
    /// * `db` - Reference to the Ableton database connection
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the database query succeeds (regardless of whether
    /// the plugin was found), or a [`DatabaseError`] if the query fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use studio_project_manager::models::{Plugin, PluginFormat};
    /// use studio_project_manager::ableton_db::AbletonDatabase;
    ///
    /// let mut plugin = Plugin::new(
    ///     "Operator".to_string(),
    ///     "operator_live".to_string(),
    ///     PluginFormat::VST2Instrument,
    /// );
    ///
    /// let db = AbletonDatabase::new("path/to/ableton.db".into()).unwrap();
    /// plugin.reparse(&db).unwrap();
    ///
    /// if plugin.installed {
    ///     println!("Plugin {} by {} is installed", plugin.name, plugin.vendor.unwrap_or("Unknown".to_string()));
    /// }
    /// ```
    pub fn reparse(&mut self, db: &AbletonDatabase) -> Result<(), DatabaseError> {
        if let Some(db_plugin) = db.get_plugin_by_dev_identifier(&self.dev_identifier)? {
            self.plugin_id = Some(db_plugin.plugin_id);
            self.module_id = db_plugin.module_id;
            self.name = db_plugin.name;
            self.vendor = db_plugin.vendor;
            self.version = db_plugin.version;
            self.sdk_version = db_plugin.sdk_version;
            self.flags = db_plugin.flags;
            self.scanstate = db_plugin.parsestate;
            self.enabled = db_plugin.enabled;
            self.installed = true;
        } else {
            self.installed = false;
        }
        Ok(())
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct PluginInfo {
    pub name: String,
    pub dev_identifier: String,
    pub plugin_format: PluginFormat,
}

impl fmt::Display for PluginInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.plugin_format, self.name)
    }
}

// Plugin implementations

#[allow(unused_variables)]
static INSTALLED_PLUGINS: Lazy<Arc<Result<HashSet<(String, PluginFormat)>, DatabaseError>>> =
    Lazy::new(|| {
        Arc::new({
            (|| {
                let config = CONFIG
                    .as_ref()
                    .map_err(|e| DatabaseError::ConfigError(e.clone()))?;
                let db_dir = PathBuf::from(&config.live_database_dir);
                let db_path = get_most_recent_db_file(&db_dir)?;

                let db = AbletonDatabase::new(db_path)?;

                db.get_database_plugins()
                    .map(|vec| vec.into_iter().collect::<HashSet<_>>())
            })()
        })
    });

/// Returns a set of all plugins installed on the system.
///
/// This function queries Ableton's plugin database to get a list of all
/// currently installed plugins. The result is cached globally for performance.
///
/// # Returns
///
/// Returns an `Arc<Result<HashSet<(String, PluginFormat)>, DatabaseError>>` where:
/// - The `HashSet` contains tuples of `(dev_identifier, plugin_format)`
/// - The `Arc` allows sharing the result between threads
/// - The `Result` indicates whether the database query succeeded
///
/// # Errors
///
/// Returns a [`DatabaseError`] if:
/// - The configuration cannot be loaded
/// - The Ableton database file cannot be found or opened
/// - The database query fails
///
/// # Examples
///
/// ```rust,no_run
/// use studio_project_manager::models::get_installed_plugins;
///
/// match get_installed_plugins().as_ref() {
///     Ok(plugins) => {
///         println!("Found {} installed plugins", plugins.len());
///         for (dev_id, format) in plugins.iter() {
///             println!("  {} ({})", dev_id, format);
///         }
///     }
///     Err(e) => eprintln!("Failed to get installed plugins: {}", e),
/// }
/// ```
#[allow(dead_code)]
pub fn get_installed_plugins() -> Arc<Result<HashSet<(String, PluginFormat)>, DatabaseError>> {
    INSTALLED_PLUGINS.clone()
}

// Sample types

/// Represents an audio sample used in an Ableton Live project.
///
/// This struct contains information about audio samples referenced in project files,
/// including their file system location and whether they are currently present
/// on the system. The presence check is performed to identify missing samples
/// that may cause playback issues.
///
/// # Sample Presence
///
/// The [`Sample::is_present`] field indicates whether the sample file exists
/// at the specified path. This is useful for identifying projects with missing
/// samples that may not play correctly.
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::Sample;
/// use std::path::PathBuf;
///
/// let sample = Sample::new(
///     "kick.wav".to_string(),
///     PathBuf::from("/path/to/kick.wav"),
/// );
///
/// if sample.is_present {
///     println!("Sample {} is available", sample.name);
/// } else {
///     println!("Sample {} is missing!", sample.name);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sample {
    /// Unique identifier for our database
    pub id: Uuid,
    /// Human-readable sample name (usually the filename)
    pub name: String,
    /// File system path to the sample file
    pub path: PathBuf,
    /// Whether the sample file exists on the system
    pub is_present: bool,
}

#[allow(dead_code)]
impl Sample {
    pub fn new(name: String, path: PathBuf) -> Self {
        let is_present = path.exists();
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            is_present,
        }
    }

    pub fn from_pre_11_data(data: &str) -> Result<Self, SampleError> {
        let cleaned_data = data.replace('\t', "").replace('\n', "");
        let byte_data = hex::decode(&cleaned_data).map_err(SampleError::HexDecodeError)?;

        let utf16_chunks: Vec<u16> = byte_data
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        let path_string = String::from_utf16(&utf16_chunks)
            .map_err(|_| SampleError::InvalidUtf16Encoding)?
            .replace('\0', "");

        let path = PathBuf::from(path_string);

        if !path.exists() {
            return Err(SampleError::FileNotFound(path));
        }

        let name = path
            .file_name()
            .and_then(|osstr| osstr.to_str())
            .map(String::from)
            .unwrap_or_else(|| "Unknown".to_string());

        Ok(Self::new(name, path))
    }

    pub fn from_11_plus_data(path_value: &str) -> Self {
        let path = PathBuf::from(path_value);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Self::new(name, path)
    }

    pub fn is_present(&self) -> bool {
        self.is_present
    }

    pub fn update_presence(&mut self) {
        self.is_present = self.path.exists();
    }
}

/// Musical time signature with validation support.
///
/// This struct represents a time signature as used in music theory, consisting
/// of a numerator (beats per measure) and denominator (note value that gets
/// the beat). The struct includes validation to ensure the time signature
/// values are musically valid.
///
/// # Validation Rules
///
/// - Numerator must be between 1 and 99 inclusive
/// - Denominator must be between 1 and 16 inclusive and be a power of 2
///
/// # Examples
///
/// ```rust
/// use studio_project_manager::models::TimeSignature;
///
/// // Common time signatures
/// let four_four = TimeSignature { numerator: 4, denominator: 4 };
/// let three_four = TimeSignature { numerator: 3, denominator: 4 };
/// let six_eight = TimeSignature { numerator: 6, denominator: 8 };
///
/// assert!(four_four.is_valid());
/// assert!(three_four.is_valid());
/// assert!(six_eight.is_valid());
///
/// // Invalid time signature
/// let invalid = TimeSignature { numerator: 4, denominator: 3 }; // 3 is not a power of 2
/// assert!(!invalid.is_valid());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSignature {
    /// Number of beats per measure
    pub numerator: u8,
    /// Note value that gets the beat (must be a power of 2)
    pub denominator: u8,
}

impl TimeSignature {
    pub fn is_valid(&self) -> bool {
        // Check numerator is between 1 and 99
        if !(1..=99).contains(&self.numerator) {
            return false;
        }

        // Check denominator is between 1 and 16 and is a power of 2
        if self.denominator > 16 || self.denominator < 1 {
            return false;
        }

        // Check if denominator is a power of 2
        self.denominator & (self.denominator - 1) == 0
    }

    pub fn from_encoded(encoded_value: i32) -> Result<Self, TimeSignatureError> {
        if encoded_value < 0 || encoded_value > 494 {
            return Err(TimeSignatureError::InvalidEncodedValue(encoded_value));
        }

        let numerator = Self::decode_numerator(encoded_value);
        let denominator = Self::decode_denominator(encoded_value);

        Ok(TimeSignature {
            numerator,
            denominator,
        })
    }

    fn decode_numerator(encoded_value: i32) -> u8 {
        if encoded_value < 0 {
            1
        } else if encoded_value < 99 {
            (encoded_value + 1) as u8
        } else {
            ((encoded_value % 99) + 1) as u8
        }
    }

    fn decode_denominator(encoded_value: i32) -> u8 {
        let multiple = encoded_value / 99 + 1;
        2_u8.pow((multiple - 1) as u32)
    }
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            numerator: 0,
            denominator: 0,
        }
    }
}

impl Default for KeySignature {
    fn default() -> Self {
        KeySignature {
            tonic: Tonic::Empty,
            scale: Scale::Empty,
        }
    }
}

impl fmt::Display for AbletonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ableton {}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Default for Id {
    fn default() -> Self {
        Id(0)
    }
}

/// Statistics for a collection (for status bar display)
#[derive(Debug, Clone)]
pub struct CollectionStatistics {
    /// Number of projects in the collection
    pub project_count: i32,
    /// Total duration of all projects in seconds
    pub total_duration_seconds: Option<f64>,
    /// Average tempo across all projects
    pub average_tempo: Option<f64>,
    /// Total number of unique plugins used across all projects
    pub total_plugins: i32,
    /// Total number of unique samples used across all projects
    pub total_samples: i32,
    /// Total number of unique tags used across all projects
    pub total_tags: i32,
    /// Most common key signature across all projects
    pub most_common_key: Option<String>,
    /// Most common time signature across all projects
    pub most_common_time_signature: Option<String>,
}
