use super::parser::{ParseOptions, Parser, ParserState};
use crate::error::LiveSetError;
use crate::models::{PluginFormat, Scale, TimeSignature, Tonic};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::sync::Once;

use super::parser::ParseResult;

static TIME_SIGNATURE: TimeSignature = TimeSignature {
    numerator: 4,
    denominator: 4,
};

fn setup_valid_scanner(scanner: &mut Parser) {
    scanner.current_tempo = 120.0;
    scanner.current_time_signature = TIME_SIGNATURE.clone();
}

static INIT: Once = Once::new();
fn setup() {
    let _ = INIT.call_once(|| {
        let _ = std::env::set_var("RUST_LOG", "debug");
        if let Err(_) = env_logger::try_init() {
            // Logger already initialized, that's fine
        }
    });
}

fn create_test_scanner() -> Parser {
    setup();
    let xml_data = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="11" MinorVersion="12.0_12049">
<LiveSet>
    <Tempo>
        <LomId Value="0" />
        <Manual Value="120.0" />
        <MidiControllerRange>
            <Min Value="60" />
            <Max Value="200" />
        </MidiControllerRange>
        <AutomationTarget Id="8">
            <LockEnvelope Value="0" />
        </AutomationTarget>
        <ModulationTarget Id="9">
            <LockEnvelope Value="0" />
        </ModulationTarget>
    </Tempo>
"#
    )
    .into_bytes();
    Parser::new(&xml_data, ParseOptions::default()).expect("Failed to create test scanner")
}

fn create_test_scanner_with_version(version: u32) -> Parser {
    setup();
    let xml_data = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="{}.0_12049">
<LiveSet>
    <Tempo>
        <LomId Value="0" />
        <Manual Value="120.0" />
        <MidiControllerRange>
            <Min Value="60" />
            <Max Value="200" />
        </MidiControllerRange>
        <AutomationTarget Id="8">
            <LockEnvelope Value="0" />
        </AutomationTarget>
        <ModulationTarget Id="9">
            <LockEnvelope Value="0" />
        </ModulationTarget>
    </Tempo>
"#,
        version
    )
    .into_bytes();
    Parser::new(&xml_data, ParseOptions::default()).expect("Failed to create test scanner")
}

// TESTS

#[test]
fn test_time_signature() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <EnumEvent Value="201" />
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert!(scanner.current_time_signature.is_valid());
    let time_sig = scanner.current_time_signature;
    assert_eq!(time_sig.numerator, 4);
    assert_eq!(time_sig.denominator, 4);
}

#[test]
fn test_invalid_time_signature() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <EnumEvent Value="invalid" />
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert!(!scanner.current_time_signature.is_valid());
}

#[test]
fn test_furthest_bar() {
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);

    let mut reader = Reader::from_str(
        r#"
        <CurrentEnd Value="16.0" />
        <CurrentEnd Value="32.0" />
        <CurrentEnd Value="8.0" />
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert!(result.furthest_bar.is_some());
    assert_eq!(result.furthest_bar.unwrap(), 8.0); // 32.0 / 4 beats per bar = 8.0 bars
}

#[test]
fn test_furthest_bar_no_tempo() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <CurrentEnd Value="16.0" />
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default());
    assert!(result.is_err());
    match result {
        Err(LiveSetError::InvalidProject(msg)) => {
            assert_eq!(msg, "Invalid tempo value: 0");
        }
        _ => panic!("Expected InvalidProject error"),
    }
}

#[test]
fn test_version_parsing() {
    let xml_data = r#"
        <?xml version="1.0" encoding="UTF-8"?>
            <Ableton MajorVersion="5" MinorVersion="12.0_12049" SchemaChangeCount="7" Creator="Ableton Live 12.0" Revision="5094b92fa547974769f44cf233f1474777d9434a">
                <LiveSet>
                </LiveSet>
            </Ableton>"#.as_bytes();

    let scanner = Parser::new(xml_data, ParseOptions::default()).unwrap();
    assert_eq!(scanner.ableton_version.major, 12);
    assert_eq!(scanner.ableton_version.minor, 0);
    assert_eq!(scanner.ableton_version.patch, 12049);
    assert_eq!(scanner.ableton_version.beta, false);
}

#[test]
fn test_invalid_version_format() {
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="invalid" SchemaChangeCount="7">
    <LiveSet>
    </LiveSet>
</Ableton>"#
        .as_bytes();

    let result = Parser::new(xml_data, ParseOptions::default());
    assert!(matches!(result, Err(LiveSetError::InvalidVersion(_))));
}

#[test]
fn test_missing_version() {
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" SchemaChangeCount="7">
    <LiveSet>
    </LiveSet>
</Ableton>"#
        .as_bytes();

    let result = Parser::new(xml_data, ParseOptions::default());
    assert!(matches!(result, Err(LiveSetError::MissingVersion)));
}

#[test]
fn test_beta_version() {
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12049" SchemaChangeCount="beta" Creator="Ableton Live 12.0">
    <LiveSet>
    </LiveSet>
</Ableton>"#.as_bytes();

    let scanner = Parser::new(xml_data, ParseOptions::default()).unwrap();
    assert_eq!(scanner.ableton_version.major, 12);
    assert_eq!(scanner.ableton_version.minor, 0);
    assert_eq!(scanner.ableton_version.patch, 12049);
    assert_eq!(scanner.ableton_version.beta, true);
}

#[test]
fn test_vst3_audio_fx() {
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

#[test]
fn test_sample_v12() {
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);

    let mut reader = Reader::from_str(
        r#"
        <SampleRef>
            <FileRef>
                <RelativePathType Value="1" />
                <RelativePath Value="../../../../Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                <Path Value="C:/Users/judee/Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                <Type Value="1" />
                <LivePackName Value="" />
                <LivePackId Value="" />
                <OriginalFileSize Value="146440" />
                <OriginalCrc Value="40395" />
            </FileRef>
            <LastModDate Value="1628727109" />
            <SourceContext>
                <SourceContext Id="0">
                    <OriginalFileRef>
                        <FileRef Id="0">
                            <RelativePathType Value="1" />
                            <RelativePath Value="../../../../Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                            <Path Value="C:/Users/judee/Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav" />
                            <Type Value="1" />
                            <LivePackName Value="" />
                            <LivePackId Value="" />
                            <OriginalFileSize Value="146440" />
                            <OriginalCrc Value="40395" />
                        </FileRef>
                    </OriginalFileRef>
                    <BrowserContentPath Value="view:X-Samples#FileId_689899" />
                    <LocalFiltersJson Value="" />
                </SourceContext>
            </SourceContext>
            <SampleUsageHint Value="0" />
            <DefaultDuration Value="24284" />
            <DefaultSampleRate Value="44100" />
        </SampleRef>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();

    assert_eq!(result.samples.len(), 1, "Should have collected one sample");
    let sample = result.samples.iter().next().unwrap();
    assert!(
        sample.id.to_string().len() > 0,
        "Sample should have a valid UUID"
    );
    assert_eq!(sample.path.to_str().unwrap(), "C:/Users/judee/Samples/Vintage Drum Machines/KB6_Archives_7_2017_Relaximus/Yamaha/Yamaha DTXpress/11 e - Effect 2/74 Vocal04.wav");
    assert_eq!(sample.name, "74 Vocal04.wav");

    assert_clean_state(&scanner);
}

#[test]
fn test_sample_v10() {
    let mut scanner = create_test_scanner_with_version(10);
    setup_valid_scanner(&mut scanner);

    let mut reader = Reader::from_str(
        r#"
        <SampleRef>
            <FileRef>
                <HasRelativePath Value="true" />
                <RelativePathType Value="3" />
                <RelativePath>
                    <RelativePathElement Id="9" Dir="Samples" />
                    <RelativePathElement Id="10" Dir="Processed" />
                    <RelativePathElement Id="11" Dir="Consolidate" />
                </RelativePath>
                <Name Value="YK - Retro OH (Hats) [2018-09-08 151017].wav" />
                <Type Value="1" />
                <Data>
                    43003A005C00550073006500720073005C004A007500640065005C0044006F00630075006D006500
                    6E00740073005C004D00750073006900630020004300720065006100740069006F006E005C005400
                    6500730074002000500072006F006A006500630074005C00530061006D0070006C00650073005C00
                    500072006F006300650073007300650064005C0043006F006E0073006F006C006900640061007400
                    65005C0059004B0020002D00200052006500740072006F0020004F00480020002800480061007400
                    7300290020005B0032003000310038002D00300039002D0030003800200031003500310030003100
                    37005D002E007700610076000000
                </Data>
                <RefersToFolder Value="false" />
                <SearchHint>
                    <PathHint>
                        <RelativePathElement Id="0" Dir="Users" />
                        <RelativePathElement Id="1" Dir="Jude" />
                        <RelativePathElement Id="2" Dir="Documents" />
                        <RelativePathElement Id="3" Dir="Music Creation" />
                        <RelativePathElement Id="4" Dir="Test Project" />
                        <RelativePathElement Id="5" Dir="Samples" />
                        <RelativePathElement Id="6" Dir="Processed" />
                        <RelativePathElement Id="7" Dir="Consolidate" />
                    </PathHint>
                    <FileSize Value="0" />
                    <Crc Value="0" />
                    <MaxCrcSize Value="0" />
                    <HasExtendedInfo Value="false" />
                </SearchHint>
                <LivePackName Value="" />
                <LivePackId Value="" />
            </FileRef>
            <LastModDate Value="1536412217" />
            <SourceContext />
            <SampleUsageHint Value="0" />
            <DefaultDuration Value="303158" />
            <DefaultSampleRate Value="48000" />
        </SampleRef>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();

    assert_eq!(result.samples.len(), 1, "Should have collected one sample");
    let sample = result.samples.iter().next().unwrap();
    assert_eq!(sample.name, "YK - Retro OH (Hats) [2018-09-08 151017].wav");

    assert_clean_state(&scanner);
}

#[test]
fn test_sample_v9() {
    let mut scanner = create_test_scanner_with_version(9);
    setup_valid_scanner(&mut scanner);

    // DO NOT EDIT THIS XML DATA
    let mut reader = Reader::from_str(
        r#"
        <SampleRef>
            <FileRef>
                <HasRelativePath Value="true" />
                <RelativePathType Value="3" />
                <RelativePath>
                    <RelativePathElement Dir="Samples" />
                    <RelativePathElement Dir="Imported" />
                </RelativePath>
                <Name Value="Old Vinyl   Free Download   Sound Effect [High Quality].mp3" />
                <Type Value="1" />
                <Data>
                    43003A005C00550073006500720073005C004A007500640065005C0044006F00630075006D006500
                    6E00740073005C004D00750073006900630020004300720065006100740069006F006E005C005400
                    6500730074002000500072006F006A006500630074005C00530061006D0070006C00650073005C00
                    49006D0070006F0072007400650064005C004F006C0064002000560069006E0079006C0020002000
                    20004600720065006500200044006F0077006E006C006F006100640020002000200053006F007500
                    6E006400200045006600660065006300740020005B00480069006700680020005100750061006C00
                    6900740079005D002E006D00700033000000
                </Data>
                <RefersToFolder Value="false" />
                <SearchHint>
                    <PathHint>
                        <RelativePathElement Dir="Users" />
                        <RelativePathElement Dir="Jude" />
                        <RelativePathElement Dir="Documents" />
                        <RelativePathElement Dir="Music Creation" />
                        <RelativePathElement Dir="Test Project" />
                        <RelativePathElement Dir="Samples" />
                        <RelativePathElement Dir="Imported" />
                    </PathHint>
                    <FileSize Value="28009976" />
                    <Crc Value="20557" />
                    <MaxCrcSize Value="16384" />
                    <HasExtendedInfo Value="true" />
                </SearchHint>
                <LivePackName Value="" />
                <LivePackId Value="" />
            </FileRef>
            <LastModDate Value="1493044007" />
            <SourceContext />
            <SampleUsageHint Value="0" />
            <DefaultDuration Value="30883284" />
            <DefaultSampleRate Value="44100" />
        </SampleRef>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();

    assert_eq!(result.samples.len(), 1, "Should have collected one sample");
    let sample = result.samples.iter().next().unwrap();
    assert_eq!(
        sample.name,
        "Old Vinyl   Free Download   Sound Effect [High Quality].mp3"
    );

    assert_clean_state(&scanner);
}

#[test]
fn test_tempo_parsing() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <Tempo>
            <LomId Value="0" />
            <Manual Value="120.0" />
            <MidiControllerRange>
                <Min Value="60" />
                <Max Value="200" />
            </MidiControllerRange>
            <AutomationTarget Id="8">
                <LockEnvelope Value="0" />
            </AutomationTarget>
            <ModulationTarget Id="9">
                <LockEnvelope Value="0" />
            </ModulationTarget>
        </Tempo>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(scanner.current_tempo, 120.0);

    assert_clean_state(&scanner);
}

#[test]
fn test_invalid_tempo() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <Tempo>
            <LomId Value="0" />
            <Manual Value="invalid" />
        </Tempo>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    assert_eq!(scanner.current_tempo, 0.0);

    assert_clean_state(&scanner);
}

#[test]
fn test_key_signature_v12() {
    let mut scanner = create_test_scanner();
    setup_valid_scanner(&mut scanner);

    let mut reader = Reader::from_str(
        r#"
        <MidiClip Id="0" Time="0">
            <LomId Value="0" />
            <LomIdView Value="0" />
            <CurrentStart Value="0" />
            <CurrentEnd Value="8" />
            <Loop>
                <LoopStart Value="0" />
                <LoopEnd Value="8" />
                <StartRelative Value="0" />
                <LoopOn Value="true" />
                <OutMarker Value="8" />
                <HiddenLoopStart Value="0" />
                <HiddenLoopEnd Value="4" />
            </Loop>
            <Name Value="" />
            <Annotation Value="" />
            <Color Value="4" />
            <LaunchMode Value="0" />
            <LaunchQuantisation Value="0" />
            <TimeSignature>
                <TimeSignatures>
                    <RemoteableTimeSignature Id="0">
                        <Numerator Value="4" />
                        <Denominator Value="4" />
                        <Time Value="0" />
                    </RemoteableTimeSignature>
                </TimeSignatures>
            </TimeSignature>
            <Envelopes>
                <Envelopes />
            </Envelopes>
            <ScrollerTimePreserver>
                <LeftTime Value="0" />
                <RightTime Value="8" />
            </ScrollerTimePreserver>
            <TimeSelection>
                <AnchorTime Value="0" />
                <OtherTime Value="0" />
            </TimeSelection>
            <Legato Value="false" />
            <Ram Value="false" />
            <GrooveSettings>
                <GrooveId Value="-1" />
            </GrooveSettings>
            <Disabled Value="false" />
            <VelocityAmount Value="0" />
            <FollowAction>
                <FollowTime Value="4" />
                <IsLinked Value="true" />
                <LoopIterations Value="1" />
                <FollowActionA Value="4" />
                <FollowActionB Value="0" />
                <FollowChanceA Value="100" />
                <FollowChanceB Value="0" />
                <JumpIndexA Value="1" />
                <JumpIndexB Value="1" />
                <FollowActionEnabled Value="false" />
            </FollowAction>
            <Grid>
                <FixedNumerator Value="1" />
                <FixedDenominator Value="16" />
                <GridIntervalPixel Value="20" />
                <Ntoles Value="2" />
                <SnapToGrid Value="true" />
                <Fixed Value="true" />
            </Grid>
            <FreezeStart Value="0" />
            <FreezeEnd Value="0" />
            <IsWarped Value="true" />
            <TakeId Value="1" />
            <Notes>
                <KeyTracks />
                <PerNoteEventStore>
                    <EventLists />
                </PerNoteEventStore>
                <NoteProbabilityGroups />
                <ProbabilityGroupIdGenerator>
                    <NextId Value="1" />
                </ProbabilityGroupIdGenerator>
                <NoteIdGenerator>
                    <NextId Value="1" />
                </NoteIdGenerator>
            </Notes>
            <BankSelectCoarse Value="-1" />
            <BankSelectFine Value="-1" />
            <ProgramChange Value="-1" />
            <NoteEditorFoldInZoom Value="-1" />
            <NoteEditorFoldInScroll Value="0" />
            <NoteEditorFoldOutZoom Value="2620" />
            <NoteEditorFoldOutScroll Value="-1126" />
            <NoteEditorFoldScaleZoom Value="-1" />
            <NoteEditorFoldScaleScroll Value="0" />
            <ScaleInformation>
                <RootNote Value="0" />
                <Name Value="Major" />
            </ScaleInformation>
            <IsInKey Value="true" />
            <NoteSpellingPreference Value="0" />
            <AccidentalSpellingPreference Value="3" />
            <PreferFlatRootNote Value="false" />
            <ExpressionGrid>
                <FixedNumerator Value="1" />
                <FixedDenominator Value="16" />
                <GridIntervalPixel Value="20" />
                <Ntoles Value="2" />
                <SnapToGrid Value="false" />
                <Fixed Value="false" />
            </ExpressionGrid>
        </MidiClip>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.key_signature.clone().unwrap().tonic, Tonic::C);
    assert_eq!(result.key_signature.clone().unwrap().scale, Scale::Major);
}

#[test]
fn test_key_signature_v9() {
    let mut scanner = create_test_scanner_with_version(9);
    setup_valid_scanner(&mut scanner);

    let mut reader = Reader::from_str(
        r#"
        <KeySignature>
            <Tonic Value="0" />
            <Scale Value="0" />
        </KeySignature>
        <KeySignature>
            <Tonic Value="0" />
            <Scale Value="0" />
        </KeySignature>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert!(
        result.key_signature.is_none(),
        "Key signature should be None for Ableton version 9"
    );
    assert_eq!(
        scanner.key_frequencies.len(),
        0,
        "No key frequencies should be recorded for version 9"
    );
}

#[test]
fn test_key_signature_not_in_key() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <MidiClip Id="0" Time="0">
            <LomId Value="0" />
            <LomIdView Value="0" />
            <CurrentStart Value="0" />
            <CurrentEnd Value="8" />
            <Loop>
                <LoopStart Value="0" />
                <LoopEnd Value="8" />
                <StartRelative Value="0" />
                <LoopOn Value="true" />
                <OutMarker Value="8" />
                <HiddenLoopStart Value="0" />
                <HiddenLoopEnd Value="4" />
            </Loop>
            <Name Value="" />
            <Annotation Value="" />
            <Color Value="4" />
            <LaunchMode Value="0" />
            <LaunchQuantisation Value="0" />
            <TimeSignature>
                <TimeSignatures>
                    <RemoteableTimeSignature Id="0">
                        <Numerator Value="4" />
                        <Denominator Value="4" />
                        <Time Value="0" />
                    </RemoteableTimeSignature>
                </TimeSignatures>
            </TimeSignature>
            <Envelopes>
                <Envelopes />
            </Envelopes>
            <ScrollerTimePreserver>
                <LeftTime Value="0" />
                <RightTime Value="8" />
            </ScrollerTimePreserver>
            <TimeSelection>
                <AnchorTime Value="0" />
                <OtherTime Value="0" />
            </TimeSelection>
            <Legato Value="false" />
            <Ram Value="false" />
            <GrooveSettings>
                <GrooveId Value="-1" />
            </GrooveSettings>
            <Disabled Value="false" />
            <VelocityAmount Value="0" />
            <FollowAction>
                <FollowTime Value="4" />
                <IsLinked Value="true" />
                <LoopIterations Value="1" />
                <FollowActionA Value="4" />
                <FollowActionB Value="0" />
                <FollowChanceA Value="100" />
                <FollowChanceB Value="0" />
                <JumpIndexA Value="1" />
                <JumpIndexB Value="1" />
                <FollowActionEnabled Value="false" />
            </FollowAction>
            <Grid>
                <FixedNumerator Value="1" />
                <FixedDenominator Value="16" />
                <GridIntervalPixel Value="20" />
                <Ntoles Value="2" />
                <SnapToGrid Value="true" />
                <Fixed Value="true" />
            </Grid>
            <FreezeStart Value="0" />
            <FreezeEnd Value="0" />
            <IsWarped Value="true" />
            <TakeId Value="1" />
            <Notes>
                <KeyTracks />
                <PerNoteEventStore>
                    <EventLists />
                </PerNoteEventStore>
                <NoteProbabilityGroups />
                <ProbabilityGroupIdGenerator>
                    <NextId Value="1" />
                </ProbabilityGroupIdGenerator>
                <NoteIdGenerator>
                    <NextId Value="1" />
                </NoteIdGenerator>
            </Notes>
            <BankSelectCoarse Value="-1" />
            <BankSelectFine Value="-1" />
            <ProgramChange Value="-1" />
            <NoteEditorFoldInZoom Value="-1" />
            <NoteEditorFoldInScroll Value="0" />
            <NoteEditorFoldOutZoom Value="2620" />
            <NoteEditorFoldOutScroll Value="-1126" />
            <NoteEditorFoldScaleZoom Value="-1" />
            <NoteEditorFoldScaleScroll Value="0" />
            <ScaleInformation>
                <RootNote Value="0" />
                <Name Value="Major" />
            </ScaleInformation>
            <IsInKey Value="false" />
            <NoteSpellingPreference Value="0" />
            <AccidentalSpellingPreference Value="3" />
            <PreferFlatRootNote Value="false" />
            <ExpressionGrid>
                <FixedNumerator Value="1" />
                <FixedDenominator Value="16" />
                <GridIntervalPixel Value="20" />
                <Ntoles Value="2" />
                <SnapToGrid Value="false" />
                <Fixed Value="false" />
            </ExpressionGrid>
        </MidiClip>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    setup_valid_scanner(&mut scanner);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.key_signature.clone().unwrap().tonic, Tonic::Empty);
    assert_eq!(result.key_signature.clone().unwrap().scale, Scale::Empty);
    assert_eq!(scanner.key_frequencies.len(), 0);
}

#[test]
fn test_multiple_key_signatures() {
    let mut scanner = create_test_scanner();
    let mut reader = Reader::from_str(
        r#"
        <MidiClip>
            <ScaleInformation>
                <RootNote Value="0" />
                <Name Value="Major" />
            </ScaleInformation>
            <IsInKey Value="true" />
        </MidiClip>
        <MidiClip>
            <ScaleInformation>
                <RootNote Value="0" />
                <Name Value="Major" />
            </ScaleInformation>
            <IsInKey Value="true" />
        </MidiClip>
        <MidiClip>
            <ScaleInformation>
                <RootNote Value="9" />
                <Name Value="Minor" />
            </ScaleInformation>
            <IsInKey Value="true" />
        </MidiClip>
    "#,
    );

    process_xml(&mut scanner, &mut reader);

    setup_valid_scanner(&mut scanner);

    let result = scanner.finalize_result(ParseResult::default()).unwrap();
    assert_eq!(result.key_signature.clone().unwrap().tonic, Tonic::C);
    assert_eq!(result.key_signature.clone().unwrap().scale, Scale::Major);
    assert_eq!(scanner.key_frequencies.len(), 2);
}

fn process_xml(scanner: &mut Parser, reader: &mut Reader<&[u8]>) {
    let mut buf = Vec::new();
    let mut byte_pos = 0;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                scanner
                    .handle_start_event(e, reader, &mut byte_pos)
                    .unwrap();
            }
            Ok(Event::End(ref e)) => {
                scanner.handle_end_event(e).unwrap();
            }
            Ok(Event::Text(ref e)) => {
                scanner.handle_text_event(e).unwrap();
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn assert_clean_state(scanner: &Parser) {
    assert_eq!(
        scanner.state,
        ParserState::Root,
        "Scanner should be in Root state"
    );
    assert_eq!(
        scanner.in_source_context, false,
        "Scanner should not be in source context"
    );
    assert_eq!(
        scanner.current_branch_info, None,
        "Scanner should have no branch info"
    );
}
