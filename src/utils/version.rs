use std::str::from_utf8;

use log::debug;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{VersionError, XmlParseError};
use crate::models::AbletonVersion;

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
