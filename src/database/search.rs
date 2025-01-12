#![allow(unused_imports)]
use crate::database::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use chrono::{DateTime, Local, TimeZone};
use log::{debug, info, warn};
use rusqlite::{params, types::ToSql, Connection, OptionalExtension, Result as SqliteResult};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use super::LiveSetDatabase;

#[derive(Debug, Default)]
#[allow(unused)]
pub struct SearchQuery {
    // File properties
    pub path: Option<String>,
    pub name: Option<String>,
    pub date_created: Option<String>,
    pub date_modified: Option<String>,
    
    // Musical properties
    pub version: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<String>,
    pub time_signature: Option<String>,
    pub estimated_duration: Option<String>,
    
    // Content properties
    pub plugin: Option<String>,
    pub sample: Option<String>,
    pub tag: Option<String>,
    
    // Full text search
    pub text: String,
}

#[derive(Debug)]
pub struct SearchResult {
    pub project: LiveSet,
    pub rank: f64,
    pub match_reason: Vec<MatchReason>,
}

#[derive(Debug)]
pub enum MatchReason {
    Name(String),
    Path(String),
    Plugin(String),
    Sample(String),
    Tag(String),
    KeySignature(String),
    TimeSignature(String),
    Tempo(String),
    Version(String),
    Notes(String),
}

impl SearchQuery {
    fn strip_quotes(value: &str) -> String {
        let value = value.trim();
        if (value.starts_with('"') && value.ends_with('"')) || 
           (value.starts_with('\'') && value.ends_with('\'')) {
            value[1..value.len()-1].to_string()
        } else {
            value.to_string()
        }
    }

    pub fn parse(input: &str) -> Self {
        debug!("Parsing search query input: '{}'", input);
        let mut query = SearchQuery::default();
        let mut remaining_text = Vec::new();
        
        for term in input.split_whitespace() {
            debug!("Processing term: '{}'", term);
            if let Some((operator, value)) = term.split_once(':') {
                debug!("Found operator '{}' with value '{}'", operator, value);
                let cleaned_value = Self::strip_quotes(value);
                debug!("Cleaned value: '{}'", cleaned_value);
                match operator {
                    "path" => query.path = Some(cleaned_value),
                    "name" => query.name = Some(cleaned_value),
                    "dc" => query.date_created = Some(cleaned_value),
                    "dm" => query.date_modified = Some(cleaned_value),
                    "version" => query.version = Some(cleaned_value),
                    "key" => query.key = Some(cleaned_value),
                    "bpm" => query.bpm = Some(cleaned_value),
                    "ts" => query.time_signature = Some(cleaned_value),
                    "ed" => query.estimated_duration = Some(cleaned_value),
                    "plugin" => query.plugin = Some(cleaned_value),
                    "sample" => query.sample = Some(cleaned_value),
                    "tag" => query.tag = Some(cleaned_value),
                    _ => {
                        debug!("Unknown operator '{}', treating as text", operator);
                        remaining_text.push(term)
                    },
                }
            } else {
                debug!("No operator found, adding to remaining text: '{}'", term);
                remaining_text.push(term);
            }
        }
        
        query.text = remaining_text.join(" ");
        debug!("Final query state: {:?}", query);
        query
    }

    fn build_fts5_query(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // Helper function to add a condition
        let mut add_condition = |field: &str, value: &str| {
            conditions.push(format!("{} MATCH ?", field));
            params.push(value.to_string());
        };

        // Add specific field conditions
        if let Some(ref path) = self.path {
            add_condition("path", path);
        }
        if let Some(ref name) = self.name {
            add_condition("name", name);
        }
        if let Some(ref version) = self.version {
            add_condition("version", version);
        }
        if let Some(ref key) = self.key {
            add_condition("key_signature", key);
        }
        if let Some(ref bpm) = self.bpm {
            add_condition("tempo", bpm);
        }
        if let Some(ref ts) = self.time_signature {
            add_condition("time_signature", ts);
        }
        if let Some(ref plugin) = self.plugin {
            add_condition("plugins", plugin);
        }
        if let Some(ref sample) = self.sample {
            add_condition("samples", sample);
        }
        if let Some(ref tag) = self.tag {
            add_condition("tags", tag);
        }

        // Add full text search if present
        if !self.text.is_empty() {
            conditions.push("project_search MATCH ?".to_string());
            params.push(self.text.clone());
        }

        let query = if conditions.is_empty() {
            "SELECT * FROM project_search ORDER BY rank".to_string()
        } else {
            format!(
                "SELECT *, rank FROM project_search WHERE {} ORDER BY rank",
                conditions.join(" AND ")
            )
        };

        (query, params)
    }
}

impl LiveSetDatabase {
    pub fn search_fts(&mut self, query: &SearchQuery) -> Result<Vec<SearchResult>, DatabaseError> {
        debug!("Performing FTS5 search with query: {:?}", query);
        
        // First get all matching project paths and ranks
        let (fts_query, params) = query.build_fts5_query();
        debug!("FTS5 query: {}", fts_query);
        debug!("Query params: {:?}", params);

        let params_refs: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
        let mut matching_projects = Vec::new();

        let tx = self.conn.transaction()?;
        
        // Create new scope to ensure stmt is dropped before commit
        {
            let mut stmt = tx.prepare(&fts_query)?;
            matching_projects = stmt.query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>("path")?,
                    row.get::<_, f64>("rank")?,
                    row.get::<_, String>("name")?,
                    row.get::<_, String>("plugins")?,
                    row.get::<_, String>("samples")?,
                    row.get::<_, String>("tags")?,
                ))
            })?.collect::<SqliteResult<Vec<_>>>()?;
        }
        
        tx.commit()?;

        debug!("Found {} potential matches", matching_projects.len());
        
        // Now get full project details and build search results
        let mut results = Vec::new();
        for (path, rank, name, plugins, samples, tags) in matching_projects {
            if let Ok(Some(project)) = self.get_project_by_path(&path) {
                let mut match_reasons = Vec::new();

                // Determine match reasons based on the query and matched columns
                if let Some(ref name_query) = query.name {
                    if name.contains(name_query) {
                        match_reasons.push(MatchReason::Name(name_query.clone()));
                    }
                }
                if let Some(ref plugin_query) = query.plugin {
                    if plugins.contains(plugin_query) {
                        match_reasons.push(MatchReason::Plugin(plugin_query.clone()));
                    }
                }
                if let Some(ref sample_query) = query.sample {
                    if samples.contains(sample_query) {
                        match_reasons.push(MatchReason::Sample(sample_query.clone()));
                    }
                }
                if let Some(ref tag_query) = query.tag {
                    if tags.contains(tag_query) {
                        match_reasons.push(MatchReason::Tag(tag_query.clone()));
                    }
                }

                results.push(SearchResult {
                    project,
                    rank,
                    match_reason: match_reasons,
                });
            }
        }

        debug!("Found {} matching projects with full details", results.len());
        
        // Sort by rank (highest first)
        results.sort_by(|a, b| b.rank.partial_cmp(&a.rank).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::LiveSetBuilder;
    use crate::live_set::LiveSet;

    use std::sync::Once;

    static INIT: Once = Once::new();
    fn setup() {
        let _ = INIT.call_once(|| {
            let _ = std::env::set_var("RUST_LOG", "debug");
            if let Err(_) = env_logger::try_init() {
                // Logger already initialized, that's fine
            }
        });
    }

    #[test]
    fn test_query_parser() {
        setup();
        let query = SearchQuery::parse("drum mix bpm:120-140 plugin:serum path:\"C:/Music\"");
        
        assert_eq!(query.text, "drum mix");
        assert_eq!(query.bpm, Some("120-140".to_string()));
        assert_eq!(query.plugin, Some("serum".to_string()));
        assert_eq!(query.path, Some("C:/Music".to_string()));
    }

    #[test]
    fn test_query_parser_interleaved() {
        setup();
        let query = SearchQuery::parse("bpm:98 plugin:omnisphere big dog beat path:\"C:/Music\"");
        
        assert_eq!(query.text, "big dog beat");
        assert_eq!(query.bpm, Some("98".to_string()));
        assert_eq!(query.plugin, Some("omnisphere".to_string()));
        assert_eq!(query.path, Some("C:/Music".to_string()));
    }

    #[test]
    fn test_multiple_operators() {
        setup();
        let query = SearchQuery::parse("tag:wip ts:4/4 key:cmaj");
        
        assert!(query.text.is_empty());
        assert_eq!(query.tag, Some("wip".to_string()));
        assert_eq!(query.time_signature, Some("4/4".to_string()));
        assert_eq!(query.key, Some("cmaj".to_string()));
    }

    #[test]
    fn test_fts_search() {
        setup();
        let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");

        // Create test projects
        let edm_scan = LiveSetBuilder::new()
            .with_plugin("Serum")
            .with_plugin("Massive")
            .with_installed_plugin("Pro-Q 3", Some("FabFilter".to_string()))
            .with_sample("kick.wav")
            .with_tempo(140.0)
            .build();

        let rock_scan = LiveSetBuilder::new()
            .with_plugin("Guitar Rig 6")
            .with_installed_plugin("Pro-R", Some("FabFilter".to_string()))
            .with_sample("guitar_riff.wav")
            .with_tempo(120.0)
            .build();

        // Convert scan results to LiveSets
        let edm_project = LiveSet {
            file_path: PathBuf::from("EDM Project.als"),
            file_name: String::from("EDM Project.als"),
            file_hash: String::from("dummy_hash"),
            created_time: Local::now(),
            modified_time: Local::now(),
            last_scan_timestamp: Local::now(),
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
            xml_data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton/>".to_vec(),
        };

        let rock_project = LiveSet {
            file_path: PathBuf::from("Rock Band.als"),
            file_name: String::from("Rock Band.als"),
            file_hash: String::from("dummy_hash"),
            created_time: Local::now(),
            modified_time: Local::now(),
            last_scan_timestamp: Local::now(),
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
            xml_data: b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton/>".to_vec(),
        };

        // Insert projects
        db.insert_project(&edm_project).expect("Failed to insert EDM project");
        db.insert_project(&rock_project).expect("Failed to insert rock project");

        // Test various search queries
        let plugin_query = SearchQuery::parse("plugin:serum");
        let results = db.search_fts(&plugin_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert!(results[0].match_reason.iter().any(|r| matches!(r, MatchReason::Plugin(p) if p == "serum")));

        let tempo_query = SearchQuery::parse("bpm:140");
        let results = db.search_fts(&tempo_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.tempo, 140.0);

        let vendor_query = SearchQuery::parse("FabFilter");
        let results = db.search_fts(&vendor_query).expect("Search failed");
        assert_eq!(results.len(), 2); // Both projects have FabFilter plugins
    }
}

