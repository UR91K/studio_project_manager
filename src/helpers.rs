use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Local};
use crc32fast::Hasher;
use elementtree::Element;
use flate2::read::GzDecoder;
use log::{debug, error, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;
use encoding_rs::UTF_16LE;

use crate::custom_types::{AbletonVersion, XmlTag};
use crate::errors::{DecodeSamplePathError, LiveSetError, TimeSignatureError, XmlParseError, AttributeError};

pub const TIME_SIGNATURE_ENUM_EVENT: i32 = -63072000;
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB

pub fn extract_gzipped_data_parallel(file_path: &Path) -> Result<Vec<u8>, String> {
    let file = match File::open(&file_path) {
        Ok(file) => file,
        Err(err) => return Err(format!("Failed to open file {}: {}", file_path.display(), err)),
    };

    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let num_chunks = (file_size as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;

    let file = Arc::new(Mutex::new(file));
    let decompressed_data = Arc::new(Mutex::new(Vec::new()));

    let mut threads = Vec::new();
    for _ in 0..num_chunks {
        let file = Arc::clone(&file);
        let decompressed_data = Arc::clone(&decompressed_data);

        let thread = thread::spawn(move || {
            let mut chunk = Vec::with_capacity(CHUNK_SIZE);
            let mut locked_file = file.lock().unwrap();
            let mut gzip_decoder = GzDecoder::new(&mut *locked_file);

            if let Err(err) = gzip_decoder.read_to_end(&mut chunk) {
                eprintln!("Failed to decompress chunk: {}", err);
                return;
            }

            let mut locked_data = decompressed_data.lock().unwrap();
            locked_data.extend_from_slice(&chunk);
        });

        threads.push(thread);
    }

    for thread in threads {
        thread.join().unwrap();
    }

    let decompressed_data = Arc::try_unwrap(decompressed_data)
        .unwrap()
        .into_inner()
        .map_err(|_| "Failed to retrieve decompressed data".to_string())?;

    Ok(decompressed_data)
}

pub fn extract_gzipped_data(file_path: &Path) -> Result<Vec<u8>, String> {
    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(err) => return Err(format!("Failed to open file {}: {}", file_path.display(), err)),
    };

    let mut gzip_decoder = GzDecoder::new(&mut file);
    let mut decompressed_data = Vec::new();
    if let Err(err) = gzip_decoder.read_to_end(&mut decompressed_data) {
        return Err(format!("Failed to decompress file {}: {}", file_path.display(), err));
    }

    Ok(decompressed_data)
}

pub fn parse_encoded_value(value: &str) -> Result<i32, LiveSetError> {
    value
        .parse::<i32>()
        .map_err(|e| LiveSetError::TimeSignatureError(TimeSignatureError::ParseEncodedError(e)))
}

pub fn validate_time_signature(value: i32) -> Result<i32, TimeSignatureError> {
    if value >= 0 && value <= 16777215 {
        Ok(value)
    } else {
        Err(TimeSignatureError::InvalidEncodedValue(value))
    }
}

pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if size < KB {
        format!("{} B", size)
    } else if size < MB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else if size < GB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else {
        format!("{:.2} GB", size as f64 / GB as f64)
    }
}

pub fn parse_xml_data(xml_data: &[u8], file_name: &Option<String>, file_path: &Path) -> Result<Element, String> {
    let xml_data_str = match std::str::from_utf8(xml_data) {
        Ok(s) => s,
        Err(err) => return Err(format!("{:?}: Failed to convert decompressed data to UTF-8 string: {}", file_name, err)),
    };

    let xml_start = match xml_data_str.find("<?xml") {
        Some(start) => start,
        None => return Err(format!("{:?}: No XML data found in decompressed file", file_name)),
    };

    let xml_slice = &xml_data_str[xml_start..];

    let start_time_xml = Instant::now();
    let root = match Element::from_reader(Cursor::new(xml_slice.as_bytes())) {
        Ok(root) => root,
        Err(err) => return Err(format!("{:?}: {} is not a valid XML file: {}", file_name, file_path.display(), err)),
    };
    let duration = start_time_xml.elapsed();
    println!("Creating XML Element: {:.2?}", duration);

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
pub fn find_tags(xml_data: &[u8], search_queries: &[&str], target_depth: u8)
                 -> Result<HashMap<String, Vec<Vec<XmlTag>>>, LiveSetError> {
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
                    .map_err(|e| LiveSetError::XmlParseError(XmlParseError::Utf8Error(e)))?
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
                        .map_err(|e| LiveSetError::XmlParseError(XmlParseError::Utf8Error(e)))?
                        .to_string();
                    let mut attributes = Vec::new();
                    for attr in event.attributes() {
                        let attr = attr.map_err(|_| LiveSetError::XmlParseError(XmlParseError::AttributeError))?;
                        let key = std::str::from_utf8(attr.key.as_ref())
                            .map_err(|e| LiveSetError::XmlParseError(XmlParseError::Utf8Error(e)))?
                            .to_string();
                        let value = std::str::from_utf8(&attr.value)
                            .map_err(|e| LiveSetError::XmlParseError(XmlParseError::Utf8Error(e)))?
                            .to_string();
                        attributes.push((key, value));
                    }
                    current_tags.get_mut(&current_query)
                        .ok_or_else(|| LiveSetError::XmlParseError(XmlParseError::InvalidStructure))?
                        .push(XmlTag {
                            name,
                            attributes,
                        });
                }
            }
            Ok(Event::End(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref())
                    .map_err(|e| LiveSetError::XmlParseError(XmlParseError::Utf8Error(e)))?
                    .to_string();
                if name == current_query {
                    in_target_tag = false;
                    all_tags.entry(current_query.clone()).or_default()
                        .push(current_tags[&current_query].clone());
                    current_tags.get_mut(&current_query)
                        .ok_or_else(|| LiveSetError::XmlParseError(XmlParseError::InvalidStructure))?
                        .clear();
                } else if in_target_tag {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(LiveSetError::XmlParseError(XmlParseError::QuickXmlError(e))),
            _ => (),
        }
        buf.clear();
    }
    Ok(all_tags)
}


pub fn find_attribute(tags: &[XmlTag], tag_query: &str, attribute_query: &str) -> Result<String, LiveSetError> {
    for tag in tags {
        if tag.name == tag_query {
            for (key, value) in &tag.attributes {
                if key == attribute_query {
                    return Ok(value.clone());
                }
            }
            // If we found the tag but not the attribute
            return Err(LiveSetError::AttributeError(AttributeError::ValueNotFound(attribute_query.to_string())));
        }
    }
    // If we didn't find the tag at all
    Err(LiveSetError::AttributeError(AttributeError::NotFound(tag_query.to_string())))
}

pub fn find_empty_event(xml_data: &[u8], search_query: &str) -> Option<HashMap<String, String>> {
    debug!("Searching for empty event with query: {}", search_query);
    trace!("XML data: {:?}", std::str::from_utf8(xml_data));

    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                trace!("Found empty event with name: {}", name);

                if name == search_query {
                    debug!("Empty event {} matches search query {}", name, search_query);

                    let mut attributes = HashMap::new();
                    for attr in event.attributes() {
                        let attr = attr.unwrap();
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap().to_string();
                        let value = std::str::from_utf8(attr.value.as_ref()).unwrap().to_string();
                        debug!("Found attribute: {} = {}", key, value);
                        attributes.insert(key, value);
                    }

                    trace!("Attributes: {:?}", attributes);
                    return Some(attributes);
                }
            }
            Ok(Event::Eof) => {
                debug!("Reached end of XML data");
                break;
            }
            _ => (),
        }
        buf.clear();
    }

    debug!("Empty event {} not found", search_query);
    None
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

pub fn get_file_timestamps(file_path: &PathBuf) -> Result<(DateTime<Local>, DateTime<Local>), String> {
    let metadata = match fs::metadata(file_path) {
        Ok(meta) => meta,
        Err(error) => return Err(format!("Failed to retrieve file metadata: {}", error)),
    };

    let modified_time = match metadata.modified() {
        Ok(time) => DateTime::<Local>::from(time),
        Err(error) => return Err(format!("Failed to retrieve modified time: {}", error)),
    };

    let created_time = match metadata.created() {
        Ok(time) => DateTime::<Local>::from(time),
        Err(_) => Local::now(),
    };

    Ok((modified_time, created_time))
}

pub fn get_file_hash(file_path: &PathBuf) -> Result<String, String> {
    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(error) => return Err(format!("Failed to open file: {}", error)),
    };

    let mut hasher = Hasher::new();
    let mut buffer = [0; 1024];

    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(bytes) => bytes,
            Err(error) => return Err(format!("Failed to read file: {}", error)),
        };

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    let hash_string = format!("{:08x}", hash);

    Ok(hash_string)
}

pub fn get_file_name(file_path: &PathBuf) -> Result<String, String> {
    match file_path.file_name() {
        Some(file_name) => match file_name.to_str() {
            Some(name) => Ok(name.to_string()),
            None => Err("File name is not valid UTF-8".to_string()),
        },
        None => Err("File name is not present".to_string()),
    }
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