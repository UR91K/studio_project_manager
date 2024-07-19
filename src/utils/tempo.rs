// /src/utils/tempo.rs

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{TempoError, XmlParseError};
use crate::utils::xml_parsing::{find_attribute, find_tags};
use crate::utils::StringResultExt;

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
