use crate::{scan::parser::basic::{assert_clean_state, create_test_scanner, process_xml, setup_valid_scanner}};
use studio_project_manager::scan::parser::ParseResult;
use studio_project_manager::models::PluginFormat;
use quick_xml::Reader;

#[test]
fn test_interleaved_plugins_and_sample() {
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
        <SampleRef>
            <FileRef>
                <Path Value="C:/test/sample.wav" />
            </FileRef>
        </SampleRef>
        <SourceContext>
            <Value>
                <BranchSourceContext Id="1">
                    <OriginalFileRef />
                    <BrowserContentPath Value="view:X-Plugins#Altiverb%207" />
                    <LocalFiltersJson Value="{&quot;local-filters&quot;:{&quot;devtype&quot;:[&quot;audio-fx&quot;],&quot;devarch&quot;:[&quot;plugin-vst&quot;]}}" />
                    <PresetRef />
                    <BranchDeviceId Value="device:vst:audiofx:1096184373?n=Altiverb%207" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <VstPluginInfo Id="1">
                <PlugName Value="Altiverb 7" />
            </VstPluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.plugins.len(), 2);
    assert_eq!(result.samples.len(), 1);

    // Check plugins
    let proq3 = result.plugins.iter().find(|p| p.name == "Pro-Q 3").unwrap();
    assert!(
        proq3.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(
        proq3.dev_identifier,
        "device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d"
    );
    assert_eq!(proq3.plugin_format, PluginFormat::VST3AudioFx);

    let altiverb = result
        .plugins
        .iter()
        .find(|p| p.name == "Altiverb 7")
        .unwrap();
    assert!(
        altiverb.id.to_string().len() > 0,
        "Plugin should have a valid UUID"
    );
    assert_eq!(
        altiverb.dev_identifier,
        "device:vst:audiofx:1096184373?n=Altiverb%207"
    );
    assert_eq!(altiverb.plugin_format, PluginFormat::VST2AudioFx);

    // Check sample
    let sample = result.samples.iter().next().unwrap();
    assert!(
        sample.id.to_string().len() > 0,
        "Sample should have a valid UUID"
    );
    assert_eq!(sample.name, "sample.wav");
    assert_eq!(sample.path.to_str().unwrap(), "C:/test/sample.wav");

    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_missing_browser_path() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="0">
                    <BranchDeviceId Value="device:vst3:audiofx:72c4db71-7a4d-459a-b97e-51745d84b39d" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="0">
                <Name Value="Should Not Appear" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        0,
        "Should not collect plugin without browser path"
    );
    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_missing_device_id() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="1">
                    <BrowserContentPath Value="query:Everything#Missing-Device" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="1">
                <Name Value="Should Not Appear Either" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        0,
        "Should not collect plugin without device ID"
    );
    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_multiple_plugin_info() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="2">
                    <BrowserContentPath Value="query:Everything#Valid-Plugin" />
                    <BranchDeviceId Value="device:vst3:audiofx:valid-plugin-id" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="2">
                <Name Value="Valid Plugin" />
            </Vst3PluginInfo>
            <VstPluginInfo Id="2">
                <PlugName Value="Should Be Ignored" />
            </VstPluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        1,
        "Should only collect the first plugin info"
    );
    let plugin_info = scanner.plugin_info_tags.values().next().unwrap();
    assert_eq!(plugin_info.name, "Valid Plugin");
    assert_eq!(
        plugin_info.dev_identifier,
        "device:vst3:audiofx:valid-plugin-id"
    );
    assert_eq!(plugin_info.plugin_format, PluginFormat::VST3AudioFx);
    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_invalid_device_id() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <SourceContext>
            <Value>
                <BranchSourceContext Id="3">
                    <BrowserContentPath Value="query:Everything#Invalid-Format" />
                    <BranchDeviceId Value="invalid:format:not-a-plugin" />
                </BranchSourceContext>
            </Value>
        </SourceContext>
        <PluginDesc>
            <Vst3PluginInfo Id="3">
                <Name Value="Should Not Appear" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        0,
        "Should not collect plugin with invalid device ID"
    );
    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_orphaned_plugin_desc() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <PluginDesc>
            <Vst3PluginInfo Id="4">
                <Name Value="Should Not Appear" />
            </Vst3PluginInfo>
        </PluginDesc>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        0,
        "Should not collect orphaned plugin desc"
    );
    assert_clean_state(&scanner);
}

#[test]
fn test_malformed_orphaned_plugin_info() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <Vst3PluginInfo Id="5">
            <Name Value="Should Not Appear" />
        </Vst3PluginInfo>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(
        scanner.plugin_info_tags.len(),
        0,
        "Should not collect orphaned plugin info"
    );
    assert_clean_state(&scanner);
}
