#![allow(unused_imports)]
use iced::Color;
use log::{debug, trace, warn};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

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

pub struct Theme {
    // Background Colors
    SurfaceBackground: Color,       // background colour of surfaces - default #363636
    Desktop: Color,                 // background colour behind surfaces - default #2a2a2a
    ControlBackground: Color,       // button background colour - default #1e1e1e
    DetailViewBackground: Color,    // background colour for deactivated controls on displays - default #3e3e3e
    DisplayBackground: Color,       // background colour for displays - default #181818
    
    // Text Colors
    ControlForeground: Color,        // main text colour - #b5b5b5
    TextDisabled: Color,             // disabled / secondary text colour - default #757575
    ControlOnForeground: Color,      // text colour for selected elements - default #070707
    ControlOffForeground: Color,     // text colour for non-selected elements - default #b5b5b5
    
    // UI Elements
    SurfaceHighlight: Color,        // highlight colour for surfaces - used for headers or highlighted parts of surfaces
    ControlFillHandle: Color,
    SelectionFrame: Color,          // frame of selected surface - default #757575
    ControlContrastFrame: Color,
    
    // Interactive Elements
    ViewCheckControlEnabledOn: Color,
    ViewCheckControlEnabledOff: Color,
    SelectionBackground: Color,      // background colour for selected elements on selected surfaces - default #b0ddeb
    StandbySelectionBackground: Color, // background colour for selected elements on non selected surfaces- default #637e86
    Progress: Color,
    
    // Accent Colors
    ChosenDefault: Color,
    Alert: Color, // use for errors or warnings in the status bar - default #e76942
    ChosenAlternative: Color,
    
    // Additional Useful Colors
    SurfaceArea: Color,             // surface area colour - usually used for dropdown menus. - default #242424
    ScrollbarInnerHandle: Color,
    ScrollbarInnerTrack: Color,
    GridLabel: Color,
    GridLineBase: Color,
}
