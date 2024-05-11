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
use log::{debug, trace};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::custom_types::XmlTag;
use crate::errors::{LiveSetError, TimeSignatureError};

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

pub fn find_tags(xml_data: &[u8], search_queries: &[&str]) -> HashMap<String, Vec<Vec<XmlTag>>> {
    /// Retrieves all the empty tags which are immediate children of the search queries
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
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
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
                if in_target_tag && depth == 0 {
                    let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                    let mut attributes = Vec::new();
                    for attr in event.attributes() {
                        let attr = attr.unwrap();
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap().to_string();
                        let value = std::str::from_utf8(attr.value.as_ref()).unwrap().to_string();
                        attributes.push((key, value));
                    }
                    current_tags.get_mut(&current_query).unwrap().push(XmlTag {
                        name,
                        attributes,
                    });
                }
            }
            Ok(Event::End(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                if name == current_query {
                    in_target_tag = false;
                    all_tags.entry(current_query.clone()).or_default().push(current_tags[&current_query].clone());
                    current_tags.get_mut(&current_query).unwrap().clear();
                } else if in_target_tag {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => {
                break;
            }
            _ => (),
        }
        buf.clear();
    }
    all_tags
}

pub fn find_attribute(tags: &[XmlTag], tag_query: &str, attribute_query: &str) -> Option<String> {
    // println!("Searching for attribute '{}' in tag '{}'", attribute_query, tag_query);
    for tag in tags {
        if tag.name == tag_query {
            for (key, value) in &tag.attributes {
                if key == attribute_query {
                    // println!("Found attribute '{}' with value: {}", attribute_query, value);
                    return Some(value.clone());
                }
            }
        }
    }
    // println!("Attribute '{}' not found in tag '{}'", attribute_query, tag_query);
    None
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
                // debug!("Found empty event with name: {}", name);

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

pub fn find_all_plugins(xml_data: &[u8]) -> HashMap<String, Vec<String>> {
    let search_queries = &["VstPluginInfo", "Vst3PluginInfo"];
    let plugin_tags = find_tags(xml_data, search_queries);

    let mut plugin_names: HashMap<String, Vec<String>> = HashMap::new();

    for (query, tags_list) in plugin_tags {
        let mut names = Vec::new();

        for tags in tags_list {
            let attribute_name = match query.as_str() {
                "VstPluginInfo" => "PlugName",
                "Vst3PluginInfo" => "Name",
                _ => continue,
            };

            if let Some(name) = find_attribute(&tags, attribute_name, "Value") {
                names.push(name);
            }
        }

        plugin_names.insert(query, names);
    }

    plugin_names
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