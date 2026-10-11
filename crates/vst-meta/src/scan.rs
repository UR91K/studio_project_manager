#![allow(deprecated)]
//! Loading plugin binaries and normalizing what they report.
//!
//! Everything in this module runs third-party native code in the current process. It
//! is only safe to call from the supervised worker binary -- see [`crate::protocol`].

use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use vst::host::{Host, PluginLoader};
use vst::plugin::Plugin;
use vst3_host::discovery::get_detailed_plugin_info;

use crate::meta::PluginMeta;
use crate::protocol::{ErrorType, Outcome};
use crate::shell;

/// A bare-minimum Host shell required to meet the VST2 trait bounds.
///
/// The one callback that matters here is `get_plugin_id`. A shell plugin asks the
/// host "which of my sub-plugins am I being loaded as?" during its own init, and
/// whatever we answer determines which plugin the returned instance actually is.
pub struct MetadataHost {
    /// Unique id to hand back for `CurrentId`. 0 means "the container itself".
    plugin_id: AtomicI32,
}

impl MetadataHost {
    fn new() -> Self {
        MetadataHost {
            plugin_id: AtomicI32::new(0),
        }
    }
}

impl Host for MetadataHost {
    fn get_plugin_id(&self) -> i32 {
        self.plugin_id.load(Ordering::SeqCst)
    }
}

/// Scan one path, classifying any failure rather than propagating a bare string.
///
/// This is the only entry point the worker needs: it never returns `Err`, because a
/// plugin that refuses to load is an ordinary result to be recorded, not an error in
/// the scan itself.
pub fn scan_path(path: &Path) -> Outcome {
    if !path.exists() {
        return Outcome::Error {
            error_type: ErrorType::FileNotFound,
            error: format!("No such file or directory: {}", path.display()),
        };
    }

    // Dispatch on the bundle/file extension: `.vst3` is VST3, anything else is VST2.
    let is_vst3 = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("vst3"))
        .unwrap_or(false);

    let result = if is_vst3 {
        read_vst3(path).map(|p| vec![p])
    } else {
        read_vst2(path)
    };

    match result {
        Ok(plugins) => Outcome::Success { plugins },
        Err((error_type, error)) => Outcome::Error { error_type, error },
    }
}

type ScanFailure = (ErrorType, String);

/// Load a VST2 binary and normalize its `Info` block.
///
/// If the binary turns out to be a shell, this returns the container record followed
/// by one record per plugin inside it.
fn read_vst2(plugin_path: &Path) -> Result<Vec<PluginMeta>, ScanFailure> {
    let mut plugins = read_vst2_unnamed_ok(plugin_path)?;
    for plugin in &mut plugins {
        fill_empty_name(plugin, plugin_path);
    }
    Ok(plugins)
}

/// The `vst` crate fills `Info::name` from `effGetProductString` (opcode 48) alone and
/// offers no way to dispatch `effGetEffectName` (opcode 45), which is the only name
/// some plugins implement (SoundTheory Gullfoss, Gullfoss Live, Gullfoss Master).
/// Those come back empty, so fall back to the file name, which is what a host shows
/// for a plugin that gives it nothing better.
fn fill_empty_name(plugin: &mut PluginMeta, plugin_path: &Path) {
    if plugin.name.trim().is_empty() {
        // Record what the plugin *did* answer, so a blank name can be told apart from
        // a plugin that returned nothing at all.
        eprintln!(
            "  empty product name for {}: vendor={:?} uid={} category={} shell={} \
             params={:?} presets={:?}; using file name",
            plugin_path.display(),
            plugin.vendor,
            plugin.uid,
            plugin.category,
            plugin.is_shell,
            plugin.parameters,
            plugin.presets,
        );
        if let Some(stem) = plugin_path.file_stem() {
            plugin.name = stem.to_string_lossy().into_owned();
        }
    }
}

fn read_vst2_unnamed_ok(plugin_path: &Path) -> Result<Vec<PluginMeta>, ScanFailure> {
    let container = load_vst2_instance(plugin_path, 0)?;

    if !container.is_shell {
        return Ok(vec![container]);
    }

    // Ask the container what it holds. Each entry is a (unique id, name) pair.
    let entries = shell::enumerate(plugin_path).map_err(|e| (ErrorType::LoadFailed, e))?;
    eprintln!(
        "Shell plugin: enumerating {} contained plugins...",
        entries.len()
    );

    let parent_uid = container.uid.clone();
    let mut plugins = vec![container];

    for entry in entries {
        // Re-instantiate the binary, this time answering `CurrentId` with the target
        // id so the plugin materializes as that sub-plugin.
        match load_vst2_instance(plugin_path, entry.unique_id) {
            Ok(mut plugin) => {
                // Some shells report an empty product name per sub-plugin; the name
                // from the enumeration pass is the reliable one.
                if plugin.name.trim().is_empty() {
                    plugin.name = entry.name.clone();
                }
                plugin.shell_parent_uid = Some(parent_uid.clone());
                plugins.push(plugin);
            }
            Err((_, e)) => {
                // One bad sub-plugin does not invalidate the rest of the shell.
                eprintln!(
                    "  skipping shell entry {} ({}): {}",
                    entry.name, entry.unique_id, e
                );
            }
        }
    }

    Ok(plugins)
}

/// Instantiate a VST2 binary as the sub-plugin identified by `plugin_id`
/// (0 for a normal plugin, or the shell container itself).
fn load_vst2_instance(plugin_path: &Path, plugin_id: i32) -> Result<PluginMeta, ScanFailure> {
    // Wrap the host shell in the required thread-safe containers, pre-loaded with the
    // id we want the plugin to see.
    let host = Arc::new(Mutex::new(MetadataHost::new()));
    host.lock()
        .unwrap()
        .plugin_id
        .store(plugin_id, Ordering::SeqCst);

    // Load the library binary dynamically. Failure here means the file is not a VST2
    // at all -- no entry point, wrong architecture, not a library.
    let mut loader = PluginLoader::load(plugin_path, host.clone()).map_err(|e| {
        (
            ErrorType::InvalidFormat,
            format!("Failed to read or load the VST2 binary: {:?}", e),
        )
    })?;

    // Instantiate the plugin to populate its structural fields. Failure here means it
    // *is* a VST2 but would not initialize -- a licensing refusal, a missing
    // dependency, an unhappy plugin.
    let instance = loader.instance().map_err(|e| {
        (
            ErrorType::LoadFailed,
            format!("Failed to initialize the plugin instance: {:?}", e),
        )
    })?;

    // Pull the native VST 2.4 'Info' metadata object from the instance. Unlike VST3,
    // there is no manifest to read -- the plugin has to be instantiated to answer.
    let info = instance.get_info();

    Ok(PluginMeta::from_vst2(
        &plugin_path.display().to_string(),
        &info,
    ))
}

/// Query a VST3 bundle's manifest factory and normalize the result. Windows bundles
/// live under "C:\Program Files\Common Files\VST3", macOS under
/// "/Library/Audio/Plug-Ins/VST3", Linux under "~/.vst3".
fn read_vst3(plugin_path: &Path) -> Result<PluginMeta, ScanFailure> {
    let detailed = get_detailed_plugin_info(plugin_path).map_err(|e| {
        (
            ErrorType::LoadFailed,
            format!("Failed to parse VST3 metadata: {:?}", e),
        )
    })?;

    Ok(PluginMeta::from_vst3(&detailed))
}
