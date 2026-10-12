//! The wire protocol between the scanner worker and its parent process.
//!
//! Reading plugin metadata means loading arbitrary third-party native code, which
//! segfaults, hangs, and occasionally calls `exit()`. None of that is recoverable
//! in-process, so the worker is supervised and restarted rather than trusted.
//!
//! The worker writes one JSON object per line to stdout and flushes after each,
//! and everything else -- logging, progress -- goes to stderr so stdout stays
//! parseable.
//!
//! The ordering is what makes crash attribution possible:
//!
//! ```text
//! {"event":"begin","path":"...\\Serum.vst3"}     <- BEFORE the load is attempted
//! {"event":"result","path":"...","status":"success","plugins":[...]}
//! {"event":"done","scanned":1}
//! ```
//!
//! A `begin` with no matching `result` names the plugin that killed the worker. Without
//! that line a crash tells the parent only that the batch died, leaving it unable to
//! either skip the offender or record a useful error against it.

use serde::{Deserialize, Serialize};

use crate::meta::PluginMeta;

/// Prefix on the worker's own diagnostic lines on stderr.
///
/// Plugins write to stderr too, often a great deal. The marker lets the parent tell the
/// worker's account of what it is doing from whatever the plugin in flight is printing.
pub const DIAG_MARKER: &str = "@vst-meta ";

/// One line of the worker's stdout stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Emitted immediately before a plugin binary is touched. The path named here is
    /// the one to blame if the worker dies before the matching `Result` arrives.
    Begin { path: String },

    /// The outcome of a single path. Arrives exactly once per `Begin`.
    Result {
        path: String,
        #[serde(flatten)]
        outcome: Outcome,
    },

    /// The batch finished cleanly. Absence of this event means the worker died.
    Done { scanned: usize },
}

/// What became of one scanned path.
///
/// A success carries a *list* because one binary can hold many logical plugins: a VST2
/// shell yields its container record followed by one record per plugin inside it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Success {
        plugins: Vec<PluginMeta>,
    },
    Error {
        error_type: ErrorType,
        error: String,
    },
}

/// Why a scan failed.
///
/// `Crashed`, `Timeout` and `ResultLost` are never emitted by the worker -- by definition it is in no
/// position to report either. The parent synthesizes them for the in-flight path when
/// the worker exits abnormally or goes silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// The path does not exist.
    FileNotFound,
    /// The file exists but is not a plugin this scanner recognizes.
    InvalidFormat,
    /// The binary loaded but refused to yield metadata.
    LoadFailed,
    /// The worker produced no output for the configured timeout and was killed.
    Timeout,
    /// The worker died while this path was in flight.
    Crashed,
    /// The worker announced this path but its result never arrived intact: something,
    /// almost always the plugin, corrupted the line on stdout. The plugin may well have
    /// loaded; the scan just does not know what it said (ADR-0083).
    ResultLost,
}

impl ErrorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorType::FileNotFound => "file not found",
            ErrorType::InvalidFormat => "invalid format",
            ErrorType::LoadFailed => "load failed",
            ErrorType::Timeout => "timeout",
            ErrorType::Crashed => "crashed",
            ErrorType::ResultLost => "result lost",
        }
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
