//! Logging (ADR-0071).
//!
//! Every program that links this crate logs to standard output. The daemon also writes
//! daily files under `%APPDATA%\Seula\logs`, because started from the tray it has no
//! console and a tester's problem would otherwise leave nothing behind.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;

/// How many daily files are kept; the oldest goes when another appears.
pub const KEEP_LOG_FILES: usize = 7;

/// `seula.YYYY-MM-DD.log`, so a tester can be told which file to send.
const FILE_PREFIX: &str = "seula";
const FILE_SUFFIX: &str = "log";

/// Where the daemon's log files go: beside `config.toml` and `seula.db`. It does not follow
/// a moved `database_path`, so the documentation can always name one folder.
pub fn log_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Seula").join("logs"))
}

/// The handle that swaps the filter while the program runs (ADR-0072). Set once by `init`.
static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// The filter for `level`: `RUST_LOG` first, then `level` as the bare directive. See `init`
/// for why the order matters.
fn build_filter(level: LevelFilter) -> EnvFilter {
    EnvFilter::builder()
        .parse_lossy(std::env::var("RUST_LOG").unwrap_or_default())
        .add_directive(level.into())
}

/// Changes the log level of the running program. Returns false when logging was never
/// started by `init` (a test, say) or the level is not one of the five names; an unknown
/// name is left alone rather than defaulted, because a live change should not quietly
/// turn into `info`.
pub fn set_level(log_level: &str) -> bool {
    let Some(handle) = FILTER.get() else {
        return false;
    };
    let level = match log_level.to_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => return false,
    };
    handle.reload(build_filter(level)).is_ok()
}

/// The bare level directive currently in force, for tests and diagnostics.
pub fn current_max_level() -> Option<LevelFilter> {
    FILTER.get()?.with_current(|f| f.max_level_hint()).ok()?
}

/// Start logging at `log_level`. With `to_file`, also write the daily files.
///
/// Keep the returned guard alive until the program ends: dropping it flushes the file.
/// A log file that cannot be opened is reported on standard error and logging carries on
/// without it, because a full disk or a locked folder must not stop the daemon starting.
pub fn init(log_level: &str, to_file: bool) -> Option<WorkerGuard> {
    let level = parse_level(log_level);

    // Reproduces `env_logger::Builder::from_default_env().filter_level(level)`.
    // There, RUST_LOG is parsed first and `filter_level` then *replaces* the
    // bare (no-target) directive while leaving per-target ones alone -- so
    // `RUST_LOG=seula::scan=trace` wins for that module, but a bare
    // `RUST_LOG=debug` loses to the config value. `add_directive` with a bare
    // level has the same replace-on-same-key behaviour, so the precedence is
    // unchanged.
    let (filter, handle) = reload::Layer::new(build_filter(level));
    let _ = FILTER.set(handle);

    let stdout = tracing_subscriber::fmt::layer();
    let (file, guard) = match to_file.then(open_file).flatten() {
        Some((writer, guard)) => (
            Some(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer)
                    .boxed(),
            ),
            Some(guard),
        ),
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout)
        .with(file)
        .init();

    install_panic_hook();
    guard
}

fn parse_level(log_level: &str) -> LevelFilter {
    match log_level.to_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => {
            eprintln!("Invalid log level '{}', defaulting to 'info'", log_level);
            LevelFilter::INFO
        }
    }
}

fn open_file() -> Option<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let dir = match log_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("No log file: the app data folder could not be found");
            return None;
        }
    };
    match rolling_writer(&dir) {
        Ok(appender) => Some(tracing_appender::non_blocking(appender)),
        Err(e) => {
            eprintln!("No log file: {}: {}", dir.display(), e);
            None
        }
    }
}

/// The daily writer for `dir`, creating the folder. Separate from `open_file` so a test can
/// point it at a scratch folder.
fn rolling_writer(dir: &Path) -> Result<tracing_appender::rolling::RollingFileAppender, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(KEEP_LOG_FILES)
        .build(dir)
        .map_err(|e| e.to_string())
}

/// A panic that ends the daemon would otherwise leave nothing: with no console the default
/// hook's message goes nowhere. This writes it to the log first, then lets the default hook
/// run as before.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_string());
        tracing::error!("panic at {}: {}", place, message);
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_log_folder_is_under_the_app_data_folder() {
        let dir = log_dir().expect("a data dir on this machine");
        assert!(dir.ends_with(Path::new("Seula").join("logs")), "{dir:?}");
    }

    #[test]
    fn levels_parse_and_an_unknown_one_falls_back_to_info() {
        assert_eq!(parse_level("ERROR"), LevelFilter::ERROR);
        assert_eq!(parse_level("debug"), LevelFilter::DEBUG);
        assert_eq!(parse_level("loud"), LevelFilter::INFO);
    }

    #[test]
    fn the_writer_makes_the_folder_and_names_the_file_for_the_day() {
        let scratch = tempfile::tempdir().unwrap();
        let dir = scratch.path().join("nested").join("logs");

        let mut writer = rolling_writer(&dir).expect("the folder should be created");
        writeln!(writer, "a line").unwrap();
        writer.flush().unwrap();

        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let name = &names[0];
        assert!(
            name.starts_with("seula.") && name.ends_with(".log"),
            "unexpected log file name {name}"
        );
        // seula.YYYY-MM-DD.log
        assert_eq!(name.len(), "seula.2026-10-11.log".len(), "{name}");
    }

    #[test]
    fn a_folder_that_cannot_be_made_is_an_error_not_a_panic() {
        let scratch = tempfile::tempdir().unwrap();
        let file = scratch.path().join("in-the-way");
        std::fs::write(&file, b"x").unwrap();
        // A file where the folder should be.
        assert!(rolling_writer(&file.join("logs")).is_err());
    }

    #[test]
    fn only_the_newest_files_are_kept() {
        let scratch = tempfile::tempdir().unwrap();
        // Ten old days, then a fresh writer: the cap is applied when it starts.
        for day in 1..=10 {
            std::fs::write(
                scratch.path().join(format!("seula.2026-09-{day:02}.log")),
                b"x",
            )
            .unwrap();
        }
        let mut writer = rolling_writer(scratch.path()).unwrap();
        writeln!(writer, "today").unwrap();
        writer.flush().unwrap();

        let count = std::fs::read_dir(scratch.path()).unwrap().count();
        assert_eq!(
            count, KEEP_LOG_FILES,
            "expected the cap to prune to {KEEP_LOG_FILES}"
        );
    }
}
