//! Parser tests
//!
//! This module contains all tests for the XML parser functionality
//! These tests were previously in src/scan/parser_test.rs

pub mod basic;
pub mod plugins;
pub mod samples;
pub mod metadata;
pub mod edge_cases;

// Common imports for parser tests
use studio_project_manager::scan::parser::*;
use studio_project_manager::error::LiveSetError;
use studio_project_manager::models::*;
use crate::common::setup;
use quick_xml::events::Event;
use quick_xml::Reader; 