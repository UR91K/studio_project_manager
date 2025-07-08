//! Basic parser functionality tests
//!
//! MIGRATION INSTRUCTIONS:
//! Move the following from src/scan/parser_test.rs:
//! - TIME_SIGNATURE constant (line ~9)
//! - setup_valid_scanner() helper (line ~16)
//! - create_test_scanner() helper (line ~23)
//! - test_tempo_parsing() (line ~76)
//! - test_basic_tag_parsing() (line ~93)
//! - test_version_parsing() (line ~107)
//! - test_time_signature_parsing() (line ~127)
//! - test_key_signature_parsing() (line ~148)

use super::*;
use crate::common::setup;

// TODO: Move TIME_SIGNATURE constant from src/scan/parser_test.rs (line ~9)
// TODO: Move setup_valid_scanner() helper function from src/scan/parser_test.rs (line ~16)
// TODO: Move create_test_scanner() helper function from src/scan/parser_test.rs (line ~23)
// TODO: Move basic parsing tests (tempo, tags, version, time signature, key signature)
// Total: ~5 tests + 2 helper functions + 1 constant to move 

static TIME_SIGNATURE: TimeSignature = TimeSignature {
    numerator: 4,
    denominator: 4,
};

pub fn process_xml(scanner: &mut Parser, reader: &mut Reader<&[u8]>) {
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

pub fn assert_clean_state(scanner: &Parser) {
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

pub fn setup_valid_scanner(scanner: &mut Parser) {
    scanner.current_tempo = 120.0;
    scanner.current_time_signature = TIME_SIGNATURE.clone();
}

pub fn create_test_scanner() -> Parser {
    setup("error");
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

pub fn create_test_scanner_with_version(version: u32) -> Parser {
    setup("error");
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

//--TEMPO TESTS--

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

//--VERSION TESTS-- 

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

//--TIME SIGNATURE TESTS--

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

//--FURTHEST BAR TESTS--

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

//--KEY SIGNATURE TESTS--

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
