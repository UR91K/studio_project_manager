#![allow(dead_code)]
use crate::database::LiveSetDatabase;
use log::{debug, info, warn};
use notify::{
    self,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::SystemTime;
use tokio::sync::Mutex;
use walkdir::WalkDir;

pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    watch_paths: HashSet<PathBuf>,
    event_tx: mpsc::Sender<FileEvent>,
    db: Arc<Mutex<LiveSetDatabase>>,
}

#[derive(Debug)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

impl FileWatcher {
    /// Creates a new FileWatcher instance
    pub fn new(
        db: Arc<Mutex<LiveSetDatabase>>,
    ) -> notify::Result<(Self, mpsc::Receiver<FileEvent>)> {
        debug!("Creating new FileWatcher instance");
        let (tx, rx) = mpsc::channel();
        let event_tx = tx.clone();

        let watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    debug!("Received filesystem event: {:?}", event.kind);
                    if let Err(e) = handle_fs_event(&event, &event_tx) {
                        warn!("Failed to handle filesystem event: {}", e);
                    }
                }
                Err(e) => warn!("Error from filesystem watcher: {}", e),
            })?;

        info!("FileWatcher created successfully");
        Ok((
            Self {
                watcher,
                watch_paths: HashSet::new(),
                event_tx: tx,
                db,
            },
            rx,
        ))
    }

    /// Add a new directory to watch
    pub fn add_watch_path(&mut self, path: PathBuf) -> notify::Result<()> {
        debug!("Adding watch path: {:?}", path);
        if self.watch_paths.insert(path.clone()) {
            self.watcher.watch(&path, RecursiveMode::Recursive)?;
            info!("Successfully added watch path: {:?}", path);
        } else {
            debug!("Path already being watched: {:?}", path);
        }
        Ok(())
    }

    /// Remove a watched directory
    pub fn remove_watch_path(&mut self, path: &Path) -> notify::Result<()> {
        debug!("Removing watch path: {:?}", path);
        if self.watch_paths.remove(path) {
            self.watcher.unwatch(path)?;
            info!("Successfully removed watch path: {:?}", path);
        } else {
            debug!("Path was not being watched: {:?}", path);
        }
        Ok(())
    }

    /// Check if a path is being watched
    pub fn is_watching(&self, path: &Path) -> bool {
        let is_watching = self.watch_paths.contains(path);
        debug!("Checking if path is watched: {:?} = {}", path, is_watching);
        is_watching
    }

    /// Get list of watched paths
    pub fn get_watch_paths(&self) -> &HashSet<PathBuf> {
        &self.watch_paths
    }

    /// Check for changes that occurred while the application was not running
    pub async fn check_offline_changes(&self) -> Result<(), Box<dyn Error>> {
        debug!("Checking for offline changes");
        let db_guard = self.db.lock().await;
        let active_projects = db_guard.get_all_projects_with_status(Some(true))?;
        debug!("Found {} active projects to check", active_projects.len());
        drop(db_guard);

        for project in active_projects {
            debug!("Checking project: {}", project.name);
            let path = &project.file_path;
            match path.metadata() {
                Ok(metadata) => {
                    let modified: SystemTime = metadata.modified()?;
                    if modified > project.last_parsed_timestamp.into() {
                        debug!("Project modified while offline: {}", project.name);
                        self.event_tx.send(FileEvent::Modified(path.clone()))?;
                    }
                }
                Err(e) => {
                    debug!("Project file no longer exists: {} ({})", project.name, e);
                    self.event_tx.send(FileEvent::Deleted(path.clone()))?;
                }
            }
        }
        info!("Completed offline changes check");
        Ok(())
    }

    /// Scan watched directories for new files
    pub async fn scan_for_new_files(&self) -> Result<(), Box<dyn Error>> {
        debug!("Starting scan for new files");
        let mut found_paths = HashSet::new();

        // First collect all .als files
        for watch_path in &self.watch_paths {
            debug!("Scanning directory: {:?}", watch_path);
            for entry in WalkDir::new(watch_path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "als") {
                    debug!("Found .als file: {:?}", path);
                    found_paths.insert(path.to_path_buf());
                }
            }
        }

        debug!("Found {} potential new files", found_paths.len());
        let mut db_guard = self.db.lock().await;
        for path in found_paths {
            if db_guard
                .get_project_by_path(&path.to_string_lossy())?
                .is_none()
            {
                debug!("New file detected: {:?}", path);
                self.event_tx.send(FileEvent::Created(path))?;
            }
        }

        info!("Completed scan for new files");
        Ok(())
    }
}

/// Helper function to handle filesystem events
fn handle_fs_event(event: &Event, tx: &mpsc::Sender<FileEvent>) -> notify::Result<()> {
    debug!("Handling filesystem event: {:?}", event.kind);
    match event.kind {
        EventKind::Create(create_kind) => match create_kind {
            CreateKind::File => {
                for path in &event.paths {
                    if path.extension().map_or(false, |ext| ext == "als") {
                        debug!("File created: {:?}", path);
                        tx.send(FileEvent::Created(path.clone()))?;
                    }
                }
            }
            _ => debug!("Ignoring non-file creation event"),
        },
        EventKind::Modify(modify_kind) => match modify_kind {
            ModifyKind::Data(_) => {
                for path in &event.paths {
                    if path.extension().map_or(false, |ext| ext == "als") {
                        debug!("File modified: {:?}", path);
                        tx.send(FileEvent::Modified(path.clone()))?;
                    }
                }
            }
            ModifyKind::Name(rename_mode) => match rename_mode {
                RenameMode::Both => {
                    if event.paths.len() == 2 {
                        let from = &event.paths[0];
                        let to = &event.paths[1];
                        if to.extension().map_or(false, |ext| ext == "als") {
                            debug!("File renamed: {:?} -> {:?}", from, to);
                            tx.send(FileEvent::Renamed {
                                from: from.clone(),
                                to: to.clone(),
                            })?;
                        }
                    }
                }
                _ => debug!("Ignoring non-Both rename event: {:?}", rename_mode),
            },
            _ => debug!(
                "Ignoring non-data/name modification event: {:?}",
                modify_kind
            ),
        },
        EventKind::Remove(remove_kind) => match remove_kind {
            RemoveKind::File => {
                for path in &event.paths {
                    if path.extension().map_or(false, |ext| ext == "als") {
                        debug!("File deleted: {:?}", path);
                        tx.send(FileEvent::Deleted(path.clone()))?;
                    }
                }
            }
            _ => debug!("Ignoring non-file removal event"),
        },
        _ => debug!("Ignoring unhandled event kind: {:?}", event.kind),
    }
    Ok(())
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        debug!("Dropping FileWatcher, cleaning up watch paths");
        for path in self.watch_paths.iter() {
            if let Err(e) = self.watcher.unwatch(path) {
                warn!("Failed to unwatch path during cleanup: {:?} ({})", path, e);
            }
        }
    }
}
