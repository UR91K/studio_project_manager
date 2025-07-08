//! Collections-related gRPC tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following tests from src/grpc/server.rs:
//! - test_get_collections_empty() (line ~831)
//! - test_create_collection() (line ~842)
//! - test_get_collections_with_data() (line ~857)
//! - test_update_collection() (line ~888)
//! - test_update_collection_partial() (line ~918)
//! - test_update_nonexistent_collection() (line ~934)
//! - test_add_project_to_collection() (line ~971)
//! - test_add_multiple_projects_to_collection() (line ~1020)
//! - test_remove_project_from_collection() (line ~1063)
//! - test_remove_project_maintains_order() (line ~1111)
//! - test_add_project_to_nonexistent_collection() (line ~1128)
//! - test_remove_project_from_nonexistent_collection() (line ~1144)
//! - test_collection_timestamps() (line ~1191)

use super::*;
use crate::common::setup;

// TODO: Move all collection-related gRPC tests from src/grpc/server.rs
// This includes all GetCollections, CreateCollection, UpdateCollection, 
// AddProjectToCollection, and RemoveProjectFromCollection tests
// Total: ~13 tests to move 