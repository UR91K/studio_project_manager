//! Test for unicode encoding issues in sample path decoding
//!
//! This test focuses on the problematic Play.als project that contains
//! samples with unicode control characters and encoding issues.

use std::path::PathBuf;
use studio_project_manager::LiveSet;

use crate::{common::setup, scan::parser::macos_sample_paths};

#[test]
fn test_play_project_unicode_encoding() {
    setup("error");
    // Path to the problematic project
    let project_path = PathBuf::from("C:\\Users\\judee\\Documents\\Projects\\Misc\\Play\\Play.als");

    // Verify the file exists
    assert!(
        project_path.exists(),
        "Play.als project file not found at {:?}",
        project_path
    );

    println!("Testing unicode encoding issues in Play.als project...");

    // Parse the project
    match LiveSet::new(project_path) {
        Ok(live_set) => {
            println!("Successfully parsed Play.als project");
            println!("Project name: {}", live_set.name);
            println!("Found {} samples", live_set.samples.len());

            // Check for problematic samples with unicode control characters
            let problematic_samples: Vec<_> = live_set
                .samples
                .iter()
                .filter(|sample| {
                    sample.name.contains('\u{0002}')
                        || sample.name.contains('\u{0000}')
                        || sample.path.to_string_lossy().contains('\u{0002}')
                        || sample.path.to_string_lossy().contains('\u{0000}')
                })
                .collect();

            println!(
                "Found {} samples with unicode control characters:",
                problematic_samples.len()
            );

            for (i, sample) in problematic_samples.iter().enumerate() {
                println!("Problematic sample {}:", i + 1);
                println!("  ID: {}", sample.id);
                println!("  Name: {:?}", sample.name);
                println!("  Path: {:?}", sample.path);

                // Analyze the unicode characters in the name
                println!("  Name bytes: {:?}", sample.name.as_bytes());
                println!(
                    "  Name chars: {:?}",
                    sample.name.chars().collect::<Vec<_>>()
                );

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
    let problematic_samples: Vec<_> = live_set
        .samples
        .iter()
        .filter(|sample| sample.name.contains('\u{0002}') || sample.name.contains('\u{0000}'))
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
    setup("error");
    // Test various unicode edge cases that might appear in sample paths
    let test_cases = vec![
        "\u{0002}test.wav",     // Starts with STX
        "test\u{0000}test.wav", // Contains NULL
        "\u{0001}test.wav",     // Starts with SOH
        "test\u{0003}test.wav", // Contains ETX
        "test\u{0004}test.wav", // Contains EOT
        "test\u{0005}test.wav", // Contains ENQ
        "test\u{0006}test.wav", // Contains ACK
        "test\u{0007}test.wav", // Contains BEL
        "test\u{0008}test.wav", // Contains BS
        "test\u{0009}test.wav", // Contains TAB
        "test\u{000A}test.wav", // Contains LF
        "test\u{000B}test.wav", // Contains VT
        "test\u{000C}test.wav", // Contains FF
        "test\u{000D}test.wav", // Contains CR
        "test\u{000E}test.wav", // Contains SO
        "test\u{000F}test.wav", // Contains SI
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
        assert_eq!(
            path_str,
            reconstructed.to_string_lossy(),
            "Path string reconstruction should be consistent"
        );
    }
}

#[test]
fn test_unicode_normalization() {
    setup("error");
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

#[test]
fn test_path_decoding_macos_alias() {
    setup("error");
    let path_data: &str = macos_sample_paths::HEX_DATA;
    let path = studio_project_manager::utils::samples::decode_sample_path(path_data);
    let path = path.unwrap();
    println!("Path: {:?}", path);
    let file_name = path.file_name().unwrap().to_string_lossy();
    println!("File name: {:?}", file_name);
    assert_eq!(file_name, "6-Audio 0002 [2020-03-09 225038].aif");
}

#[test]
fn test_path_decoding_valid() {
    setup("error");
    let path_data: &str = "
            45003A005C00530061006D0070006C00650073005C006400720075006D0073005C0059006F007500
            6E00670020004B00690063006F0020002D0020004300680072006F006E00690063006C0065007300
            20004F00660020005400680065002000410074006C0061006E007400690063002000540072006100
            70002000280053006F0075006E00640020004B006900740029002000400059006F0075006E006700
            4B00690063006F005C0030003200200038003000380073005C003800300038002000310020002800
            4C0041005900450052002000570049005400480020004B00490043004B002000310029002E007700
            610076000000
        ";
    use hex::decode;
    let cleaned_path = path_data
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let byte_data = decode(&cleaned_path).unwrap();
    let utf16_chunks: Vec<u16> = byte_data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    println!(
        "First 32 u16 values: {:?}",
        &utf16_chunks[..32.min(utf16_chunks.len())]
    );
    // Print the entire lossy-decoded string
    let lossy = String::from_utf16_lossy(&utf16_chunks);
    println!("Lossy UTF-16 decode: {:?}", lossy);
    let pat = [0x0036, 0x002D, 0x0041];
    let idx = utf16_chunks.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = idx {
        println!("Found expected filename start at index {}", i);
        println!(
            "Next 64 u16 values: {:?}",
            &utf16_chunks[i..i + 64.min(utf16_chunks.len() - i)]
        );
        let s = String::from_utf16(&utf16_chunks[i..i + 64.min(utf16_chunks.len() - i)])
            .unwrap_or_else(|_| "<decode error>".to_string());
        println!("Decoded string from there: {:?}", s);
    } else {
        println!("Did not find expected filename sequence in utf16_chunks");
    }
    let path = studio_project_manager::utils::samples::decode_sample_path(path_data);
    if let Err(ref e) = path {
        eprintln!("Path decoding failed with error: {:?}", e);
    }
    use std::path::Path;
    let path_buf = path.unwrap();
    let file_name = Path::new(&path_buf).file_name().unwrap().to_string_lossy();
    assert_eq!(file_name, "808 1 (LAYER WITH KICK 1).wav");
}
