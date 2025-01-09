mod plugin;
mod state;

pub use state::ScannerState;
pub use plugin::*;  // We'll make this more specific once we determine exactly which functions should be public

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use quick_xml::Reader;
use parking_lot;

use crate::error::LiveSetError;
use crate::models::{Id, Plugin, PluginInfo, Sample, TimeSignature};
use crate::utils::plugins::LineTrackingBuffer;

/// Configuration for what should be scanned
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub scan_plugins: bool,
    pub scan_samples: bool,
    pub scan_tempo: bool,
    pub scan_time_signature: bool,
    pub scan_midi: bool,
    pub scan_audio: bool,
    pub scan_automation: bool,
    pub scan_return_tracks: bool,
    pub scan_master_track: bool,
    pub estimate_duration: bool,
    pub calculate_furthest_bar: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            scan_plugins: true,
            scan_samples: true,
            scan_tempo: true,
            scan_time_signature: true,
            scan_midi: true,
            scan_audio: true,
            scan_automation: true,
            scan_return_tracks: true,
            scan_master_track: true,
            estimate_duration: true,
            calculate_furthest_bar: true,
        }
    }
}

/// Holds the results of the scanning process
#[derive(Default)]
pub struct ScanResult {
    pub samples: HashSet<Sample>,
    pub plugins: HashSet<Plugin>,
    pub tempo: Option<f64>,
    pub time_signature: Option<TimeSignature>,
    pub furthest_bar: Option<f64>,
}

/// The main scanner that processes the XML data
pub struct Scanner {
    // Core scanner state
    pub(crate) state: ScannerState,
    pub(crate) depth: i32,
    pub(crate) ableton_version: u32,
    pub(crate) options: ScanOptions,
    pub(crate) line_tracker: LineTrackingBuffer,
    
    // Sample scanning state
    pub(crate) sample_paths: HashSet<PathBuf>,
    pub(crate) current_sample_data: Option<String>,
    
    // Plugin scanning state
    pub(crate) dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>>,
    pub(crate) current_branch_info: Option<String>,
    pub(crate) plugin_info_tags: HashMap<String, PluginInfo>,
    pub(crate) in_source_context: bool,
    pub(crate) plugin_info_processed: bool,
    
    // Tempo scanning state
    pub(crate) current_tempo: Option<f64>,
    
    // Time signature scanning state
    pub(crate) current_time_signature: Option<TimeSignature>,
} 