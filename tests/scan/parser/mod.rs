//! Parser tests
//!
//! This module contains all tests for the XML parser functionality
//! These tests were previously in src/scan/parser_test.rs

pub mod basic;
pub mod plugins;
pub mod samples;
pub mod edge_cases;
pub mod unicode_encoding;

// Common imports for parser tests
use studio_project_manager::scan::parser::*;
use studio_project_manager::error::LiveSetError;
use studio_project_manager::models::*;
use quick_xml::events::Event;
use quick_xml::Reader; 