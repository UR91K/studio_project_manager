use std::path::PathBuf;

#[allow(unused_imports)]
use log::{debug, error, trace, warn};

#[allow(unused_imports)]
use crate::error::{AttributeError, SampleError, XmlParseError};
use crate::utils::macos_formats::{detect_mac_format, decode_mac_path, MacFormat};

/// Check if the byte data looks like UTF-16LE encoded path data
/// UTF-16LE paths have null bytes every other position (e.g., 45003A005C00...)
pub fn looks_like_utf16le_path(data: &[u8]) -> bool {
    if data.len() < 16 {
        return false; // Too short to be a meaningful path
    }
    
    // Check the first 16 bytes for the UTF-16LE pattern
    // Every odd-indexed byte should be 0x00
    for i in (1..16).step_by(2) {
        if data[i] != 0x00 {
            return false;
        }
    }
    
    // Also check that we have some non-null bytes in even positions
    let mut has_non_null = false;
    for i in (0..16).step_by(2) {
        if data[i] != 0x00 {
            has_non_null = true;
            break;
        }
    }
    
    has_non_null
}

/// Decode byte data as UTF-16 with specified endianness and convert to PathBuf
fn decode_utf16_path(byte_data: &[u8], is_little_endian: bool) -> Result<PathBuf, SampleError> {
    let (cow, _, had_errors) = if is_little_endian {
        encoding_rs::UTF_16LE.decode(byte_data)
    } else {
        encoding_rs::UTF_16BE.decode(byte_data)
    };

    if had_errors {
        warn!("Errors encountered during UTF-16 decoding");
    }

    let path_string = cow.replace('\0', "");
    let path = PathBuf::from(path_string);
    trace!("Decoded UTF-16 path: {:?}", path);

    match path.canonicalize() {
        Ok(canonical_path) => {
            trace!("Canonicalized path: {:?}", canonical_path);
            Ok(canonical_path)
        }
        Err(e) => {
            trace!(
                "Failed to canonicalize path: {}. Using non-canonicalized path.",
                e
            );
            Ok(path)
        }
    }
}

/// Decode byte data as UTF-16LE and convert to PathBuf (for backward compatibility)
fn decode_utf16le_path(byte_data: &[u8]) -> Result<PathBuf, SampleError> {
    decode_utf16_path(byte_data, true)
}

/// Decode POSIX path bytes, automatically detecting UTF-8 vs UTF-16 encoding
pub fn decode_posix_path_bytes(bytes: &[u8]) -> Result<String, SampleError> {
    // Check if this looks like UTF-16LE (every odd byte is 0)
    if bytes.len() >= 2 && bytes.len() % 2 == 0 && bytes[1..].iter().step_by(2).all(|&b| b == 0) {
        // Looks like UTF-16LE
        let (cow, _, had_errors) = encoding_rs::UTF_16LE.decode(bytes);
        if had_errors {
            warn!("Errors encountered during UTF-16LE decoding of POSIX path");
        }
        return Ok(cow.to_string());
    }
    
    // Check if this looks like UTF-16BE (every even byte is 0)
    if bytes.len() >= 2 && bytes.len() % 2 == 0 && bytes[..bytes.len()-1].iter().step_by(2).all(|&b| b == 0) {
        // Looks like UTF-16BE
        let (cow, _, had_errors) = encoding_rs::UTF_16BE.decode(bytes);
        if had_errors {
            warn!("Errors encountered during UTF-16BE decoding of POSIX path");
        }
        return Ok(cow.to_string());
    }
    
    // Try UTF-8
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => Ok(s),
        Err(e) => {
            warn!("Failed to decode POSIX path as UTF-8: {:?}", e);
            Err(SampleError::PathProcessingError("Failed to decode POSIX path encoding".to_string()))
        }
    }
}

pub fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, SampleError> {
    trace!("Starting sample path decoding");

    let cleaned_path = abs_hash_path
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    trace!("Cleaned absolute hash path: {:?}", cleaned_path);

    let byte_data = hex::decode(&cleaned_path).map_err(|e| {
        warn!("Failed to decode hex string: {:?}", e);
        SampleError::HexDecodeError(e)
    })?;
    trace!("Decoded {} bytes", byte_data.len());

    // Check if this looks like UTF-16LE encoded path data (every other byte is null)
    if looks_like_utf16le_path(&byte_data) {
        trace!("Detected UTF-16LE path pattern, using direct decoding");
        return decode_utf16le_path(&byte_data);
    }

    // First, try to detect if this is a Mac OS format (Alias or Bookmark)
    match detect_mac_format(&byte_data) {
        Ok(MacFormat::Alias(_)) => {
            trace!("Detected Mac OS Alias format");
            decode_mac_path(&byte_data).map_err(|e| {
                warn!("Failed to decode Mac OS alias: {:?}", e);
                SampleError::MacAliasDecodeError(e)
            })
        }
        Ok(MacFormat::Bookmark(_)) => {
            // TODO: Implement Mac OS Bookmark decoding if needed.
            trace!("Detected Mac OS Bookmark format - not yet implemented");
            Err(SampleError::PathProcessingError("Mac OS Bookmark decoding not yet implemented".to_string()))
        }
        Ok(MacFormat::Unknown) => {
            trace!("Data does not match UTF-16LE pattern or Mac OS formats");
            Err(SampleError::PathProcessingError("Data does not match any known path format".to_string()))
        }
        Err(e) => {
            warn!("Mac OS format detection failed: {:?}", e);
            Err(SampleError::MacFormatDetectionError(e))
        }
    }
}
