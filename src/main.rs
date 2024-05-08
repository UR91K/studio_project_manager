mod custom_types;
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

use std::collections::HashSet;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{Read, Cursor, Error};
use std::time::{Instant};
use std::fs;

use colored::*;
use chrono::{DateTime, Utc};
use elementtree::Element;
use zune_inflate::DeflateDecoder;
use flate2::read::GzDecoder;

use quick_xml::Reader;
use quick_xml::events::Event;

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

fn decode_numerator(encoded_value: i32) -> i32 {
    // this is done because the numerator is encoded weirdly
    return if encoded_value < 0 {
        1
    } else if encoded_value < 99 {
        encoded_value + 1
    } else {
        (encoded_value % 99) + 1
    }
}

fn decode_denominator(encoded_value: i32) -> i32 {
    let multiple = encoded_value / 99 + 1;
    2_i32.pow((multiple - 1) as u32)
}

fn extract_gzipped_als_data(file_path: &Path) -> Result<Vec<u8>, String> {
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



fn zune_decode_als_data(file_path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;

    let start_time = Instant::now();

    let mut compressed_data = Vec::new();
    file.read_to_end(&mut compressed_data)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let mut decoder = DeflateDecoder::new(&compressed_data);
    let decompressed_data = decoder.decode_gzip()
        .map_err(|e| format!("Failed to decompress data: {}", e))?;

    let duration = start_time.elapsed();
    println!("zune_inflate: decompressing the file: {:.2?}", duration);

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

fn find_tags(xml_data: &[u8], search_query: &str) -> Vec<Vec<XmlTag>> {
    // println!("Starting to find tags with search query: {}", search_query);
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut all_tags = Vec::new();
    let mut current_tags = Vec::new();

    let mut in_target_tag = false;
    let mut depth: u8 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                if name == search_query {
                    // println!("Entering target tag: {}", name);
                    in_target_tag = true;
                    depth = 0;
                } else if in_target_tag {
                    depth += 1;
                }
            }
            Ok(Event::Empty(ref event)) => {
                if in_target_tag && depth == 0 {
                    let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                    // println!("Found empty tag: {}", name);
                    let mut attributes = Vec::new();
                    for attr in event.attributes() {
                        let attr = attr.unwrap();
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap().to_string();
                        let value = std::str::from_utf8(attr.value.as_ref()).unwrap().to_string();
                        // println!("Found attribute in {}: {} = {}", name, key, value);
                        attributes.push((key, value));
                    }
                    current_tags.push(XmlTag {
                        name,
                        attributes,
                    });
                }
            }
            Ok(Event::End(ref event)) => {
                let name = std::str::from_utf8(event.name().as_ref()).unwrap().to_string();
                if name == search_query {
                    // println!("Exiting target tag: {}", name);
                    in_target_tag = false;
                    all_tags.push(current_tags.clone());
                    current_tags.clear();
                } else if in_target_tag {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => {
                // println!("Reached end of XML data");
                break;
            }
            _ => (),
        }
        buf.clear();
    }
    // println!("Found {} tag(s) matching search query: {}", all_tags.len(), search_query);
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

fn find_vst_plugins(xml_data: &[u8]) -> Vec<String> {
    // println!("Starting to find VST plugins");
    let vst_plugin_tags = find_tags(xml_data, "VstPluginInfo");
    let mut vst_plugin_names = Vec::new();

    for tags in vst_plugin_tags {
        if let Some(plug_name) = find_attribute(&tags, "PlugName", "Value") {
            // println!("Found VST plugin: {}", plug_name);
            vst_plugin_names.push(plug_name);
        }
    }

    // println!("Found {} VST plugin(s)", vst_plugin_names.len());
    vst_plugin_names
}

fn find_vst3_plugins(xml_data: &[u8]) -> Vec<String> {
    println!("Starting to find VST3 plugins");
    let vst3_plugin_tags = find_tags(xml_data, "Vst3PluginInfo");
    println!("VST3 {:?}", vst3_plugin_tags);
    let mut vst3_plugin_names = Vec::new();

    for tags in vst3_plugin_tags {
        if let Some(name) = find_attribute(&tags, "Name", "Value") {
            println!("Found VST plugin: {}", name);
            vst3_plugin_names.push(name);
        }
    }

    println!("Found {} VST3 plugin(s)", vst3_plugin_names.len());
    vst3_plugin_names
}


#[derive(Debug)]
struct LiveSet {
    id: Id,

    file_path: PathBuf,
    file_name: Option<String>,
    raw_xml_data: Option<Vec<u8>>,
    file_hash: Option<String>,

    created_time: DateTime<Utc>,
    modified_time: DateTime<Utc>,

    last_scan_timestamp: DateTime<Utc>,
    ableton_version: AbletonVersion,
    ableton_version_readable: String,
    key_signature: KeySignature,
    tempo: f32,
    time_signature: TimeSignature,
    estimated_duration: chrono::Duration,
    furthest_bar: u32,

    vst2_plugin_names: HashSet<String>,
    vst3_plugin_names: HashSet<String>,
    samples: HashSet<Id>,
}

impl LiveSet {
    fn new(path: PathBuf) -> Result<Self, String> {
        let mut live_set = LiveSet {
            id: Id::default(),

            file_path: path,
            file_name: None,
            raw_xml_data: None,
            file_hash: None,

            created_time: Utc::now(),
            modified_time: Utc::now(),

            last_scan_timestamp: Utc::now(),
            ableton_version: AbletonVersion::default(),
            ableton_version_readable: String::new(),
            key_signature: KeySignature::default(),
            tempo: 0.0,
            time_signature: TimeSignature::default(),
            estimated_duration: chrono::Duration::zero(),
            furthest_bar: 0,

            vst2_plugin_names: HashSet::new(),
            vst3_plugin_names: HashSet::new(),

            samples: HashSet::new(),
        };

        live_set.load_raw_xml_data()
            .and_then(|_| live_set.update_file_name().map_err(|err| err.to_string()))
            .and_then(|_| live_set.find_vst2_plugins().map_err(|err| err.to_string()))
            .and_then(|_| live_set.find_vst3_plugins().map_err(|err| err.to_string()))
            .map(|_| live_set)
    }

    pub fn update_file_name(&mut self) -> Result<(), String> {
        if let Some(file_name) = self.file_path.file_name() {
            if let Some(name) = file_name.to_str() {
                self.file_name = Some(name.to_string());
                Ok(())
            } else {
                Err("File name is not valid UTF-8".to_string())
            }
        } else {
            Err("File name is not present".to_string())
        }
    }

    pub fn update_last_modification_time(&mut self) -> Result<(), Error> {
        let metadata = fs::metadata(&self.file_path)?;

        let modified_time = metadata.modified()?;
        let modified_time = DateTime::<Utc>::from(modified_time);

        let created_time = metadata.created().ok().map_or_else(|| Utc::now(), |time| {
            DateTime::<Utc>::from(time)
        });

        self.modified_time = modified_time;
        self.created_time = created_time;

        Ok(())
    }

    fn load_raw_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.file_path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{:?}: is either inaccessible or not a valid Ableton Live Set file", self.file_path));
        }

        let decompressed_data = match extract_gzipped_als_data(&path) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };
        
        self.raw_xml_data = Some(decompressed_data);

        Ok(())
    }

    pub fn find_vst2_plugins(&mut self) -> Result<(), &'static str> {
        let start_time = Instant::now();

        let xml_data = self.raw_xml_data.as_deref().ok_or("XML data not found")?;
        let vst2_plugin_tags = find_tags(xml_data, "VstPluginInfo");
        let mut vst2_plugin_names = HashSet::new();

        for tags in vst2_plugin_tags {
            if let Some(plug_name) = find_attribute(&tags, "PlugName", "Value") {
                vst2_plugin_names.insert(plug_name);
            }
        }


        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;

        println!("{}: found {:?} VST2 Plugins: {:?} in {:.2} ms", self.file_name.as_deref().unwrap().to_string().bold().purple(), vst2_plugin_names.len(), vst2_plugin_names, duration_ms);

        self.vst2_plugin_names = vst2_plugin_names;

        Ok(())
    }

    pub fn find_vst3_plugins(&mut self) -> Result<(), &'static str> {
        let start_time = Instant::now();

        let xml_data = self.raw_xml_data.as_deref().ok_or("XML data not found")?;
        let vst3_plugin_tags = find_tags(xml_data, "Vst3PluginInfo");
        let mut vst3_plugin_names = HashSet::new();

        for tags in vst3_plugin_tags {
            if let Some(name) = find_attribute(&tags, "Name", "Value") {
                vst3_plugin_names.insert(name);
            }
        }

        let end_time = Instant::now();
        let duration = end_time - start_time;
        let duration_ms = duration.as_secs_f64() * 1000.0;

        println!("{}: found {:?} VST3 plugins: {:?} in {:.2} ms", self.file_name.as_deref().unwrap().to_string().bold().purple(), vst3_plugin_names.len(), vst3_plugin_names, duration_ms);

        self.vst3_plugin_names = vst3_plugin_names;

        Ok(())
    }

    fn update_time_signature(&self) -> Result<(), String> {
        let TIME_SIGNATURE_EVENT_TIME: i32 = -63072000;
        Ok(())
    }

}

fn main() {
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
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}