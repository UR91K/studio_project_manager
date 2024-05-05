use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{Read, Cursor};
use std::time::Instant;

use chrono::{DateTime, Utc};
use elementtree::Element;
use flate2::read::GzDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Id(u64);

impl Default for Id {
    fn default() -> Self {
        Id(0)
    }
}

#[derive(Debug)]
struct TimeSignature {
    numerator: u8,
    denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        TimeSignature {
            numerator: 4,
            denominator: 4,
        }
    }
}

#[derive(Debug)]
struct AbletonVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Default for AbletonVersion {
    fn default() -> Self {
        AbletonVersion {
            major: 0,
            minor: 0,
            patch: 0,
        }
    }
}

impl fmt::Display for AbletonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ableton {}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Scale {
    Empty,
    Major,
    Minor,
    Dorian,
    Mixolydian,
    Aeolian,
    Phrygian,
    Locrian,
    WholeTone,
    HalfWholeDim,
    WholeHalfDim,
    MinorBlues,
    MinorPentatonic,
    MajorPentatonic,
    HarmonicMinor,
    MelodicMinor,
    Dorian4,
    PhrygianDominant,
    LydianDominant,
    LydianAugmented,
    HarmonicMajor,
    SuperLocrian,
    BToneSpanish,
    HungarianMinor,
    Hirajoshi,
    Iwato,
    PelogSelisir,
    PelogTembung,
    Messiaen1,
    Messiaen2,
    Messiaen3,
    Messiaen4,
    Messiaen5,
    Messiaen6,
    Messiaen7,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Tonic {
    Empty,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

#[derive(Debug)]
struct KeySignature {
    tonic: Tonic,
    scale: Scale,
}

impl Default for KeySignature {
    fn default() -> Self {
        KeySignature {
            tonic: Tonic::Empty,
            scale: Scale::Empty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PluginFormat {
    AU,
    VST2,
    VST3,
}

#[derive(Debug)]
struct Plugin {
    id: Id,
    name: String,
    plugin_format: PluginFormat,
    is_installed: bool
}

#[derive(Debug)]
struct Sample {
    id: Id,
    name: String,
    path: PathBuf,
    is_present: bool
}

#[derive(Debug)]
struct LiveSet {
    id: Id,
    path: PathBuf,
    file_hash: String,
    last_scan_timestamp: DateTime<Utc>,
    file_name: String,
    creation_time: DateTime<Utc>,
    last_modification_time: DateTime<Utc>,
    creator: String,
    key_signature: KeySignature,
    ableton_version: AbletonVersion,
    tempo: f32,
    time_signature: TimeSignature,
    estimated_duration: chrono::Duration,
    furthest_bar: u32,
    plugins: HashSet<Id>,
    samples: HashSet<Id>,
    xml_root: Option<Element>,
}

fn decompress_file(file_path: &Path) -> Result<Vec<u8>, String> {
    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(err) => return Err(format!("Failed to open file {}: {}", file_path.display(), err)),
    };

    // Timing: Decompressing the file
    let start_time = Instant::now();

    let mut gzip_decoder = GzDecoder::new(&mut file);
    let mut data = Vec::new();
    if let Err(err) = gzip_decoder.read_to_end(&mut data) {
        return Err(format!("Failed to decompress file {}: {}", file_path.display(), err));
    }

    let duration = start_time.elapsed();
    println!("Decompressing the file: {:.2?}", duration);

    Ok(data)
}

fn parse_xml_data(xml_data: &[u8], file_name: &str, file_path: &Path) -> Result<Element, String> {
    let xml_data_str = match std::str::from_utf8(xml_data) {
        Ok(s) => s,
        Err(err) => return Err(format!("{}: Failed to convert decompressed data to UTF-8 string: {}", file_name, err)),
    };

    let xml_start = match xml_data_str.find("<?xml") {
        Some(start) => start,
        None => return Err(format!("{}: No XML data found in decompressed file", file_name)),
    };

    let xml_slice = &xml_data_str[xml_start..];

    let start_time_xml = Instant::now();
    let root = match Element::from_reader(Cursor::new(xml_slice.as_bytes())) {
        Ok(root) => root,
        Err(err) => return Err(format!("{}: {} is not a valid XML file: {}", file_name, file_path.display(), err)),
    };
    let duration = start_time_xml.elapsed();
    println!("Creating XML Element: {:.2?}", duration);

    Ok(root)
}

impl LiveSet {
    fn new(path: PathBuf) -> Result<Self, String> {
        let mut live_set = LiveSet {
            id: Id::default(),
            path,
            file_hash: String::new(),
            last_scan_timestamp: Utc::now(),
            file_name: String::new(),
            creation_time: Utc::now(),
            last_modification_time: Utc::now(),
            creator: String::new(),
            key_signature: KeySignature::default(),
            ableton_version: AbletonVersion::default(),
            tempo: 0.0,
            time_signature: TimeSignature::default(),
            estimated_duration: chrono::Duration::zero(),
            furthest_bar: 0,
            plugins: HashSet::new(),
            samples: HashSet::new(),
            xml_root: None,
        };

        match live_set.load_xml_data() {
            Ok(_) => Ok(live_set),
            Err(err) => Err(err),
        }


    }

    fn load_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.path);

        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{}: is not a valid Ableton Live Set file", self.file_name));
        }

        let decompressed_data = match decompress_file(&path) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        let root = match parse_xml_data(&decompressed_data, &self.file_name, &path) {
            Ok(root) => root,
            Err(err) => return Err(err),
        };

        self.xml_root = Some(root);

        Ok(())
    }
}

fn main() {
    let path: PathBuf = r"C:\Users\judee\Documents\Projects\AMAPIANO 3 Project\amapiano4.als".into();
    let start_time = Instant::now();
    let live_set_result = LiveSet::new(path);
    let end_time = Instant::now();
    let duration = end_time - start_time;
    let duration_ms = duration.as_secs_f64() * 1000.0;
    match live_set_result {
        Ok(_) => println!("Success loading Live set. Time taken: {:.2} ms", duration_ms),
        Err(err) => eprintln!("Error: {}", err),
    }
}