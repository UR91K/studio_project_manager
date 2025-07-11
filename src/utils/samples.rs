use std::path::PathBuf;

#[allow(unused_imports)]
use log::{debug, error, trace, warn};

#[allow(unused_imports)]
use crate::error::{AttributeError, SampleError, XmlParseError};

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

    // Try UTF-16LE first (Windows format)
    let (cow, _, had_errors_le) = encoding_rs::UTF_16LE.decode(&byte_data);
    if !had_errors_le {
        let path_string = cow.replace('\0', "");
        let path = PathBuf::from(path_string);
        trace!("Decoded path (UTF-16LE): {:?}", path);
        return Ok(path);
    }

    // Try UTF-16BE (macOS format)
    let (cow, _, had_errors_be) = encoding_rs::UTF_16BE.decode(&byte_data);
    trace!("UTF-16BE decoding had errors: {}", had_errors_be);
    if !had_errors_be {
        let path_string = cow.replace('\0', "");
        trace!("UTF-16BE decoded string: {:?}", path_string);
        let path = PathBuf::from(path_string);
        trace!("Decoded path (UTF-16BE): {:?}", path);
        return Ok(path);
    }

    warn!("Errors encountered during UTF-16 decoding - data may be corrupted");
    // Try to find a valid UTF-16 sequence
    let utf16_chunks: Vec<u16> = byte_data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    
    // Look for the first valid UTF-16 sequence
    for i in 0..utf16_chunks.len() {
        for j in (i + 1)..=utf16_chunks.len() {
            if let Ok(path_string) = String::from_utf16(&utf16_chunks[i..j]) {
                // Skip empty strings, null characters, and very short strings
                if path_string.len() > 3 && 
                   !path_string.chars().all(|c| c == '\0') &&
                   path_string.chars().any(|c| c.is_alphanumeric()) &&
                   path_string.chars().all(|c| c.is_ascii() || c.is_alphanumeric() || c.is_whitespace() || "\\/:.-_()[]{}".contains(c)) {
                    trace!("Found valid path substring at indices {}..{}: {:?}", i, j, path_string);
                    let path = PathBuf::from(path_string);
                    return Ok(path);
                }
            }
        }
    }
    
    return Err(SampleError::InvalidUtf16Encoding);
}
