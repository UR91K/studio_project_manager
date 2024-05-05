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
use std::io::{Read, Cursor};
use std::time::{Instant};
use std::fs;

use colored::*;
use chrono::{DateTime, Utc};
use elementtree::Element;
use zune_inflate::DeflateDecoder;
use flate2::read::GzDecoder;

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

fn decode_als_data(file_path: &Path) -> Result<Vec<u8>, String> {
    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(err) => return Err(format!("Failed to open file {}: {}", file_path.display(), err)),
    };

    let start_time = Instant::now();

    let mut gzip_decoder = GzDecoder::new(&mut file);
    let mut decompressed_data = Vec::new();
    if let Err(err) = gzip_decoder.read_to_end(&mut decompressed_data) {
        return Err(format!("Failed to decompress file {}: {}", file_path.display(), err));
    }

    let duration = start_time.elapsed();
    // println!("flate2: decompressing the file: {:.2?}", duration);

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

    plugins: HashSet<Id>,
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

            plugins: HashSet::new(),
            samples: HashSet::new(),
        };

        live_set.load_raw_xml_data()
            .and_then(|_| live_set.update_file_name().map_err(|err| err.to_string()))
            .map(|_| live_set)
    }

    fn update_file_name(&mut self) -> Result<(), String> {
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

    fn update_last_modification_time(&mut self) {
        let metadata = fs::metadata(&self.file_path).expect("Failed to get metadata");

        let modified_time = metadata.modified().expect("Failed to get modified time");
        let modified_time = DateTime::<Utc>::from(modified_time);

        let created_time = metadata.created().ok().map_or_else(|| Utc::now(), |time| {
            DateTime::<Utc>::from(time)
        });

        self.modified_time = modified_time;
        self.created_time = created_time;
    }

    fn load_raw_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.file_path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{:?}: is either inaccessible or not a valid Ableton Live Set file", self.file_path));
        }

        let decompressed_data = match decode_als_data(&path) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };
        
        self.raw_xml_data = Some(decompressed_data);

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
                "Loaded {} ({}) in {:.2} ms",
                path.file_name().unwrap().to_string_lossy().bold().purple(),
                formatted_size,
                duration_ms
            ),
            Err(err) => eprintln!("Error: {}", err),
        }
    }
}