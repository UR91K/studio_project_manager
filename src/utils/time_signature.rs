use std::str::FromStr;

use log::{debug, error, trace};

use crate::error::TimeSignatureError;

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