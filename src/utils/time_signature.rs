use std::str::FromStr;

use log::{debug, error, info, trace};

use crate::error::{LiveSetError, TimeSignatureError, XmlParseError};
use crate::models::TimeSignature;
use crate::utils::xml_parsing::find_empty_event;

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
