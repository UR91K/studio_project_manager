use std::fmt;
use std::path::PathBuf;
use std::str;
use std::str::FromStr;

use log::debug;
use quick_xml::events::attributes::Attribute;
use quick_xml::name::QName;
use quick_xml::events::attributes::AttrError;

use crate::errors::LiveSetError;
use crate::errors::TimeSignatureError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u64);

#[derive(Debug, Clone)]
pub struct XmlTag {
    pub(crate) name: String,
    pub(crate) attributes: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct AbletonVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub beta: bool
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    Empty,
    Major,
    Minor,
    Dorian,
    Mixolydian,
    Aeolian,
    Phrygian,
    Locrian,
    WholeTone,
    HalfWholeDim,
    WholeHalfDim,
    MinorBlues,
    MinorPentatonic,
    MajorPentatonic,
    HarmonicMinor,
    MelodicMinor,
    Dorian4,
    PhrygianDominant,
    LydianDominant,
    LydianAugmented,
    HarmonicMajor,
    SuperLocrian,
    BToneSpanish,
    HungarianMinor,
    Hirajoshi,
    Iwato,
    PelogSelisir,
    PelogTembung,
    Messiaen1,
    Messiaen2,
    Messiaen3,
    Messiaen4,
    Messiaen5,
    Messiaen6,
    Messiaen7,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Tonic {
    Empty,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

#[derive(Debug)]
pub struct KeySignature {
    tonic: Tonic,
    scale: Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginFormat {
    AU,
    VST2,
    VST3,
}

#[derive(Debug)]
pub struct Plugin {
    id: Id,
    name: String,
    plugin_format: PluginFormat,
    is_installed: bool
}

#[derive(Debug)]
pub struct Sample {
    id: Id,
    name: String,
    path: PathBuf,
    is_present: bool
}



#[derive(Debug)]
pub struct TimeSignature {
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

impl TimeSignature {
    pub fn from_encoded(encoded_value: i32) -> Result<Self, TimeSignatureError> {
        if encoded_value < 0 || encoded_value > 494 {
            return Err(TimeSignatureError::InvalidEncodedValue(encoded_value));
        }

        let numerator = Self::decode_numerator(encoded_value);
        let denominator = Self::decode_denominator(encoded_value);

        Ok(TimeSignature {
            numerator,
            denominator,
        })
    }

    fn decode_numerator(encoded_value: i32) -> u8 {
        if encoded_value < 0 {
            1
        } else if encoded_value < 99 {
            (encoded_value + 1) as u8
        } else {
            ((encoded_value % 99) + 1) as u8
        }
    }

    fn decode_denominator(encoded_value: i32) -> u8 {
        let multiple = encoded_value / 99 + 1;
        2_u8.pow((multiple - 1) as u32)
    }
}

impl Default for KeySignature {
    fn default() -> Self {
        KeySignature {
            tonic: Tonic::Empty,
            scale: Scale::Empty,
        }
    }
}

impl fmt::Display for AbletonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ableton {}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl AbletonVersion {
    pub fn from_attributes<'a, I>(attributes: I) -> Result<Self, LiveSetError>
    where
        I: Iterator<Item = Result<Attribute<'a>, AttrError>>,
    {
        let mut creator = None;
        let mut beta = false;

        for attr in attributes {
            let attr = attr.map_err(|e| LiveSetError::XmlAttrError(e))?;
            match attr.key {
                QName(b"Creator") => {
                    creator = Some(str::from_utf8(&attr.value)?.to_string());
                },
                _ => {}
            }
        }

        let creator = creator.ok_or(LiveSetError::MissingVersionInfo)?;
        debug!("Creator: {}", creator);

        let version_str = creator.strip_prefix("Ableton Live ")
            .ok_or(LiveSetError::InvalidVersionFormat)?;

        beta = version_str.to_lowercase().contains("beta");

        let version_str = version_str.replace("Beta", "").trim().to_string();

        let mut version_parts = vec![0, 0, 0];
        for (i, part) in version_str.split('.').enumerate() {
            if i >= 3 { break; }
            version_parts[i] = u32::from_str(part)
                .map_err(|_| LiveSetError::InvalidVersionFormat)?;
        }

        Ok(AbletonVersion {
            major: version_parts[0],
            minor: version_parts[1],
            patch: version_parts[2],
            beta,
        })
    }
}


impl Default for Id {
    fn default() -> Self {
        Id(0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::attributes::Attribute;
    use quick_xml::name::QName;
    use std::borrow::Cow;

    #[test]
    fn test_from_attributes() {
        let attributes = vec![
            Ok(Attribute {
                key: QName(b"Creator"),
                value: Cow::Borrowed(b"Ableton Live 11.0.12"),
            }),
        ];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 11);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 12);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_from_attributes_beta() {
        let attributes = vec![
            Ok(Attribute {
                key: QName(b"Creator"),
                value: Cow::Borrowed(b"Ableton Live 12.0 Beta"),
            }),
        ];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 12);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, true);
    }

    #[test]
    fn test_from_attributes_no_patch() {
        let attributes = vec![
            Ok(Attribute {
                key: QName(b"Creator"),
                value: Cow::Borrowed(b"Ableton Live 12.0"),
            }),
        ];

        let version = AbletonVersion::from_attributes(attributes.into_iter()).unwrap();
        assert_eq!(version.major, 12);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.beta, false);
    }

    #[test]
    fn test_from_attributes_missing_creator() {
        let attributes = vec![
            Ok(Attribute {
                key: QName(b"SomeOtherAttribute"),
                value: Cow::Borrowed(b"SomeValue"),
            }),
        ];

        assert!(AbletonVersion::from_attributes(attributes.into_iter()).is_err());
    }

    #[test]
    fn test_from_attributes_invalid_format() {
        let attributes = vec![
            Ok(Attribute {
                key: QName(b"Creator"),
                value: Cow::Borrowed(b"Not Ableton Live Version"),
            }),
        ];

        assert!(AbletonVersion::from_attributes(attributes.into_iter()).is_err());
    }
}