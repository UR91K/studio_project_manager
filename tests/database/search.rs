//! Database search functionality tests

use std::collections::HashSet;

use super::*;
use crate::common::{setup, LiveSetBuilder};
use studio_project_manager::database::search::{MatchReason, SearchQuery};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use uuid::Uuid;

fn setup_test_projects() -> (LiveSetDatabase, DateTime<Local>, DateTime<Local>, DateTime<Local>, DateTime<Local>) {
    let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

    // Create timestamps for testing
    let edm_created = Local.from_local_datetime(
        &NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap()
        )
    ).unwrap();
    let edm_modified = Local.from_local_datetime(
        &NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            NaiveTime::from_hms_opt(15, 30, 0).unwrap()
        )
    ).unwrap();
    let rock_created = Local.from_local_datetime(
        &NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2024, 1, 3).unwrap(),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        )
    ).unwrap();
    let rock_modified = Local.from_local_datetime(
        &NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
            NaiveTime::from_hms_opt(14, 45, 0).unwrap()
        )
    ).unwrap();

    // Create test projects
    let edm_scan = LiveSetBuilder::new()
        .with_plugin("Serum")
        .with_plugin("Massive")
        .with_installed_plugin("Pro-Q 3", Some("FabFilter".to_string()))
        .with_sample("kick.wav")
        .with_tempo(140.0)
        .with_created_time(edm_created)
        .with_modified_time(edm_modified)
        .build();

    let rock_scan = LiveSetBuilder::new()
        .with_plugin("Guitar Rig 6")
        .with_installed_plugin("Pro-R", Some("FabFilter".to_string()))
        .with_sample("guitar_riff.wav")
        .with_tempo(120.0)
        .with_created_time(rock_created)
        .with_modified_time(rock_modified)
        .build();

    // Convert scan results to LiveSets
    let edm_project = LiveSet {
        is_active: true,
        file_path: PathBuf::from("EDM Project.als"),
        name: String::from("EDM Project.als"),
        file_hash: String::from("dummy_hash"),
        created_time: edm_created,
        modified_time: edm_modified,
        last_parsed_timestamp: Local::now(),
        tempo: edm_scan.tempo,
        time_signature: edm_scan.time_signature,
        key_signature: None,
        furthest_bar: None,
        estimated_duration: None,
        ableton_version: edm_scan.version,
        plugins: edm_scan.plugins,
        samples: edm_scan.samples,
        tags: HashSet::new(),
        id: Uuid::new_v4(),
    };

    let rock_project = LiveSet {
        is_active: true,
        file_path: PathBuf::from("Rock Band.als"),
        name: String::from("Rock Band.als"),
        file_hash: String::from("dummy_hash"),
        created_time: rock_created,
        modified_time: rock_modified,
        last_parsed_timestamp: Local::now(),
        tempo: rock_scan.tempo,
        time_signature: rock_scan.time_signature,
        key_signature: None,
        furthest_bar: None,
        estimated_duration: None,
        ableton_version: rock_scan.version,
        plugins: rock_scan.plugins,
        samples: rock_scan.samples,
        tags: HashSet::new(),
        id: Uuid::new_v4(),
    };

    // Insert projects
    db.insert_project(&edm_project).expect("Failed to insert EDM project");
    db.insert_project(&rock_project).expect("Failed to insert rock project");

    (db, edm_created, edm_modified, rock_created, rock_modified)
}

#[test]
fn test_query_parser() {
    setup("debug");
    let query = SearchQuery::parse("drum mix bpm:120-140 plugin:serum path:\"C:/Music\"");
    
    assert_eq!(query.text, "drum mix");
    assert_eq!(query.bpm, Some("120-140".to_string()));
    assert_eq!(query.plugin, Some("serum".to_string()));
    assert_eq!(query.path, Some("C:/Music".to_string()));
}

#[test]
fn test_query_parser_interleaved() {
    setup("debug");
    let query = SearchQuery::parse("bpm:98 plugin:omnisphere big dog beat path:\"C:/Music\"");
    
    assert_eq!(query.text, "big dog beat");
    assert_eq!(query.bpm, Some("98".to_string()));
    assert_eq!(query.plugin, Some("omnisphere".to_string()));
    assert_eq!(query.path, Some("C:/Music".to_string()));
}

#[test]
fn test_multiple_operators() {
    setup("debug");
    let query = SearchQuery::parse("tag:wip ts:4/4 key:cmaj");
    
    assert!(query.text.is_empty());
    assert_eq!(query.tag, Some("wip".to_string()));
    assert_eq!(query.time_signature, Some("4/4".to_string()));
    assert_eq!(query.key, Some("cmaj".to_string()));
}

#[test]
fn test_search_plugins() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();

    // Test specific plugin search
    let plugin_query = SearchQuery::parse("plugin:serum");
    let results = db.search_fts(&plugin_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert!(results[0].match_reason.iter().any(|r| matches!(r, MatchReason::Plugin(p) if p == "serum")));

    // Test vendor search
    let vendor_query = SearchQuery::parse("FabFilter");
    let results = db.search_fts(&vendor_query).expect("Search failed");
    assert_eq!(results.len(), 2); // Both projects have FabFilter plugins
}

#[test]
fn test_search_tempo() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();

    let tempo_query = SearchQuery::parse("bpm:140");
    let results = db.search_fts(&tempo_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project.tempo, 140.0);
    assert!(results[0].match_reason.iter().any(|r| matches!(r, MatchReason::Tempo(t) if t == "140")));
}

#[test]
fn test_search_exact_creation_date() {
    setup("debug");
    let (mut db, edm_created, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:2024-01-01");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project.created_time.date_naive(), edm_created.date_naive());
}

#[test]
fn test_search_exact_modified_date() {
    setup("debug");
    let (mut db, _, _, _, rock_modified) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dm:2024-01-04");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project.modified_time.date_naive(), rock_modified.date_naive());
}

#[test]
fn test_search_full_timestamp() {
    setup("debug");
    let (mut db, edm_created, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:2024-01-01 08:00:00");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project.created_time, edm_created);
}

#[test]
fn test_search_partial_date_match() {
    setup("debug");
    let (mut db, _, edm_modified, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dm:2024-01-02");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project.modified_time.date_naive(), edm_modified.date_naive());
}

#[test]
fn test_search_nonexistent_date() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:2023-12-31");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 0, "Should not find any projects for non-existent date");
}

#[test]
fn test_search_invalid_date_format() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:not-a-date");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 0, "Should not find any projects for invalid date format");
}

#[test]
fn test_search_year_month_only() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:2024-01");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 2, "Should find both projects from January 2024");
}

#[test]
fn test_search_year_only() {
    setup("debug");
    let (mut db, _, _, _, _) = setup_test_projects();
    
    let date_query = SearchQuery::parse("dc:2024");
    let results = db.search_fts(&date_query).expect("Search failed");
    assert_eq!(results.len(), 2, "Should find both projects from 2024");
}