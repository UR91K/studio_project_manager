//! The running daemon's config (ADR-0072).
//!
//! `CONFIG` is what the process started with. This module holds what is in force now: a
//! cell that the config routes, `POST /config/reload` and a watcher on `config.toml`
//! replace, and that everything else reads as a snapshot through `current()`.
//!
//! Take one snapshot when an operation starts and use it throughout; never keep one
//! across a scan's end or cache a field out of it. A failed load never replaces a good
//! config: the reason is kept for `reload_error()` and the old values stay in force.

use super::{Config, CONFIG};
use crate::error::ConfigError;
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::watch;

/// How long the config file must stay quiet before it is read. Editors save in several
/// steps (write a temporary file, rename it over the real one).
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// The cell. A `watch` channel is used as a cell because it is one: `send_replace`
/// swaps the value with no receiver and no runtime needed, `borrow` is a cheap read, and
/// `subscribe` lets a task (the folder watcher) wait for the next replacement.
static CELL: Lazy<Result<watch::Sender<Arc<Config>>, ConfigError>> = Lazy::new(|| {
    CONFIG
        .as_ref()
        .map(|c| watch::channel(Arc::new(c.clone())).0)
        .map_err(|e| e.clone())
});

/// Why the last attempt to load `config.toml` was refused, until a load succeeds.
static RELOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Serialises everything that changes the cell, so two API calls (or an API call and a
/// file edit) cannot each start from the same snapshot and lose one of the changes.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

fn write_lock() -> MutexGuard<'static, ()> {
    WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn cell() -> Result<&'static watch::Sender<Arc<Config>>, ConfigError> {
    CELL.as_ref().map_err(|e| e.clone())
}

/// The config in force now. Cheap: an `Arc` clone.
pub fn current() -> Result<Arc<Config>, ConfigError> {
    Ok(cell()?.borrow().clone())
}

/// A receiver that wakes when the config is replaced with different values.
pub fn subscribe() -> Result<watch::Receiver<Arc<Config>>, ConfigError> {
    Ok(cell()?.subscribe())
}

/// Why the last load of `config.toml` was refused, or `None` when the last one was good.
pub fn reload_error() -> Option<String> {
    RELOAD_ERROR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn set_reload_error(error: Option<String>) {
    *RELOAD_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = error;
}

/// The keys whose value in force now (the file's) differs from the one the process started
/// with. They belong to something already running -- the socket, the open database, the
/// media store -- so they wait for a restart. `http_port` is left out when
/// `SEULA_HTTP_PORT` is set: the variable wins at startup too, so the file value would
/// change nothing even after a restart.
pub fn restart_required() -> Vec<String> {
    let (Ok(now), Ok(started)) = (current(), CONFIG.as_ref()) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    if now.http_port != started.http_port && std::env::var_os("SEULA_HTTP_PORT").is_none() {
        keys.push("http_port".to_string());
    }
    if now.database_path != started.database_path
        && std::env::var_os("SEULA_DATABASE_PATH").is_none()
    {
        keys.push("database_path".to_string());
    }
    if now.media_storage_dir != started.media_storage_dir {
        keys.push("media_storage_dir".to_string());
    }
    keys
}

/// Puts `new` in force. Equal to what is in force is a no-op, which is how a save made by
/// the daemon itself is recognised when the file watcher reads it back. Caller holds the
/// write lock.
fn install(new: Config) -> Result<Arc<Config>, ConfigError> {
    let cell = cell()?;
    let old = cell.borrow().clone();
    if *old == new {
        return Ok(old);
    }
    if old.log_level() != new.log_level() {
        if !crate::logging::set_level(&new.log_level()) {
            tracing::debug!("log level not applied live (logging not initialised)");
        }
    }
    let new = Arc::new(new);
    cell.send_replace(Arc::clone(&new));
    tracing::info!("Config changed, now in force");
    Ok(new)
}

/// Runs `change` on a copy of the config in force, which is expected to validate and save
/// the file (the `Config::update_*` methods do), then puts the result in force. If it
/// fails nothing changes.
pub fn mutate<R>(
    change: impl FnOnce(&mut Config) -> Result<R, ConfigError>,
) -> Result<(Arc<Config>, R), ConfigError> {
    let _guard = write_lock();
    let mut copy = (*current()?).clone();
    let result = change(&mut copy)?;
    // The file is saved and good; any earlier complaint about it is now out of date.
    set_reload_error(None);
    Ok((install(copy)?, result))
}

fn load_and_install(path: &Path) -> Result<(Arc<Config>, Vec<String>), ConfigError> {
    let _guard = write_lock();
    let loaded = Config::from_file(path).and_then(|c| c.validate().map(|w| (c, w)));
    match loaded {
        Ok((config, warnings)) => {
            set_reload_error(None);
            Ok((install(config)?, warnings))
        }
        Err(e) => {
            tracing::error!(
                "config.toml was not applied, the previous config stays in force: {}",
                e
            );
            set_reload_error(Some(e.to_string()));
            Err(e)
        }
    }
}

/// Reads `config.toml` again and, if it is good, puts it in force. This is what
/// `POST /api/v1/config/reload` does. A bad file is an error and changes nothing.
pub fn reload() -> Result<(Arc<Config>, Vec<String>), ConfigError> {
    let path = super::loader::find_config_file()?;
    load_and_install(&path)
}

/// Keeps a thread watching `config.toml`'s folder. Dropping it stops the watch.
pub struct ConfigFileWatcher {
    _watcher: notify::RecommendedWatcher,
    path: PathBuf,
}

impl ConfigFileWatcher {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Watches the folder of the config file (not the file: an editor's save-by-rename would
/// lose a watch on the file) and reloads half a second after the last event for it. A
/// missing file, as in the middle of a rename, is ignored.
pub fn watch_file() -> Result<ConfigFileWatcher, ConfigError> {
    watch_path(super::loader::find_config_file()?, DEBOUNCE)
}

fn watch_path(path: PathBuf, debounce: Duration) -> Result<ConfigFileWatcher, ConfigError> {
    use notify::{RecursiveMode, Watcher};

    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let dir = path
        .parent()
        .ok_or_else(|| ConfigError::InvalidPath("config file has no folder".into()))?
        .to_path_buf();
    let file_name = path.file_name().map(|n| n.to_os_string());

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(event.kind, notify::EventKind::Access(_)) {
                return;
            }
            if event
                .paths
                .iter()
                .any(|p| p.file_name() == file_name.as_deref())
            {
                let _ = tx.send(());
            }
        }
    })
    .map_err(|e| ConfigError::InvalidPath(format!("cannot watch the config file: {}", e)))?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| ConfigError::InvalidPath(format!("cannot watch {}: {}", dir.display(), e)))?;

    let reload_path = path.clone();
    std::thread::Builder::new()
        .name("config-watch".into())
        .spawn(move || {
            // Ends when the watcher (and with it the sender) is dropped.
            while rx.recv().is_ok() {
                loop {
                    match rx.recv_timeout(debounce) {
                        Ok(()) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if !reload_path.exists() {
                    continue;
                }
                // Failure is logged and recorded inside.
                let _ = load_and_install(&reload_path);
            }
        })
        .map_err(ConfigError::IoError)?;

    Ok(ConfigFileWatcher {
        _watcher: watcher,
        path,
    })
}
