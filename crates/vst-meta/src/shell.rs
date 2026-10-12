//! VST2 shell-plugin enumeration.
//!
//! A "shell" plugin is one binary that contains many logical plugins (Waves bundles,
//! some Kontakt builds). Loading it normally yields a single container reporting
//! `Category::Shell`, and none of the real plugins inside it.
//!
//! The VST 2.4 protocol for unpacking it is a two-step handshake:
//!
//! 1. The host repeatedly dispatches `effShellGetNextPlugin` (here
//!    `plugin::OpCode::ShellGetNextPlugin`) to the container. Each call returns the
//!    next sub-plugin's unique id and writes its name into a caller-supplied buffer.
//!    A return of 0 ends the list.
//! 2. To then load one specific sub-plugin, the host instantiates the binary *again*
//!    and answers the plugin's `CurrentId` host-callback with the desired unique id.
//!    The plugin consults that during its own init and becomes that sub-plugin.
//!
//! Step 2 is reachable through the safe API: that is exactly what `Host::get_plugin_id`
//! is for (see [`crate::scan::MetadataHost`]). Step 1 is not -- `PluginInstance` keeps its
//! `AEffect` pointer private and the crate's `Dispatch` trait is not exported, so
//! there is no way to send opcode `ShellGetNextPlugin` through it. This module
//! therefore opens its own short-lived handle to the binary and dispatches directly.

use crate::diag::phase;
use libloading::Library;
use std::ffi::c_void;
use std::path::Path;
use std::ptr;
use vst::api::consts::MAX_PRODUCT_STR_LEN;
use vst::api::{AEffect, PluginMain};
use vst::plugin::OpCode;

/// One logical plugin found inside a shell binary.
#[derive(Clone, Debug)]
pub struct ShellEntry {
    pub unique_id: i32,
    pub name: String,
}

/// A minimal host callback used only for the enumeration pass.
///
/// The plugin calls this during its init. We answer the few opcodes a plugin
/// reasonably expects before it will talk to us, and decline everything else.
/// `CurrentId` deliberately answers 0 here: we want the shell container itself,
/// not any particular sub-plugin.
extern "C" fn enumeration_callback(
    _effect: *mut AEffect,
    opcode: i32,
    _index: i32,
    _value: isize,
    _ptr: *mut c_void,
    _opt: f32,
) -> isize {
    // Values from `host::OpCode`, which the crate marks `#[doc(hidden)]`.
    const VERSION: i32 = 1;
    const CURRENT_ID: i32 = 2;

    match opcode {
        VERSION => 2400, // We speak VST 2.4.
        CURRENT_ID => 0,
        _ => 0,
    }
}

/// Dispatch one opcode to a raw `AEffect`.
///
/// # Safety
/// `effect` must be a live pointer returned by the plugin's entry point.
unsafe fn dispatch(effect: *mut AEffect, opcode: OpCode, ptr: *mut c_void) -> isize {
    unsafe {
        let dispatcher = (*effect).dispatcher;
        dispatcher(effect, opcode.into(), 0, 0, ptr, 0.0)
    }
}

/// Ask a shell binary for the list of plugins it contains.
///
/// Returns an empty list for a binary that is not a shell, since a non-shell plugin
/// answers `ShellGetNextPlugin` with 0 on the first call.
pub fn enumerate(path: &Path) -> Result<Vec<ShellEntry>, String> {
    // This deliberately does not reuse the caller's `PluginLoader`. We need a raw
    // `AEffect` to dispatch against, and we close this handle as soon as the list
    // is read so the real per-sub-plugin loads start from a clean state.
    let lib_phase = phase("shell LoadLibrary");
    let lib = unsafe { Library::new(path) }.map_err(|e| {
        format!(
            "Failed to open the VST2 binary for shell enumeration: {}",
            e
        )
    })?;
    drop(lib_phase);

    let mut entries = Vec::new();

    unsafe {
        let main: libloading::Symbol<PluginMain> = lib
            .get(b"VSTPluginMain")
            .map_err(|_| "The binary has no VSTPluginMain entry point".to_string())?;

        let effect = {
            let _phase = phase("shell VSTPluginMain");
            main(enumeration_callback)
        };
        if effect.is_null() {
            return Err("The plugin entry point returned no instance".to_string());
        }

        // Real hosts open the effect before enumerating; some shells populate their
        // internal list during init and return nothing if asked before it.
        {
            let _phase = phase("shell effOpen");
            dispatch(effect, OpCode::Initialize, ptr::null_mut());
        }

        let loop_phase = phase("shell effShellGetNextPlugin loop");

        // Each call fills `buf` with the next name and returns its unique id.
        // A zero return means the list is exhausted.
        loop {
            let mut buf = vec![0u8; MAX_PRODUCT_STR_LEN];
            let unique_id = dispatch(
                effect,
                OpCode::ShellGetNextPlugin,
                buf.as_mut_ptr() as *mut c_void,
            );

            if unique_id == 0 {
                break;
            }

            let name: String = String::from_utf8_lossy(&buf)
                .chars()
                .take_while(|c| *c != '\0')
                .collect();

            entries.push(ShellEntry {
                unique_id: unique_id as i32,
                name: name.trim().to_string(),
            });

            // A malformed plugin could loop forever; cap it at something no real
            // bundle approaches.
            if entries.len() > 4096 {
                break;
            }
        }

        drop(loop_phase);
        crate::diag!("shell lists {} plugin(s)", entries.len());

        // Close the effect before the library handle drops.
        let _phase = phase("shell effClose");
        dispatch(effect, OpCode::Shutdown, ptr::null_mut());
    }

    {
        let _phase = phase("shell FreeLibrary");
        drop(lib);
    }

    Ok(entries)
}
