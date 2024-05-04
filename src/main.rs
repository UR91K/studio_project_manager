use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use xmltree::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Id(u64);

#[derive(Debug)]
struct TimeSignature {
    numerator: u32,
    denominator: u32,
}

#[derive(Debug)]
struct AbletonVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl fmt::Display for AbletonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ableton {}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Scale {
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
    _xml_root: Option<Element>, // Private field to hold XML data
}

impl LiveSet {
    fn new(path: PathBuf) -> Result<Self, String> {
        let mut live_set = LiveSet {
            id: Id::new(),
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
            _xml_root: None,
        };

        // Attempt to load XML data
        match live_set.load_xml_data() {
            Ok(_) => Ok(live_set), // Return the initialized LiveSet if XML data loading succeeds
            Err(err) => Err(err), // Return an error if XML data loading fails
        }
    }

    fn load_xml_data(&mut self) -> Result<(), String> {
        let path = Path::new(&self.path);
        if !path.exists() || !path.is_file() || path.extension().unwrap_or_default() != "als" {
            return Err(format!("{}: is not a valid Ableton Live Set file", self.file_name));
        }

        if self.last_modification_time.is_none() || self.creation_time.is_none() {
            eprintln!("{}: Missing file times", self.file_name);
            self.update_file_times();
        }

        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(err) => return Err(format!("{}: Failed to open file {}: {}", self.file_name, path.display(), err)),
        };

        let mut data = Vec::new();
        if let Err(err) = file.read_to_end(&mut data) {
            return Err(format!("{}: Failed to read file {}: {}", self.file_name, path.display(), err));
        }

        match Element::parse(data.as_slice()) {
            Ok(root) => {
                self._xml_root = Some(root);
                Ok(())
            }
            Err(err) => Err(format!("{}: {} is not a valid XML file: {}", self.file_name, path.display(), err)),
        }
    }
}


fn main() {

}