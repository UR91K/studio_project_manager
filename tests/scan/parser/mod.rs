//! Parser tests
//!
//! This module contains all tests for the XML parser functionality
//! These tests were previously in src/scan/parser_test.rs

pub mod basic;
pub mod edge_cases;
pub mod macos_sample_paths;
pub mod plugins;
pub mod samples;
pub mod unicode_encoding;

// Common imports for parser tests
use quick_xml::events::Event;
use quick_xml::Reader;
use studio_project_manager::error::LiveSetError;
use studio_project_manager::models::*;
use studio_project_manager::scan::parser::*;
