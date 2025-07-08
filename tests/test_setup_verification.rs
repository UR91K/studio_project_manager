//! Test to verify the common utilities work correctly

mod common;
use common::{LiveSetBuilder, setup};

#[test]
fn test_common_utilities_work() {
    setup("debug");
    
    let result = LiveSetBuilder::new()
        .with_plugin("Test Plugin")
        .with_sample("test.wav")
        .with_tempo(140.0)
        .build();

    assert_eq!(result.tempo, 140.0);
    assert_eq!(result.plugins.len(), 1);
    assert_eq!(result.samples.len(), 1);
} 