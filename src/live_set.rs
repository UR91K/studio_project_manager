// /src/live_set.rs
use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Local};

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::LiveSetError;
use crate::{debug_fn, info_fn};
use crate::models::{AbletonVersion, Id, KeySignature, Plugin, Sample, TimeSignature};
use crate::utils::metadata::{load_file_hash, load_file_name, load_file_timestamps};
use crate::utils::plugins::get_most_recent_db_file;
use crate::utils::{decompress_gzip_file, validate_ableton_file};
use crate::scan::{Scanner, ScanOptions as ScannerOptions};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LiveSet {
    id: Id,
    file_path: PathBuf,
    file_name: String,
    file_hash: String,
    created_time: DateTime<Local>,
    modified_time: DateTime<Local>,
    xml_data: Vec<u8>,
    last_scan_timestamp: DateTime<Local>,

    ableton_version: AbletonVersion,
    
    key_signature: Option<KeySignature>,
    tempo: f64,
    time_signature: TimeSignature,
    furthest_bar: Option<f64>,
    plugins: HashSet<Plugin>,
    samples: HashSet<Sample>,
    
    estimated_duration: Option<chrono::Duration>,
}

impl LiveSet {
    pub fn new(file_path: PathBuf) -> Result<Self, LiveSetError> {
        validate_ableton_file(&file_path)?;

        let file_name: String = load_file_name(&file_path)?;
        let (modified_time, created_time) = load_file_timestamps(&file_path)?;
        let file_hash: String = load_file_hash(&file_path)?;
        let xml_data: Vec<u8> = decompress_gzip_file(&file_path)?;

        // Use our new scanner to load everything
        let scanner_options = ScannerOptions::default();
        let mut scanner = Scanner::new(&xml_data, scanner_options)?;
        let scan_result = scanner.scan(&xml_data)?;

        //TODO: don't use defaults, make sure project can only ever be valid data from the file
        let mut live_set = LiveSet {
            id: Id::default(),
            file_path,
            file_name,
            file_hash,
            created_time,
            modified_time,
            xml_data,
            last_scan_timestamp: Local::now(),

            ableton_version: scan_result.version,
            key_signature: scan_result.key_signature,
            tempo: scan_result.tempo,
            time_signature: scan_result.time_signature,
            furthest_bar: scan_result.furthest_bar,
            plugins: scan_result.plugins,
            samples: scan_result.samples,

            estimated_duration: None,
        };

        live_set.calculate_duration()?;
        
        Ok(live_set)
    }

    pub fn calculate_duration(&mut self) -> Result<(), LiveSetError> {
        if let (tempo, Some(furthest_bar)) = (self.tempo, self.furthest_bar) {
            let beats_per_second = tempo / 60.0;
            let total_seconds = furthest_bar * 4.0 / beats_per_second;
            self.estimated_duration = Some(Duration::seconds(total_seconds as i64));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn rescan_plugins(&mut self) -> Result<(), LiveSetError> {
        let config = CONFIG
            .as_ref()
            .map_err(|e| LiveSetError::ConfigError(e.clone()))?;
        let db_dir = &config.live_database_dir;
        let ableton_db = AbletonDatabase::new(
            get_most_recent_db_file(&PathBuf::from(db_dir)).map_err(LiveSetError::DatabaseError)?,
        )
        .map_err(LiveSetError::DatabaseError)?;

        let mut updated_plugins = HashSet::new();

        for plugin in self.plugins.iter() {
            let mut updated_plugin = plugin.clone();
            updated_plugin
                .rescan(&ableton_db)
                .map_err(|e| LiveSetError::DatabaseError(e))?;
            updated_plugins.insert(updated_plugin);
        }

        self.plugins = updated_plugins;

        Ok(())
    }

    pub fn log_info(&self) {
        info_fn!(
            "log_info",
            "{} - {} plugins, {} samples",
            self.file_name.bold().purple(),
            self.plugins.len(),
            self.samples.len()
        );

        if let Some(duration) = self.estimated_duration {
            debug_fn!(
                "log_info",
                "Estimated duration: {}",
                duration.to_string().green()
            );
        }
    }
}