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

impl Clone for XmlParseError {
    fn clone(&self) -> Self {
        match self {
            Self::DataNotFound => Self::DataNotFound,
            Self::RootTagNotFound => Self::RootTagNotFound,
            Self::Utf8Error(e) => Self::Utf8Error(*e),
            Self::AttrError(e) => Self::AttrError(e.clone()),
            Self::InvalidStructure => Self::InvalidStructure,
            Self::QuickXmlError(e) => Self::QuickXmlError(e.clone()),
            Self::ElementTreeError(_) => Self::InvalidStructure, // Convert to a similar error since ElementTreeError doesn't implement Clone
            Self::EventNotFound(s) => Self::EventNotFound(s.clone()),
            Self::MissingRequiredAttribute(s) => Self::MissingRequiredAttribute(s.clone()),
            Self::UnknownPluginFormat(s) => Self::UnknownPluginFormat(s.clone()),
        }
    }
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

impl Clone for FileError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidFormat(s) => Self::InvalidFormat(s.clone()),
            Self::NameError(s) => Self::NameError(s.clone()),
            Self::NotFound(p) => Self::NotFound(p.clone()),
            Self::NotAFile(p) => Self::NotAFile(p.clone()),
            Self::InvalidExtension(p) => Self::InvalidExtension(p.clone()),
            Self::InvalidLiveSetFile(p) => Self::InvalidLiveSetFile(p.clone()),
            Self::XmlError(e) => Self::XmlError(e.clone()),
            Self::MetadataError { path, source } => Self::MetadataError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            Self::HashingError { path, source } => Self::HashingError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
            Self::GzipDecompressionError { path, source } => Self::GzipDecompressionError {
                path: path.clone(),
                source: io::Error::new(source.kind(), source.to_string()),
            },
        }
    }
}

#[derive(Error, Debug, Clone)]
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

#[derive(Error, Debug, Clone)]
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
}

impl Clone for SampleError {
    fn clone(&self) -> Self {
        match self {
            Self::HexDecodeError(e) => Self::HexDecodeError(e.clone()),
            Self::InvalidUtf16Encoding => Self::InvalidUtf16Encoding,
            Self::PathProcessingError(s) => Self::PathProcessingError(s.clone()),
            Self::FileNotFound(p) => Self::FileNotFound(p.clone()),
            Self::FileReadError(e) => Self::PathProcessingError(e.to_string()),
            Self::XmlError(e) => Self::XmlError(e.clone()),
            Self::AttributeError(e) => Self::AttributeError(e.clone()),
        }
    }
}

#[derive(Error, Debug, Clone)]
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

#[derive(Error, Debug, Clone)]
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

    #[error("File system error: {0}")]
    FileError(#[from] FileError),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

impl Clone for DatabaseError {
    fn clone(&self) -> Self {
        match self {
            Self::SqliteError(_) => Self::QueryError("SQLite error occurred".to_string()),
            Self::DatabaseNotFound(p) => Self::DatabaseNotFound(p.clone()),
            Self::ConnectionError(s) => Self::ConnectionError(s.clone()),
            Self::QueryError(s) => Self::QueryError(s.clone()),
            Self::ParseError(s) => Self::ParseError(s.clone()),
            Self::InvalidSchema(s) => Self::InvalidSchema(s.clone()),
            Self::FileError(e) => Self::FileError(e.clone()),
            Self::ConfigError(e) => Self::ConfigError(e.clone()),
            Self::InvalidOperation(s) => Self::InvalidOperation(s.clone()),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    ReadError(io::Error),
    ParseError(toml::de::Error),
    HomeDirError,
    InvalidPath(String),
}

impl std::error::Error for ConfigError {}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ReadError(e) => write!(f, "Failed to read config file: {}", e),
            ConfigError::ParseError(e) => write!(f, "Failed to parse config file: {}", e),
            ConfigError::HomeDirError => write!(f, "Failed to get home directory"),
            ConfigError::InvalidPath(s) => write!(f, "Invalid path in config: {}", s),
        }
    }
}

impl Clone for ConfigError {
    fn clone(&self) -> Self {
        match self {
            ConfigError::ReadError(e) => {
                ConfigError::ReadError(io::Error::new(e.kind(), e.to_string()))
            }
            ConfigError::ParseError(e) => {
                ConfigError::ParseError(toml::de::Error::custom(e.to_string()))
            }
            ConfigError::HomeDirError => ConfigError::HomeDirError,
            ConfigError::InvalidPath(s) => ConfigError::InvalidPath(s.clone()),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        ConfigError::ReadError(error)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> Self {
        ConfigError::ParseError(error)
    }
}

#[derive(Error, Debug, Clone)]
pub enum TempoError {
    #[error("XML parsing error: {0}")]
    XmlError(#[from] XmlParseError),

    #[error("Tempo not found")]
    TempoNotFound,

    #[error("Invalid tempo value")]
    InvalidTempoValue,
}

#[derive(Error, Debug, Clone)]
pub enum PatternError {
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
    
    #[error("Pattern matching failed: {0}")]
    MatchError(String),
}

#[derive(Error, Debug)]
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

impl Clone for LiveSetError {
    fn clone(&self) -> Self {
        match self {
            Self::IoError(e) => Self::CreateLiveSetError(format!("IO error: {}", e)),
            Self::XmlError(e) => Self::XmlError(e.clone()),
            Self::InvalidVersion(s) => Self::InvalidVersion(s.clone()),
            Self::UnsupportedVersion(v) => Self::UnsupportedVersion(*v),
            Self::MissingVersion => Self::MissingVersion,
            Self::TempoError(e) => Self::TempoError(e.clone()),
            Self::TimeSignatureError(e) => Self::TimeSignatureError(e.clone()),
            Self::SampleError(e) => Self::SampleError(e.clone()),
            Self::FileError(e) => Self::FileError(e.clone()),
            Self::VersionError(e) => Self::VersionError(e.clone()),
            Self::AttributeError(e) => Self::AttributeError(e.clone()),
            Self::AttrError(e) => Self::AttrError(e.clone()),
            Self::PluginError(e) => Self::PluginError(e.clone()),
            Self::PatternError(e) => Self::PatternError(e.clone()),
            Self::CreateLiveSetError(s) => Self::CreateLiveSetError(s.clone()),
            Self::DatabaseError(e) => Self::DatabaseError(e.clone()),
            Self::ConfigError(e) => Self::ConfigError(e.clone()),
            Self::InvalidProject(s) => Self::InvalidProject(s.clone()),
        }
    }
}

impl From<quick_xml::Error> for LiveSetError {
    fn from(err: quick_xml::Error) -> Self {
        LiveSetError::XmlError(XmlParseError::QuickXmlError(err))
    }
}
