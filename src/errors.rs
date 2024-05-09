use thiserror::Error;

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
}