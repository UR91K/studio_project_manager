//errors.rs
use thiserror::Error;
use quick_xml::Error as XmlError;
use std::str::Utf8Error;
use quick_xml::events::attributes::AttrError;

#[derive(Error, Debug)]
pub enum TimeSignatureError {
    #[error("Failed to parse encoded time signature: {0}")]
    ParseEncodedError(#[from] std::num::ParseIntError),
    #[error("Retrieved time signature value ({0}) is outside of valid range (0-494)")]
    InvalidEncodedValue(i32),
}

#[derive(Error, Debug)]
pub enum LiveSetError {
    #[error("XML data not found")]
    XmlDataNotFound,

    #[error("Time signature enum event not found")]
    EnumEventNotFound,

    #[error("'Value' attribute not found")]
    ValueAttributeNotFound,

    #[error("Failed to update time signature: {0}")]
    TimeSignatureError(#[from] TimeSignatureError),

    #[error("Failed to find plugins: {0}")]
    FindPluginsError(String),

    #[error("Failed to load raw XML data: {0}")]
    LoadRawXmlDataError(String),

    #[error("Failed to create LiveSet: {0}")]
    CreateLiveSetError(String),

    #[error("Failed to parse XML: {0}")]
    XmlParseError(#[from] XmlError),

    #[error("Root tag not found")]
    RootTagNotFound,

    #[error("Failed to parse version: {0}")]
    ParseVersionError(#[from] std::num::ParseIntError),

    #[error("Missing version information")]
    MissingVersionInfo,

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("Invalid file format: {0}")]
    InvalidFileFormat(String),

    #[error("XML attribute error: {0}")]
    XmlAttrError(#[from] AttrError),

    #[error("Invalid version format")]
    InvalidVersionFormat,
}

#[derive(Error, Debug)]
pub enum SamplePathError {
    #[error("Failed to decode hexadecimal string: {0}")]
    HexDecodeError(#[from] hex::FromHexError),

    #[error("Failed to convert path from bytes to UTF-16 string")]
    Utf16ConversionError,
}

#[derive(Error, Debug)]
pub enum DecodeSamplePathError {
    #[error("Failed to decode hex string: {0}")]
    HexDecodeError(#[from] hex::FromHexError),

    #[error("Invalid UTF-16 encoding")]
    InvalidUtf16Encoding,

    #[error("Failed to process path: {0}")]
    PathProcessingError(String),
}
//end of errors.rs