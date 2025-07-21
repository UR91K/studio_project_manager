use log::{debug, trace};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::error::LiveSetError;
use crate::live_set::LiveSet;

/// Result type for parsing operations
type ParseResult = Result<(PathBuf, LiveSet), (PathBuf, LiveSetError)>;

/// Worker for parsing individual Live Set files
pub struct ParserWorker {
    sender: Sender<ParseResult>,
}

impl ParserWorker {
    fn new(sender: Sender<ParseResult>) -> Self {
        Self { sender }
    }

    fn process_file(&self, path: PathBuf) {
        let result = LiveSet::new(path.clone())
            .map(|live_set| (path.clone(), live_set))
            .map_err(|err| (path, err));

        // Send result back to coordinator
        let _ = self.sender.send(result);
    }
}

/// Manages parallel parsing of Live Set files
pub struct ParallelParser {
    #[allow(unused)]
    thread_count: usize,
    workers: Vec<JoinHandle<()>>,
    results_rx: Receiver<ParseResult>,
    work_tx: Arc<Mutex<Option<Sender<PathBuf>>>>,
}

impl ParallelParser {
    /// Create a new parallel parser with specified thread count
    pub fn new(thread_count: usize) -> Self {
        let (results_tx, results_rx): (Sender<ParseResult>, Receiver<ParseResult>) = channel();
        let (work_tx, work_rx): (Sender<PathBuf>, Receiver<PathBuf>) = channel();
        let work_tx = Arc::new(Mutex::new(Some(work_tx)));
        let work_rx = Arc::new(Mutex::new(work_rx));

        // Create worker threads
        let mut workers = Vec::with_capacity(thread_count);
        for thread_id in 0..thread_count {
            let results_tx = results_tx.clone();
            let work_rx = Arc::clone(&work_rx);

            let handle = thread::spawn(move || {
                trace!("Worker thread {} started", thread_id);
                let worker = ParserWorker::new(results_tx);

                while let Ok(path) = work_rx.lock().unwrap().recv() {
                    trace!("Worker {} processing file: {}", thread_id, path.display());
                    worker.process_file(path);
                }
                trace!("Worker thread {} exiting", thread_id);
            });

            workers.push(handle);
        }

        Self {
            thread_count,
            workers,
            results_rx,
            work_tx,
        }
    }

    /// Submit paths for parsing
    pub fn submit_paths(&self, paths: Vec<PathBuf>) -> Result<(), LiveSetError> {
        debug!("Submitting {} paths to worker threads", paths.len());
        if let Some(tx) = self.work_tx.lock().unwrap().as_ref() {
            for path in paths {
                trace!("Sending path to worker: {}", path.display());
                tx.send(path).map_err(|_| {
                    LiveSetError::InvalidProject("Failed to send path to worker thread".to_string())
                })?;
            }
            trace!("Finished submitting all paths");
            Ok(())
        } else {
            Err(LiveSetError::InvalidProject(
                "Worker threads are no longer available".to_string(),
            ))
        }
    }

    /// Get receiver for parsing results
    pub fn get_results_receiver(&self) -> &Receiver<ParseResult> {
        &self.results_rx
    }
}

impl Drop for ParallelParser {
    fn drop(&mut self) {
        trace!("ParallelParser being dropped, signaling workers to stop");
        // Drop work sender to signal workers to stop
        self.work_tx.lock().unwrap().take();

        trace!("Waiting for {} workers to complete", self.workers.len());
        // Wait for all workers to complete
        for (i, worker) in self.workers.drain(..).enumerate() {
            trace!("Waiting for worker {} to complete", i);
            let _ = worker.join();
            trace!("Worker {} completed", i);
        }
        debug!("All workers completed");
    }
}
