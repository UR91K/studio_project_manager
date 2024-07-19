use std::fs;
use std::io::BufRead;
use std::path::PathBuf;

use colored::*;
use log::debug;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::{DatabaseError, FileError, PluginError, XmlParseError};
use crate::models::{Plugin, PluginFormat, PluginInfo};
use crate::utils::xml_parsing::{parse_event_attributes, read_value};
use crate::utils::StringResultExt;
use crate::{debug_fn, trace_fn};

pub(crate) fn find_all_plugins(xml_data: &[u8]) -> Result<Vec<Plugin>, PluginError> {
    trace_fn!("find_all_plugins", "Starting find_all_plugins function");
    let plugin_infos = find_plugin_tags(xml_data)?;
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

    let mut plugins = Vec::with_capacity(plugin_infos.len());
    for (index, info) in plugin_infos.iter().enumerate() {
        trace_fn!(
            "find_all_plugins",
            "Processing plugin info {}: {:?}",
            index,
            info.dev_identifier
        );
        let db_plugin = ableton_db.get_plugin_by_dev_identifier(&info.dev_identifier)?;
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
                    dev_identifier: db_plugin.dev_identifier,
                    name: db_plugin.name,
                    vendor: db_plugin.vendor,
                    version: db_plugin.version,
                    sdk_version: db_plugin.sdk_version,
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
                    dev_identifier: info.dev_identifier.clone(),
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
        plugins.push(plugin);
    }

    debug!("Found {} plugins", plugins.len());
    Ok(plugins)
}

#[derive(Debug, Default, Clone)]
struct SourceContext {
    branch_device_id: Option<String>,
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

pub(crate) fn find_plugin_tags(xml_data: &[u8]) -> Result<Vec<PluginInfo>, XmlParseError> {
    trace_fn!("find_plugin_tags", "Starting function");
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut plugin_info_tags = Vec::new();
    let mut current_source_context: Option<SourceContext> = None;
    let mut depth = 0;
    let mut in_plugin_desc = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = event.name().to_string_result()?;

                depth += 1;

                match name.as_str() {
                    "SourceContext" => {
                        trace_fn!("find_plugin_tags", "Found SourceContext event");
                        current_source_context = parse_source_context(&mut reader, &mut depth)?;
                    }

                    "PluginDesc" => {
                        trace_fn!("find_plugin_tags", "Entered PluginDesc event");
                        in_plugin_desc = true;
                    }

                    "VstPluginInfo" | "Vst3PluginInfo" if in_plugin_desc => {
                        trace_fn!("find_plugin_tags", "Found PluginInfo event");
                        if let Some(source_context) = current_source_context.take() {
                            if let Some(plugin_info) =
                                parse_plugin_info(&source_context, &mut reader, &mut depth)?
                            {
                                debug_fn!("find_plugin_tags", "Found plugin: {:?}", plugin_info);
                                plugin_info_tags.push(plugin_info);
                            } else {
                                trace_fn!("find_plugin_tags", "Plugin info parsed but not valid");
                            }
                        } else {
                            trace_fn!(
                                "find_plugin_tags",
                                "No valid SourceContext found for plugin"
                            );
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref event)) => {
                let name = event.name().to_string_result()?;
                depth -= 1;

                if name == "PluginDesc" {
                    trace_fn!("find_plugin_tags", "Exited PluginDesc event");
                    in_plugin_desc = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                trace_fn!("find_plugin_tags", "Error parsing XML: {:?}", e);
                return Err(XmlParseError::QuickXmlError(e));
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

fn parse_source_context<R: BufRead>(
    reader: &mut Reader<R>,
    depth: &mut i32,
) -> Result<Option<SourceContext>, XmlParseError> {
    trace_fn!("parse_source_context", "Starting function");
    let mut buf = Vec::new();
    let start_depth = *depth;
    let mut in_value = false;
    let mut in_branch_source_context = false;
    let mut source_context = SourceContext::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                *depth += 1;
                let name = event.name().to_string_result()?;
                match name.as_str() {
                    "Value" => in_value = true,
                    "BranchSourceContext" if in_value => in_branch_source_context = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref event)) if in_branch_source_context => {
                let name = event.name().to_string_result()?;
                if name == "BranchDeviceId" {
                    let attributes = parse_event_attributes(event)?;
                    if let Some(value) = attributes.get("Value") {
                        trace_fn!("parse_source_context", "Found BranchDeviceId: {:?}", value);
                        source_context.branch_device_id = Some(value.clone());
                        break;
                    }
                }
            }
            Ok(Event::End(ref event)) => {
                let name = event.name().to_string_result()?;
                match name.as_str() {
                    "Value" => in_value = false,
                    "BranchSourceContext" => in_branch_source_context = false,
                    "SourceContext" if *depth == start_depth => break,
                    _ => {}
                }
                *depth -= 1;
            }
            Ok(Event::Eof) => return Err(XmlParseError::InvalidStructure),
            Err(e) => return Err(XmlParseError::QuickXmlError(e)),
            _ => (),
        }
    }

    if let Some(branch_device_id) = &source_context.branch_device_id {
        trace_fn!(
            "parse_source_context",
            "Found valid SourceContext with BranchDeviceId: {:?}",
            branch_device_id
        );
        Ok(Some(source_context))
    } else {
        trace_fn!("parse_source_context", "No valid BranchDeviceId found");
        Ok(None)
    }
}

fn parse_plugin_info<R: BufRead>(
    source_context: &SourceContext,
    reader: &mut Reader<R>,
    depth: &mut i32,
) -> Result<Option<PluginInfo>, XmlParseError> {
    trace_fn!("parse_plugin_info", "Starting function");

    let dev_identifier = match &source_context.branch_device_id {
        Some(id) => id,
        None => return Ok(None),
    };
    trace_fn!(
        "parse_plugin_info",
        "Found dev_identifier: {:?}",
        dev_identifier
    );

    let plugin_format = match parse_plugin_format(dev_identifier) {
        Some(format) => {
            trace_fn!(
                "parse_plugin_info",
                "Successfully parsed plugin format: {:?}",
                format
            );
            format
        }
        None => {
            trace_fn!(
                "parse_plugin_info",
                "Unable to determine plugin format for dev_identifier: {}",
                dev_identifier
            );
            return Ok(None);
        }
    };

    let mut buf = Vec::new();
    let mut name = String::new();
    let start_depth = *depth;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                *depth += 1;
                let tag_name = event.name().to_string_result()?;
                if matches!(tag_name.as_str(), "PlugName" | "Name") {
                    trace_fn!(
                        "parse_plugin_info",
                        "Found PlugName: {:?}",
                        read_value(reader)?
                    );

                    name = read_value(reader)?;
                }
            }
            Ok(Event::End(ref _e)) => {
                *depth -= 1;
                if *depth == start_depth {
                    break;
                }
            }
            Ok(Event::Eof) => return Err(XmlParseError::InvalidStructure),
            Err(e) => return Err(XmlParseError::QuickXmlError(e)),
            _ => (),
        }
    }
    trace_fn!(
        "parse_plugin_info",
        "Found plugin: {} ({:?})",
        name,
        plugin_format
    );

    Ok(Some(PluginInfo {
        name,
        dev_identifier: dev_identifier.to_string(),
        plugin_format,
    }))
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
