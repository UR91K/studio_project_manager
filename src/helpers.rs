use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Local};
use crc32fast::Hasher;
use elementtree::Element;
use flate2::read::GzDecoder;
use log::{info, debug, error, trace, warn};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use encoding_rs::UTF_16LE;

use crate::custom_types::{AbletonVersion, XmlTag};
use crate::errors::{DecodeSamplePathError, LiveSetError, TimeSignatureError, XmlParseError, AttributeError};

/// Extracts and decompresses gzipped data from a file.
///
/// This function opens a gzipped file, decompresses its contents, and returns the raw data.
///
/// # Arguments
///
/// * `file_path` - A reference to a `Path` that points to the gzipped file to be extracted.
///
/// # Returns
///
/// * `Result<Vec<u8>, LiveSetError>` - A `Result` containing either:
///   - `Ok(Vec<u8>)`: A vector of bytes containing the decompressed data.
///   - `Err(LiveSetError)`: An error if file opening or decompression fails.
///
/// # Errors
///
/// This function will return a `LiveSetError::GzipDecompressionError` if:
/// * The file cannot be opened (e.g., due to permissions or if it doesn't exist).
/// * The file's contents cannot be decompressed (e.g., if it's not a valid gzip file).
///
/// # Example
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("path/to/gzipped/file.gz");
/// match extract_gzipped_data(path) {
///     Ok(data) => println!("Decompressed {} bytes", data.len()),
///     Err(error) => eprintln!("Failed to extract data: {}", error),
/// }
/// ```
pub fn extract_gzipped_data(file_path: &Path) -> Result<Vec<u8>, LiveSetError> {
    info!("Attempting to extract gzipped data from: {:?}", file_path);
    trace!("Opening file for gzip decompression");

    let file = File::open(file_path).map_err(|error| {
        error!("Failed to open file for gzip decompression: {:?}", file_path);
        LiveSetError::GzipDecompressionError {
            path: file_path.to_path_buf(),
            source: error,
        }
    })?;

    debug!("File opened successfully, creating GzDecoder");
    let mut gzip_decoder = GzDecoder::new(file);
    let mut decompressed_data = Vec::new();

    trace!("Beginning decompression of gzipped data");
    gzip_decoder.read_to_end(&mut decompressed_data).map_err(|e| {
        error!("Failed to decompress gzipped data from: {:?}", file_path);
        LiveSetError::GzipDecompressionError {
            path: file_path.to_path_buf(),
            source: e,
        }
    })?;

    let decompressed_size = decompressed_data.len();
    info!("Successfully decompressed {} bytes from: {:?}", decompressed_size, file_path);
    debug!("Decompressed data size: {} bytes", decompressed_size);

    Ok(decompressed_data)
}

/// Parses an encoded string value into an i32 integer.
///
/// This function is specifically designed to parse encoded values that represent
/// time signatures in Ableton Live project files.
///
/// # Arguments
///
/// * `value` - A string slice that contains the encoded value to be parsed.
///
/// # Returns
///
/// * `Result<i32, LiveSetError>` - A `Result` containing either:
///   - `Ok(i32)`: The successfully parsed integer value.
///   - `Err(LiveSetError)`: An error if parsing fails.
///
/// # Errors
///
/// This function will return a `LiveSetError::TimeSignatureError` if:
/// * The input string cannot be parsed as an i32 integer.
///
/// # Example
///
/// ```
/// let encoded_value = "123";
/// match parse_encoded_value(encoded_value) {
///     Ok(value) => println!("Parsed value: {}", value),
///     Err(e) => eprintln!("Failed to parse value: {}", e),
/// }
/// ```
pub fn parse_encoded_value(value: &str) -> Result<i32, LiveSetError> {
    trace!("Attempting to parse encoded value: '{}'", value);

    match i32::from_str(value) {
        Ok(parsed_value) => {
            debug!("Successfully parsed encoded value '{}' to {}", value, parsed_value);
            Ok(parsed_value)
        },
        Err(e) => {
            error!("Failed to parse encoded value '{}': {}", value, e);
            Err(LiveSetError::TimeSignatureError(TimeSignatureError::ParseEncodedError(e)))
        }
    }
}

/// Validates an encoded time signature value.
///
/// This function checks whether the given integer value falls within the valid range
/// for encoded time signatures in Ableton Live project files. The valid range is
/// from 0 to 16777215 (inclusive).
///
/// # Arguments
///
/// * `value` - An i32 integer representing the encoded time signature value.
///
/// # Returns
///
/// * `Result<i32, TimeSignatureError>` - A `Result` containing either:
///   - `Ok(i32)`: The input value, if it's within the valid range.
///   - `Err(TimeSignatureError)`: An error if the value is outside the valid range.
///
/// # Errors
///
/// This function will return a `TimeSignatureError::InvalidEncodedValue` if:
/// * The input value is less than 0 or greater than 16777215.
///
/// # Example
///
/// ```
/// let encoded_value = 12345;
/// match validate_time_signature(encoded_value) {
///     Ok(value) => println!("Valid time signature value: {}", value),
///     Err(e) => eprintln!("Invalid time signature value: {}", e),
/// }
/// ```
pub fn validate_time_signature(value: i32) -> Result<i32, TimeSignatureError> {
    trace!("Validating time signature value: {}", value);

    const MIN_VALUE: i32 = 0;
    const MAX_VALUE: i32 = 16777215; // 2^24 - 1

    if value >= MIN_VALUE && value <= MAX_VALUE {
        debug!("Time signature value {} is valid", value);
        Ok(value)
    } else {
        error!("Invalid time signature value: {}. Valid range is {} to {}", value, MIN_VALUE, MAX_VALUE);
        Err(TimeSignatureError::InvalidEncodedValue(value))
    }
}

/// Formats a file size in bytes into a human-readable string.
///
/// This function takes a file size in bytes and returns a formatted string
/// representing the size in the most appropriate unit (B, KB, MB, or GB).
/// The function uses binary prefixes (1 KB = 1024 B).
///
/// # Arguments
///
/// * `size` - A u64 integer representing the file size in bytes.
///
/// # Returns
///
/// * `String` - A formatted string representing the file size with appropriate units.
///
/// # Examples
///
/// ```
/// assert_eq!(format_file_size(500), "500 B");
/// assert_eq!(format_file_size(1500), "1.46 KB");
/// assert_eq!(format_file_size(1500000), "1.43 MB");
/// assert_eq!(format_file_size(1500000000), "1.40 GB");
/// ```
///
/// # Note
///
/// This function uses binary prefixes (1 KB = 1024 B) rather than
/// decimal prefixes (1 KB = 1000 B). This is common in computing contexts,
/// but may differ from some other size representations.
pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    trace!("Formatting file size: {} bytes", size);

    let formatted = if size < KB {
        format!("{} B", size)
    } else if size < MB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else if size < GB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else {
        format!("{:.2} GB", size as f64 / GB as f64)
    };

    trace!("Formatted file size: {}", formatted);
    formatted
}

/// Parses XML data from a byte slice into an Element tree.
///
/// This function takes raw XML data, converts it to a UTF-8 string,
/// finds the start of the XML content, and parses it into an Element tree.
///
/// # Arguments
///
/// * `xml_data` - A byte slice containing the raw XML data.
/// * `file_name` - An optional reference to the name of the file being parsed.
/// * `file_path` - A reference to the Path of the file being parsed.
///
/// # Returns
///
/// * `Result<Element, LiveSetError>` - A Result containing either:
///   - `Ok(Element)`: The root Element of the parsed XML tree.
///   - `Err(LiveSetError)`: An error if parsing fails.
///
/// # Errors
///
/// This function will return a `LiveSetError` if:
/// * The input data cannot be converted to a UTF-8 string.
/// * No XML start tag is found in the data.
/// * The XML data is not valid or cannot be parsed.
///
/// # Performance
///
/// This function logs the time taken to create the XML Element tree.
pub fn parse_xml_data(xml_data: &[u8], file_name: &Option<String>, file_path: &Path) -> Result<Element, LiveSetError> {
    trace!("Starting XML parsing for file: {:?}", file_name);

    let xml_data_str = std::str::from_utf8(xml_data).map_err(|err| {
        let msg = format!("{:?}: Failed to convert decompressed data to UTF-8 string", file_name);
        error!("{}: {}", msg, err);
        LiveSetError::XmlError(XmlParseError::Utf8Error(err))
    })?;

    let xml_start = xml_data_str.find("<?xml").ok_or_else(|| {
        let msg = format!("{:?}: No XML data found in decompressed file", file_name);
        warn!("{}", msg);
        LiveSetError::XmlError(XmlParseError::InvalidStructure)
    })?;

    let xml_slice = &xml_data_str[xml_start..];
    trace!("XML start found at index: {}", xml_start);

    let start_time_xml = Instant::now();
    let root = Element::from_reader(Cursor::new(xml_slice.as_bytes())).map_err(|err| {
        let msg = format!("{:?}: {} is not a valid XML file", file_name, file_path.display());
        error!("{}: {}", msg, err);
        LiveSetError::XmlError(XmlParseError::ElementTreeError(err))
    })?;

    let duration = start_time_xml.elapsed();

    debug!("XML Element created in {:.2?}", duration);

    trace!("XML parsing completed successfully");
    Ok(root)
}

/// Searches for and extracts specific XML tags and their attributes from the given XML data.
///
/// # Arguments
///
/// * `xml_data` - A byte slice containing the XML data to be parsed.
/// * `search_queries` - A slice of string slices representing the tag names to search for.
/// * `target_depth` - The depth at which to extract child tags relative to the matched search query tags.
///
/// # Returns
///
/// A `Result` containing either:
/// * A `HashMap` where:
///   - Keys are strings representing the matched search query tags.
///   - Values are vectors of vectors of `XmlTag`s. Each inner vector represents a group of tags
///     found at the specified `target_depth` under a single instance of a matched search query tag.
/// * A `LiveSetError` if any parsing or processing error occurs.
///
/// # Errors
///
/// This function will return an error if:
/// * There are issues with XML parsing.
/// * UTF-8 conversion fails for tag names or attribute values.
/// * Attribute parsing fails.
pub fn find_tags(xml_data: &[u8], search_queries: &[&str], target_depth: u8) -> Result<HashMap<String, Vec<Vec<XmlTag>>>, LiveSetError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut all_tags: HashMap<String, Vec<Vec<XmlTag>>> = HashMap::new();
    let mut current_tags: HashMap<String, Vec<XmlTag>> = HashMap::new();

    let mut in_target_tag = false;
    let mut depth: u8 = 0;
    let mut current_query = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref())
                    .map_err(|e| LiveSetError::XmlError(XmlParseError::Utf8Error(e)))?
                    .to_string();
                if search_queries.contains(&name.as_str()) {
                    in_target_tag = true;
                    depth = 0;
                    current_query = name;
                    current_tags.entry(current_query.clone()).or_default();
                } else if in_target_tag {
                    depth += 1;
                }
            }
            Ok(Event::Empty(ref event)) => {
                if in_target_tag && depth == target_depth {
                    let name = std::str::from_utf8(event.name().as_ref())
                        .map_err(|e| LiveSetError::XmlError(XmlParseError::Utf8Error(e)))?
                        .to_string();
                    let mut attributes = Vec::new();
                    for attr in event.attributes() {
                        let attr = attr.map_err(|_| LiveSetError::XmlError(XmlParseError::AttributeError))?;
                        let key = std::str::from_utf8(attr.key.as_ref())
                            .map_err(|e| LiveSetError::XmlError(XmlParseError::Utf8Error(e)))?
                            .to_string();
                        let value = std::str::from_utf8(&attr.value)
                            .map_err(|e| LiveSetError::XmlError(XmlParseError::Utf8Error(e)))?
                            .to_string();
                        attributes.push((key, value));
                    }
                    current_tags.get_mut(&current_query)
                        .ok_or_else(|| LiveSetError::XmlError(XmlParseError::InvalidStructure))?
                        .push(XmlTag {
                            name,
                            attributes,
                        });
                }
            }
            Ok(Event::End(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref())
                    .map_err(|e| LiveSetError::XmlError(XmlParseError::Utf8Error(e)))?
                    .to_string();
                if name == current_query {
                    in_target_tag = false;
                    all_tags.entry(current_query.clone()).or_default()
                        .push(current_tags[&current_query].clone());
                    current_tags.get_mut(&current_query)
                        .ok_or_else(|| LiveSetError::XmlError(XmlParseError::InvalidStructure))?
                        .clear();
                } else if in_target_tag {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LiveSetError::XmlError(XmlParseError::QuickXmlError(e))),
            _ => (),
        }
        buf.clear();
    }
    Ok(all_tags)
}

/// Searches for a specific attribute within a collection of XML tags.
///
/// This function iterates through a slice of XmlTag structs, looking for a tag
/// with a specific name. If found, it then searches for a specific attribute
/// within that tag.
///
/// # Arguments
///
/// * `tags` - A slice of XmlTag structs to search through.
/// * `tag_query` - The name of the tag to look for.
/// * `attribute_query` - The name of the attribute to find within the matching tag.
///
/// # Returns
///
/// * `Result<String, LiveSetError>` - A Result containing either:
///   - `Ok(String)`: The value of the found attribute.
///   - `Err(LiveSetError)`: An error if the tag or attribute is not found.
///
/// # Errors
///
/// This function will return a `LiveSetError::AttributeError` if:
/// * The specified tag is not found in the collection.
/// * The specified attribute is not found within the matching tag.
pub fn find_attribute(tags: &[XmlTag], tag_query: &str, attribute_query: &str) -> Result<String, LiveSetError> {
    trace!("Searching for attribute '{}' in tag '{}'", attribute_query, tag_query);

    for tag in tags {
        if tag.name == tag_query {
            debug!("Found matching tag: '{}'", tag_query);
            for (key, value) in &tag.attributes {
                if key == attribute_query {
                    debug!("Found attribute '{}' with value '{}'", attribute_query, value);
                    return Ok(value.clone());
                }
            }
            debug!("Attribute '{}' not found in tag '{}'", attribute_query, tag_query);
            return Err(LiveSetError::AttributeError(AttributeError::ValueNotFound(attribute_query.to_string())));
        }
    }

    debug!("Tag '{}' not found", tag_query);
    Err(LiveSetError::AttributeError(AttributeError::NotFound(tag_query.to_string())))
}

/// Searches for a specific empty XML event and extracts its attributes.
///
/// This function parses the given XML data, looking for an empty event that matches
/// the provided search query. If found, it extracts all attributes of that event
/// into a HashMap.
///
/// # Arguments
///
/// * `xml_data` - A byte slice containing the XML data to search.
/// * `search_query` - The name of the empty event to search for.
///
/// # Returns
///
/// * `Result<HashMap<String, String>, XmlParseError>` - A Result containing either:
///   - `Ok(HashMap<String, String>)`: A map of attribute names to values if the event is found.
///   - `Err(XmlParseError)`: If an error occurs during XML parsing, UTF-8 conversion, or if the event is not found.
///
/// # Errors
///
/// This function will return an error if:
/// * The XML data cannot be parsed correctly.
/// * There's a UTF-8 conversion error.
/// * The specified event is not found in the XML data.
///
/// # Example
///
/// ```
/// let xml_data = b"<root><EmptyEvent attr1='value1' attr2='value2'/></root>";
/// let result = find_empty_event(xml_data, "EmptyEvent");
/// assert!(result.is_ok());
/// ```
pub fn find_empty_event(xml_data: &[u8], search_query: &str) -> Result<HashMap<String, String>, XmlParseError> {
    debug!("Searching for empty event with query: {}", search_query);

    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(ref event)) => {
                let name = event.name();
                let event_name = std::str::from_utf8(name.as_ref())?;

                trace!("Found empty event with name: {}", event_name);

                if event_name == search_query {
                    debug!("Empty event {} matches search query", event_name);

                    let attributes = parse_event_attributes(event)?;

                    trace!("Attributes: {:?}", attributes);
                    return Ok(attributes);
                }
            }
            Ok(Event::Eof) => {
                debug!("Reached end of XML data without finding the event");
                return Err(XmlParseError::EventNotFound(search_query.to_string()));
            }
            Err(error) => return Err(XmlParseError::QuickXmlError(error)),
            _ => (),
        }
        buffer.clear();
    }
}

/// Parses attributes from an XML event into a HashMap.
///
/// This helper function extracts all attributes from a given XML event and
/// stores them in a HashMap, with attribute names as keys and their values as values.
///
/// # Arguments
///
/// * `event` - A reference to a `quick_xml::events::BytesStart` representing the XML event.
///
/// # Returns
///
/// * `Result<HashMap<String, String>, XmlParseError>` - A Result containing either:
///   - `Ok(HashMap<String, String>)`: A map of attribute names to their values.
///   - `Err(XmlParseError)`: If an error occurs during attribute parsing or UTF-8 conversion.
///
/// # Errors
///
/// This function will return an error if:
/// * There's an error while iterating over the attributes.
/// * There's a UTF-8 conversion error for attribute names or values.
///
/// # Example
///
/// This function is intended to be used internally by `find_empty_event`:
///
/// ```
/// let attributes = parse_event_attributes(&event)?;
/// ```
fn parse_event_attributes(event: &BytesStart) -> Result<HashMap<String, String>, XmlParseError> {
    let mut attributes = HashMap::new();
    for attribute_result in event.attributes() {
        let attribute = attribute_result?;
        let key = std::str::from_utf8(attribute.key.as_ref())?;
        let value = std::str::from_utf8(&attribute.value)?;
        debug!("Found attribute: {} = {}", key, value);
        attributes.insert(key.to_string(), value.to_string());
    }
    Ok(attributes)
}

pub fn find_sample_path_data(xml_data: &[u8]) -> Vec<String> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut in_sample_ref = false;
    let mut in_data_tag = false;
    let mut data_list = Vec::new();
    let mut current_data = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                if name == "SampleRef" {
                    in_sample_ref = true;
                } else if in_sample_ref && name == "Data" {
                    in_data_tag = true;
                }
            }
            Ok(Event::Text(ref event)) => {
                if in_data_tag {
                    current_data = std::str::from_utf8(event.as_ref()).unwrap().to_string();
                }
            }
            Ok(Event::End(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                if name == "Data" {
                    in_data_tag = false;
                    if !current_data.is_empty() {
                        data_list.push(current_data.clone());
                        current_data.clear();
                    }
                } else if name == "SampleRef" {
                    in_sample_ref = false;
                }
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    data_list
}


pub fn find_all_plugins(xml_data: &[u8]) -> Result<HashMap<String, Vec<String>>, LiveSetError> {
    let search_queries = &["VstPluginInfo", "Vst3PluginInfo"];
    let target_depth: u8 = 0;
    let plugin_tags = find_tags(xml_data, search_queries, target_depth)?;

    let mut plugin_names: HashMap<String, Vec<String>> = HashMap::new();

    for (query, tags_list) in plugin_tags {
        let mut names = Vec::new();

        for tags in tags_list {
            let attribute_name = match query.as_str() {
                "VstPluginInfo" => "PlugName",
                "Vst3PluginInfo" => "Name",
                _ => continue,
            };

            match find_attribute(&tags, attribute_name, "Value") {
                Ok(name) => names.push(name),
                Err(LiveSetError::AttributeError(AttributeError::NotFound(_))) => {
                    warn!("Expected tag '{}' not found while searching for plugin info. This may indicate an unexpected XML structure.", attribute_name);
                    continue;
                },
                Err(LiveSetError::AttributeError(AttributeError::ValueNotFound(_))) => {
                    warn!("Tag '{}' found, but 'Value' attribute is missing. Plugin type: {}. This may indicate corrupted or unexpected plugin data.", attribute_name, query);
                    continue;
                },
                Err(e) => return Err(e), // Propagate other errors
            }
        }

        plugin_names.insert(query, names);
    }

    Ok(plugin_names)
}


pub fn find_all_samples(xml_data: &[u8], major_version: u32) -> Result<HashMap<String, Vec<PathBuf>>, LiveSetError> {
    let mut sample_paths: HashMap<String, Vec<PathBuf>> = HashMap::new();

    if major_version < 11 {
        debug!("Processing samples for Ableton version < 11");
        let sample_data: Vec<String> = find_sample_path_data(xml_data);
        let mut decoded_paths = Vec::new();
        for (index, data) in sample_data.iter().enumerate() {
            match decode_sample_path(data) {
                Ok(path) => {
                    debug!("Successfully decoded sample path {}: {:?}", index, path);
                    decoded_paths.push(path);
                },
                Err(e) => {
                    warn!("Failed to decode sample path {}: {:?}", index, e);
                    return Err(LiveSetError::DecodeSamplePathError(e));
                },
            }
        }
        debug!("Found {} samples for version < 11", decoded_paths.len());
        sample_paths.insert("SampleData".to_string(), decoded_paths);

    } else {
        debug!("Processing samples for Ableton version >= 11");
        let search_queries = &["SampleRef"];
        let target_depth: u8 = 1;
        let sample_tags = find_tags(xml_data, search_queries, target_depth)?;

        for (query, tags_list) in sample_tags {
            let mut paths = Vec::new();
            for (index, tags) in tags_list.iter().enumerate() {
                match find_attribute(tags, "Path", "Value") {
                    Ok(path) => {
                        debug!("Found sample path {} for '{}': {:?}", index, query, path);
                        paths.push(PathBuf::from(path));
                    },
                    Err(LiveSetError::AttributeError(AttributeError::NotFound(_))) => {
                        warn!("Expected 'Path' tag not found for sample {} in '{}'. This may indicate an unexpected XML structure.", index, query);
                        continue;
                    },
                    Err(LiveSetError::AttributeError(AttributeError::ValueNotFound(_))) => {
                        warn!("'Path' tag found for sample {} in '{}', but 'Value' attribute is missing. This may indicate corrupted or unexpected sample data.", index, query);
                        continue;
                    },
                    Err(e) => {
                        warn!("Unexpected error while processing sample {} in '{}': {:?}", index, query, e);
                        return Err(e);
                    },
                }
            }
            debug!("Found {} samples for '{}'", paths.len(), query);
            sample_paths.insert(query, paths);
        }
    }

    debug!("Total sample collections found: {}", sample_paths.len());
    Ok(sample_paths)
}


fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, DecodeSamplePathError> {
    let abs_hash_path = abs_hash_path.replace("\\t", "").replace("\\n", "");

    let byte_data = hex::decode(&abs_hash_path).map_err(DecodeSamplePathError::HexDecodeError)?;

    let (cow, _, had_errors) = UTF_16LE.decode(&byte_data);
    if had_errors {
        return Err(DecodeSamplePathError::InvalidUtf16Encoding);
    }

    let path_string = cow.replace("\u{0}", "");
    let path = PathBuf::from(path_string);

    if let Err(e) = path.canonicalize() {
        return Err(DecodeSamplePathError::PathProcessingError(format!(
            "Failed to canonicalize path: {}",
            e
        )));
    }

    Ok(path)
}

/// Extracts the Ableton version information from the given XML data.
///
/// This function parses the XML data of an Ableton Live project file to extract
/// the version information. It looks for the "Ableton" tag and its "Creator" attribute.
///
/// # Arguments
///
/// * `xml_data` - A byte slice containing the XML data of the Ableton Live project file.
///
/// # Returns
///
/// * `Result<AbletonVersion, LiveSetError>` - The parsed Ableton version if successful,
///   or an error if parsing fails.
///
/// # Examples
///
/// ```
/// let xml_data = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton Creator=\"Ableton Live 11.0.12\">";
/// let version = extract_version(xml_data).unwrap();
/// assert_eq!(version.major, 11);
/// assert_eq!(version.minor, 0);
/// assert_eq!(version.patch, 12);
/// ```
pub fn extract_version(xml_data: &[u8]) -> Result<AbletonVersion, LiveSetError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(_)) => continue,
            Ok(Event::Start(ref event)) => {
                let name = event.name();
                let name_str = std::str::from_utf8(name.as_ref())
                    .map_err(|e| LiveSetError::Utf8Error(e))?;

                if name_str != "Ableton" {
                    return Err(LiveSetError::InvalidFileFormat(format!("First element is '{}', expected 'Ableton'", name_str)));
                }
                debug!("Found Ableton tag, attributes:");
                for attr_result in event.attributes() {
                    match attr_result {
                        Ok(attr) => debug!("  {}: {:?}", String::from_utf8_lossy(attr.key.as_ref()), String::from_utf8_lossy(&attr.value)),
                        Err(e) => debug!("  Error parsing attribute: {:?}", e),
                    }
                }
                let ableton_version = AbletonVersion::from_attributes(event.attributes());
                debug!("Parsed version: {:?}", &ableton_version);
                return ableton_version;
            }
            Ok(Event::Eof) => {
                return Err(LiveSetError::InvalidFileFormat("Reached end of file without finding Ableton tag".into()));
            }
            Ok(_) => continue,
            Err(e) => return Err(e.into()),
        }
    }
}


pub fn get_file_timestamps(file_path: &PathBuf) -> Result<(DateTime<Local>, DateTime<Local>), LiveSetError> {
    let metadata = fs::metadata(file_path).map_err(|e| LiveSetError::FileMetadataError {
        path: file_path.clone(),
        source: e,
    })?;

    let modified_time = metadata.modified()
        .map(DateTime::<Local>::from)
        .map_err(|e| LiveSetError::FileMetadataError {
            path: file_path.clone(),
            source: e,
        })?;

    let created_time = metadata.created()
        .map(DateTime::<Local>::from)
        .unwrap_or_else(|_| Local::now());

    Ok((modified_time, created_time))
}


pub fn get_file_hash(file_path: &PathBuf) -> Result<String, LiveSetError> {
    let mut file = File::open(file_path).map_err(|e| LiveSetError::FileHashingError {
        path: file_path.clone(),
        source: e,
    })?;

    let mut hasher = Hasher::new();
    let mut buffer = [0; 1024];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| LiveSetError::FileHashingError {
            path: file_path.clone(),
            source: e,
        })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    let hash_string = format!("{:08x}", hash);

    Ok(hash_string)
}


pub fn get_file_name(file_path: &PathBuf) -> Result<String, LiveSetError> {
    file_path
        .file_name()
        .ok_or_else(|| LiveSetError::FileNameError("File name is not present".to_string()))?
        .to_str()
        .ok_or_else(|| LiveSetError::FileNameError("File name is not valid UTF-8".to_string()))
        .map(|s| s.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        let xml_data = br#"<?xml version="1.0" encoding="UTF-8"?>
        <Ableton MajorVersion="11" MinorVersion="0" SchemaChangeCount="3" Creator="Ableton Live 11.0.1" Revision="1b1951c0f4b3d5a5ad5d1ac69c3d9b5aa7a36dd8">"#;

        let version = extract_version(xml_data).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 1);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_extract_version_beta() {
        let xml_data = br#"<?xml version="1.0" encoding="UTF-8"?>
        <Ableton MajorVersion="11" MinorVersion="1" SchemaChangeCount="0" Creator="Ableton Live 11.1 Beta" Revision="1b1951c0f4b3d5a5ad5d1ac69c3d9b5aa7a36dd8">"#;

        let version = extract_version(xml_data).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, true);
    }

    #[test]
    fn test_extract_version_invalid_xml() {
        let xml_data = b"<Invalid>XML</Invalid>";
        assert!(extract_version(xml_data).is_err());
    }
}