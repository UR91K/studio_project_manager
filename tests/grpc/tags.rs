//! Tags-related gRPC tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following tests from src/grpc/server.rs:
//! - test_get_tags_empty() (line ~1204)
//! - test_create_tag() (line ~1225)
//! - test_get_tags_with_data() (line ~1269)
//! - test_tag_project() (line ~1303)
//! - test_untag_project() (line ~1517)
//! - test_tag_project_nonexistent_project() (line ~1535)
//! - test_tag_project_nonexistent_tag() (line ~1565)
//! - test_create_duplicate_tag() (line ~1596)
//! - test_tag_project_idempotent() (line ~1596) 

use super::*;
use crate::common::setup;

// TODO: Move all tag-related gRPC tests from src/grpc/server.rs
// This includes all GetTags, CreateTag, TagProject, and UntagProject tests
// Total: ~9 tests to move
// These tests are in the "TAG TESTS" section starting around line 1204 