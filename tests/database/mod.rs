//! Database tests
//!
//! This module contains all database-related tests

pub mod core;
pub mod search;
pub mod batch;
pub mod collections;
pub mod tags;

// Common imports for database tests
use studio_project_manager::database::LiveSetDatabase;
use studio_project_manager::live_set::LiveSet;
use crate::common::setup;
use std::path::PathBuf;
use tempfile::tempdir; 