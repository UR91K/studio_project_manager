use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::from_utf8;

use flate2::read::GzDecoder;
use log::{debug, error, info, trace};
use quick_xml::name::QName;

use crate::error::{FileError, XmlParseError};

pub mod metadata;
pub mod plugins;
pub mod samples;
pub mod tempo;
pub mod time_signature;
pub mod version;
pub mod xml_parsing;

#[macro_export]
macro_rules! trace_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::trace!("[{}] {}", $fn_name.bright_blue().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! debug_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::debug!("[{}] {}", $fn_name.cyan().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! info_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::info!("[{}] {}", $fn_name.green().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! warn_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::warn!("[{}] {}", $fn_name.yellow().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! error_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::error!("[{}] {}", $fn_name.red().bold(), format!($($arg)+))
    };
}

pub(crate) trait StringResultExt {
    fn to_string_result(&self) -> Result<String, XmlParseError>;
    fn to_str_result(&self) -> Result<&str, XmlParseError>;
}

impl<'a> StringResultExt for QName<'a> {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        self.to_str_result().map(String::from)
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        from_utf8(self.as_ref()).map_err(XmlParseError::Utf8Error)
    }
}

impl StringResultExt for &[u8] {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        from_utf8(self)
            .map(String::from)
            .map_err(XmlParseError::Utf8Error)
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        from_utf8(self).map_err(XmlParseError::Utf8Error)
    }
}

impl<'a> StringResultExt for Cow<'a, [u8]> {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        String::from_utf8(self.to_vec()).map_err(|e| XmlParseError::Utf8Error(e.utf8_error()))
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        match self {
            Cow::Borrowed(bytes) => from_utf8(bytes).map_err(XmlParseError::Utf8Error),
            Cow::Owned(vec) => from_utf8(vec).map_err(XmlParseError::Utf8Error),
        }
    }
}

pub(crate) fn validate_ableton_file(file_path: &Path) -> Result<(), FileError> {
    if !file_path.exists() {
        return Err(FileError::NotFound(file_path.to_path_buf()));
    }

    if !file_path.is_file() {
        return Err(FileError::NotAFile(file_path.to_path_buf()));
    }

    if file_path.extension().unwrap_or_default() != "als" {
        return Err(FileError::InvalidExtension(file_path.to_path_buf()));
    }

    Ok(())
}

/// Formats a file size in bytes to a human-readable string (B, KB, MB, or GB).
///
/// # Examples
///
/// ```
/// use studio_project_manager::helpers::format_file_size;
///
/// assert_eq!(format_file_size(1023), "1023 B");
/// assert_eq!(format_file_size(1024), "1.00 KB");
/// assert_eq!(format_file_size(1_048_576), "1.00 MB");
/// assert_eq!(format_file_size(1_073_741_824), "1.00 GB");
/// ```
pub(crate) fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    let formatted = if size < KB {
        format!("{} B", size)
    } else if size < MB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else if size < GB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else {
        format!("{:.2} GB", size as f64 / GB as f64)
    };

    formatted
}

/// Decompresses a gzip file and returns its contents as a byte vector.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use studio_project_manager::helpers::decompress_gzip_file;
///
/// let file_path = Path::new("path/to/compressed/file.gz");
/// let decompressed_data = decompress_gzip_file(&file_path).expect("Failed to decompress file");
/// println!("Decompressed {} bytes", decompressed_data.len());
/// ```
pub(crate) fn decompress_gzip_file(file_path: &Path) -> Result<Vec<u8>, FileError> {
    info!("Attempting to extract gzipped data from: {:?}", file_path);
    trace!("Opening file for gzip decompression");

    let file = File::open(file_path).map_err(|error| {
        error!(
            "Failed to open file for gzip decompression: {:?}",
            file_path
        );
        FileError::GzipDecompressionError {
            path: file_path.to_path_buf(),
            source: error,
        }
    })?;

    debug!("File opened successfully, creating GzDecoder");
    let mut gzip_decoder = GzDecoder::new(file);
    let mut decompressed_data = Vec::new();

    trace!("Beginning decompression of gzipped data");
    gzip_decoder
        .read_to_end(&mut decompressed_data)
        .map_err(|error| {
            error!("Failed to decompress gzipped data from: {:?}", file_path);
            FileError::GzipDecompressionError {
                path: file_path.to_path_buf(),
                source: error,
            }
        })?;

    let decompressed_size = decompressed_data.len();
    info!(
        "Successfully decompressed {} bytes from: {:?}",
        decompressed_size, file_path
    );
    debug!("Decompressed data size: {} bytes", decompressed_size);

    Ok(decompressed_data)
}