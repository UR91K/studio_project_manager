use std::collections::HashSet;
use std::path;
use std::path::Path;
use chrono::{DateTime, Utc};
use uuid::Uuid;

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
    path: Path,
    is_present: bool
}

#[derive(Debug)]
struct LiveSet {
    uuid: Uuid,
    identifier: u64,
    path: path::PathBuf,
    file_hash: String,
    last_scan_timestamp: DateTime<Utc>,
    name: String,
    creation_time: DateTime<Utc>,
    last_modification_time: DateTime<Utc>,
    creator: String,
    key: String,
    major_version: u32,
    minor_version: u32,
    ableton_version: AbletonVersion,
    tempo: f32,
    time_signature: TimeSignature,
    estimated_duration: chrono::Duration,
    furthest_bar: u32,
    plugins: HashSet<Id>,
    samples: HashSet<Id>,
}

fn main() {}