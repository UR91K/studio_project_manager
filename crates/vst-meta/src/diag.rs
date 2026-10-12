//! The worker's own diagnostics, on stderr.
//!
//! Every line starts with [`DIAG_MARKER`] and carries the process id and the time since
//! the worker started, so the parent can file it apart from plugin noise and a log
//! spanning several restarts still reads in order:
//!
//! ```text
//! @vst-meta pid=4120 +12.345s main: begin [17/453] C:\...\Serum.dll
//! ```
//!
//! The point is to make a stall *locatable*. Work that runs plugin code is wrapped in a
//! [`phase`], and a watchdog thread reports any phase that has gone on for longer than
//! [`WATCHDOG_INTERVAL`] -- naming the whole stack, so "stuck in `FreeLibrary` of X"
//! reads differently from "stuck in X's init".
//!
//! This is how `plugin-scan-hangs-after-last-plugin` was found: a plugin DLL hanging in
//! its exit handlers. The worker now ends without running them (ADR-0081), but a plugin
//! can still call `exit()` itself, which is what [`register_exit_hook`] is for.

use std::fmt;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::DIAG_MARKER;

/// How often the watchdog reports a phase that has not finished.
pub const WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);

fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The open phases, outermost first.
static PHASES: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

/// Write one diagnostic line. Use the [`diag!`](crate::diag!) macro.
pub fn log(args: fmt::Arguments) {
    let thread = thread::current();
    let name = thread.name().map(str::to_string).unwrap_or_else(|| format!("{:?}", thread.id()));
    log_as(&name, args);
}

/// [`log`] with the thread name given, for where asking for it is not safe.
fn log_as(name: &str, args: fmt::Arguments) {
    // Locked and written in one call so a line is not split by another thread's.
    // Errors are ignored: if stderr is gone there is nowhere to report it.
    let _ = writeln!(
        std::io::stderr().lock(),
        "{}pid={} +{:.3}s {}: {}",
        DIAG_MARKER,
        std::process::id(),
        started().elapsed().as_secs_f64(),
        name,
        args
    );
}

/// Log a diagnostic line, `format!`-style.
#[macro_export]
macro_rules! diag {
    ($($arg:tt)*) => {
        $crate::diag::log(format_args!($($arg)*))
    };
}

/// An open phase. Dropping it logs how long it took and closes it.
pub struct Phase {
    depth: usize,
}

/// Open a phase: log its start and keep it visible to the watchdog until dropped.
pub fn phase(name: impl Into<String>) -> Phase {
    let name = name.into();
    crate::diag!("> {}", name);
    let mut phases = PHASES.lock().unwrap_or_else(|e| e.into_inner());
    phases.push((name, Instant::now()));
    Phase {
        depth: phases.len(),
    }
}

impl Drop for Phase {
    fn drop(&mut self) {
        let mut phases = PHASES.lock().unwrap_or_else(|e| e.into_inner());
        // Phases nest by construction; truncating rather than popping keeps the stack
        // honest even if one is dropped out of order.
        let closed = phases.get(self.depth - 1).cloned();
        phases.truncate(self.depth - 1);
        drop(phases);
        if let Some((name, since)) = closed {
            crate::diag!("< {} ({} ms)", name, since.elapsed().as_millis());
        }
    }
}

/// Start the watchdog thread. Call once, early in `main`.
pub fn start_watchdog() {
    let _ = started();
    let spawned = thread::Builder::new()
        .name("watchdog".to_string())
        .spawn(|| {
            loop {
                thread::sleep(WATCHDOG_INTERVAL);
                let phases = PHASES.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some((_, since)) = phases.last() {
                    let elapsed = since.elapsed();
                    if elapsed >= WATCHDOG_INTERVAL {
                        let stack: Vec<&str> = phases.iter().map(|(n, _)| n.as_str()).collect();
                        crate::diag!(
                            "still running after {:.1}s: {}",
                            elapsed.as_secs_f64(),
                            stack.join(" > ")
                        );
                    }
                }
            }
        });
    if let Err(e) = spawned {
        crate::diag!("could not start the watchdog thread: {}", e);
    }
}

/// Register a C runtime exit handler that logs when it runs.
///
/// The worker itself never exits this way (ADR-0081), but a plugin can call `exit()`,
/// which runs the C runtime's `atexit` handlers (last registered first) and then
/// terminates the process, detaching every loaded DLL. This handler is registered
/// first, so it runs last of the executable's own: seeing its line means a plugin
/// ended the process, and everything up to the operating system's part has finished.
pub fn register_exit_hook() {
    unsafe extern "C" {
        fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
    }
    extern "C" fn on_exit() {
        // Not `diag!`: that asks for the current thread, which is not something to
        // risk this late in exit, where a panic cannot unwind and aborts instead.
        log_as(
            "atexit",
            format_args!("C runtime exit handlers have run; the process is now tearing down"),
        );
    }
    let rc = unsafe { atexit(on_exit) };
    if rc != 0 {
        crate::diag!("atexit registration failed ({})", rc);
    }
}

/// Log the modules and threads still in the process.
///
/// Called when the batch is done. A plugin DLL still loaded here gets detached at
/// process exit, and one that hangs in its detach handler hangs the exit with it; a
/// thread still running here belongs to someone, and a plugin that left one behind
/// is the usual suspect.
pub fn log_process_state(context: &str) {
    #[cfg(windows)]
    windows::log_process_state(context);
    #[cfg(not(windows))]
    crate::diag!("{}: module and thread listing is only implemented on Windows", context);
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW,
        TH32CS_SNAPMODULE, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    fn wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    pub fn log_process_state(context: &str) {
        let pid = std::process::id();

        // Modules. The ones under the Windows directory are system DLLs and are only
        // counted; everything else is a plugin, or something a plugin pulled in.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
            if snap == INVALID_HANDLE_VALUE {
                crate::diag!("{}: could not snapshot modules", context);
            } else {
                let mut entry: MODULEENTRY32W = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
                let mut listed = Vec::new();
                let mut system = 0usize;
                let mut ok = Module32FirstW(snap, &mut entry) != 0;
                while ok {
                    let path = wide(&entry.szExePath);
                    if path.to_ascii_lowercase().contains("\\windows\\") {
                        system += 1;
                    } else {
                        listed.push((path, entry.ProccntUsage));
                    }
                    ok = Module32NextW(snap, &mut entry) != 0;
                }
                CloseHandle(snap);
                crate::diag!(
                    "{}: {} module(s) loaded outside the Windows directory, {} system",
                    context,
                    listed.len(),
                    system
                );
                for (path, refs) in listed {
                    crate::diag!("{}:   module {} (refcount {})", context, path, refs);
                }
            }
        }

        // Threads. Toolhelp snapshots every thread on the machine; keep ours.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                crate::diag!("{}: could not snapshot threads", context);
            } else {
                let mut entry: THREADENTRY32 = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
                let mut ids = Vec::new();
                let mut ok = Thread32First(snap, &mut entry) != 0;
                while ok {
                    if entry.th32OwnerProcessID == pid {
                        ids.push(entry.th32ThreadID);
                    }
                    ok = Thread32Next(snap, &mut entry) != 0;
                }
                CloseHandle(snap);
                crate::diag!(
                    "{}: {} thread(s) in the process (this one is {}; it and the watchdog are ours): {:?}",
                    context,
                    ids.len(),
                    GetCurrentThreadId(),
                    ids
                );
            }
        }
    }
}
