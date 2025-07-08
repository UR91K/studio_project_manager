//! Common test utilities and shared setup
//! 
//! This module contains all shared test utilities that were previously in src/test_utils.rs
//! 
//! MIGRATION INSTRUCTIONS:
//! 1. Copy the entire contents of src/test_utils.rs to this file
//! 2. Remove the #[cfg(test)] attributes since this is a dedicated test module
//! 3. Make all functions and structs public (pub)
//! 4. Update any relative imports to use absolute paths from the crate root

pub mod builders;
pub mod helpers;

use std::sync::Once;
use std::env;

// Global INIT for all tests - ensures logger is initialized only once across all tests
static INIT: Once = Once::new();

/// Shared test setup function that can be used across all test files
/// This should be called at the beginning of each test to ensure proper logging setup
pub fn setup(log_level: &str) {
    let _ = INIT.call_once(|| {
        let _ = env::set_var("RUST_LOG", log_level);
        if let Err(_) = env_logger::try_init() {
            // Logger already initialized, that's fine
        }
    });
}

// TODO: Move the following items from src/test_utils.rs to this file:
// - LiveSetBuilder struct and its implementation
// - All generate_test_* functions
// - All test helper functions
// - Make sure to update imports to use crate:: paths 