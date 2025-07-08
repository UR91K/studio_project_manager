//! Plugin parsing tests

use super::*;
use crate::{scan::parser::basic::{create_test_scanner, process_xml, setup_valid_scanner}};

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
