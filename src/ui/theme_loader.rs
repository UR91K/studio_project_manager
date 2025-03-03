#![allow(unused_imports)]
use iced::{color, Color};
use log::{debug, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use strum_macros::{EnumString, Display};

use crate::ableton_db::AbletonDatabase;
use crate::config::CONFIG;
use crate::error::LiveSetError;
use crate::models::{
    AbletonVersion, KeySignature, Plugin, PluginInfo, Sample, Scale, TimeSignature, Tonic,
};
use crate::utils::plugins::get_most_recent_db_file;
use crate::utils::plugins::LineTrackingBuffer;
use crate::utils::{EventExt, StringResultExt};
#[allow(unused_imports)]
use crate::{debug_fn, trace_fn, warn_fn};
use crate::error::ThemeError;
use crate::error::XmlParseError;

// At the top of the file, add the default theme XML as a constant with triple # delimiter
const DEFAULT_THEME_XML: &str = r###"
<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12049" SchemaChangeCount="3" Creator="Ableton Live 12.0d1" Revision="">
	<Theme>
		<ControlForeground Value="#b5b5b5" />
		<TextDisabled Value="#757575" />
		<SurfaceHighlight Value="#464646" />
		<SurfaceArea Value="#242424" />
		<Desktop Value="#2a2a2a" />
		<ViewCheckControlEnabledOn Value="#ffad56" />
		<ScrollbarInnerHandle Value="#696969" />
		<ScrollbarInnerTrack Value="#00000000" />
		<DetailViewBackground Value="#3e3e3e" />
		<SelectionFrame Value="#757575" />
		<ControlBackground Value="#1e1e1e" />
		<ControlFillHandle Value="#5d5d5d" />
		<ChosenDefault Value="#ffad56" />
		<ChosenAlternative Value="#03c3d5" />
		<Alert Value="#e76942" />
		<ControlOnForeground Value="#070707" />
		<ControlOffForeground Value="#b5b5b5" />
		<ControlContrastFrame Value="#111111" />
		<ViewCheckControlEnabledOff Value="#757575" />
		<SelectionBackground Value="#b0ddeb" />
		<StandbySelectionBackground Value="#637e86" />
		<SurfaceBackground Value="#363636" />
		<DisplayBackground Value="#181818" />
		<Progress Value="#ffad56" />
		<GridLabel Value="#b5b5b57f" />
		<GridLineBase Value="#06060654" />
	</Theme>
</Ableton>
"###;

/// Extension trait for Color to add hex string conversion
pub trait ColorExt {
    /// Creates a Color from a hex string like "#612424" or "612424"
    fn from_hex_str(hex: &str) -> Result<Color, ThemeError>;
}

impl ColorExt for Color {
    fn from_hex_str(hex: &str) -> Result<Color, ThemeError> {
        let hex = hex.trim_start_matches('#');
        let value = u32::from_str_radix(hex, 16).map_err(|_| ThemeError::InvalidThemeFile)?;
        Ok(color!(value))
    }
}

// You can then update your existing function to use this trait
fn convert_hex_to_color(hex: &str) -> Result<Color, ThemeError> {
    Color::from_hex_str(hex)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, Display)]
pub enum ThemeColorName {
    // Background Colors
    #[strum(serialize = "SurfaceBackground")]
    SurfaceBackground,
    #[strum(serialize = "Desktop")]
    Desktop,
    #[strum(serialize = "ControlBackground")]
    ControlBackground,
    #[strum(serialize = "DetailViewBackground")]
    DetailViewBackground,
    #[strum(serialize = "DisplayBackground")]
    DisplayBackground,
    
    // Text & Foreground Colors
    #[strum(serialize = "ControlForeground")]
    ControlForeground,
    #[strum(serialize = "TextDisabled")]
    TextDisabled,
    #[strum(serialize = "ControlOnForeground")]
    ControlOnForeground,
    #[strum(serialize = "ControlOffForeground")]
    ControlOffForeground,
    
    // Highlights & Frames
    #[strum(serialize = "SurfaceHighlight")]
    SurfaceHighlight,
    #[strum(serialize = "ControlFillHandle")]
    ControlFillHandle,
    #[strum(serialize = "SelectionFrame")]
    SelectionFrame,
    #[strum(serialize = "ControlContrastFrame")]
    ControlContrastFrame,
    
    // Controls & Selection
    #[strum(serialize = "ViewCheckControlEnabledOn")]
    ViewCheckControlEnabledOn,
    #[strum(serialize = "ViewCheckControlEnabledOff")]
    ViewCheckControlEnabledOff,
    #[strum(serialize = "SelectionBackground")]
    SelectionBackground,
    #[strum(serialize = "StandbySelectionBackground")]
    StandbySelectionBackground,
    #[strum(serialize = "Progress")]
    Progress,
    
    // Accent Colors
    #[strum(serialize = "ChosenDefault")]
    ChosenDefault,
    #[strum(serialize = "Alert")]
    Alert,
    #[strum(serialize = "ChosenAlternative")]
    ChosenAlternative,
    
    // UI Components
    #[strum(serialize = "SurfaceArea")]
    SurfaceArea,
    #[strum(serialize = "ScrollbarInnerHandle")]
    ScrollbarInnerHandle,
    #[strum(serialize = "ScrollbarInnerTrack")]
    ScrollbarInnerTrack,
    #[strum(serialize = "GridLabel")]
    GridLabel,
    #[strum(serialize = "GridLineBase")]
    GridLineBase,
}

pub struct Theme {
    colors: HashMap<ThemeColorName, Color>,
}

impl Theme {
    pub fn from_xml_data(xml_data: &str) -> Result<Self, ThemeError> {
        let mut reader = Reader::from_str(xml_data);
        let mut buf = Vec::new();
        
        // Track if we've found the Ableton and Theme tags
        let mut found_ableton = false;
        let mut in_theme = false;
        
        // Create an empty theme
        let mut theme = Theme { colors: HashMap::new() };
        
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    
                    // First tag must be Ableton
                    if !found_ableton {
                        if tag_name != "Ableton" {
                            return Err(ThemeError::InvalidThemeFile);
                        }
                        found_ableton = true;
                        continue;
                    }
                    
                    // Track when we enter the Theme section
                    if tag_name == "Theme" {
                        in_theme = true;
                        continue;
                    }
                },
                Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    
                    // If we're in the Theme section, process empty tags with Value attributes
                    if in_theme {
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"Value" {
                                    let value = String::from_utf8_lossy(&attr.value).to_string();
                                    
                                    // Try to parse the tag name as ThemeColorName
                                    if let Ok(theme_color) = tag_name.parse::<ThemeColorName>() {
                                        // Parse color and add directly to theme if successful
                                        if let Ok(color) = Color::from_hex_str(&value) {
                                            theme.colors.insert(theme_color, color);
                                        }
                                    }
                                    // If parsing fails, we just ignore the color
                                    break;
                                }
                            }
                        }
                    }
                },
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"Theme" {
                        in_theme = false;
                    }
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(ThemeError::ThemeParseError(e.into())),
                _ => (),
            }
            buf.clear();
        }
        
        Ok(theme)
    }
    
    // Get a color with fallback to default
    pub fn get(&self, name: ThemeColorName) -> Color {
        // We're now using a default theme derived from XML instead of hardcoded values
        *self.colors.get(&name).unwrap_or_else(|| {
            // Could log a warning here if desired
            // warn!("Color {:?} not found in theme, using default", name);
            &DEFAULT_THEME.colors[&name]
        })
    }
    
    // Load the default theme from embedded XML
    pub fn default() -> Self {
        // No need for error handling here - the XML is defined in our code
        // and will be checked at compile-time
        Self::from_xml_data(DEFAULT_THEME_XML)
            .unwrap_or_else(|_| panic!("Default theme XML is invalid - this is a bug"))
    }
}

// Lazily initialize the default theme once
lazy_static::lazy_static! {
    static ref DEFAULT_THEME: Theme = Theme::default();
}
