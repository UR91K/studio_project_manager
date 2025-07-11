//! Test for unicode encoding issues in sample path decoding
//! 
//! This test focuses on the problematic Play.als project that contains
//! samples with unicode control characters and encoding issues.

use std::path::PathBuf;
use studio_project_manager::{
    scan::parser::Parser,
    LiveSet,
    error::LiveSetError,
};

#[test]
fn test_play_project_unicode_encoding() {
    // Path to the problematic project
    let project_path = PathBuf::from("C:\\Users\\judee\\Documents\\Projects\\Misc\\Play\\Play.als");
    
    // Verify the file exists
    assert!(project_path.exists(), "Play.als project file not found at {:?}", project_path);
    
    println!("Testing unicode encoding issues in Play.als project...");
    
    // Parse the project
    match LiveSet::new(project_path) {
        Ok(live_set) => {
            println!("Successfully parsed Play.als project");
            println!("Project name: {}", live_set.name);
            println!("Found {} samples", live_set.samples.len());
            
            // Check for problematic samples with unicode control characters
            let problematic_samples: Vec<_> = live_set.samples
                .iter()
                .filter(|sample| {
                    sample.name.contains('\u{0002}') || 
                    sample.name.contains('\u{0000}') ||
                    sample.path.to_string_lossy().contains('\u{0002}') ||
                    sample.path.to_string_lossy().contains('\u{0000}')
                })
                .collect();
            
            println!("Found {} samples with unicode control characters:", problematic_samples.len());
            
            for (i, sample) in problematic_samples.iter().enumerate() {
                println!("Problematic sample {}:", i + 1);
                println!("  ID: {}", sample.id);
                println!("  Name: {:?}", sample.name);
                println!("  Path: {:?}", sample.path);
                
                // Analyze the unicode characters in the name
                println!("  Name bytes: {:?}", sample.name.as_bytes());
                println!("  Name chars: {:?}", sample.name.chars().collect::<Vec<_>>());
                
                // Check for specific problematic patterns
                if sample.name.starts_with('\u{0002}') {
                    println!("  WARNING: Sample name starts with STX control character");
                }
                
                if sample.name.contains('\u{0000}') {
                    println!("  WARNING: Sample name contains NULL control character");
                }
            }
            
            // Test the reverse lookup functionality
            test_reverse_lookup(&live_set);
            
        }
        Err(e) => {
            eprintln!("Failed to parse Play.als project: {:?}", e);
            panic!("Project parsing failed: {:?}", e);
        }
    }
}

fn test_reverse_lookup(live_set: &LiveSet) {
    println!("\nTesting reverse lookup for problematic samples...");
    
    // Find samples with unicode issues
    let problematic_samples: Vec<_> = live_set.samples
        .iter()
        .filter(|sample| {
            sample.name.contains('\u{0002}') || 
            sample.name.contains('\u{0000}')
        })
        .collect();
    
    for sample in problematic_samples {
        println!("Sample ID: {}", sample.id);
        println!("Sample name: {:?}", sample.name);
        
        // Test that we can find this sample in the project
        let found = live_set.samples.iter().find(|s| s.id == sample.id);
        assert!(found.is_some(), "Should be able to find sample by ID");
        
        // Test that the sample path can be converted to string
        let path_str = sample.path.to_string_lossy();
        println!("Path as string: {:?}", path_str);
        
        // Test that we can create a PathBuf from the string
        let reconstructed_path = PathBuf::from(path_str.to_string());
        println!("Reconstructed path: {:?}", reconstructed_path);
        
        // Test that the path exists (if it's a real file)
        if sample.path.exists() {
            println!("Sample file exists at: {:?}", sample.path);
        } else {
            println!("WARNING: Sample file does not exist at: {:?}", sample.path);
        }
    }
}

#[test]
fn test_sample_path_decoding_edge_cases() {
    // Test various unicode edge cases that might appear in sample paths
    let test_cases = vec![
        "\u{0002}test.wav",           // Starts with STX
        "test\u{0000}test.wav",       // Contains NULL
        "\u{0001}test.wav",           // Starts with SOH
        "test\u{0003}test.wav",       // Contains ETX
        "test\u{0004}test.wav",       // Contains EOT
        "test\u{0005}test.wav",       // Contains ENQ
        "test\u{0006}test.wav",       // Contains ACK
        "test\u{0007}test.wav",       // Contains BEL
        "test\u{0008}test.wav",       // Contains BS
        "test\u{0009}test.wav",       // Contains TAB
        "test\u{000A}test.wav",       // Contains LF
        "test\u{000B}test.wav",       // Contains VT
        "test\u{000C}test.wav",       // Contains FF
        "test\u{000D}test.wav",       // Contains CR
        "test\u{000E}test.wav",       // Contains SO
        "test\u{000F}test.wav",       // Contains SI
    ];
    
    for test_case in test_cases {
        println!("Testing path: {:?}", test_case);
        
        // Test PathBuf creation
        let path = PathBuf::from(test_case);
        let path_str = path.to_string_lossy();
        println!("  PathBuf created: {:?}", path);
        println!("  As string: {:?}", path_str);
        
        // Test that we can reconstruct the path
        let reconstructed = PathBuf::from(path_str.to_string());
        println!("  Reconstructed: {:?}", reconstructed);
        
        // Test that the string representation is consistent
        assert_eq!(path_str, reconstructed.to_string_lossy(), 
                  "Path string reconstruction should be consistent");
    }
}

#[test]
fn test_unicode_normalization() {
    // Test various unicode normalization approaches
    let problematic_string = "\u{0002}Ȁ䌃䅏쯔剳⭈̀᥾匕慮敲㠭㠰吭湯㍥爭眮癡̀㑾痚䣃￿￿";
    
    println!("Testing unicode normalization for problematic string:");
    println!("Original: {:?}", problematic_string);
    println!("Bytes: {:?}", problematic_string.as_bytes());
    
    // Test removing control characters
    let cleaned: String = problematic_string
        .chars()
        .filter(|&c| c >= ' ' || c == '\n' || c == '\r' || c == '\t')
        .collect();
    
    println!("After removing control chars: {:?}", cleaned);
    
    // Test UTF-8 validation
    match std::str::from_utf8(problematic_string.as_bytes()) {
        Ok(s) => println!("UTF-8 validation passed: {:?}", s),
        Err(e) => println!("UTF-8 validation failed: {:?}", e),
    }
    
    // Test if the string can be safely used in a file path
    let test_path = PathBuf::from(problematic_string);
    println!("As PathBuf: {:?}", test_path);
    println!("Path as string: {:?}", test_path.to_string_lossy());
} 