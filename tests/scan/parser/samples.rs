//! Sample parsing tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following sample-related tests from src/scan/parser_test.rs:
//! - test_sample_parsing() (line ~523)
//! - test_sample_path_normalization() (line ~554)
//! - test_sample_deduplication() (line ~577)
//! - test_missing_sample_detection() (line ~598)
//! - test_sample_with_special_characters() (line ~658)

use super::*;
use crate::common::setup;

// TODO: Move all sample-related parsing tests from src/scan/parser_test.rs
// TODO: These tests focus on sample path parsing, normalization, and deduplication
// Total: ~5 tests to move 