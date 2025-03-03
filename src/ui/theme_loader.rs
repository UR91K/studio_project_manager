#![allow(unused_imports)]
//! # Theme System
//! 
//! This module provides a simple theme system for loading and accessing colors from Ableton Live theme files.
//! 
//! ## Usage
//! 
//! ```rust
//! use crate::ui::theme_loader::{get_current_theme};
//! 
//! // Get a color by its name in the Ableton theme file
//! let background = get_current_theme().get_color("Desktop");
//! let text = get_current_theme().get_color("ControlForeground");
//! 
//! // Access VU meter colors
//! if let Some(vu_meter) = get_current_theme().get_vu_meter("StandardVuMeter") {
//!     let max_color = vu_meter.maximum;
//!     let min_color = vu_meter.minimum;
//! }
//! ```
//! 
//! ## Loading Themes
//! 
//! Themes can be loaded from XML files using the `load_theme` function:
//! 
//! ```rust
//! use std::path::Path;
//! use crate::ui::theme_loader::load_theme;
//! 
//! // Load a theme from a file
//! let theme_path = Path::new("path/to/theme.xml");
//! let theme = load_theme(Some(&theme_path)).unwrap();
//! 
//! // Or use the default theme
//! let default_theme = load_theme(None).unwrap();
//! ```

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
use std::sync::RwLock;
use std::path::Path;
use std::fs;

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
use crate::ui::default_theme::DEFAULT_THEME_XML;

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

/// A theme containing a set of colors from an Ableton theme file
pub struct LiveTheme {
    colors: HashMap<String, Color>,
}
impl LiveTheme {
    /// Create a new Theme from XML data
    /// 
    /// This parses the XML data and extracts all color values from the theme section.
    pub fn from_xml_data(xml_data: &str) -> Result<Self, ThemeError> {
        let mut reader = Reader::from_str(xml_data);
        let mut buf = Vec::new();
        
        // Track if we've found the Ableton and Theme tags
        let mut found_ableton = false;
        let mut in_theme = false;
        let mut skip_depth = 0; // Track depth when skipping nested tags like VU meters
        
        // Create an empty theme
        let mut theme = LiveTheme { 
            colors: HashMap::new(),
        };
        
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
                    
                    // If we're in the Theme section and not already skipping
                    if in_theme && skip_depth == 0 {
                        // Check if this is a complex tag like VU meter (has child tags)
                        // We'll skip these for now as they're not needed
                        if tag_name.ends_with("VuMeter") || tag_name.contains("VuMeter") {
                            skip_depth = 1; // Start skipping
                            continue;
                        }
                    } else if skip_depth > 0 {
                        // If we're already skipping, increase the depth counter
                        skip_depth += 1;
                    }
                },
                Ok(Event::Empty(ref e)) => {
                    // Skip processing if we're inside a complex tag
                    if skip_depth > 0 {
                        continue;
                    }
                    
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    
                    // If we're in the Theme section, process empty tags with Value attributes
                    if in_theme {
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"Value" {
                                    let value = String::from_utf8_lossy(&attr.value).to_string();
                                    
                                    // Parse color and add to colors map
                                    if let Ok(color) = Color::from_hex_str(&value) {
                                        theme.colors.insert(tag_name, color);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                },
                Ok(Event::End(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    
                    if tag_name == "Theme" {
                        in_theme = false;
                    } else if skip_depth > 0 {
                        // Decrease the depth counter when exiting a tag
                        skip_depth -= 1;
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
    
    /// Get a color by its name in the Ableton theme file
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the color as defined in the Ableton theme file
    /// 
    /// # Returns
    /// 
    /// * `Color` - The color if found, or a default color if not found
    pub fn get_color(&self, name: &str) -> Color {
        self.colors.get(name).copied().unwrap_or_else(|| {
            // Log a warning and return a default color
            warn!("Color '{}' not found in theme, using default", name);
            
            // Try to get from default theme
            DEFAULT_THEME.colors.get(name).copied().unwrap_or_else(|| {
                // If not in default theme either, return a fallback color
                warn!("Color '{}' not found in default theme either", name);
                Color::from_rgb(0.5, 0.5, 0.5) // Medium gray as ultimate fallback
            })
        })
    }
    
    /// Load the default theme from embedded XML
    pub fn default() -> Self {
        // No need for error handling here - the XML is defined in our code
        // and will be checked at compile-time
        Self::from_xml_data(DEFAULT_THEME_XML)
            .unwrap_or_else(|_| panic!("Default theme XML is invalid - this is a bug"))
    }
    
    /// Load a theme from a file path
    pub fn from_file(path: &Path) -> Result<Self, ThemeError> {
        let xml_data = fs::read_to_string(path)
            .map_err(|_| ThemeError::ThemeFileNotFound)?;
        Self::from_xml_data(&xml_data)
    }
}

// Lazily initialize the default theme once
lazy_static::lazy_static! {
    static ref DEFAULT_THEME: LiveTheme = LiveTheme::default();
    
    // Global theme state that can be accessed from anywhere
    pub static ref CURRENT_THEME: RwLock<Arc<LiveTheme>> = RwLock::new(Arc::new(LiveTheme::default()));
}

// Public functions for theme management

/// Load a theme from a file path or use the default theme if None is provided
/// 
/// This function loads a theme from the specified file path, or uses the default
/// theme if None is provided. It also updates the global theme state.
/// 
/// # Arguments
/// 
/// * `path` - An optional path to a theme XML file
/// 
/// # Returns
/// 
/// * `Result<Arc<LiveTheme>, ThemeError>` - The loaded theme or an error
pub fn load_theme(path: Option<&Path>) -> Result<Arc<LiveTheme>, ThemeError> {
    let theme = match path {
        Some(path) => Arc::new(LiveTheme::from_file(path)?),
        None => Arc::new(LiveTheme::default()),
    };
    
    // Update the global theme state
    if let Ok(mut current_theme) = CURRENT_THEME.write() {
        *current_theme = theme.clone();
    }
    
    Ok(theme)
}

/// Get the current theme
/// 
/// This function returns the current theme as an Arc<LiveTheme>.
pub fn get_current_theme() -> Arc<LiveTheme> {
    CURRENT_THEME.read().expect("Failed to read current theme").clone()
}

/// Set the current theme
/// 
/// This function sets the current theme to the specified theme.
pub fn set_current_theme(theme: Arc<LiveTheme>) {
    if let Ok(mut current_theme) = CURRENT_THEME.write() {
        *current_theme = theme;
    }
}
