//! Plugin parsing tests

use super::*;
use crate::{common::setup, scan::parser::basic::{create_test_scanner, process_xml, setup_valid_scanner}};

#[test]
fn test_vst3_audio_fx() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Pro-Q%203" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="Pro-Q 3" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(plugin.name, "Pro-Q 3");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3AudioFx);
}

#[test]
fn test_vst2_audio_fx() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="view:X-Plugins#Altiverb%207" />
                    <LocalFiltersJson Value="{&quot;local-filters&quot;:{&quot;devtype&quot;:[&quot;audio-fx&quot;],&quot;devarch&quot;:[&quot;plugin-vst&quot;]}}" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst:audiofx:1096184373?n=Altiverb%207" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <VstPluginInfo Id="0">
                <PlugName Value="Altiverb 7" />
            </VstPluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(plugin.name, "Altiverb 7");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst:audiofx:1096184373?n=Altiverb%207"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST2AudioFx);
}

#[test]
fn test_vst3_instrument() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Omnisphere" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:instr:84e8de5f-9255-2222-96fa-e4133c935a18" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="Omnisphere" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(plugin.name, "Omnisphere");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:instr:84e8de5f-9255-2222-96fa-e4133c935a18"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3Instrument);
}

#[test]
fn test_empty_plugin_name() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Empty%20Plugin" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    // The plugin name should be "Pro-Q 3" from the database, not empty
    assert_eq!(plugin.name, "Pro-Q 3");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3AudioFx);
}

#[test]
fn test_whitespace_only_plugin_name() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Whitespace%20Plugin" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="   " />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    // The plugin name should be "Pro-Q 3" from the database, not the whitespace-only name
    assert_eq!(plugin.name, "Pro-Q 3");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3AudioFx);
}

#[test]
fn test_empty_plugin_name_unique_device_id() {
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Unique%20Plugin" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:audiofx:unique-device-id-that-wont-exist-in-db" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    // This should be empty because the device ID won't be found in the database
    assert_eq!(plugin.name, "");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:audiofx:unique-device-id-that-wont-exist-in-db"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3AudioFx);
    // Should not be installed since it's not in the database
    assert_eq!(plugin.installed, false);
}

#[test]
fn test_plugin_with_non_empty_xml_name_but_empty_db_name() {
    setup("debug");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Everything#Plugin%20With%20Name" />
                    <LocalFiltersJson Value="" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:instr:f2aee70d-00de-4f4e-4675-427566623333" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="Plugin With Name" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    // The plugin should have the name from the XML, not from the database
    // If the database has an empty name, it should not override the XML name
    assert_eq!(plugin.name, "Plugin With Name");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:instr:f2aee70d-00de-4f4e-4675-427566623333"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3Instrument);
}

#[test]
fn test_psp_springbox_xml_parsing() {
    use crate::scan::parser::basic::{create_test_scanner, process_xml};
    use crate::scan::parser::ParseResult;
    use seula::models::PluginFormat;
    
    setup("error");
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);
    
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <OriginalFileRef />
                    <BrowserContentPath Value="query:Plugins#VST3:PSPaudioware.com:PSP%20SpringBox" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst3:audiofx:13b117f4-1b21-3a38-7923-ff895d3b3131" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <WinPosX Value="47" />
                <WinPosY Value="72" />
                <Preset>
                    <Vst3Preset Id="3">
                        <OverwriteProtectionNumber Value="2561" />
                        <ParameterSettings />
                        <IsOn Value="true" />
                        <PowerMacroControlIndex Value="-1" />
                        <PowerMacroMappingRange>
                            <Min Value="64" />
                            <Max Value="127" />
                        </PowerMacroMappingRange>
                        <IsFolded Value="false" />
                        <StoredAllParameters Value="true" />
                        <DeviceLomId Value="0" />
                        <DeviceViewLomId Value="0" />
                        <IsOnLomId Value="0" />
                        <ParametersListWrapperLomId Value="0" />
                        <Uid>
                            <Fields.0 Value="330373108" />
                            <Fields.1 Value="455162424" />
                            <Fields.2 Value="2032402313" />
                            <Fields.3 Value="1564160305" />
                        </Uid>
                        <DeviceType Value="2" />
                        <ProcessorState>
                            00000BA00F03F
                        </ProcessorState>
                        <ControllerState>
                            0000FFFFA0AB0
                        </ControllerState>
                        <Name Value="" />
                        <PresetRef />
                    </Vst3Preset>
                </Preset>
                <Name Value="PSP SpringBox" />
                <Uid>
                    <Fields.0 Value="330373108" />
                    <Fields.1 Value="455162424" />
                    <Fields.2 Value="2032402313" />
                    <Fields.3 Value="1564160305" />
                </Uid>
                <DeviceType Value="2" />
            </Vst3PluginInfo>
        </PluginDesc>
        "#,
    );

    process_xml(&mut scanner, &mut reader);

    // Debug: Check what was stored in the parser
    println!("Debug: current_branch_info = {:?}", scanner.current_branch_info);
    println!("Debug: plugin_info_tags = {:?}", scanner.plugin_info_tags);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 1);
    let plugin = result.plugins.iter().next().unwrap();
    assert!(
        plugin.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(plugin.name, "PSP SpringBox", "Plugin name should be 'PSP SpringBox'");
    assert_eq!(
        plugin.dev_identifier,
        "device:vst3:audiofx:13b117f4-1b21-3a38-7923-ff895d3b3131"
    );
    assert_eq!(plugin.plugin_format, PluginFormat::VST3AudioFx);
}

#[test]
fn test_psp_springbox_plugin_from_real_project() {
    use std::path::PathBuf;
    use std::collections::HashSet;
    use seula::database::LiveSetDatabase;
    use seula::scan::project_scanner::ProjectPathScanner;
    use seula::database::batch::BatchInsertManager;
    use std::sync::Arc;
    
    setup("error");
    
    // Path to the real project file
    let project_path = PathBuf::from(r"C:\Users\judee\Documents\Projects\Misc\FAMILY DUB Project\FAMILY DUB.als");
    
    // Check if the project file exists
    if !project_path.exists() {
        eprintln!("Project file not found: {:?}", project_path);
        return; // Skip test if file doesn't exist
    }
    
    // Create a temporary database for testing
    let temp_db_path = std::env::temp_dir().join("test_psp_springbox.db");
    let mut db = LiveSetDatabase::new(temp_db_path.clone()).expect("Failed to create test database");
    
    // Use the same logic as the actual library
    // 1. Create a scanner and scan the directory
    let scanner = ProjectPathScanner::new().expect("Failed to create scanner");
    let mut found_projects = HashSet::new();
    
    // Scan the directory containing the project
    let project_dir = project_path.parent().unwrap();
    let projects = scanner.scan_directory(project_dir).expect("Failed to scan directory");
    found_projects.extend(projects);
    
    // Filter to only include our specific project
    found_projects.retain(|p| p == &project_path);
    
    if found_projects.is_empty() {
        panic!("Project not found in scan results");
    }
    
    // 2. Preprocess projects (same as library)
    let mut preprocessed = Vec::new();
    for path in found_projects {
        match seula::live_set::LiveSetPreprocessed::new(path.clone()) {
            Ok(metadata) => {
                println!("Successfully preprocessed: {}", metadata.name);
                preprocessed.push(metadata);
            }
            Err(e) => {
                eprintln!("Failed to preprocess {}: {}", path.display(), e);
                continue;
            }
        }
    }
    
    if preprocessed.is_empty() {
        panic!("No projects successfully preprocessed");
    }
    
    // 3. Parse projects (same as library)
    let mut successful_live_sets = Vec::new();
    for preprocessed_project in preprocessed {
        match seula::live_set::LiveSet::from_preprocessed(preprocessed_project) {
            Ok(live_set) => {
                println!("Successfully parsed: {}", live_set.name);
                
                // Debug: Check if PSP SpringBox plugin is present in the parsed LiveSet
                let psp_springbox_device_id = "device:vst3:audiofx:13b117f4-1b21-3a38-7923-ff895d3b3131";
                let psp_plugin = live_set.plugins.iter().find(|p| p.dev_identifier == psp_springbox_device_id);
                if let Some(plugin) = psp_plugin {
                    println!("✅ Found PSP SpringBox plugin in parsed LiveSet: name='{}', dev_id='{}'", plugin.name, plugin.dev_identifier);
                } else {
                    println!("❌ PSP SpringBox plugin NOT found in parsed LiveSet");
                    println!("Available plugins in parsed LiveSet:");
                    for plugin in &live_set.plugins {
                        println!("  - '{}' (dev_id: {})", plugin.name, plugin.dev_identifier);
                    }
                }
                
                successful_live_sets.push(live_set);
            }
            Err(e) => {
                eprintln!("Failed to parse project: {:?}", e);
                continue;
            }
        }
    }
    
    if successful_live_sets.is_empty() {
        panic!("No projects successfully parsed");
    }
    
    // 4. Insert into database using batch insert (same as library)
    let num_live_sets = successful_live_sets.len();
    println!("Inserting {} projects into database", num_live_sets);
    let live_sets = Arc::new(successful_live_sets);
    let mut batch_manager = BatchInsertManager::new(&mut db.conn, live_sets);
    let stats = batch_manager.execute().expect("Failed to execute batch insert");
    
    println!(
        "Batch insert complete: {} projects, {} plugins, {} samples",
        stats.projects_inserted, stats.plugins_inserted, stats.samples_inserted
    );
    
    // 5. Check for the specific plugin that should be PSP SpringBox
    let (plugins, _) = db.get_plugins_by_installed_status(false, Some(1000), Some(0), Some("name".to_string()), Some(false))
        .expect("Failed to get plugins");
    
    // Look for the plugin with the specific device ID that should be PSP SpringBox
    let psp_springbox_device_id = "device:vst3:audiofx:13b117f4-1b21-3a38-7923-ff895d3b3131";
    let psp_springbox = plugins.iter().find(|p| p.dev_identifier == psp_springbox_device_id);
    
    if let Some(plugin) = psp_springbox {
        println!("Found plugin with PSP SpringBox device ID: {:?}", plugin);
        if plugin.name.is_empty() {
            println!("❌ PROBLEM CONFIRMED: Plugin has empty name but should be 'PSP SpringBox'");
            println!("This confirms the issue - the plugin name from XML is being lost during database insertion");
            panic!("Plugin with device ID {} should have name 'PSP SpringBox', but has empty name", psp_springbox_device_id);
        } else {
            println!("✅ Plugin has name: '{}'", plugin.name);
            assert_eq!(plugin.name, "PSP SpringBox", "Plugin name should be 'PSP SpringBox'");
        }
    } else {
        // If not found, check all plugins to see what we have
        println!("Plugin with device ID {} not found. Available plugins:", psp_springbox_device_id);
        for plugin in &plugins {
            println!("  - '{}' (dev_id: {})", plugin.name, plugin.dev_identifier);
        }
        panic!("Plugin with device ID {} should be found in the database", psp_springbox_device_id);
    }
    
    // Clean up
    let _ = std::fs::remove_file(&temp_db_path);
}
