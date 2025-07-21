#![allow(dead_code)]

use elementtree::Error as ElementTreeError;
use quick_xml::events::attributes::AttrError;
use quick_xml::Error as QuickXmlError;
use rusqlite;
use serde::de::Error;
use std::io;
use std::path::PathBuf;
use std::str::Utf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum XmlParseError {
    #[error("XML data not found")]
    DataNotFound,

    #[error("Root tag not found")]
    RootTagNotFound,

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("XML attribute error: {0}")]
    AttrError(#[from] AttrError),

    #[error("Invalid XML structure")]
    InvalidStructure,

    #[error("XML parsing error: {0}")]
    QuickXmlError(#[from] QuickXmlError),

    #[error("ElementTree error: {0}")]
    ElementTreeError(#[from] ElementTreeError),

    #[error("Requested event '{0}' not found")]
    EventNotFound(String),

    #[error("Required attribute not found: {0}")]
    MissingRequiredAttribute(String),

    #[error("Unknown plugin format: {0}")]
    UnknownPluginFormat(String),
}

#[derive(Error, Debug)]
pub enum FileError {
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("File name error: {0}")]
    NameError(String),

    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("Path is not a file: {0}")]
    NotAFile(PathBuf),

    #[error("Invalid file extension: {0}")]
    InvalidExtension(PathBuf),

    #[error("Invalid Ableton Live Set file: {0}")]
    InvalidLiveSetFile(PathBuf),

    #[error("XML error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("File metadata error for {path:?}")]
    MetadataError {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("File hashing error for {path:?}")]
    HashingError {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("Gzip decompression error for {path:?}")]
    GzipDecompressionError {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Error, Debug)]
pub enum VersionError {
    #[error("Failed to parse version: {0}")]
    ParseError(#[from] std::num::ParseIntError),

    #[error("Missing version information")]
    MissingInfo,

    #[error("Invalid version format")]
    InvalidFormat,

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("XML parsing error: {0}")]
    XmlParseError(#[from] XmlParseError),

    #[error("Invalid file structure: {0}")]
    InvalidFileStructure(String),

    #[error("Missing required attribute: {0}")]
    MissingRequiredAttribute(String),

    #[error("XML attribute error: {0}")]
    AttrError(#[from] AttrError),
}

#[derive(Error, Debug)]
pub enum AttributeError {
    #[error("'Value' attribute not found")]
    ValueNotFound(String),

    #[error("Attribute not found: {0}")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum SampleError {
    #[error("Failed to decode hex string: {0}")]
    HexDecodeError(#[from] hex::FromHexError),

    #[error("Invalid UTF-16 encoding")]
    InvalidUtf16Encoding,

    #[error("Failed to process path: {0}")]
    PathProcessingError(String),

    #[error("Sample file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to read sample file: {0}")]
    FileReadError(#[from] io::Error),

    #[error("XML parsing error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("Attribute error: {0}")]
    AttributeError(#[from] AttributeError),

    #[error("Mac OS format detection failed: {0}")]
    MacFormatDetectionError(String),

    #[error("Mac OS alias decoding failed: {0}")]
    MacAliasDecodeError(String),

    #[error("Mac OS bookmark decoding failed: {0}")]
    MacBookmarkDecodeError(String),

    #[error("No path found in Mac OS format data")]
    NoPathFound,
}

#[derive(Error, Debug)]
pub enum TimeSignatureError {
    #[error("Failed to parse encoded time signature: {0}")]
    ParseEncodedError(#[from] std::num::ParseIntError),
    #[error("Retrieved time signature value ({0}) is outside of valid range (0-494)")]
    InvalidEncodedValue(i32),
    #[error("Time signature enum event not found")]
    EnumEventNotFound,
    #[error("Value attribute not found in time signature event")]
    ValueAttributeNotFound,
}

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("XML parsing error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("Attribute error: {0}")]
    AttributeError(#[from] AttributeError),

    #[error("Unexpected plugin type: {0}")]
    UnexpectedPluginType(String),

    #[error("Failed to access Ableton database file: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Database not found at path: {0}")]
    DatabaseNotFound(PathBuf),

    #[error("Failed to open database connection: {0}")]
    ConnectionError(String),

    #[error("Query execution failed: {0}")]
    QueryError(String),

    #[error("Failed to parse database result: {0}")]
    ParseError(String),

    #[error("Invalid database schema: {0}")]
    InvalidSchema(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("File system error: {0}")]
    FileError(#[from] FileError),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    ParseError(toml::de::Error),
    HomeDirError,
    InvalidPath(String),
    InvalidValue(String),
}

impl std::error::Error for ConfigError {}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error in config: {}", e),
            ConfigError::ParseError(e) => write!(f, "Failed to parse config file: {}", e),
            ConfigError::HomeDirError => write!(f, "Failed to get home directory"),
            ConfigError::InvalidPath(s) => write!(f, "Invalid path in config: {}", s),
            ConfigError::InvalidValue(s) => write!(f, "Invalid configuration value: {}", s),
        }
    }
}

impl Clone for ConfigError {
    fn clone(&self) -> Self {
        match self {
            ConfigError::IoError(e) => {
                ConfigError::IoError(io::Error::new(e.kind(), e.to_string()))
            }
            ConfigError::ParseError(e) => {
                ConfigError::ParseError(toml::de::Error::custom(e.to_string()))
            }
            ConfigError::HomeDirError => ConfigError::HomeDirError,
            ConfigError::InvalidPath(s) => ConfigError::InvalidPath(s.clone()),
            ConfigError::InvalidValue(s) => ConfigError::InvalidValue(s.clone()),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        ConfigError::IoError(error)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> Self {
        ConfigError::ParseError(error)
    }
}

#[derive(Error, Debug)]
pub enum TempoError {
    #[error("XML parsing error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("Tempo not found")]
    TempoNotFound,

    #[error("Invalid tempo value")]
    InvalidTempoValue,
}

#[derive(Error, Debug)]
pub enum PatternError {
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
    
    #[error("Pattern matching failed: {0}")]
    MatchError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LiveSetError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("XML error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("Invalid version format: {0}")]
    InvalidVersion(String),

    #[error("Unsupported Ableton Live version: {0}")]
    UnsupportedVersion(u32),

    #[error("Missing version in Live set file")]
    MissingVersion,

    #[error("Tempo error: {0}")]
    TempoError(#[from] TempoError),

    #[error("Time signature error: {0}")]
    TimeSignatureError(#[from] TimeSignatureError),

    #[error("Sample error: {0}")]
    SampleError(#[from] SampleError),

    #[error("File error: {0}")]
    FileError(#[from] FileError),

    #[error("Version error: {0}")]
    VersionError(#[from] VersionError),

    #[error("Attribute error: {0}")]
    AttributeError(#[from] AttributeError),

    #[error("Attribute error: {0}")]
    AttrError(#[from] AttrError),

    #[error("Plugin error: {0}")]
    PluginError(#[from] PluginError),

    #[error("Pattern error: {0}")]
    PatternError(#[from] PatternError),

    #[error("Failed to create LiveSet: {0}")]
    CreateLiveSetError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("Invalid project: {0}")]
    InvalidProject(String),
}

impl From<quick_xml::Error> for LiveSetError {
    fn from(err: quick_xml::Error) -> Self {
        LiveSetError::XmlError(XmlParseError::QuickXmlError(err))
    }
}
