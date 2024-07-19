// /src/utils/samples.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::from_utf8;

use encoding_rs::UTF_16LE;
use log::{debug, warn};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{AttributeError, SampleError, XmlParseError};
use crate::utils::xml_parsing::{find_attribute, find_tags};

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

pub(crate) fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, SampleError> {
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
