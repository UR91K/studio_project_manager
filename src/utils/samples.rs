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

pub(crate) fn decode_sample_path(abs_hash_path: &str) -> Result<PathBuf, SampleError> {
    trace!("Starting sample path decoding");

    let cleaned_path = abs_hash_path
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
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
        }
        Err(e) => {
            warn!(
                "Failed to canonicalize path: {}. Using non-canonicalized path.",
                e
            );
            Ok(path)
        }
    }
}
