use std::fmt;

#[derive(Debug)]
pub enum MediaError {
    IoError(String),
    InvalidMediaType(String),
    FileTooLarge {
        actual_size_mb: f64,
        max_size_mb: f64,
    },
    UnsupportedFormat {
        format: String,
        allowed_formats: Vec<String>,
    },
    FileNotFound(String),
    ChecksumMismatch {
        expected: String,
        actual: String,
    },
    InvalidFileId(String),
    DatabaseError(String),
    ConfigurationError(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::IoError(msg) => write!(f, "IO error: {}", msg),
            MediaError::InvalidMediaType(media_type) => {
                write!(f, "Invalid media type: {}", media_type)
            }
            MediaError::FileTooLarge {
                actual_size_mb,
                max_size_mb,
            } => {
                write!(
                    f,
                    "File too large: {:.2}MB exceeds maximum of {:.2}MB",
                    actual_size_mb, max_size_mb
                )
            }
            MediaError::UnsupportedFormat {
                format,
                allowed_formats,
            } => {
                write!(
                    f,
                    "Unsupported format '{}'. Allowed formats: {}",
                    format,
                    allowed_formats.join(", ")
                )
            }
            MediaError::FileNotFound(file_id) => write!(f, "File not found: {}", file_id),
            MediaError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "Checksum mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            MediaError::InvalidFileId(file_id) => write!(f, "Invalid file ID: {}", file_id),
            MediaError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            MediaError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for MediaError {}

impl From<std::io::Error> for MediaError {
    fn from(error: std::io::Error) -> Self {
        MediaError::IoError(error.to_string())
    }
}

impl From<crate::error::DatabaseError> for MediaError {
    fn from(error: crate::error::DatabaseError) -> Self {
        MediaError::DatabaseError(error.to_string())
    }
}

impl From<crate::error::ConfigError> for MediaError {
    fn from(error: crate::error::ConfigError) -> Self {
        MediaError::ConfigurationError(error.to_string())
    }
}
