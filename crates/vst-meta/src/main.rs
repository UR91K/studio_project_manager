//! The scanner worker.
//!
//! Reads a list of plugin paths, scans each one, and writes NDJSON to stdout. This
//! process is expected to die -- plugins crash it, hang it, and occasionally call
//! `exit()` on it -- so the protocol is designed to let a supervising parent attribute
//! the failure and resume. See [`vst_meta::protocol`].

use std::io::{BufRead, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use vst_meta::diag;
use vst_meta::protocol::{Event, Outcome};
use vst_meta::scan;

#[derive(Parser)]
#[command(
    name = "vst-meta",
    about = "Extract metadata from VST2 and VST3 plugins"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan plugin paths and emit one NDJSON record per path on stdout.
    Scan {
        /// Paths to scan. Omit when using --stdin.
        paths: Vec<PathBuf>,

        /// Read paths from stdin, one per line. Avoids command-line length limits on
        /// large libraries.
        #[arg(long)]
        stdin: bool,

        /// Print human-readable output instead of NDJSON. For debugging by hand.
        #[arg(long)]
        human: bool,
    },
}

fn main() {
    // A crashing plugin should fail fast rather than block on a Windows error dialog
    // waiting for a click that will never come. This is process-global, which is
    // exactly why it belongs here and not in the parent application.
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
        };
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }

    diag::start_watchdog();
    diag::register_exit_hook();
    vst_meta::diag!(
        "started: {} {} on {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            paths,
            stdin,
            human,
        } => {
            let paths = if stdin {
                let _phase = diag::phase("reading the path list from stdin");
                let paths = read_paths_from_stdin();
                vst_meta::diag!("read {} path(s) from stdin", paths.len());
                paths
            } else {
                paths
            };

            if paths.is_empty() {
                eprintln!("No paths given. Pass paths as arguments or use --stdin.");
                std::process::exit(2);
            }

            if human {
                run_human(&paths);
            } else {
                run_ndjson(&paths);
            }
        }
    }

    exit_without_cleanup(0);
}

/// End the process at once, without running anyone's exit code (ADR-0081).
///
/// A normal exit runs the C runtime's exit handlers and then every loaded DLL's detach
/// handler, and the plugins this process has loaded are still loaded: many pin
/// themselves past `FreeLibrary`. Native Instruments' `ni-platform.dll`, which Kontakt,
/// Massive and Reaktor pull in, blocks forever in `WSAPoll` from its exit handlers, and
/// the worker never exits (`plugin-scan-hangs-after-last-plugin`). Nothing this process
/// owns needs tidying -- its output is written and flushed, and the operating system
/// reclaims the rest -- so the cleanup is all risk and no benefit.
#[allow(unreachable_code)] // the fallback after TerminateProcess / _exit
fn exit_without_cleanup(code: u32) -> ! {
    // Everything we wrote is already flushed after each line; this is belt and braces,
    // since what is still in a buffer when the process ends is lost.
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    vst_meta::diag!("exiting with code {} without running exit handlers", code);

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        TerminateProcess(GetCurrentProcess(), code);
    }
    #[cfg(unix)]
    unsafe {
        // `_exit`, not `exit`: it skips the atexit handlers, as TerminateProcess does.
        unsafe extern "C" {
            fn _exit(status: std::ffi::c_int) -> !;
        }
        _exit(code as std::ffi::c_int);
    }

    // TerminateProcess on the current process does not return. If it somehow did,
    // fall back to an ordinary exit rather than carry on.
    std::process::exit(code as i32)
}

fn read_paths_from_stdin() -> Vec<PathBuf> {
    std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The supervised path: one JSON object per line, flushed after every write.
///
/// Flushing is not optional. A buffered `begin` line that never reaches the parent
/// before the plugin crashes leaves the failure unattributable, which defeats the
/// entire point of running out of process.
fn run_ndjson(paths: &[PathBuf]) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let mut scanned = 0usize;
    let total = paths.len();

    for (index, path) in paths.iter().enumerate() {
        let display = path.display().to_string();

        // Announce before touching the binary, so a crash names its cause.
        vst_meta::diag!("begin [{}/{}] {}", index + 1, total, display);
        emit(
            &mut out,
            &Event::Begin {
                path: display.clone(),
            },
        );

        let outcome = {
            let _phase = diag::phase(format!("scan {}", display));
            scan::scan_path(path)
        };
        match &outcome {
            Outcome::Success { plugins } => vst_meta::diag!(
                "result [{}/{}] success, {} record(s): {}",
                index + 1,
                total,
                plugins.len(),
                display
            ),
            Outcome::Error { error_type, error } => vst_meta::diag!(
                "result [{}/{}] {}: {} ({})",
                index + 1,
                total,
                error_type,
                display,
                error
            ),
        }

        emit(
            &mut out,
            &Event::Result {
                path: display,
                outcome,
            },
        );
        scanned += 1;
    }

    diag::log_process_state("after the last plugin");
    emit(&mut out, &Event::Done { scanned });
    vst_meta::diag!("done: {} of {} scanned, done line written", scanned, total);
}

fn emit(out: &mut impl Write, event: &Event) {
    match serde_json::to_string(event) {
        Ok(line) => {
            // A closed pipe means the parent gave up on us, so there is nothing to do
            // about a write error -- but say so on stderr, which may still be read.
            if let Err(e) = writeln!(out, "{}", line) {
                vst_meta::diag!("writing a {}-byte event to stdout failed: {}", line.len(), e);
            }
            if let Err(e) = out.flush() {
                vst_meta::diag!("flushing stdout failed: {}", e);
            }
        }
        Err(e) => eprintln!("Failed to serialize event: {}", e),
    }
}

/// The by-hand path: the original pretty-printed dump, kept for debugging.
fn run_human(paths: &[PathBuf]) {
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            println!();
        }

        match scan::scan_path(path) {
            vst_meta::protocol::Outcome::Success { plugins } => {
                for (j, plugin) in plugins.iter().enumerate() {
                    if j > 0 {
                        println!();
                    }
                    plugin.print();
                }
                if plugins.len() > 1 {
                    println!();
                    println!("({} records extracted from this binary)", plugins.len());
                }
            }
            vst_meta::protocol::Outcome::Error { error_type, error } => {
                eprintln!("{}: {} ({})", path.display(), error, error_type);
            }
        }
    }
}
