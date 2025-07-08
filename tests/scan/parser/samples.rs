//! Sample parsing tests

use super::*;
use crate::{scan::parser::basic::{assert_clean_state, create_test_scanner, create_test_scanner_with_version, process_xml, setup_valid_scanner}};

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
