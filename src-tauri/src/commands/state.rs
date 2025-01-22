use std::sync::{Arc, Mutex};
use crate::database::LiveSetDatabase;
use std::path::PathBuf;

pub struct AppState {
    pub is_scanning: Arc<Mutex<bool>>,
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let db = LiveSetDatabase::new(db_path)?;
        Ok(Self {
            is_scanning: Arc::new(Mutex::new(false)),
            db: Arc::new(Mutex::new(db)),
        })
    }
} 