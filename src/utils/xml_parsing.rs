use crate::error::{AttributeError, XmlParseError};
use crate::models::XmlTag;
use crate::utils::StringResultExt;
use log::{debug, trace};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;

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

pub(crate) fn parse_event_attributes(
    event: &BytesStart,
) -> Result<HashMap<String, String>, XmlParseError> {
    let mut attributes = HashMap::new();
    for attribute_result in event.attributes() {
        let attribute = attribute_result?;
        let key = attribute.key.to_string_result()?;
        let value = attribute.value.to_string_result()?;
        debug!("Found attribute: {} = {}", key, value);
        attributes.insert(key.to_string(), value.to_string());
    }
    Ok(attributes)
}

pub(crate) fn get_value_as_string_result(event: &BytesStart) -> Result<Option<String>, XmlParseError> {
    for attribute_result in event.attributes() {
        let attribute = attribute_result?;
        if attribute.key.as_ref() == b"Value" {
            return Ok(Some(attribute.value.to_string_result()?));
        }
    }
    Ok(None)
}