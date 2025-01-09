use std::collections::HashMap;
use quick_xml::events::BytesStart;
use log::{debug, trace};

use crate::error::LiveSetError;
use crate::models::PluginInfo;
use crate::utils::StringResultExt;
use crate::{debug_fn, trace_fn};
use super::state::ScannerState;

pub fn handle_source_context(
    state: &mut ScannerState,
    in_source_context: &mut bool,
    depth: i32,
    line: usize,
) -> Result<(), LiveSetError> {
    trace_fn!(
        "handle_start_event",
        "[{}] Entering SourceContext at depth {}",
        line,
        depth
    );
    *in_source_context = true;
    if !matches!(state, ScannerState::InPluginDesc { .. }) {
        *state = ScannerState::InSourceContext;
    }
    Ok(())
}

pub fn handle_value(
    state: &mut ScannerState,
    depth: i32,
    line: usize,
) -> Result<(), LiveSetError> {
    trace_fn!(
        "handle_start_event",
        "[{}] Entering Value tag inside SourceContext at depth {}",
        line,
        depth
    );
    if !matches!(state, ScannerState::InPluginDesc { .. }) {
        *state = ScannerState::InValue;
    }
    Ok(())
}

pub fn handle_branch_source_context(
    state: &mut ScannerState,
    depth: i32,
    line: usize,
) -> Result<(), LiveSetError> {
    trace_fn!(
        "handle_start_event",
        "[{}] Found BranchSourceContext at depth {}, looking for device ID",
        line,
        depth
    );
    *state = ScannerState::InBranchSourceContext;
    Ok(())
}

pub fn handle_plugin_desc(
    state: &mut ScannerState,
    current_branch_info: &Option<String>,
    plugin_info_processed: &mut bool,
    depth: i32,
    line: usize,
) -> Result<(), LiveSetError> {
    if let Some(device_id) = current_branch_info {
        debug_fn!(
            "handle_start_event",
            "[{}] Entering PluginDesc at depth {} for device: {}",
            line,
            depth,
            device_id
        );
        *plugin_info_processed = false;  // Reset the flag for new PluginDesc
        *state = ScannerState::InPluginDesc { device_id: device_id.clone() };
    } else {
        trace_fn!(
            "handle_start_event",
            "[{}] Found PluginDesc at depth {} but no current device ID",
            line,
            depth
        );
    }
    Ok(())
}

pub fn handle_plugin_info(
    state: &mut ScannerState,
    plugin_info_processed: &bool,
    current_branch_info: &Option<String>,
    depth: i32,
    line: usize,
    name: &str,
) -> Result<(), LiveSetError> {
    if let ScannerState::InPluginDesc { device_id } = &state {
        if *plugin_info_processed {
            debug_fn!(
                "handle_start_event",
                "[{}] Ignoring subsequent plugin info tag at depth {}: {} for device: {} (already processed)",
                line,
                depth,
                name,
                device_id
            );
        } else {
            debug_fn!(
                "handle_start_event",
                "[{}] Found plugin info tag at depth {}: {} for device: {}",
                line,
                depth,
                name,
                device_id
            );
            *state = if name == "Vst3PluginInfo" {
                ScannerState::InVst3PluginInfo
            } else {
                ScannerState::InVstPluginInfo
            };
        }
    } else {
        trace_fn!(
            "handle_start_event",
            "[{}] Found plugin info tag at depth {} but not in PluginDesc state: {:?}",
            line,
            depth,
            state
        );
    }
    Ok(())
}

pub fn handle_plugin_name(
    state: &ScannerState,
    plugin_info_processed: &mut bool,
    plugin_info_tags: &mut HashMap<String, PluginInfo>,
    current_branch_info: &Option<String>,
    depth: i32,
    line: usize,
    value: String,
) -> Result<(), LiveSetError> {
    match state {
        ScannerState::InVst3PluginInfo | ScannerState::InVstPluginInfo => {
            if !*plugin_info_processed {
                if let Some(device_id) = current_branch_info {
                    if let Some(plugin_format) = crate::utils::plugins::parse_plugin_format(device_id) {
                        debug_fn!(
                            "handle_start_event",
                            "[{}] Found plugin name at depth {}: {} for device: {}",
                            line,
                            depth,
                            value,
                            device_id
                        );
                        let plugin_info = PluginInfo {
                            name: value,
                            dev_identifier: device_id.clone(),
                            plugin_format,
                        };
                        debug_fn!(
                            "handle_start_event",
                            "[{}] Adding plugin info at depth {}: {:?}",
                            line,
                            depth,
                            plugin_info
                        );
                        plugin_info_tags.insert(device_id.clone(), plugin_info);
                        *plugin_info_processed = true;
                    }
                }
            } else {
                debug_fn!(
                    "handle_start_event",
                    "[{}] Ignoring plugin name at depth {} (already processed): {}",
                    line,
                    depth,
                    value
                );
            }
        }
        _ => {
            trace_fn!(
                "handle_start_event",
                "[{}] Found plugin name at depth {} but not in correct state: {:?}",
                line,
                depth,
                state
            );
        }
    }
    Ok(())
}

// Helper functions
pub fn validate_plugin_device_id(device_id: &str) -> bool {
    device_id.starts_with("device:vst:") || device_id.starts_with("device:vst3:")
}

pub fn process_browser_content_path(
    found_browser_content_path: &mut bool,
    depth: i32,
    line: usize,
) {
    debug_fn!(
        "handle_start_event",
        "[{}] Found BrowserContentPath at depth {}",
        line,
        depth
    );
    *found_browser_content_path = true;
}

pub fn process_device_id(
    device_id: &mut Option<String>,
    id: String,
    depth: i32,
    line: usize,
) {
    debug_fn!(
        "handle_start_event",
        "[{}] Found device ID at depth {}: {}",
        line,
        depth,
        id
    );
    *device_id = Some(id);
}

pub fn store_valid_plugin_device_id(
    current_branch_info: &mut Option<String>,
    device_id: String,
    depth: i32,
    line: usize,
) {
    debug_fn!(
        "handle_start_event",
        "[{}] Storing valid plugin device ID at depth {}: {}",
        line,
        depth,
        device_id
    );
    *current_branch_info = Some(device_id);
}

pub fn ignore_nested_plugin_desc(
    found_nested_plugin_desc: &mut bool,
    depth: i32,
    line: usize,
) {
    debug_fn!(
        "handle_start_event",
        "[{}] Found nested PluginDesc at depth {}, ignoring device ID",
        line,
        depth
    );
    *found_nested_plugin_desc = true;
}
