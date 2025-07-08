//! Database search functionality tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the entire tests module from src/database/search.rs (starting around line 304)
//! Including:
//! - setup_test_projects() helper function (line ~313)
//! - test_query_parser() (line ~409)
//! - test_query_parser_interleaved() (line ~420)
//! - test_multiple_operators() (line ~431)
//! - test_fts_search_by_name() (line ~442)
//! - test_search_by_plugin() (line ~459)
//! - test_search_by_bpm() (line ~471)
//! - test_search_by_path() (line ~482)
//! - test_search_by_tag() (line ~493)
//! - test_search_by_time_signature() (line ~504)
//! - test_search_by_key() (line ~515)
//! - test_search_by_created_date() (line ~525)
//! - test_search_by_modified_date() (line ~535)
//! - test_combined_search_criteria() (line ~545)

use super::*;
use crate::common::{setup, LiveSetBuilder};
use studio_project_manager::database::search::SearchQuery;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

// TODO: Move the entire tests module from src/database/search.rs
// TODO: Update imports to use crate::common instead of crate::test_utils
// Total: ~13 tests + helper function to move 