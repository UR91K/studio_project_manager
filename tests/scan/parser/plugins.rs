//! Plugin parsing tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following plugin-related tests from src/scan/parser_test.rs:
//! - test_plugin_parsing() (line ~164)
//! - test_vst3_plugin_parsing() (line ~177)
//! - test_au_plugin_parsing() (line ~190)
//! - test_max_for_live_device_parsing() (line ~205)
//! - test_plugin_deduplication() (line ~247)
//! - test_plugin_with_missing_fields() (line ~289)
//! - test_plugin_format_detection() (line ~331)
//! - test_complex_plugin_parsing() (line ~422)
//! - test_malformed_plugin_data() (line ~452)
//! - test_plugin_flags_parsing() (line ~482)

use super::*;
use crate::common::setup;

// TODO: Move all plugin-related parsing tests from src/scan/parser_test.rs
// TODO: These tests focus on parsing different plugin formats (VST, VST3, AU, Max4Live)
// TODO: Also includes plugin deduplication and error handling tests
// Total: ~10 tests to move 