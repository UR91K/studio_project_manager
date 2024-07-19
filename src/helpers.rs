use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::str::{from_utf8, FromStr};

use anyhow::Result;
use chrono::{DateTime, Local};
use colored::*;
use crc32fast::Hasher;
use encoding_rs::UTF_16LE;
use flate2::read::GzDecoder;
use log::{debug, error, info, trace, warn};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::Reader;

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::custom_types::{
    AbletonVersion, Plugin, PluginFormat, PluginInfo, TimeSignature, XmlTag,
};
use crate::errors::{
    AttributeError, DatabaseError, FileError, LiveSetError, PluginError, SampleError, TempoError,
    TimeSignatureError, VersionError, XmlParseError,
};

#[macro_export]
macro_rules! trace_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::trace!("[{}] {}", $fn_name.bright_blue().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! debug_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::debug!("[{}] {}", $fn_name.cyan().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! info_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::info!("[{}] {}", $fn_name.green().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! warn_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::warn!("[{}] {}", $fn_name.yellow().bold(), format!($($arg)+))
    };
}

#[macro_export]
macro_rules! error_fn {
    ($fn_name:expr, $($arg:tt)+) => {
        log::error!("[{}] {}", $fn_name.red().bold(), format!($($arg)+))
    };
}

//ACTUAL HELPERS

pub(crate) trait StringResultExt {
    fn to_string_result(&self) -> Result<String, XmlParseError>;
    fn to_str_result(&self) -> Result<&str, XmlParseError>;
}

impl<'a> StringResultExt for QName<'a> {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        self.to_str_result().map(String::from)
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        from_utf8(self.as_ref()).map_err(XmlParseError::Utf8Error)
    }
}

impl StringResultExt for &[u8] {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        from_utf8(self)
            .map(String::from)
            .map_err(XmlParseError::Utf8Error)
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        from_utf8(self).map_err(XmlParseError::Utf8Error)
    }
}

impl<'a> StringResultExt for Cow<'a, [u8]> {
    fn to_string_result(&self) -> Result<String, XmlParseError> {
        String::from_utf8(self.to_vec()).map_err(|e| XmlParseError::Utf8Error(e.utf8_error()))
    }

    fn to_str_result(&self) -> Result<&str, XmlParseError> {
        match self {
            Cow::Borrowed(bytes) => from_utf8(bytes).map_err(XmlParseError::Utf8Error),
            Cow::Owned(vec) => from_utf8(vec).map_err(XmlParseError::Utf8Error),
        }
    }
}

pub(crate) fn validate_ableton_file(file_path: &Path) -> Result<(), FileError> {
    if !file_path.exists() {
        return Err(FileError::NotFound(file_path.to_path_buf()));
    }

    if !file_path.is_file() {
        return Err(FileError::NotAFile(file_path.to_path_buf()));
    }

    if file_path.extension().unwrap_or_default() != "als" {
        return Err(FileError::InvalidExtension(file_path.to_path_buf()));
    }

    Ok(())
}

/// Formats a file size in bytes to a human-readable string (B, KB, MB, or GB).
///
/// # Examples
///
/// ```
/// use studio_project_manager::helpers::format_file_size;
///
/// assert_eq!(format_file_size(1023), "1023 B");
/// assert_eq!(format_file_size(1024), "1.00 KB");
/// assert_eq!(format_file_size(1_048_576), "1.00 MB");
/// assert_eq!(format_file_size(1_073_741_824), "1.00 GB");
/// ```
pub(crate) fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    let formatted = if size < KB {
        format!("{} B", size)
    } else if size < MB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else if size < GB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else {
        format!("{:.2} GB", size as f64 / GB as f64)
    };

    formatted
}

/// Decompresses a gzip file and returns its contents as a byte vector.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use studio_project_manager::helpers::decompress_gzip_file;
///
/// let file_path = Path::new("path/to/compressed/file.gz");
/// let decompressed_data = decompress_gzip_file(&file_path).expect("Failed to decompress file");
/// println!("Decompressed {} bytes", decompressed_data.len());
/// ```
pub(crate) fn decompress_gzip_file(file_path: &Path) -> Result<Vec<u8>, FileError> {
    info!("Attempting to extract gzipped data from: {:?}", file_path);
    trace!("Opening file for gzip decompression");

    let file = File::open(file_path).map_err(|error| {
        error!(
            "Failed to open file for gzip decompression: {:?}",
            file_path
        );
        FileError::GzipDecompressionError {
            path: file_path.to_path_buf(),
            source: error,
        }
    })?;

    debug!("File opened successfully, creating GzDecoder");
    let mut gzip_decoder = GzDecoder::new(file);
    let mut decompressed_data = Vec::new();

    trace!("Beginning decompression of gzipped data");
    gzip_decoder
        .read_to_end(&mut decompressed_data)
        .map_err(|error| {
            error!("Failed to decompress gzipped data from: {:?}", file_path);
            FileError::GzipDecompressionError {
                path: file_path.to_path_buf(),
                source: error,
            }
        })?;

    let decompressed_size = decompressed_data.len();
    info!(
        "Successfully decompressed {} bytes from: {:?}",
        decompressed_size, file_path
    );
    debug!("Decompressed data size: {} bytes", decompressed_size);

    Ok(decompressed_data)
}

pub(crate) fn find_tags(
    xml_data: &[u8],
    search_queries: &[&str],
    target_depth: u8,
) -> Result<HashMap<String, Vec<Vec<XmlTag>>>, XmlParseError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut all_tags: HashMap<String, Vec<Vec<XmlTag>>> = HashMap::new();
    let mut current_tags: HashMap<String, Vec<XmlTag>> = HashMap::new();

    let mut in_target_tag = false;
    let mut depth: u8 = 0;
    let mut current_query = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = event.name().to_string_result()?;

                if search_queries.contains(&name.as_str()) {
                    in_target_tag = true;
                    depth = 0;
                    current_query = name.to_string();
                    current_tags.entry(current_query.clone()).or_default();
                } else if in_target_tag {
                    depth += 1;
                }
            }

            Ok(Event::Empty(ref event)) => {
                if in_target_tag && depth == target_depth {
                    let name = event.name().to_string_result()?;
                    let mut attributes = Vec::new();
                    for attr_result in event.attributes() {
                        let attr = attr_result.map_err(XmlParseError::AttrError)?;
                        let key = attr.key.as_ref().to_string_result()?;
                        let value = attr.value.to_string_result()?;
                        attributes.push((key, value));
                    }
                    current_tags
                        .get_mut(&current_query)
                        .ok_or(XmlParseError::InvalidStructure)?
                        .push(XmlTag { name, attributes });
                }
            }

            Ok(Event::End(ref event)) => {
                let name = event.name().to_string_result()?;
                if name == current_query {
                    in_target_tag = false;
                    all_tags
                        .entry(current_query.clone())
                        .or_default()
                        .push(current_tags[&current_query].clone());
                    current_tags
                        .get_mut(&current_query)
                        .ok_or(XmlParseError::InvalidStructure)?
                        .clear();
                } else if in_target_tag {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(XmlParseError::QuickXmlError(e)),
            _ => (),
        }
        buf.clear();
    }
    Ok(all_tags)
}

fn read_value<R: BufRead>(reader: &mut Reader<R>) -> Result<String, XmlParseError> {
    let mut buf = Vec::new();
    match reader.read_event_into(&mut buf)? {
        Event::Text(e) => Ok(e
            .unescape()
            .map_err(|_| XmlParseError::InvalidStructure)?
            .to_string()),
        Event::Empty(e) | Event::Start(e) => {
            for attr in e.attributes() {
                let attr = attr.map_err(|e| XmlParseError::AttrError(e))?;
                if attr.key.as_ref() == b"Value" {
                    return Ok(attr
                        .unescape_value()
                        .map_err(XmlParseError::QuickXmlError)?
                        .to_string());
                }
            }
            Err(XmlParseError::InvalidStructure)
        }
        _ => Err(XmlParseError::InvalidStructure),
    }
}

pub(crate) fn find_attribute(
    tags: &[XmlTag],
    tag_query: &str,
    attribute_query: &str,
) -> Result<String, AttributeError> {
    trace!(
        "Searching for attribute '{}' in tag '{}'",
        attribute_query,
        tag_query
    );

    for tag in tags {
        if tag.name == tag_query {
            debug!("Found matching tag: '{}'", tag_query);
            for (key, value) in &tag.attributes {
                if key == attribute_query {
                    debug!(
                        "Found attribute '{}' with value '{}'",
                        attribute_query, value
                    );
                    return Ok(value.clone());
                }
            }
            debug!(
                "Attribute '{}' not found in tag '{}'",
                attribute_query, tag_query
            );
            return Err(AttributeError::ValueNotFound(attribute_query.to_string()));
        }
    }

    debug!("Tag '{}' not found", tag_query);
    Err(AttributeError::NotFound(tag_query.to_string()))
}

pub(crate) fn find_empty_event(
    xml_data: &[u8],
    search_query: &str,
) -> Result<HashMap<String, String>, XmlParseError> {
    debug!("Searching for empty event with query: {}", search_query);

    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(ref event)) => {
                let name = event.name().to_string_result()?;

                // trace!("Found empty event with name: {}", name);

                if name == search_query {
                    debug!("Empty event {} matches search query", name);

                    let attributes = parse_event_attributes(event)?;

                    trace!("Attributes: {:?}", attributes);
                    return Ok(attributes);
                }
            }
            Ok(Event::Eof) => {
                debug!("Reached end of XML data without finding the event");
                return Err(XmlParseError::EventNotFound(search_query.to_string()));
            }
            Err(error) => {
                debug!(
                    "Error while searching for empty event named {:?}: {:?}",
                    search_query, error
                );
                return Err(XmlParseError::QuickXmlError(error));
            }
            _ => (),
        }
        buffer.clear();
    }
}

fn parse_event_attributes(event: &BytesStart) -> Result<HashMap<String, String>, XmlParseError> {
    let mut attributes = HashMap::new();
    for attribute_result in event.attributes() {
        let attribute = attribute_result.map_err(XmlParseError::AttrError)?;
        let key = from_utf8(attribute.key.as_ref()).map_err(XmlParseError::Utf8Error)?;
        let value = from_utf8(&attribute.value).map_err(XmlParseError::Utf8Error)?;
        debug!("Found attribute: {} = {}", key, value);
        attributes.insert(key.to_string(), value.to_string());
    }
    Ok(attributes)
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

//PLUGINS

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

//SAMPLES

pub(crate) fn find_sample_path_data(xml_data: &[u8]) -> Result<Vec<String>, XmlParseError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut in_sample_ref = false;
    let mut in_data_tag = false;
    let mut data_list = Vec::new();
    let mut current_data = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                let name = from_utf8(event.name().as_ref())
                    .map_err(XmlParseError::Utf8Error)?
                    .to_string();
                if name == "SampleRef" {
                    in_sample_ref = true;
                } else if in_sample_ref && name == "Data" {
                    in_data_tag = true;
                }
            }
            Ok(Event::Text(ref event)) => {
                if in_data_tag {
                    current_data = from_utf8(event.as_ref())
                        .map_err(XmlParseError::Utf8Error)?
                        .to_string();
                }
            }
            Ok(Event::End(ref event)) => {
                let name = from_utf8(event.name().as_ref())
                    .map_err(XmlParseError::Utf8Error)?
                    .to_string();
                if name == "Data" {
                    in_data_tag = false;
                    if !current_data.is_empty() {
                        data_list.push(current_data.clone());
                        current_data.clear();
                    }
                } else if name == "SampleRef" {
                    in_sample_ref = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(XmlParseError::QuickXmlError(e)),
            _ => (),
        }
        buf.clear();
    }

    Ok(data_list)
}

pub(crate) fn parse_sample_paths(
    xml_data: &[u8],
    major_version: u32,
) -> Result<HashMap<String, Vec<PathBuf>>, SampleError> {
    let mut sample_paths: HashMap<String, Vec<PathBuf>> = HashMap::new();

    if major_version < 11 {
        debug!("Processing samples for Ableton version < 11");
        let sample_data: Vec<String> = find_sample_path_data(xml_data)?;
        let mut decoded_paths = Vec::new();
        for (index, data) in sample_data.iter().enumerate() {
            match decode_sample_path(data) {
                Ok(path) => {
                    debug!("Successfully decoded sample path {}: {:?}", index, path);
                    decoded_paths.push(path);
                }
                Err(e) => {
                    warn!("Failed to decode sample path {}: {:?}", index, e);
                    return Err(e);
                }
            }
        }
        debug!("Found {} samples for version < 11", decoded_paths.len());
        sample_paths.insert("SampleData".to_string(), decoded_paths);
    } else {
        debug!("Processing samples for Ableton version >= 11");
        let search_queries = &["SampleRef"];
        let target_depth: u8 = 1;
        let sample_tags = find_tags(xml_data, search_queries, target_depth)?;

        for (query, tags_list) in sample_tags {
            let mut paths = Vec::new();
            for (index, tags) in tags_list.iter().enumerate() {
                match find_attribute(tags, "Path", "Value") {
                    Ok(path) => {
                        debug!("Found sample path {} for '{}': {:?}", index, query, path);
                        paths.push(PathBuf::from(path));
                    }
                    Err(AttributeError::NotFound(_)) => {
                        warn!("Expected 'Path' tag not found for sample {} in '{}'. This may indicate an unexpected XML structure.", index, query);
                        continue;
                    }
                    Err(AttributeError::ValueNotFound(_)) => {
                        warn!("'Path' tag found for sample {} in '{}', but 'Value' attribute is missing. This may indicate corrupted or unexpected sample data.", index, query);
                        continue;
                    }
                }
            }
            debug!("Found {} samples for '{}'", paths.len(), query);
            sample_paths.insert(query, paths);
        }
    }

    debug!("Total sample collections found: {}", sample_paths.len());
    Ok(sample_paths)
}

fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, SampleError> {
    let abs_hash_path = abs_hash_path.replace("\\t", "").replace("\\n", "");

    let byte_data = hex::decode(&abs_hash_path).map_err(SampleError::HexDecodeError)?;

    let (cow, _, had_errors) = UTF_16LE.decode(&byte_data);
    if had_errors {
        return Err(SampleError::InvalidUtf16Encoding);
    }

    let path_string = cow.replace("\u{0}", "");
    let path = PathBuf::from(path_string);

    path.canonicalize().map_err(|e| {
        SampleError::PathProcessingError(format!("Failed to canonicalize path: {}", e))
    })
}

//TIME SIGNATURE

pub(crate) fn load_time_signature(xml_data: &[u8]) -> Result<TimeSignature, LiveSetError> {
    debug!("Updating time signature");

    let search_query = "EnumEvent";

    let event_attributes = find_empty_event(xml_data, search_query).map_err(|e| match e {
        XmlParseError::EventNotFound(_) => {
            LiveSetError::TimeSignatureError(TimeSignatureError::EnumEventNotFound)
        }
        _ => LiveSetError::XmlError(e),
    })?;

    debug!("Found time signature enum event");
    trace!("Attributes: {:?}", event_attributes);

    let value_attribute = event_attributes
        .get("Value")
        .ok_or(LiveSetError::TimeSignatureError(
            TimeSignatureError::ValueAttributeNotFound,
        ))?;

    debug!("Found 'Value' attribute");
    trace!("Value: {}", value_attribute);

    let encoded_value =
        parse_encoded_time_signature(value_attribute).map_err(LiveSetError::TimeSignatureError)?;
    debug!("Parsed encoded value: {}", encoded_value);

    let time_signature =
        TimeSignature::from_encoded(encoded_value).map_err(LiveSetError::TimeSignatureError)?;

    debug!("Decoded time signature: {:?}", time_signature);

    info!(
        "Time signature updated: {}/{}",
        time_signature.numerator, time_signature.denominator
    );

    Ok(time_signature)
}

/// Parses an encoded time signature string into an i32 value.
///
/// # Examples
///
/// ```
/// use studio_project_manager::helpers::parse_encoded_time_signature;
///
/// let result = parse_encoded_time_signature("4").unwrap();
/// assert_eq!(result, 4);
///
/// let error = parse_encoded_time_signature("invalid").unwrap_err();
/// assert!(matches!(error, TimeSignatureError::ParseEncodedError(_)));
/// ```
pub(crate) fn parse_encoded_time_signature(value: &str) -> Result<i32, TimeSignatureError> {
    trace!(
        "Attempting to parse encoded time signature value: '{}'",
        value
    );

    i32::from_str(value)
        .map(|parsed_value| {
            debug!(
                "Successfully parsed encoded value '{}' to {}",
                value, parsed_value
            );
            parsed_value
        })
        .map_err(|e| {
            error!("Failed to parse encoded value '{}': {}", value, e);
            TimeSignatureError::ParseEncodedError(e)
        })
}

//VERSION

/// Extracts the Ableton version from XML data.
///
/// # Examples
///
/// ```
/// use studio_project_manager::helpers::load_version;
///
/// let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
/// <Ableton MajorVersion="5" MinorVersion="10" SchemaChangeCount="3" Creator="Ableton Live 11.0">"#.as_bytes();
///
/// let version = load_version(xml_data).expect("Failed to load version");
/// assert_eq!(version.major_version, 5);
/// assert_eq!(version.minor_version, 10);
/// assert_eq!(version.schema_change_count, 3);
/// ```
pub(crate) fn load_version(xml_data: &[u8]) -> Result<AbletonVersion, VersionError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(_)) => continue,
            Ok(Event::Start(ref event)) => {
                let name = event.name();
                let name_str = from_utf8(name.as_ref())?;

                if name_str != "Ableton" {
                    return Err(VersionError::InvalidFileStructure(format!(
                        "First element is '{}', expected 'Ableton'",
                        name_str
                    )));
                }
                debug!("Found Ableton tag, attributes:");
                for attr_result in event.attributes() {
                    match attr_result {
                        Ok(attr) => debug!(
                            "  {}: {:?}",
                            String::from_utf8_lossy(attr.key.as_ref()),
                            String::from_utf8_lossy(&attr.value)
                        ),
                        Err(e) => debug!("  Error parsing attribute: {:?}", e),
                    }
                }
                let ableton_version = AbletonVersion::from_attributes(event.attributes())?;
                debug!("Parsed version: {:?}", &ableton_version);
                return Ok(ableton_version);
            }
            Ok(Event::Eof) => {
                return Err(VersionError::InvalidFileStructure(
                    "Reached end of file without finding Ableton tag".into(),
                ));
            }
            Ok(_) => continue,
            Err(e) => return Err(VersionError::XmlParseError(XmlParseError::QuickXmlError(e))),
        }
    }
}

// TEMPO

pub(crate) fn find_post_10_tempo(xml_data: &[u8]) -> Result<f64, TempoError> {
    let mut reader = Reader::from_reader(xml_data);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut in_tempo = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref event)) => {
                if event.name().to_string_result()? == "Tempo" {
                    in_tempo = true;
                }
            }

            Ok(Event::Empty(ref event)) if in_tempo => {
                if event.name().to_string_result()? == "Manual" {
                    for attr in event.attributes().flatten() {
                        if attr.key.to_string_result()? == "Value" {
                            return attr
                                .value
                                .as_ref()
                                .to_str_result()?
                                .parse::<f64>()
                                .map_err(|_| TempoError::InvalidTempoValue);
                        }
                    }
                }
            }

            Ok(Event::End(ref event)) if in_tempo => {
                if event.name().to_string_result()? == "Tempo" {
                    in_tempo = false;
                }
            }

            Ok(Event::Eof) => break,
            Err(error) => return Err(TempoError::XmlError(XmlParseError::QuickXmlError(error))),
            _ => (),
        }
        buf.clear();
    }

    Err(TempoError::TempoNotFound)
}

pub(crate) fn find_pre_10_tempo(xml_data: &[u8]) -> Result<f64, TempoError> {
    let search_queries = &["FloatEvent"];
    let target_depth: u8 = 0;
    let float_event_tags = find_tags(xml_data, search_queries, target_depth)?;

    if let Some(float_event_list) = float_event_tags.get("FloatEvent") {
        for tags in float_event_list {
            if !tags.is_empty() {
                if let Ok(value_str) = find_attribute(&tags[..], "FloatEvent", "Value") {
                    return value_str
                        .parse::<f64>()
                        .map_err(|_| TempoError::InvalidTempoValue);
                }
            }
        }
    }

    Err(TempoError::TempoNotFound)
}

//METADATA

pub(crate) fn load_file_timestamps(
    file_path: &PathBuf,
) -> Result<(DateTime<Local>, DateTime<Local>), FileError> {
    let metadata = fs::metadata(file_path).map_err(|e| FileError::MetadataError {
        path: file_path.clone(),
        source: e,
    })?;

    let modified_time = metadata
        .modified()
        .map(DateTime::<Local>::from)
        .map_err(|e| FileError::MetadataError {
            path: file_path.clone(),
            source: e,
        })?;

    let created_time = metadata
        .created()
        .map(DateTime::<Local>::from)
        .unwrap_or_else(|_| Local::now());

    Ok((modified_time, created_time))
}

pub(crate) fn load_file_hash(file_path: &PathBuf) -> Result<String, FileError> {
    let mut file = File::open(file_path).map_err(|e| FileError::HashingError {
        path: file_path.clone(),
        source: e,
    })?;

    let mut hasher = Hasher::new();
    let mut buffer = [0; 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| FileError::HashingError {
                path: file_path.clone(),
                source: e,
            })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    let hash_string = format!("{:08x}", hash);

    Ok(hash_string)
}

pub(crate) fn load_file_name(file_path: &PathBuf) -> Result<String, FileError> {
    file_path
        .file_name()
        .ok_or_else(|| FileError::NameError("File name is not present".to_string()))?
        .to_str()
        .ok_or_else(|| FileError::NameError("File name is not valid UTF-8".to_string()))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        let xml_data = br#"<?xml version="1.0" encoding="UTF-8"?>
        <Ableton MajorVersion="11" MinorVersion="0" SchemaChangeCount="3" Creator="Ableton Live 11.0.1" Revision="1b1951c0f4b3d5a5ad5d1ac69c3d9b5aa7a36dd8">"#;

        let version = load_version(xml_data).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 1);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_extract_version_beta() {
        let xml_data = br#"<?xml version="1.0" encoding="UTF-8"?>
        <Ableton MajorVersion="11" MinorVersion="1" SchemaChangeCount="0" Creator="Ableton Live 11.1 Beta" Revision="1b1951c0f4b3d5a5ad5d1ac69c3d9b5aa7a36dd8">"#;

        let version = load_version(xml_data).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, true);
    }

    #[test]
    fn test_extract_version_invalid_xml() {
        let xml_data = b"<Invalid>XML</Invalid>";
        assert!(load_version(xml_data).is_err());
    }
}
