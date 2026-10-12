//! Supervising the plugin scanner subprocess.
//!
//! The worker is expected to die. Plugins segfault during init, hang on license
//! checks, and occasionally call `exit()`; none of that is catchable in-process, which
//! is why the scanning happens somewhere we can afford to lose. This module's job is
//! to notice, attribute the failure to the right plugin, and carry on.
//!
//! One worker handles a whole batch rather than one plugin each, because process
//! startup plus VST3 runtime init costs 20-40ms -- half a minute of pure overhead
//! across a thousand-plugin library. Crash isolation is recovered by the `begin`
//! line the worker emits before touching each binary: a `begin` with no matching
//! `result` names the plugin that killed it, so the supervisor can record the failure
//! against that path and restart from the one after it.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use vst_meta::protocol::{DIAG_MARKER, ErrorType, Event, Outcome};

/// Name of the sidecar worker binary.
const WORKER_BIN: &str = if cfg!(windows) {
    "vst-meta.exe"
} else {
    "vst-meta"
};

/// Overrides the sidecar lookup. Needed in development, where the worker sits in
/// `target/debug/` rather than next to an installed executable.
const WORKER_PATH_ENV: &str = "SEULA_VST_META_PATH";

/// Tracing target for the worker's own diagnostic lines (see `vst_meta::diag`), so
/// they can be filtered apart from the plugin noise on the same stderr.
const WORKER_TARGET: &str = "seula::scan::plugins::worker";

/// Restart budget, as a multiple of the number of candidates. A library where every
/// plugin crashes still terminates; a supervisor that respawns forever does not.
const RESTART_BUDGET_FACTOR: usize = 3;

#[derive(Debug, Error)]
pub enum PluginScanError {
    #[error(
        "Could not find the plugin scanner binary '{0}'. Expected it next to the current \
         executable, or named by the {1} environment variable."
    )]
    WorkerNotFound(String, &'static str),

    #[error("Failed to start the plugin scanner: {0}")]
    Spawn(#[from] std::io::Error),
}

/// What became of one candidate path.
#[derive(Debug, Clone)]
pub struct PluginScanResult {
    pub path: PathBuf,
    pub outcome: Outcome,
}

impl PluginScanResult {
    /// The failure classification, or `None` if the scan succeeded.
    pub fn error_type(&self) -> Option<ErrorType> {
        match &self.outcome {
            Outcome::Success { .. } => None,
            Outcome::Error { error_type, .. } => Some(*error_type),
        }
    }
}

/// The outcome of a full scan.
#[derive(Debug, Clone)]
pub struct ScanReport {
    pub results: Vec<PluginScanResult>,
    /// How many times the worker had to be restarted. Non-zero means plugins on this
    /// system crashed or hung the scanner -- exactly what the subprocess buys us.
    pub restarts: usize,
    /// True if the restart budget ran out and some candidates were never attempted.
    pub budget_exhausted: bool,
}

impl ScanReport {
    pub fn succeeded(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Success { .. }))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.results.len() - self.succeeded()
    }

    /// Total plugin records extracted. Exceeds `succeeded()` when VST2 shells expand
    /// into their contained plugins.
    pub fn plugin_count(&self) -> usize {
        self.results
            .iter()
            .map(|r| match &r.outcome {
                Outcome::Success { plugins } => plugins.len(),
                Outcome::Error { .. } => 0,
            })
            .sum()
    }
}

pub struct Spawner {
    worker: PathBuf,
    timeout: Duration,
}

impl Spawner {
    /// Locate the worker binary and prepare a supervisor.
    pub fn new(timeout: Duration) -> Result<Self, PluginScanError> {
        Ok(Self {
            worker: locate_worker()?,
            timeout,
        })
    }

    /// Supervise a specific worker binary.
    ///
    /// The recovery paths only fire when a plugin crashes or hangs the scanner, which
    /// no healthy machine reproduces on demand; this lets a test stand in a stub
    /// worker that misbehaves deliberately.
    pub fn with_worker(worker: PathBuf, timeout: Duration) -> Self {
        Self { worker, timeout }
    }

    /// Scan every path, restarting the worker past any plugin that kills it.
    ///
    /// Returns one result per attempted path, in the order given. This never fails
    /// part-way: a plugin that crashes the scanner is an ordinary result carrying
    /// [`ErrorType::Crashed`], not an error in the scan itself.
    pub fn scan(&self, paths: &[PathBuf]) -> ScanReport {
        self.scan_with_progress(paths, &mut |_, _, _| {})
    }

    /// As [`scan`](Self::scan), reporting each plugin as it is attempted.
    ///
    /// The callback fires on the worker's `begin` line — *before* the plugin is
    /// loaded, not after. That is deliberate: a scan of a real library takes minutes,
    /// and the plugin a user most wants named is the one that is currently hanging,
    /// which by definition never reports a result.
    ///
    /// Arguments are `(index, total, path)`, with `index` counting from zero across
    /// the whole scan rather than the current batch.
    pub fn scan_with_progress(
        &self,
        paths: &[PathBuf],
        on_progress: &mut dyn FnMut(usize, usize, &Path),
    ) -> ScanReport {
        let mut results: Vec<PluginScanResult> = Vec::with_capacity(paths.len());
        let mut cursor = 0usize;
        let mut restarts = 0usize;
        let budget = paths.len().saturating_mul(RESTART_BUDGET_FACTOR);
        let mut budget_exhausted = false;
        let started = Instant::now();
        tracing::debug!(
            "Plugin scan of {} candidate(s): worker {}, timeout {:?}, restart budget {}",
            paths.len(),
            self.worker.display(),
            self.timeout,
            budget
        );

        while cursor < paths.len() {
            let batch = &paths[cursor..];
            tracing::debug!(
                "Starting a worker at {} of {} (restart {})",
                cursor,
                paths.len(),
                restarts
            );
            let run = match self.run_batch(batch, cursor, paths.len(), on_progress) {
                Ok(run) => run,
                Err(e) => {
                    // The worker would not start at all. That is not a plugin's fault
                    // and retrying will not fix it, so stop rather than burn the
                    // budget spawning a binary that isn't there.
                    tracing::error!("Plugin scanner could not be started: {}", e);
                    break;
                }
            };

            tracing::debug!(
                "Worker run ended: {} result(s), interrupted: {:?}",
                run.completed.len(),
                run.interrupted.as_ref().map(|(o, t)| (batch[*o].display().to_string(), *t))
            );
            for (offset, outcome) in run.completed {
                results.push(PluginScanResult {
                    path: batch[offset].clone(),
                    outcome,
                });
            }

            match run.interrupted {
                None => {
                    // Clean exit: the whole remaining batch is accounted for.
                    cursor = paths.len();
                }
                Some((offset, error_type)) => {
                    let culprit = &batch[offset];
                    tracing::warn!(
                        "Plugin scanner {} on {} -- recording it and resuming",
                        error_type,
                        culprit.display()
                    );
                    results.push(PluginScanResult {
                        path: culprit.clone(),
                        outcome: Outcome::Error {
                            error_type,
                            error: format!(
                                "The scanner subprocess {} while loading this plugin",
                                error_type
                            ),
                        },
                    });

                    // Resume after the offender.
                    cursor += offset + 1;
                    restarts += 1;

                    if restarts > budget {
                        tracing::error!(
                            "Plugin scanner restarted {} times; abandoning the remaining {} \
                             candidates",
                            restarts,
                            paths.len().saturating_sub(cursor)
                        );
                        budget_exhausted = true;
                        break;
                    }
                }
            }
        }

        tracing::debug!(
            "Plugin scan finished in {:.1}s: {} result(s) for {} candidate(s), {} restart(s){}",
            started.elapsed().as_secs_f64(),
            results.len(),
            paths.len(),
            restarts,
            if budget_exhausted { ", budget exhausted" } else { "" }
        );

        ScanReport {
            results,
            restarts,
            budget_exhausted,
        }
    }

    /// Run one worker over `batch`, returning what it managed before stopping.
    fn run_batch(
        &self,
        batch: &[PathBuf],
        base: usize,
        total: usize,
        on_progress: &mut dyn FnMut(usize, usize, &Path),
    ) -> Result<BatchRun, PluginScanError> {
        let mut child = self.spawn(batch)?;
        let pid = child.id();
        tracing::debug!(
            "Worker pid {} started for {} candidate(s) at offset {} of {}",
            pid,
            batch.len(),
            base,
            total
        );

        // Feed the path list on stdin rather than argv: a large library would blow
        // past the command-line length limit. A dedicated thread keeps the write from
        // ever interleaving badly with our reads.
        let stdin = child.stdin.take().expect("stdin was piped");
        let list: Vec<String> = batch.iter().map(|p| p.display().to_string()).collect();
        thread::spawn(move || {
            let mut stdin = stdin;
            for (i, path) in list.iter().enumerate() {
                if let Err(e) = writeln!(stdin, "{}", path) {
                    // Worker died early; nothing to do about it here.
                    tracing::debug!(
                        "Worker pid {}: stdin closed after {} of {} paths: {}",
                        pid,
                        i,
                        list.len(),
                        e
                    );
                    return;
                }
            }
            // Dropping stdin closes the pipe, which is how the worker knows the list
            // has ended.
            tracing::trace!("Worker pid {}: wrote all {} paths, closing stdin", pid, list.len());
        });

        // Surface the worker's diagnostics without letting them pollute stdout. Its
        // own lines carry DIAG_MARKER and get their own target, so they can be read
        // or filtered apart from what the plugins print.
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let mut count = 0usize;
                for line in BufReader::new(stderr).lines() {
                    match line {
                        Ok(line) => {
                            count += 1;
                            match line.strip_prefix(DIAG_MARKER) {
                                Some(diag) => {
                                    tracing::debug!(target: WORKER_TARGET, "{}", diag)
                                }
                                None => tracing::debug!("vst-meta: {}", line),
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Worker pid {}: reading stderr failed after {} lines: {}",
                                pid,
                                count,
                                e
                            );
                            return;
                        }
                    }
                }
                tracing::trace!("Worker pid {}: stderr closed after {} lines", pid, count);
            });
        }

        let events = self.read_events(&mut child);
        let run = self.collect(events, batch, &mut child, base, total, on_progress);

        // Reap the child regardless of how we got here.
        wait_for_exit(&mut child, pid);

        Ok(run)
    }

    fn spawn(&self, batch: &[PathBuf]) -> Result<Child, PluginScanError> {
        tracing::debug!(
            "Spawning {} for {} candidate(s)",
            self.worker.display(),
            batch.len()
        );

        let mut cmd = Command::new(&self.worker);
        cmd.arg("scan")
            .arg("--stdin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Keep a plugin that tries to open a window from flashing something at the
        // user during a background scan.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        Ok(cmd.spawn()?)
    }

    /// Move stdout onto a channel so the supervisor can apply a timeout to it.
    ///
    /// There is no portable way to read a pipe with a deadline, but a channel has
    /// `recv_timeout`. When we kill a hung worker the pipe closes, the reader thread
    /// unblocks, and the channel hangs up on its own.
    fn read_events(&self, child: &mut Child) -> Receiver<StdoutLine> {
        let stdout = child.stdout.take().expect("stdout was piped");
        let pid = child.id();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut count = 0usize;
            for line in BufReader::new(stdout).lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        // Ends the stream exactly as EOF would, so the supervisor will
                        // read it as the worker dying. Say why, since it is not that.
                        tracing::warn!(
                            "Worker pid {}: reading stdout failed after {} lines, treating \
                             the stream as closed: {}",
                            pid,
                            count,
                            e
                        );
                        return;
                    }
                };
                count += 1;
                tracing::trace!("Worker pid {} stdout ({} bytes): {}", pid, line.len(), line);
                if line.trim().is_empty() {
                    continue;
                }
                let parsed = match serde_json::from_str::<Event>(&line) {
                    Ok(event) => StdoutLine::Event(event),
                    Err(e) => StdoutLine::Unparseable(line, e.to_string()),
                };
                if tx.send(parsed).is_err() {
                    tracing::trace!("Worker pid {}: supervisor stopped listening", pid);
                    return;
                }
            }
            tracing::debug!("Worker pid {}: stdout closed after {} lines", pid, count);
        });

        rx
    }

    /// Consume the event stream, tracking which path is in flight.
    fn collect(
        &self,
        events: Receiver<StdoutLine>,
        batch: &[PathBuf],
        child: &mut Child,
        base: usize,
        total: usize,
        on_progress: &mut dyn FnMut(usize, usize, &Path),
    ) -> BatchRun {
        let mut completed = Vec::new();
        // The worker processes paths in the order we gave them, so position in the
        // stream identifies the path. The echoed path is used only to sanity-check.
        let mut next = 0usize;
        let mut in_flight: Option<usize> = None;

        let pid = child.id();
        let started = Instant::now();
        // When the last line arrived, and when the in-flight plugin was announced:
        // the gaps are what a stall looks like from here.
        let mut last_event = Instant::now();
        let mut began_at = Instant::now();
        let name = |offset: Option<usize>| match offset.and_then(|o| batch.get(o)) {
            Some(path) => path.display().to_string(),
            None => "nothing".to_string(),
        };

        loop {
            let received = events.recv_timeout(self.timeout);
            let gap = last_event.elapsed();
            last_event = Instant::now();

            match received {
                Ok(StdoutLine::Event(Event::Begin { path })) => {
                    tracing::debug!(
                        "Worker pid {}: begin [{}/{}] {} ({} ms after the previous line; \
                         assigned to offset {})",
                        pid,
                        base + next + 1,
                        total,
                        path,
                        gap.as_millis(),
                        next
                    );
                    if let Some(stale) = in_flight {
                        // The previous plugin's result never arrived, so it is about to
                        // be overwritten. Everything from here is misattributed.
                        tracing::warn!(
                            "Worker pid {}: begin arrived while offset {} ({}) was still in \
                             flight -- its result line was lost",
                            pid,
                            stale,
                            name(Some(stale))
                        );
                    }
                    if next < batch.len() && batch[next].display().to_string() != path {
                        tracing::warn!(
                            "Plugin scanner reported an unexpected path: got {}, expected {}",
                            path,
                            batch[next].display()
                        );
                    }
                    in_flight = Some(next);
                    began_at = Instant::now();

                    if let Some(path) = batch.get(next) {
                        let progress_started = Instant::now();
                        on_progress(base + next, total, path);
                        let took = progress_started.elapsed();
                        // Runs inline, so while it runs nothing is read and the
                        // timeout cannot fire. It should be instant.
                        if took >= Duration::from_secs(1) {
                            tracing::warn!(
                                "Worker pid {}: the progress callback took {} ms",
                                pid,
                                took.as_millis()
                            );
                        } else {
                            tracing::trace!(
                                "Worker pid {}: progress callback took {} us",
                                pid,
                                took.as_micros()
                            );
                        }
                    }
                }

                Ok(StdoutLine::Event(Event::Result { path, outcome })) => {
                    let kind = match &outcome {
                        Outcome::Success { plugins } => format!("success, {} record(s)", plugins.len()),
                        Outcome::Error { error_type, .. } => error_type.to_string(),
                    };
                    tracing::debug!(
                        "Worker pid {}: result for {} after {} ms: {}",
                        pid,
                        path,
                        began_at.elapsed().as_millis(),
                        kind
                    );
                    if let Some(offset) = in_flight.take() {
                        if batch[offset].display().to_string() != path {
                            tracing::warn!(
                                "Worker pid {}: result for {} is being recorded against {}",
                                pid,
                                path,
                                batch[offset].display()
                            );
                        }
                        completed.push((offset, outcome));
                        next = offset + 1;
                    } else {
                        tracing::warn!("Plugin scanner sent a result with nothing in flight");
                    }
                }

                Ok(StdoutLine::Event(Event::Done { scanned })) => {
                    tracing::debug!(
                        "Worker pid {}: done, worker says {} scanned; supervisor recorded {} \
                         of {} in {:.1}s",
                        pid,
                        scanned,
                        completed.len(),
                        batch.len(),
                        started.elapsed().as_secs_f64()
                    );
                    if scanned != completed.len() || in_flight.is_some() {
                        tracing::warn!(
                            "Worker pid {}: done disagrees with the supervisor: worker scanned \
                             {}, supervisor recorded {}, still in flight: {}",
                            pid,
                            scanned,
                            completed.len(),
                            name(in_flight)
                        );
                    }
                    return BatchRun {
                        completed,
                        interrupted: None,
                    };
                }

                Ok(StdoutLine::Unparseable(line, error)) => {
                    // A plugin writing to stdout, which is our protocol channel. Named
                    // after whatever is in flight, which is almost always the culprit.
                    if line.contains("{\"event\":") {
                        tracing::warn!(
                            "Worker pid {}: a protocol line was glued to plugin output and is \
                             lost (in flight: {}): {}",
                            pid,
                            name(in_flight),
                            line
                        );
                    } else {
                        tracing::warn!(
                            "Unparseable line from plugin scanner (in flight: {}): {} ({})",
                            name(in_flight),
                            line,
                            error
                        );
                    }
                }

                Err(RecvTimeoutError::Timeout) => {
                    // Silent for the whole timeout: whatever is loading has hung.
                    // Killing the process is the only way out -- a blocked thread
                    // cannot be safely terminated from inside.
                    tracing::debug!(
                        "Worker pid {}: no output for {:.1}s with {} in flight; killing it",
                        pid,
                        gap.as_secs_f64(),
                        name(in_flight)
                    );
                    if let Err(e) = child.kill() {
                        tracing::warn!("Worker pid {}: kill failed: {}", pid, e);
                    }
                    return BatchRun {
                        interrupted: blame(in_flight, next, batch.len())
                            .map(|offset| (offset, ErrorType::Timeout)),
                        completed,
                    };
                }

                Err(RecvTimeoutError::Disconnected) => {
                    // stdout closed without a `Done`, so the worker died rather than
                    // finishing its list.
                    tracing::debug!(
                        "Worker pid {}: stdout closed without done, {} ms after the last \
                         line, with {} in flight",
                        pid,
                        gap.as_millis(),
                        name(in_flight)
                    );
                    return BatchRun {
                        interrupted: blame(in_flight, next, batch.len())
                            .map(|offset| (offset, ErrorType::Crashed)),
                        completed,
                    };
                }
            }
        }
    }
}

/// Which path to blame when the worker stops without reporting a result.
///
/// Normally it is the one in flight -- that is what the `begin` line is for. When the
/// worker went quiet before announcing anything, blame the path it was about to
/// attempt instead: something is wrong either way, and the supervisor has to keep
/// moving or every remaining candidate is dropped without explanation.
fn blame(in_flight: Option<usize>, next: usize, batch_len: usize) -> Option<usize> {
    match in_flight {
        Some(offset) => Some(offset),
        None if next < batch_len => {
            tracing::warn!(
                "Plugin scanner stopped after {} of {} candidates without announcing the next",
                next,
                batch_len
            );
            Some(next)
        }
        // Everything was accounted for; the worker simply exited.
        None => None,
    }
}

/// One line of the worker's stdout, as the reader thread hands it over.
enum StdoutLine {
    Event(Event),
    /// Not a protocol line: something a plugin printed, with the parse error. Passed
    /// on rather than logged by the reader so it can be named after the plugin in flight.
    Unparseable(String, String),
}

/// Wait for the worker to exit, reporting while it does not.
///
/// The wait has no limit, the same as a plain `wait()`: this is instrumentation, not
/// a fix. It polls so that a worker which said `done` but never exits shows up in the
/// log as exactly that, with its pid, instead of as a scan that silently stops.
fn wait_for_exit(child: &mut Child, pid: u32) {
    const POLL: Duration = Duration::from_millis(50);
    const REPORT_EVERY: Duration = Duration::from_secs(5);

    let started = Instant::now();
    let mut reported = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // On Windows a crash is an NTSTATUS, readable only in hex
                // (0xC0000005 is an access violation).
                let code = status
                    .code()
                    .map(|c| format!("{} (0x{:08X})", c, c as u32))
                    .unwrap_or_else(|| "none".to_string());
                tracing::debug!(
                    "Worker pid {} exited with code {} after {} ms of waiting",
                    pid,
                    code,
                    started.elapsed().as_millis()
                );
                return;
            }
            Ok(None) => {
                if reported.elapsed() >= REPORT_EVERY {
                    tracing::warn!(
                        "Worker pid {} has not exited {:.0}s after its output ended; still \
                         waiting",
                        pid,
                        started.elapsed().as_secs_f64()
                    );
                    reported = Instant::now();
                }
                thread::sleep(POLL);
            }
            Err(e) => {
                tracing::warn!("Worker pid {}: waiting for exit failed: {}", pid, e);
                return;
            }
        }
    }
}

/// What one worker run produced.
struct BatchRun {
    /// Results by offset into the batch.
    completed: Vec<(usize, Outcome)>,
    /// The path that was in flight when the worker stopped, and why. `None` means it
    /// finished the batch cleanly.
    interrupted: Option<(usize, ErrorType)>,
}

/// Find the sidecar worker binary.
fn locate_worker() -> Result<PathBuf, PluginScanError> {
    if let Some(path) = std::env::var_os(WORKER_PATH_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        tracing::warn!(
            "{} points at {}, which is not a file; falling back to the sidecar lookup",
            WORKER_PATH_ENV,
            path.display()
        );
    }

    let exe = std::env::current_exe()?;
    if let Some(dir) = exe.parent() {
        let candidate: &Path = &dir.join(WORKER_BIN);
        if candidate.is_file() {
            return Ok(candidate.to_path_buf());
        }
    }

    Err(PluginScanError::WorkerNotFound(
        WORKER_BIN.to_string(),
        WORKER_PATH_ENV,
    ))
}
