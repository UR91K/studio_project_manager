use chrono::Duration;
use flate2::read::GzDecoder;
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::str::from_utf8;
use std::sync::Mutex;

use log::{error, trace};
use once_cell::sync::Lazy;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;

use crate::error::{FileError, XmlParseError};

pub mod metadata;
pub mod plugins;
pub mod samples;
pub mod time_signature;
pub mod macos_formats;

#[macro_export]
macro_rules! trace_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        {
            use colored::Colorize;
            log::trace!("[{}] {}", $fn_name.to_string().bright_blue().bold(), format!($($arg)+))
        }
    };
}

#[macro_export]
macro_rules! debug_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        {
            use colored::Colorize;
            log::debug!("[{}] {}", $fn_name.to_string().cyan().bold(), format!($($arg)+))
        }
    };
}

#[macro_export]
macro_rules! info_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        {
            use colored::Colorize;
            log::info!("[{}] {}", $fn_name.to_string().green().bold(), format!($($arg)+))
        }
    };
}

#[macro_export]
macro_rules! warn_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        {
            use colored::Colorize;
            log::warn!("[{}] {}", $fn_name.to_string().yellow().bold(), format!($($arg)+))
        }
    };
}

#[macro_export]
macro_rules! error_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        {
            use colored::Colorize;
            log::error!("[{}] {}", $fn_name.to_string().red().bold(), format!($($arg)+))
        }
    };
}

#[macro_export]
macro_rules! trace_with_line {
    ($fn_name:expr, $line:expr, $($arg:tt)+) => {
        log::trace!("[{}] At line {} in xml data: {}", $fn_name.bright_blue().bold(), $line, format!($($arg)+))
    };
}

pub trait StringResultExt {
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

pub trait EventExt {
    fn get_value_as_string_result(&self) -> Result<Option<String>, XmlParseError>;
}

impl<'a> EventExt for Event<'a> {
    fn get_value_as_string_result(&self) -> Result<Option<String>, XmlParseError> {
        match self {
            Event::Empty(e) | Event::Start(e) => e.get_value_as_string_result(),
            _ => Ok(None),
        }
    }
}

impl<'a> EventExt for BytesStart<'a> {
    fn get_value_as_string_result(&self) -> Result<Option<String>, XmlParseError> {
        for attribute_result in self.attributes() {
            let attribute = attribute_result.map_err(XmlParseError::AttrError)?;
            if attribute.key == QName(b"Value") {
                return Ok(Some(attribute.value.to_string_result()?));
            }
        }
        Ok(None)
    }
}

pub fn validate_ableton_file(file_path: &Path) -> Result<(), FileError> {
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
/// use studio_project_manager::utils::format_file_size;
///
/// assert_eq!(format_file_size(1023), "1023 B");
/// assert_eq!(format_file_size(1024), "1.00 KB");
/// assert_eq!(format_file_size(1_048_576), "1.00 MB");
/// assert_eq!(format_file_size(1_073_741_824), "1.00 GB");
/// ```
#[allow(dead_code)]
pub fn format_file_size(size: u64) -> String {
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
/// ```no_run
/// use std::path::Path;
/// use studio_project_manager::decompress_gzip_file;
///
/// let file_path = Path::new("path/to/compressed/file.gz");
/// let decompressed_data = decompress_gzip_file(&file_path).expect("Failed to decompress file");
/// println!("Decompressed {} bytes", decompressed_data.len());
/// ```
pub fn decompress_gzip_file(file_path: &Path) -> Result<Vec<u8>, FileError> {
    trace!("Attempting to extract gzipped data from: {:?}", file_path);
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

    trace!("File opened successfully, creating GzDecoder");
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
    trace!(
        "Successfully decompressed {} bytes from: {:?}",
        decompressed_size, file_path
    );
    trace!("Decompressed data size: {} bytes", decompressed_size);

    Ok(decompressed_data)
}

static LINE_CACHE: Lazy<Mutex<Vec<(usize, usize)>>> = Lazy::new(|| Mutex::new(Vec::new()));

//TODO possibly delete this if we find it is no longer needed
#[allow(dead_code)]
pub fn get_line_number(file_path: &Path, byte_position: usize) -> std::io::Result<usize> {
    let mut cache = LINE_CACHE.lock().unwrap();

    if cache.is_empty() {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut line_number = 1;
        let mut current_position = 0;

        for line in reader.lines() {
            let line_length = line?.len() + 1; // +1 for the newline character
            current_position += line_length;
            cache.push((current_position, line_number));
            line_number += 1;
        }
    }

    match cache.binary_search_by(|&(pos, _)| pos.cmp(&byte_position)) {
        Ok(index) => Ok(cache[index].1),
        Err(index) => {
            if index == 0 {
                Ok(1)
            } else {
                Ok(cache[index - 1].1 + 1)
            }
        }
    }
}

#[allow(dead_code)]
pub fn format_duration(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds();
    let milliseconds = duration.num_milliseconds() % 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}.{:03}s", hours, minutes, seconds, milliseconds)
    } else {
        format!("{}m {}.{:03}s", minutes, seconds, milliseconds)
    }
}
