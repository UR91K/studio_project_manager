//! File event tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the entire content from src/watcher/file_watcher_test.rs:
//! - TestEnvironment struct and implementation (line ~15)
//! - All the test functions (they're currently commented out with //#[tokio::test])
//! - test_file_creation() (line ~137)
//! - test_file_modification() (line ~152)
//! - test_file_deletion() (line ~172)
//! - test_file_rename() (line ~192)
//! - test_multiple_events() (line ~213)
//! - test_large_file() (line ~224)
//! - test_scan_for_new_files() (line ~260)

use super::*;
use crate::common::setup;
use studio_project_manager::watcher::file_watcher::{FileEvent, FileWatcher};
use studio_project_manager::database::LiveSetDatabase;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::sleep;

// TODO: Move the entire content from src/watcher/file_watcher_test.rs
// TODO: Uncomment the #[tokio::test] attributes (remove the // prefix)
// TODO: Update any imports as needed
// Total: TestEnvironment struct + ~7 tests to move 