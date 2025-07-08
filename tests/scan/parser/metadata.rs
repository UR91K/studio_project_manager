//! Metadata parsing tests (tempo, key, time signature, etc.)
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following metadata-related tests from src/scan/parser_test.rs:
//! - test_tempo_automation_parsing() (line ~725)
//! - test_complex_time_signature() (line ~794)
//! - test_key_signature_edge_cases() (line ~823)
//! - test_project_length_calculation() (line ~842)
//! - test_metadata_edge_cases() (line ~968)

use super::*;
use crate::common::setup;

// TODO: Move all metadata-related parsing tests from src/scan/parser_test.rs
// TODO: These tests focus on musical metadata like tempo, key, time signature
// TODO: Also includes project length and other derived metadata
// Total: ~5 tests to move 