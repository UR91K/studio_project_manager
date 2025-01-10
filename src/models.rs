// /src/models.rs
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;

use log::debug;
use once_cell::sync::Lazy;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::name::QName;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{DatabaseError, SampleError, TimeSignatureError, VersionError};
use crate::utils::plugins::get_most_recent_db_file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Id(u64);

#[derive(Debug, Clone)]
pub(crate) struct XmlTag {
    pub(crate) name: String,
    pub(crate) attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbletonVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
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

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(dead_code)]
pub(crate) enum Scale {
    Empty,
    Major,
    Minor,
    Dorian,
    Mixolydian,
    Aeolian,
    Phrygian,
    Locrian,
    WholeTone,
    HalfWholeDim,
    WholeHalfDim,
    MinorBlues,
    MinorPentatonic,
    MajorPentatonic,
    HarmonicMinor,
    MelodicMinor,
    Dorian4,
    PhrygianDominant,
    LydianDominant,
    LydianAugmented,
    HarmonicMajor,
    SuperLocrian,
    BToneSpanish,
    HungarianMinor,
    Hirajoshi,
    Iwato,
    PelogSelisir,
    PelogTembung,
    Messiaen1,
    Messiaen2,
    Messiaen3,
    Messiaen4,
    Messiaen5,
    Messiaen6,
    Messiaen7,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(dead_code)]
pub(crate) enum Tonic {
    Empty,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

impl Tonic {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySignature {
    pub tonic: Tonic,
    pub scale: Scale,
}

impl fmt::Display for KeySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {:?}", self.tonic, self.scale)
    }
}

// PLUGINS

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PluginFormat {
    VST2Instrument,
    VST2AudioFx,
    VST3Instrument,
    VST3AudioFx,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Plugin {
    pub(crate) plugin_id: Option<i32>,
    pub(crate) module_id: Option<i32>,
    pub(crate) dev_identifier: String,
    pub(crate) name: String,
    pub(crate) vendor: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) sdk_version: Option<String>,
    pub(crate) flags: Option<i32>,
    pub(crate) scanstate: Option<i32>,
    pub(crate) enabled: Option<i32>,
    pub(crate) plugin_format: PluginFormat,
    pub(crate) installed: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PluginInfo {
    pub(crate) name: String,
    pub(crate) dev_identifier: String,
    pub(crate) plugin_format: PluginFormat,
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

    #[allow(dead_code)]
pub fn get_installed_plugins() -> Arc<Result<HashSet<(String, PluginFormat)>, DatabaseError>> {
    INSTALLED_PLUGINS.clone()
}

impl Plugin {
    pub fn rescan(&mut self, db: &AbletonDatabase) -> Result<(), DatabaseError> {
        if let Some(db_plugin) = db.get_plugin_by_dev_identifier(&self.dev_identifier)? {
            self.plugin_id = Some(db_plugin.plugin_id);
            self.module_id = db_plugin.module_id;
            self.name = db_plugin.name;
            self.vendor = db_plugin.vendor;
            self.version = db_plugin.version;
            self.sdk_version = db_plugin.sdk_version;
            self.flags = db_plugin.flags;
            self.scanstate = db_plugin.scanstate;
            self.enabled = db_plugin.enabled;
            self.installed = true;
        } else {
            self.installed = false;
        }
        Ok(())
    }
}

// Sample types

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Sample {
    pub(crate) id: Id,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_present: bool,
}

#[allow(dead_code)]
impl Sample {
    pub(crate) fn new(id: Id, name: String, path: PathBuf) -> Self {
        let is_present = path.exists();
        Self {
            id,
            name,
            path,
            is_present,
        }
    }

    pub(crate) fn from_pre_11_data(data: &str) -> Result<Self, SampleError> {
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

        Ok(Self::new(Id::default(), name, path))
    }

    pub(crate) fn from_11_plus_data(path_value: &str) -> Self {
        let path = PathBuf::from(path_value);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Self::new(Id::default(), name, path)
    }

    pub(crate) fn is_present(&self) -> bool {
        self.is_present
    }

    pub(crate) fn update_presence(&mut self) {
        self.is_present = self.path.exists();
    }
}

#[derive(Debug, Clone)]
pub struct TimeSignature {
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

impl TimeSignature {
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
            numerator: 4,
            denominator: 4,
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

impl AbletonVersion {
    pub fn from_attributes<'a, I>(attributes: I) -> Result<Self, VersionError>
    where
        I: Iterator<Item = Result<Attribute<'a>, AttrError>>,
    {
        let mut creator = None;

        for attr in attributes {
            let attr = attr.map_err(VersionError::AttrError)?;
            if attr.key == QName(b"Creator") {
                creator = Some(
                    str::from_utf8(&attr.value)
                        .map_err(VersionError::Utf8Error)?
                        .to_string(),
                );
            }
        }

        let creator =
            creator.ok_or_else(|| VersionError::MissingRequiredAttribute("Creator".to_string()))?;
        debug!("Creator: {}", creator);

        Self::parse_version_string(&creator)
    }

    fn parse_version_string(creator: &str) -> Result<Self, VersionError> {
        let version_str = creator
            .strip_prefix("Ableton Live ")
            .ok_or(VersionError::InvalidFormat)?;

        let beta = version_str.to_lowercase().contains("beta");

        let version_str = version_str.replace("Beta", "");
        let version_parts: Vec<&str> = version_str
            .split_ascii_whitespace()
            .next()
            .ok_or(VersionError::InvalidFormat)?
            .split('.')
            .collect();

        if version_parts.len() < 2 || version_parts.len() > 3 {
            return Err(VersionError::InvalidFormat);
        }

        let parse_version = |s: &str| s.parse().map_err(|e| VersionError::ParseError(e));

        Ok(AbletonVersion {
            major: parse_version(version_parts[0])?,
            minor: parse_version(version_parts[1])?,
            patch: version_parts.get(2).map_or(Ok(0), |&s| parse_version(s))?,
            beta,
        })
    }
}

impl Default for Id {
    fn default() -> Self {
        Id(0)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use quick_xml::events::attributes::Attribute;
    use quick_xml::name::QName;

    use super::*;

    #[test]
    fn test_from_attributes() {
        let attributes = vec![Ok(Attribute {
            key: QName(b"Creator"),
            value: Cow::Borrowed(b"Ableton Live 11.0.12"),
        })];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 12);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_from_attributes_beta() {
        let attributes = vec![Ok(Attribute {
            key: QName(b"Creator"),
            value: Cow::Borrowed(b"Ableton Live 12.0 Beta"),
        })];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 12);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, true);
    }

    #[test]
    fn test_from_attributes_no_patch() {
        let attributes = vec![Ok(Attribute {
            key: QName(b"Creator"),
            value: Cow::Borrowed(b"Ableton Live 12.0"),
        })];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 12);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_from_attributes_missing_creator() {
        let attributes = vec![Ok(Attribute {
            key: QName(b"SomeOtherAttribute"),
            value: Cow::Borrowed(b"SomeValue"),
        })];

        assert!(AbletonVersion::from_attributes(attributes.into_iter()).is_err());
    }

    #[test]
    fn test_from_attributes_invalid_format() {
        let attributes = vec![Ok(Attribute {
            key: QName(b"Creator"),
            value: Cow::Borrowed(b"Not Ableton Live Version"),
        })];

        assert!(AbletonVersion::from_attributes(attributes.into_iter()).is_err());
    }
}
