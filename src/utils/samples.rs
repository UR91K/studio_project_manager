use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::str::from_utf8;

#[allow(unused_imports)]
use log::{debug, error, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;

#[allow(unused_imports)]
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
) -> Result<HashMap<String, HashSet<PathBuf>>, SampleError> {
    let mut sample_paths: HashMap<String, HashSet<PathBuf>> = HashMap::new();

    if major_version < 11 {
        debug!("Processing samples for Ableton version < 11");
        let sample_data: HashSet<String> = find_sample_path_data(xml_data)?.into_iter().collect();
        let mut decoded_paths = HashSet::new();
        for data in sample_data {
            match decode_sample_path(&data) {
                Ok(path) => {
                    debug!("Successfully decoded sample path: {:?}", path);
                    decoded_paths.insert(path);
                }
                Err(e) => {
                    warn!("Failed to decode sample path: {:?}", e);
                }
            }
        }
        debug!("Found {} unique samples for version < 11", decoded_paths.len());
        sample_paths.insert("SampleData".to_string(), decoded_paths);
    } else {
        debug!("Processing samples for Ableton version >= 11");
        let search_queries = &["SampleRef"];
        let target_depth: u8 = 1;
        let sample_tags = find_tags(xml_data, search_queries, target_depth)?;

        for (query, tags_list) in sample_tags {
            let mut paths = HashSet::new();
            for tags in tags_list {
                if let Ok(path) = find_attribute(&tags, "Path", "Value") {
                    debug!("Found sample path for '{}': {:?}", query, path);
                    paths.insert(PathBuf::from(path));
                }
            }
            debug!("Found {} unique samples for '{}'", paths.len(), query);
            sample_paths.insert(query, paths);
        }
    }

    debug!("Total unique sample collections found: {}", sample_paths.len());
    Ok(sample_paths)
}

pub(crate) fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, SampleError> {
    trace!("Starting sample path decoding");

    let cleaned_path = abs_hash_path.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    trace!("Cleaned absolute hash path: {:?}", cleaned_path);

    let byte_data = hex::decode(&cleaned_path).map_err(|e| {
        warn!("Failed to decode hex string: {:?}", e);
        SampleError::HexDecodeError(e)
    })?;
    trace!("Decoded {} bytes", byte_data.len());

    let (cow, _, had_errors) = encoding_rs::UTF_16LE.decode(&byte_data);

    if had_errors {
        warn!("Errors encountered during UTF-16 decoding");
    }

    let path_string = cow.replace('\0', "");
    let path = PathBuf::from(path_string);
    trace!("Decoded path: {:?}", path);
    
    
    match path.canonicalize() {
        Ok(canonical_path) => {
            trace!("Canonicalized path: {:?}", canonical_path);
            Ok(canonical_path)
        },
        Err(e) => {
            warn!("Failed to canonicalize path: {}. Using non-canonicalized path.", e);
            Ok(path)
        }
    }
}
