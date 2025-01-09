
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{error::{DatabaseError, FileError}, models::PluginFormat};

// LINE TRACKER FOR DEBUGGING

#[derive(Clone)]
pub (crate) struct LineTrackingBuffer {
    data: Arc<Vec<u8>>,
    current_line: usize,
    current_position: usize,
}

impl LineTrackingBuffer {
    pub (crate) fn new(data: Vec<u8>) -> Self {
        Self {
            data: Arc::new(data),
            current_line: 1,
            current_position: 0,
        }
    }

    pub (crate) fn get_line_number(&mut self, byte_position: usize) -> usize {
        while self.current_position < byte_position && self.current_position < self.data.len() {
            if self.data[self.current_position] == b'\n' {
                self.current_line += 1;
            }
            self.current_position += 1;
        }
        self.current_line
    }

    pub (crate) fn update_position(&mut self, byte_position: usize) {
        self.get_line_number(byte_position);
    }
}

pub(crate) fn get_most_recent_db_file(directory: &PathBuf) -> Result<PathBuf, DatabaseError> {
    fs::read_dir(directory)
        .map_err(|_| FileError::NotFound(directory.clone()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("db") {
                entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .map(|modified| (path, modified))
            } else {
                None
            }
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path)
        .ok_or_else(|| FileError::NotFound(directory.clone()))
        .and_then(|path| {
            if path.is_file() {
                Ok(path)
            } else {
                Err(FileError::NotAFile(path))
            }
        })
        .map_err(DatabaseError::FileError)
}

pub(crate) fn parse_plugin_format(dev_identifier: &str) -> Option<PluginFormat> {
    if dev_identifier.starts_with("device:vst3:instr:") {
        Some(PluginFormat::VST3Instrument)
    } else if dev_identifier.starts_with("device:vst3:audiofx:") {
        Some(PluginFormat::VST3AudioFx)
    } else if dev_identifier.starts_with("device:vst:instr:") {
        Some(PluginFormat::VST2Instrument)
    } else if dev_identifier.starts_with("device:vst:audiofx:") {
        Some(PluginFormat::VST2AudioFx)
    } else {
        None
    }
}