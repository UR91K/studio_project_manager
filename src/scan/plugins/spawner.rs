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
use std::sync::{Arc, Mutex};
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

/// The error recorded for a plugin whose result line was lost (ADR-0083).
const RESULT_LOST: &str = "The scanner's result for this plugin was lost: something, almost \
     certainly the plugin itself, wrote to the scanner's output and corrupted it. Scanning \
     again should fix it.";

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
        // Info, not debug: a warning is worth little without the plugin it is about,
        // and a span below the configured level is not shown at all.
        let _scan = tracing::info_span!("plugin_scan", candidates = paths.len()).entered();
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
    ///
    /// Everything logged about this worker is inside a `worker{pid, base}` span, and
    /// everything about one plugin inside a `plugin{n, path}` span under it, so a log
    /// can be cut down to one worker or one plugin by its span. The pipe-reader threads
    /// enter those spans themselves, since a span does not follow work onto another
    /// thread: see [`CurrentPlugin`].
    fn run_batch(
        &self,
        batch: &[PathBuf],
        base: usize,
        total: usize,
        on_progress: &mut dyn FnMut(usize, usize, &Path),
    ) -> Result<BatchRun, PluginScanError> {
        let mut child = self.spawn(batch)?;
        let worker = tracing::info_span!("worker", pid = child.id(), base);
        let _worker = worker.enter();
        tracing::debug!(
            "Started for {} candidate(s) at offset {} of {}",
            batch.len(),
            base,
            total
        );
        let current = CurrentPlugin::new(worker.clone());

        // Feed the path list on stdin rather than argv: a large library would blow
        // past the command-line length limit. A dedicated thread keeps the write from
        // ever interleaving badly with our reads.
        let stdin = child.stdin.take().expect("stdin was piped");
        let list: Vec<String> = batch.iter().map(|p| p.display().to_string()).collect();
        let span = worker.clone();
        thread::spawn(move || {
            let _worker = span.enter();
            let mut stdin = stdin;
            for (i, path) in list.iter().enumerate() {
                if let Err(e) = writeln!(stdin, "{}", path) {
                    // Worker died early; nothing to do about it here.
                    tracing::debug!("stdin closed after {} of {} paths: {}", i, list.len(), e);
                    return;
                }
            }
            // Dropping stdin closes the pipe, which is how the worker knows the list
            // has ended.
            tracing::trace!("Wrote all {} paths, closing stdin", list.len());
        });

        // Surface the worker's diagnostics without letting them pollute stdout. Its
        // own lines carry DIAG_MARKER and get their own target, so they can be read
        // or filtered apart from what the plugins print.
        if let Some(stderr) = child.stderr.take() {
            let current = current.clone();
            thread::spawn(move || {
                let mut count = 0usize;
                for line in BufReader::new(stderr).lines() {
                    let span = current.span();
                    let _span = span.enter();
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
                            tracing::debug!("Reading stderr failed after {} lines: {}", count, e);
                            return;
                        }
                    }
                }
                let span = current.span();
                let _span = span.enter();
                tracing::trace!("stderr closed after {} lines", count);
            });
        }

        let events = self.read_events(&mut child, current.clone());
        let run = self.collect(events, batch, &mut child, base, total, &current, on_progress);

        // Reap the child regardless of how we got here.
        wait_for_exit(&mut child);

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
    fn read_events(&self, child: &mut Child, current: CurrentPlugin) -> Receiver<StdoutLine> {
        let stdout = child.stdout.take().expect("stdout was piped");
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut count = 0usize;
            for line in BufReader::new(stdout).lines() {
                let span = current.span();
                let _span = span.enter();
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        // Ends the stream exactly as EOF would, so the supervisor will
                        // read it as the worker dying. Say why, since it is not that.
                        tracing::warn!(
                            "Reading stdout failed after {} lines, treating the stream as \
                             closed: {}",
                            count,
                            e
                        );
                        return;
                    }
                };
                count += 1;
                tracing::trace!("stdout ({} bytes): {}", line.len(), line);
                if line.trim().is_empty() {
                    continue;
                }
                let parsed = match serde_json::from_str::<Event>(&line) {
                    Ok(event) => StdoutLine::Event(event),
                    Err(e) => StdoutLine::Unparseable(line, e.to_string()),
                };
                if tx.send(parsed).is_err() {
                    tracing::trace!("Supervisor stopped listening");
                    return;
                }
            }
            let span = current.span();
            let _span = span.enter();
            tracing::debug!("stdout closed after {} lines", count);
        });

        rx
    }

    /// Consume the event stream, tracking which path is in flight.
    #[allow(clippy::too_many_arguments)]
    fn collect(
        &self,
        events: Receiver<StdoutLine>,
        batch: &[PathBuf],
        child: &mut Child,
        base: usize,
        total: usize,
        current: &CurrentPlugin,
        on_progress: &mut dyn FnMut(usize, usize, &Path),
    ) -> BatchRun {
        let mut completed = Vec::new();
        // Results are matched by path, not by position (ADR-0083). The worker echoes the
        // path on every line, and a result is recorded against the batch entry with that
        // path. `next` is only where to start looking: the worker goes in order, so the
        // entry is almost always `batch[next]`.
        let mut next = 0usize;
        let mut in_flight: Option<usize> = None;
        // The span of the plugin in flight, entered on this thread for as long as it
        // is in flight.
        let mut plugin: Option<tracing::span::EnteredSpan> = None;

        let started = Instant::now();
        // When the last line arrived, and when the in-flight plugin was announced:
        // the gaps are what a stall looks like from here.
        let mut last_event = Instant::now();
        let mut began_at = Instant::now();
        let name = |offset: Option<usize>| match offset.and_then(|o| batch.get(o)) {
            Some(path) => path.display().to_string(),
            None => "nothing".to_string(),
        };
        // The first entry at or after `from` with this path.
        let find = |path: &str, from: usize| {
            batch
                .get(from..)
                .and_then(|rest| rest.iter().position(|p| p.display().to_string() == path))
                .map(|i| from + i)
        };
        let lost = |offset: usize| {
            (
                offset,
                Outcome::Error {
                    error_type: ErrorType::ResultLost,
                    error: RESULT_LOST.to_string(),
                },
            )
        };

        loop {
            let received = events.recv_timeout(self.timeout);
            let gap = last_event.elapsed();
            last_event = Instant::now();

            match received {
                Ok(StdoutLine::Event(Event::Begin { path })) => {
                    if let Some(stale) = in_flight.take() {
                        // The worker has moved on, so it did write a result for the
                        // previous plugin, and that line did not survive.
                        tracing::warn!(
                            "Begin for {} arrived while {} was still in flight -- its result \
                             line was lost; recording it as result lost",
                            path,
                            name(Some(stale))
                        );
                        completed.push(lost(stale));
                        next = stale + 1;
                    }
                    // Closed before the next opens, or it would become the next's parent.
                    drop(plugin.take());

                    let offset = find(&path, next);
                    let span = tracing::info_span!(
                        "plugin",
                        n = base + offset.unwrap_or(next) + 1,
                        path = %path
                    );
                    current.set(Some(span.clone()));
                    plugin = Some(span.entered());

                    tracing::debug!(
                        "Begin [{}/{}] ({} ms after the previous line; offset {:?})",
                        base + offset.unwrap_or(next) + 1,
                        total,
                        gap.as_millis(),
                        offset
                    );
                    match offset {
                        None => tracing::warn!(
                            "Plugin scanner announced {}, which is not among the {} \
                             candidates still to come",
                            path,
                            batch.len().saturating_sub(next)
                        ),
                        Some(offset) if offset != next => tracing::warn!(
                            "Plugin scanner skipped from {} to {}",
                            name(Some(next)),
                            path
                        ),
                        Some(_) => {}
                    }
                    in_flight = offset;
                    began_at = Instant::now();

                    if let Some(offset) = offset {
                        let progress_started = Instant::now();
                        on_progress(base + offset, total, &batch[offset]);
                        let took = progress_started.elapsed();
                        // Runs inline, so while it runs nothing is read and the
                        // timeout cannot fire. It should be instant.
                        if took >= Duration::from_secs(1) {
                            tracing::warn!("The progress callback took {} ms", took.as_millis());
                        } else {
                            tracing::trace!("Progress callback took {} us", took.as_micros());
                        }
                    }
                }

                Ok(StdoutLine::Event(Event::Result { path, outcome })) => {
                    let kind = match &outcome {
                        Outcome::Success { plugins } => {
                            format!("success, {} record(s)", plugins.len())
                        }
                        Outcome::Error { error_type, .. } => error_type.to_string(),
                    };
                    tracing::debug!(
                        "Result for {} after {} ms: {}",
                        path,
                        began_at.elapsed().as_millis(),
                        kind
                    );

                    let offset = match in_flight {
                        Some(o) if batch[o].display().to_string() == path => Some(o),
                        _ => find(&path, next),
                    };
                    match (in_flight, offset) {
                        (Some(stale), Some(offset)) if stale != offset => {
                            // This result's begin was lost, and the plugin that was in
                            // flight never reported: the worker went on past it.
                            tracing::warn!(
                                "Result for {} arrived while {} was in flight -- recording \
                                 that one as result lost",
                                path,
                                name(Some(stale))
                            );
                            completed.push(lost(stale));
                        }
                        _ => {}
                    }
                    in_flight = None;
                    match offset {
                        Some(offset) => {
                            completed.push((offset, outcome));
                            next = offset + 1;
                        }
                        None => tracing::warn!(
                            "Plugin scanner sent a result for {}, which is not among the \
                             candidates still to come; dropping it",
                            path
                        ),
                    }
                    current.set(None);
                    plugin = None;
                }

                Ok(StdoutLine::Event(Event::Done { scanned })) => {
                    if let Some(stale) = in_flight.take() {
                        tracing::warn!(
                            "Done arrived while {} was still in flight -- its result line \
                             was lost; recording it as result lost",
                            name(Some(stale))
                        );
                        completed.push(lost(stale));
                    }
                    tracing::debug!(
                        "Done: worker says {} scanned; supervisor recorded {} of {} in {:.1}s",
                        scanned,
                        completed.len(),
                        batch.len(),
                        started.elapsed().as_secs_f64()
                    );
                    if scanned != completed.len() {
                        tracing::warn!(
                            "Done disagrees with the supervisor: worker scanned {}, \
                             supervisor recorded {}",
                            scanned,
                            completed.len()
                        );
                    }
                    drop(plugin);
                    return BatchRun {
                        completed,
                        interrupted: None,
                    };
                }

                Ok(StdoutLine::Unparseable(line, error)) => {
                    // A plugin writing to stdout, which is our protocol channel. Logged
                    // in the in-flight plugin's span, which is almost always the culprit.
                    if line.contains("{\"event\":") {
                        tracing::warn!(
                            "A protocol line was glued to plugin output and is lost: {}",
                            line
                        );
                    } else {
                        tracing::warn!("Unparseable line from plugin scanner: {} ({})", line, error);
                    }
                }

                Err(RecvTimeoutError::Timeout) => {
                    // Silent for the whole timeout: whatever is loading has hung.
                    // Killing the process is the only way out -- a blocked thread
                    // cannot be safely terminated from inside.
                    tracing::debug!(
                        "No output for {:.1}s with {} in flight; killing the worker",
                        gap.as_secs_f64(),
                        name(in_flight)
                    );
                    if let Err(e) = child.kill() {
                        tracing::warn!("Killing the worker failed: {}", e);
                    }
                    drop(plugin);
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
                        "stdout closed without done, {} ms after the last line, with {} in \
                         flight",
                        gap.as_millis(),
                        name(in_flight)
                    );
                    drop(plugin);
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

/// The span of the plugin in flight, shared with the pipe-reader threads.
///
/// A span does not follow work onto another thread, and the threads reading the
/// worker's stdout and stderr cannot know which plugin is in flight -- only `collect`
/// does. It keeps this up to date, and the readers log each line inside whatever it
/// holds, so what a plugin prints lands in that plugin's span.
///
/// Approximate at the edges: a reader can be a line or two ahead of `collect`, so the
/// raw `stdout:` trace of a `begin` usually lands in the plugin *before* it. The lines
/// `collect` itself logs are exact.
#[derive(Clone)]
struct CurrentPlugin {
    worker: tracing::Span,
    plugin: Arc<Mutex<Option<tracing::Span>>>,
}

impl CurrentPlugin {
    fn new(worker: tracing::Span) -> Self {
        Self {
            worker,
            plugin: Arc::new(Mutex::new(None)),
        }
    }

    fn set(&self, plugin: Option<tracing::Span>) {
        *self.plugin.lock().unwrap_or_else(|e| e.into_inner()) = plugin;
    }

    /// The plugin's span, or the worker's when nothing is in flight.
    fn span(&self) -> tracing::Span {
        self.plugin
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_else(|| self.worker.clone())
    }
}

/// Wait for the worker to exit, killing it if it does not (ADR-0082).
///
/// By the time this runs, every result the worker will give is already collected, so
/// nothing is waited *for* -- this only reaps the process. A worker that has said
/// `done` and still not exited after [`EXIT_GRACE`] is stuck in someone's exit code
/// (`plugin-scan-hangs-after-last-plugin`) and is killed. If even the kill does not
/// take within another grace period the handle is let go: the scan's results are
/// complete, and blocking the whole scan on one process that will not die is the bug
/// this exists to prevent.
fn wait_for_exit(child: &mut Child) {
    let started = Instant::now();
    if let Some(code) = poll_exit(child, EXIT_GRACE) {
        tracing::debug!(
            "Exited with code {} after {} ms of waiting",
            code,
            started.elapsed().as_millis()
        );
        return;
    }

    tracing::warn!(
        "Worker has not exited {:.0}s after its output ended; killing it",
        EXIT_GRACE.as_secs_f64()
    );
    if let Err(e) = child.kill() {
        tracing::warn!("Killing the worker failed: {}", e);
    }
    match poll_exit(child, EXIT_GRACE) {
        Some(code) => tracing::debug!("Killed; exited with code {}", code),
        None => tracing::error!(
            "Worker did not exit {:.0}s after being killed; leaving it and carrying on",
            EXIT_GRACE.as_secs_f64()
        ),
    }
}

/// How long a worker that has finished gets to exit before it is killed. It needs
/// milliseconds: since ADR-0081 it ends itself without running any exit code.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// Poll for exit for up to `limit`, returning the exit code as text if it exited.
fn poll_exit(child: &mut Child, limit: Duration) -> Option<String> {
    const POLL: Duration = Duration::from_millis(10);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            // On Windows a crash is an NTSTATUS, readable only in hex
            // (0xC0000005 is an access violation).
            Ok(Some(status)) => {
                return Some(
                    status
                        .code()
                        .map(|c| format!("{} (0x{:08X})", c, c as u32))
                        .unwrap_or_else(|| "none".to_string()),
                );
            }
            Ok(None) if started.elapsed() < limit => thread::sleep(POLL),
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!("Waiting for the worker to exit failed: {}", e);
                return None;
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
