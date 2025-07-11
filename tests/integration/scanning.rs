//! Scanning integration tests

use crate::common::setup;
use studio_project_manager::LiveSet;
use studio_project_manager::{
    config::CONFIG,
    scan::project_scanner::ProjectPathScanner,
    database::LiveSetDatabase,
    process_projects,
    process_projects_with_progress,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_process_projects_integration() {
    setup("debug");

    // Get expected project paths from config
    let config = CONFIG.as_ref().expect("Failed to load config");
    let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
    
    // Scan configured paths to know what we expect to find
    let mut expected_projects = HashSet::new();
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            let projects = scanner.scan_directory(&path).expect("Failed to scan directory");
            expected_projects.extend(projects);
        }
    }
    
    assert!(!expected_projects.is_empty(), "No projects found in configured paths");
    let expected_project_names: HashSet<String> = expected_projects
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    // Run process_projects
    process_projects().expect("process_projects failed");

    // Open database and verify contents
    let database_path = config.database_path.as_ref().expect("Database path should be set by config initialization");
    let db = LiveSetDatabase::new(PathBuf::from(database_path))
        .expect("Failed to open database");

    // Get actual project names from database
    let mut stmt = db.conn.prepare("SELECT name FROM projects").expect("Failed to prepare query");
    let project_names: HashSet<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("Failed to execute query")
        .map(|r| r.expect("Failed to get project name"))
        .collect();

    // Verify all expected projects were processed
    for expected_name in &expected_project_names {
        assert!(
            project_names.contains(expected_name),
            "Project '{}' not found in database", expected_name
        );
    }

    // Get some statistics
    let project_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .expect("Failed to count projects");
    let plugin_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
        .expect("Failed to count plugins");
    let sample_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))
        .expect("Failed to count samples");

    println!("\nDatabase Statistics:");
    println!("Projects found: {}", project_count);
    println!("Plugins found: {}", plugin_count);
    println!("Samples found: {}", sample_count);
    println!("\nProjects processed:");
    for name in &project_names {
        println!("- {}", name);
    }
}

#[test]
fn test_process_projects_with_progress() {
    setup("info");

    // Get expected project paths from config
    let config = CONFIG.as_ref().expect("Failed to load config");
    let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
    
    // Scan configured paths to know what we expect to find
    let mut expected_projects = HashSet::new();
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            let projects = scanner.scan_directory(&path).expect("Failed to scan directory");
            expected_projects.extend(projects);
        }
    }
    
    if expected_projects.is_empty() {
        println!("No projects found in configured paths, skipping test");
        return;
    }

    // Track progress updates
    let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(u32, u32, f32, String, String)>::new()));
    let progress_updates_clone = Arc::clone(&progress_updates);

    // Create progress callback that captures all updates
    let progress_callback = move |completed: u32, total: u32, progress: f32, message: String, phase: &str| {
        let mut updates = progress_updates_clone.lock().unwrap();
        updates.push((completed, total, progress, message.clone(), phase.to_string()));
        println!("Progress: {}/{} ({:.1}%) - {} [{}]", completed, total, progress * 100.0, message, phase);
    };

    // Run process_projects_with_progress
    let result = process_projects_with_progress(Some(progress_callback));
    assert!(result.is_ok(), "process_projects_with_progress failed: {:?}", result.err());

    // Verify progress updates
    let updates = progress_updates.lock().unwrap();
    assert!(!updates.is_empty(), "No progress updates received");

    // Check that we got the expected progression of phases
    let phases: Vec<String> = updates.iter().map(|(_, _, _, _, phase)| phase.clone()).collect();
    println!("Received phases: {:?}", phases);

    // Should have at least starting and completed phases
    assert!(phases.contains(&"starting".to_string()), "Missing 'starting' phase");
    assert!(phases.contains(&"completed".to_string()) || phases.contains(&"preprocessing".to_string()), 
        "Missing completion phase");

    // Check that progress values make sense
    for (i, (completed, total, progress, _, _)) in updates.iter().enumerate() {
        // Progress should be between 0 and 1
        assert!(*progress >= 0.0 && *progress <= 1.0, 
            "Progress out of range at update {}: {}", i, progress);
        
        // If total > 0, completed should not exceed total
        if *total > 0 {
            assert!(*completed <= *total, 
                "Completed {} exceeds total {} at update {}", completed, total, i);
        }
    }

    // Check final progress should be complete
    if let Some((_, _, final_progress, _, final_phase)) = updates.last() {
        if final_phase == "completed" {
            assert_eq!(*final_progress, 1.0, "Final progress should be 1.0, got {}", final_progress);
        }
    }

    println!("✅ Progress streaming test passed with {} updates", updates.len());
}

#[test]
fn test_path_decoding_invalid() {
    setup("trace");
    let path_data: &str = "
            0000000002380002000003434F41000000000000000000000000000000000000000000000000D4CB
            7352482B00000003899B1F362D417564696F2030303032205B323032302D30332333383942352E61
            69660000000000000000000000000000000000000000000000000000000000000000000389B5DA8C
            78BE4149464600000000FFFFFFFF00000900000000000000000000000000000000085265636F7264
            6564001000080000D4CB57320000001100080000DA8C5C9E00000001001C0003899B00037E180003
            7E1000039078000331B50003272100032ED800020076434F413A432E4F2E412E3A00574950203A00
            536F6E67733A004D69782026204D61737465723A0044616E636520566964656F202050726F6A6563
            742031333562706D3A0053616D706C65733A005265636F726465643A00362D417564696F20303030
            32205B323032302D30332333383942352E616966000E004A00240036002D0041007500640069006F
            002000300030003000320020005B0032003000320030002D00300033002D00300039002000320032
            0035003000330038005D002E006100690066000F000800030043004F0041001200712F432E4F2E41
            2E2F574950202F536F6E67732F4D69782026204D61737465722F44616E636520566964656F202050
            726F6A6563742031333562706D2F53616D706C65732F5265636F726465642F362D417564696F2030
            303032205B323032302D30332D3039203232353033385D2E616966000013000C2F566F6C756D6573
            2F434F41FFFF0000
        ";
    use hex::decode;
    let cleaned_path = path_data.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let byte_data = decode(&cleaned_path).unwrap();
    let utf16_chunks: Vec<u16> = byte_data.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]])).collect();
    println!("First 32 u16 values: {:?}", &utf16_chunks[..32.min(utf16_chunks.len())]);
    // Print the entire lossy-decoded string
    let lossy = String::from_utf16_lossy(&utf16_chunks);
    println!("Lossy UTF-16 decode: {:?}", lossy);
    let pat = [0x0036, 0x002D, 0x0041];
    let idx = utf16_chunks.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = idx {
        println!("Found expected filename start at index {}", i);
        println!("Next 64 u16 values: {:?}", &utf16_chunks[i..i+64.min(utf16_chunks.len()-i)]);
        let s = String::from_utf16(&utf16_chunks[i..i+64.min(utf16_chunks.len()-i)]).unwrap_or_else(|_| "<decode error>".to_string());
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
    assert_eq!(file_name, "6-Audio 0002 [2020-03-09 225038].aif");
}

#[test]
fn test_path_decoding_valid() {
    setup("trace");
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
    let cleaned_path = path_data.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let byte_data = decode(&cleaned_path).unwrap();
    let utf16_chunks: Vec<u16> = byte_data.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]])).collect();
    println!("First 32 u16 values: {:?}", &utf16_chunks[..32.min(utf16_chunks.len())]);
    // Print the entire lossy-decoded string
    let lossy = String::from_utf16_lossy(&utf16_chunks);
    println!("Lossy UTF-16 decode: {:?}", lossy);
    let pat = [0x0036, 0x002D, 0x0041];
    let idx = utf16_chunks.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = idx {
        println!("Found expected filename start at index {}", i);
        println!("Next 64 u16 values: {:?}", &utf16_chunks[i..i+64.min(utf16_chunks.len()-i)]);
        let s = String::from_utf16(&utf16_chunks[i..i+64.min(utf16_chunks.len()-i)]).unwrap_or_else(|_| "<decode error>".to_string());
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

#[test]
fn test_path_decoding_multiple_strategies() {
    setup("trace");
    let path_data: &str = "
            0000000002380002000003434F41000000000000000000000000000000000000000000000000D4CB
            7352482B00000003899B1F362D417564696F2030303032205B323032302D30332333383942352E61
            69660000000000000000000000000000000000000000000000000000000000000000000389B5DA8C
            78BE4149464600000000FFFFFFFF00000900000000000000000000000000000000085265636F7264
            6564001000080000D4CB57320000001100080000DA8C5C9E00000001001C0003899B00037E180003
            7E1000039078000331B50003272100032ED800020076434F413A432E4F2E412E3A00574950203A00
            536F6E67733A004D69782026204D61737465723A0044616E636520566964656F202050726F6A6563
            742031333562706D3A0053616D706C65733A005265636F726465643A00362D417564696F20303030
            32205B323032302D30332333383942352E616966000E004A00240036002D0041007500640069006F
            002000300030003000320020005B0032003000320030002D00300033002D00300039002000320032
            0035003000330038005D002E006100690066000F000800030043004F0041001200712F432E4F2E41
            2E2F574950202F536F6E67732F4D69782026204D61737465722F44616E636520566964656F202050
            726F6A6563742031333562706D2F53616D706C65732F5265636F726465642F362D417564696F2030
            303032205B323032302D30332D3039203232353033385D2E616966000013000C2F566F6C756D6573
            2F434F41FFFF0000
    ";
    use hex::decode;
    let cleaned_path = path_data.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let byte_data = decode(&cleaned_path).unwrap();

    // Try UTF-16LE
    let utf16le: Vec<u16> = byte_data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let s_le = String::from_utf16_lossy(&utf16le);
    println!("UTF-16LE: {:?}", s_le);

    // Try UTF-16BE
    let utf16be: Vec<u16> = byte_data.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    let s_be = String::from_utf16_lossy(&utf16be);
    println!("UTF-16BE: {:?}", s_be);

    // Try UTF-8
    let s_utf8 = String::from_utf8_lossy(&byte_data);
    println!("UTF-8: {:?}", s_utf8);

    // Try ASCII
    let s_ascii: String = byte_data.iter().map(|&b| if b.is_ascii() { b as char } else { '.' }).collect();
    println!("ASCII: {:?}", s_ascii);

    // Try skipping leading bytes (0..8)
    for skip in 1..8 {
        if byte_data.len() > skip {
            let utf16le_skip: Vec<u16> = byte_data[skip..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let s_skip = String::from_utf16_lossy(&utf16le_skip);
            println!("UTF-16LE (skip {}): {:?}", skip, s_skip);
        }
    }
}