mod custom_types;
mod errors;

use custom_types::{Id,
                   TimeSignature,
                   AbletonVersion,
                   Scale,
                   Tonic,
                   KeySignature,
                   PluginFormat,
                   Plugin,
                   Sample
};

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{Read, Cursor, Error};
use std::time::{Instant};
use std::fs;
use colored::*;
use chrono::{DateTime, Local, Utc};
use elementtree::Element;
use flate2::read::GzDecoder;
use quick_xml::Reader;
use quick_xml::events::Event;
use crc32fast::Hasher;

use log::{debug, info, error, trace};
use anyhow::{anyhow, Context, Result};
use log::LevelFilter;
use env_logger::Builder;
use thiserror::Error;
use crate::errors::{LiveSetError, TimeSignatureError};


fn validate_time_signature(value: i32) -> Result<i32, TimeSignatureError> {
    if value >= 0 && value <= 16777215 {
        Ok(value)
    } else {
        Err(TimeSignatureError::InvalidEncodedValue(value))
    }
}

// This is the value of an EnumEvent tag that the time signature is stored in as a weirdly encoded number
const TIME_SIGNATURE_ENUM_EVENT: i32 = -63072000;

fn format_file_size(size: u64) -> String {
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

fn extract_gzipped_data(file_path: &Path) -> Result<Vec<u8>, String> {
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

fn parse_xml_data(xml_data: &[u8], file_name: &Option<String>, file_path: &Path) -> Result<Element, String> {
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

#[derive(Debug, Clone)]
struct XmlTag {
    name: String,
    attributes: Vec<(String, String)>,
}

/// Retrieves all the empty tags which are immediate children of the search queries
fn find_tags(xml_data: &[u8], search_queries: &[&str]) -> HashMap<String, Vec<Vec<XmlTag>>> {
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

fn find_attribute(tags: &[XmlTag], tag_query: &str, attribute_query: &str) -> Option<String> {
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

fn find_empty_event(xml_data: &[u8], search_query: &str) -> Option<HashMap<String, String>> {
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

fn find_all_plugins(xml_data: &[u8]) -> HashMap<String, Vec<String>> {
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


#[derive(Debug)]
struct LiveSet {
    id: Id,

    file_path: PathBuf,
    file_name: String,
    xml_data: Vec<u8>,
    file_hash: String,
    created_time: DateTime<Local>,
    modified_time: DateTime<Local>,
    last_scan_timestamp: DateTime<Local>,

    ableton_version: Option<AbletonVersion>,
    key_signature: Option<KeySignature>,
    tempo: Option<f32>,
    time_signature: Option<TimeSignature>,
    estimated_duration: Option<chrono::Duration>,
    furthest_bar: Option<u32>,

    vst2_plugin_names: Option<HashSet<String>>,
    vst3_plugin_names: Option<HashSet<String>>,
    sample_paths: Option<HashSet<Id>>,
}

fn get_file_timestamps(file_path: &PathBuf) -> Result<(DateTime<Local>, DateTime<Local>), String> {
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

fn get_file_hash(file_path: &PathBuf) -> Result<String, String> {
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



impl LiveSet {
    fn new(file_path: PathBuf) -> Result<Self, String> {
        let file_name = match get_file_name(&file_path) {
            Ok(name) => name,
            Err(error) => return Err(error),
        };

        let path = Path::new(&file_path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{:?}: is either inaccessible or not a valid Ableton Live Set file", file_path));
        }

        let (modified_time, created_time) = match get_file_timestamps(&file_path) {
            Ok(timestamps) => timestamps,
            Err(error) => return Err(error),
        };

        let file_hash = match get_file_hash(&file_path) {
            Ok(hash) => hash,
            Err(error) => return Err(error),
        };

        let xml_data = match extract_gzipped_data(&file_path) {
            Ok(data) => data,
            Err(error) => return Err(error),
        };

        let last_scan_timestamp = Local::now();

        let live_set = LiveSet {
            id: Id::default(),

            file_path,
            file_name,
            xml_data,
            file_hash,
            created_time,
            modified_time,
            last_scan_timestamp,

            ableton_version: None,
            key_signature: None,
            tempo: None,
            time_signature: None,
            estimated_duration: None,
            furthest_bar: None,

            vst2_plugin_names: None,
            vst3_plugin_names: None,

            sample_paths: None,
        };

        Ok(live_set)
    }

    fn load_raw_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.file_path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{:?}: is either inaccessible or not a valid Ableton Live Set file", self.file_path));
        }

        let decompressed_data = extract_gzipped_data(&path).map_err(|err| err.to_string())?;

        self.xml_data = decompressed_data;

        Ok(())
    }

    pub fn find_plugins(&mut self) -> Result<(), &'static str> {
        let start_time = Instant::now();

        let plugin_names = find_all_plugins(&self.xml_data);

        let mut vst2_plugin_names = HashSet::new();
        let mut vst3_plugin_names = HashSet::new();

        if let Some(vst2_names) = plugin_names.get("VstPluginInfo") {
            vst2_plugin_names.extend(vst2_names.iter().cloned());
        }

        if let Some(vst3_names) = plugin_names.get("Vst3PluginInfo") {
            vst3_plugin_names.extend(vst3_names.iter().cloned());
        }

        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;

        info!(
            "{}: found {} VST2 Plugins and {} VST3 Plugins in {:.2} ms",
            self.file_name.bold().purple(),
            vst2_plugin_names.len(),
            vst3_plugin_names.len(),
            duration_ms
        );

        self.vst2_plugin_names = Some(vst2_plugin_names);
        self.vst3_plugin_names = Some(vst3_plugin_names);

        Ok(())
    }

    fn update_time_signature(&mut self) -> Result<(), LiveSetError> {
        debug!("Updating time signature");

        trace!("XML data: {:?}", std::str::from_utf8(&self.xml_data));

        let search_query = "EnumEvent";

        if let Some(attributes) = find_empty_event(&self.xml_data, search_query) {
            debug!("Found time signature enum event");
            trace!("Attributes: {:?}", attributes);

            if let Some(value) = attributes.get("Value") {
                debug!("Found 'Value' attribute");
                trace!("Value: {}", value);

                let encoded_value = value
                    .parse::<i32>()
                    .map_err(|e| LiveSetError::TimeSignatureError(TimeSignatureError::ParseEncodedError(e)))?;
                debug!("Parsed encoded value: {}", encoded_value);

                let time_signature = TimeSignature::from_encoded(encoded_value)
                    .map_err(LiveSetError::TimeSignatureError)?;
                debug!("Decoded time signature: {:?}", time_signature);

                self.time_signature = Some(time_signature);
                info!(
                "Time signature updated: {}/{}",
                time_signature.numerator, time_signature.denominator
            );

                return Ok(());
            } else {
                error!("'Value' attribute not found");
                return Err(LiveSetError::ValueAttributeNotFound);
            }
        } else {
            error!("Time signature enum event not found");
            return Err(LiveSetError::EnumEventNotFound);
        }
    }
}

fn main() {
    // Builder::new()
    //     .filter_level(LevelFilter::Debug)
    //     .init();

    let mut paths: Vec<PathBuf> = Vec::new();
    /// TEST DATA:
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\rodent beats\RODENT 4 Project\RODENT 4 ver 2.als")); // max size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\Beats Project\a lot on my mind 130 Live11.als")); // mean size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\rust mastering\dp tekno 19 master Project\dp tekno 19 master.als")); // mode size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\Beats\Beats Project\SET 120.als")); // median size
    paths.push(PathBuf::from(r"C:\Users\judee\Documents\Projects\tape\white tape b Project\white tape b.als")); // min size
    for path in &paths {
        let start_time = Instant::now();
        let live_set_result = LiveSet::new(path.to_path_buf());
        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;
        let mut file_size: u64 = 0;
        let mut formatted_size: String = String::new();
        if let Ok(metadata) = fs::metadata(&path) {
            file_size = metadata.len();
            formatted_size = format_file_size(file_size);
        }

        match live_set_result {
            Ok(_) => println!(
                "{} ({}) Loaded in {:.2} ms",
                path.file_name().unwrap().to_string_lossy().bold().purple(),
                formatted_size,
                duration_ms
            ),
            Err(err) => error!("Error: {}", err),
        }
    }
}