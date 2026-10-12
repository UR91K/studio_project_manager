//! VST plugin metadata extraction.
//!
//! This crate is two things at once, split by the `host` feature:
//!
//! - **Without `host`** (`default-features = false`): just the data types --
//!   [`meta::PluginMeta`] and the [`protocol`] wire format. This is what the parent
//!   process depends on, so it can parse the worker's output without linking a VST
//!   host into an address space that must never load a plugin.
//! - **With `host`** (the default): the scanner itself, plus the `vst-meta` binary.
//!   Everything here loads third-party native code and is expected to be run as a
//!   supervised subprocess -- see [`protocol`] for why.

pub mod meta;
pub mod protocol;

#[cfg(feature = "host")]
pub mod diag;
#[cfg(feature = "host")]
pub mod scan;
#[cfg(feature = "host")]
pub mod shell;
#[cfg(feature = "host")]
pub mod stdio;

pub use meta::{FormatExtra, PluginFormat, PluginMeta, Vst3Bus, Vst3Class};
pub use protocol::{ErrorType, Event, Outcome};
