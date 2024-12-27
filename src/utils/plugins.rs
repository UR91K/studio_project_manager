use std::collections::HashMap;

#[allow(unused_imports)]
use log::{debug, trace};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{DatabaseError, FileError, PluginError, XmlParseError};
use crate::models::{Plugin, PluginFormat, PluginInfo};
use crate::utils::xml_parsing::get_value_as_string_result;
use crate::utils::{EventExt, StringResultExt};
use crate::{debug_fn, error_fn, trace_fn, warn_fn};

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

// PLUGIN SPECIFIC HELPERS

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

// PARENT FUNCTION

pub(crate) fn find_all_plugins(xml_data: &[u8]) -> Result<HashMap<String, Plugin>, PluginError> {
    debug_fn!("find_all_plugins", "{}", "Starting function".bold().purple());
    let plugin_infos: HashMap<String, PluginInfo> = find_plugin_tags(xml_data)?;
    trace_fn!(
        "find_all_plugins",
        "Found {} plugin infos",
        plugin_infos.len()
    );

    let config = CONFIG
        .as_ref()
        .map_err(|e| PluginError::ConfigError(e.clone()))?;
    let db_dir = &config.live_database_dir;
    trace_fn!("find_all_plugins", "Database directory: {:?}", db_dir);

    let db_path =
        get_most_recent_db_file(&PathBuf::from(db_dir)).map_err(PluginError::DatabaseError)?;
    trace_fn!("find_all_plugins", "Using database file: {:?}", db_path);

    let ableton_db = AbletonDatabase::new(db_path).map_err(PluginError::DatabaseError)?;
    
    let mut unique_plugins: HashMap<String, Plugin> = HashMap::new();
    
    for (dev_identifier, info) in plugin_infos.iter() {
        trace_fn!(
        "find_all_plugins",
        "Processing plugin info: {:?}",
        dev_identifier
    );
        let db_plugin = ableton_db.get_plugin_by_dev_identifier(dev_identifier)?;
        let plugin = match db_plugin {
            Some(db_plugin) => {
                debug_fn!(
                "find_all_plugins",
                "Found plugin {} {} on system, flagging as installed",
                db_plugin.vendor.as_deref().unwrap_or("Unknown").purple(),
                db_plugin.name.green()
            );
                Plugin {
                    plugin_id: Some(db_plugin.plugin_id),
                    module_id: db_plugin.module_id,
                    dev_identifier: db_plugin.dev_identifier.clone(),
                    name: db_plugin.name.clone(),
                    vendor: db_plugin.vendor.clone(),
                    version: db_plugin.version.clone(),
                    sdk_version: db_plugin.sdk_version.clone(),
                    flags: db_plugin.flags,
                    scanstate: db_plugin.scanstate,
                    enabled: db_plugin.enabled,
                    plugin_format: info.plugin_format,
                    installed: true,
                }
            }
            None => {
                debug!("Plugin not found in database: {:?}", info);
                Plugin {
                    plugin_id: None,
                    module_id: None,
                    dev_identifier: dev_identifier.clone(),
                    name: info.name.clone(),
                    vendor: None,
                    version: None,
                    sdk_version: None,
                    flags: None,
                    scanstate: None,
                    enabled: None,
                    plugin_format: info.plugin_format,
                    installed: false,
                }
            }
        };
        unique_plugins
            .entry(plugin.dev_identifier.clone())
            .and_modify(|existing| {
                if existing.plugin_id.is_none() && plugin.plugin_id.is_some() {
                    *existing = plugin.clone();
                }
            })
            .or_insert(plugin);
    }
    
    debug!("Found {} plugins", unique_plugins.len());
    
    Ok(unique_plugins)
}

// XML PARSING

pub(crate) fn find_plugin_tags(xml_data: &[u8]) -> Result<HashMap<String, PluginInfo>, XmlParseError> {
    trace_fn!("find_plugin_tags", "{}", "Starting function".bold().purple());
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut plugin_info_tags: HashMap<String, PluginInfo> = HashMap::new();
    let dev_identifiers: Arc<parking_lot::RwLock<HashMap<String, ()>>> = Arc::new(parking_lot::RwLock::new(HashMap::new()));
    
    let mut depth = 0;
    let mut current_branch_info: Option<String> = None;
    let mut in_source_context = false;
    let mut line_tracker = LineTrackingBuffer::new(xml_data.to_vec());

    loop {
        let mut byte_pos = reader.buffer_position();
        let line = line_tracker.get_line_number(byte_pos);

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = event.name().to_string_result()?;
                depth += 1;

                match name.as_str() {
                    "SourceContext" => {
                        // trace_fn!("find_plugin_tags", "Found SourceContext at line {}", line);
                        in_source_context = true;
                    }

                    "BranchSourceContext" => {
                        trace_fn!(
                            "find_plugin_tags",
                            "[{}] Found BranchSourceContext event",
                            line.to_string().yellow()
                        );

                        if in_source_context {
                            trace_fn!(
                                "find_plugin_tags",
                                "[{}] In SourceContext, parsing BranchSourceContext",
                                line.to_string().yellow()
                            );
                            current_branch_info = parse_branch_source_context(
                                &mut reader,
                                &mut depth,
                                &mut byte_pos,
                                &mut line_tracker,
                                &dev_identifiers,
                            )?;
                        }
                    }

                    "PluginDesc" => {
                        if let Some(device_id) = current_branch_info.take() {
                            trace_fn!(
                                "find_plugin_tags",
                                "[{}] Found PluginDesc, parsing...",
                                line.to_string().yellow()
                            );

                            if let Some(plugin_info) = parse_plugin_desc(
                                &mut reader,
                                &mut depth,
                                device_id.clone(),
                                &mut byte_pos,
                                &mut line_tracker,
                            )? {
                                debug_fn!(
                                    "find_plugin_tags",
                                    "[{}] Found plugin: {}",
                                    line.to_string().yellow(),
                                    plugin_info
                                );
                                plugin_info_tags.insert(device_id, plugin_info);
                            }
                        }
                    }

                    _ => {}
                }
            }
            Ok(Event::End(ref event)) => {
                let name = event.name().to_string_result()?;
                if name == "SourceContext" {
                    // trace_fn!("find_plugin_tags", "Exited SourceContext at line {}", line);
                    in_source_context = false;
                }
                if name == "PluginDesc" {
                    // trace_fn!("find_plugin_tags", "Exited PluginDesc at line {}", line);
                }
                depth -= 1;
            }

            Ok(Event::Eof) => break,

            Err(error) => {
                trace_fn!(
                    "find_plugin_tags",
                    "[{}] Error parsing XML: {:?}",
                    line.to_string().yellow(),
                    error
                );
                return Err(XmlParseError::QuickXmlError(error));
            }
            _ => (),
        }
        buf.clear();
    }

    debug_fn!(
        "find_plugin_tags",
        "Found {} plugin info tags",
        plugin_info_tags.len()
    );
    Ok(plugin_info_tags)
}

fn parse_branch_source_context<R: BufRead>(
    reader: &mut Reader<R>,
    depth: &mut i32,
    byte_pos: &mut usize,
    line_tracker: &mut LineTrackingBuffer,
    dev_identifiers: &Arc<parking_lot::RwLock<HashMap<String, ()>>>,
) -> Result<Option<String>, XmlParseError> {
    
    let mut line = line_tracker.get_line_number(*byte_pos);
    trace_fn!(
        "parse_branch_source_context",
        "[{}] {}, depth: {}",
        line.to_string().yellow(),
        "Starting function".bold().purple(),
        depth.to_string().red()
    );

    let mut buf = Vec::new();
    let mut browser_content_path = true;
    let mut branch_device_id = None;
    let mut read_count = 0;

    loop {
        *byte_pos = reader.buffer_position();
        line = line_tracker.get_line_number(*byte_pos);

        if read_count > 2 && !browser_content_path {
            trace_fn!(
                "parse_branch_source_context",
                "[{}] No BrowserContentPath found within first 2 reads; returning early",
                line.to_string().yellow()
            );
            return Ok(None);
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref _event)) => {
                read_count += 1;
                *depth += 1;
            }

            Ok(Event::Empty(ref event)) => {
                read_count += 1;

                let name = event.name().to_string_result()?;
                match name.as_str() {
                    "BranchDeviceId" => {
                        if let Some(device_id) = event.get_value_as_string_result()? {
                            let mut identifiers = dev_identifiers.write();
                            if identifiers.contains_key(&device_id) {
                                trace_fn!(
                                "parse_branch_source_context",
                                "[{}] Duplicate device ID found: {}",
                                line.to_string().yellow(),
                                device_id
                            );
                                return Ok(None);
                            } else {
                                identifiers.insert(device_id.clone(), ());
                                branch_device_id = Some(device_id);
                            }
                        }
                        break;
                    }
                    
                    "BrowserContentPath" => {
                        trace_fn!(
                            "parse_branch_source_context",
                            "[{}] Found BrowserContentPath",
                            line.to_string().yellow()
                        );
                        browser_content_path = true;
                    }
                    
                    _ => {}
                }
            }

            Ok(Event::End(ref _event)) => {
                *depth -= 1;
                // warn_fn!(
                //     "parse_branch_source_context",
                //     "Found unexpected End event {}",
                //     line
                // );
                break;
            }

            Ok(Event::Eof) => {
                warn_fn!(
                    "parse_branch_source_context",
                    "[{}] EOF reached",
                    line.to_string().yellow()
                );
                return Err(XmlParseError::InvalidStructure);
            }

            Err(e) => {
                error_fn!(
                    "parse_branch_source_context",
                    "[{}] Error parsing XML: {:?}",
                    line,
                    e
                );
                return Err(XmlParseError::QuickXmlError(e));
            }

            _ => (),
        }
    }

    *byte_pos = reader.buffer_position();
    line_tracker.update_position(*byte_pos);

    if !browser_content_path {
        trace_fn!("parse_branch_source_context", "Missing BrowserContentPath");
        return Ok(None);
    }

    let Some(device_id) = branch_device_id else {
        // trace_fn!("parse_branch_source_context", "Missing BranchDeviceId");
        return Ok(None);
    };

    if !device_id.starts_with("device:vst:") && !device_id.starts_with("device:vst3:") {
        trace_fn!("parse_branch_source_context", "Not a VST/VST3 plugin");
        return Ok(None);
    }

    let vst_type = if device_id.contains("vst3:") {
        "VST 3"
    } else if device_id.contains("vst:") {
        "VST 2"
    } else {
        ""
    };

    trace_fn!(
        "parse_branch_source_context",
        "[{}] Found valid {} plugin info",
        line.to_string().yellow(),
        vst_type,
    );
    Ok(Some(device_id))
}

fn parse_plugin_desc<R: BufRead>(
    reader: &mut Reader<R>,
    depth: &mut i32,
    device_id: String,
    byte_pos: &mut usize,
    line_tracker: &mut LineTrackingBuffer,
) -> Result<Option<PluginInfo>, XmlParseError> {
    let mut line = line_tracker.get_line_number(*byte_pos);
    trace_fn!(
        "parse_plugin_desc", 
        "[{}] Starting function", 
        line.to_string().yellow()
    );

    let mut buf = Vec::new();
    let start_depth = *depth;
    // trace!("start depth: {}", start_depth);
    let plugin_name;

    loop {
        *byte_pos = reader.buffer_position();
        line = line_tracker.get_line_number(*byte_pos);

        match reader.read_event_into(&mut buf)? {
            Event::Start(ref _event) => {
                *depth += 1;
            }

            Event::Empty(ref event) => {
                let name = event.name().to_string_result()?;
                // if line < 50000 {
                //     trace!(
                //         "Found empty event, name: {:?}, line: {}, depth: {}",
                //         name,
                //         line,
                //         depth
                //     );
                // }

                if (name == "PlugName" || name == "Name") && *depth == start_depth + 1 {
                    if let Some(value) = get_value_as_string_result(event)? {
                        plugin_name = Some(value);

                        trace_fn!(
                            "parse_plugin_desc",
                            "[{}] Found plugin name: {}",
                            line.to_string().yellow(),
                            plugin_name.as_deref().unwrap_or("None"),
                        );
                        break;
                    }
                }
            }

            Event::End(ref _event) => {
                *depth -= 1;
                // warn_fn!("parse_plugin_desc", "[{}] Found unexpected end tag in PluginDesc; could indicate failure to parse plugin name", line);
                // break;
            }

            Event::Eof => {
                error_fn!(
                    "parse_plugin_desc",
                    "Found unexpected end of file while parsing"
                );
                return Err(XmlParseError::InvalidStructure);
            }

            _ => (),
        }
    }

    *byte_pos = reader.buffer_position();
    line_tracker.update_position(*byte_pos);

    if let Some(name) = plugin_name {
        let plugin_format = parse_plugin_format(&device_id)
            .ok_or_else(|| XmlParseError::UnknownPluginFormat(device_id.clone()))?;

        let plugin_info = Some(PluginInfo {
            name,
            dev_identifier: device_id,
            plugin_format,
        });
        trace_fn!(
            "parse_plugin_desc",
            "[{}] Successfully collected plugin info: {}",
            line.to_string().yellow(),
            match &plugin_info {
                Some(info) => info.to_string(),
                None => "No plugin info available".to_string(),
            }
        );

        Ok(plugin_info)
    } else {
        warn_fn!("parse_plugin_desc", "Missing plugin name");
        Ok(None)
    }
}
