#![allow(unused_imports)]
use crate::database::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use chrono::{DateTime, Local, TimeZone, NaiveDateTime, NaiveDate, NaiveTime};
use log::debug;
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
    DateCreated(String),
    DateModified(String),
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
        let mut current_pos = 0;
        
        while current_pos < input.len() {
            let rest = &input[current_pos..];
            let mut term_end = rest.find(' ').unwrap_or(rest.len());
            
            // If we find a colon, check if this is a date operator
            if let Some(colon_pos) = rest[..term_end].find(':') {
                let operator = &rest[..colon_pos];
                match operator {
                    "dc" | "dm" => {
                        // For date operators, look for the next non-date character
                        // This allows spaces in the date value
                        let value_start = colon_pos + 1;
                        let mut value_end = value_start;
                        while value_end < rest.len() {
                            let c = rest.as_bytes()[value_end];
                            if !matches!(c, b'0'..=b'9' | b'-' | b':' | b' ') {
                                break;
                            }
                            value_end += 1;
                        }
                        term_end = value_end;
                        
                        let value = &rest[value_start..value_end];
                        debug!("Found date operator '{}' with value '{}'", operator, value);
                        let cleaned_value = Self::strip_quotes(value);
                        match operator {
                            "dc" => query.date_created = Some(cleaned_value),
                            "dm" => query.date_modified = Some(cleaned_value),
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        // Handle other operators as before
                        let value = &rest[colon_pos + 1..term_end];
                        debug!("Found operator '{}' with value '{}'", operator, value);
                        let cleaned_value = Self::strip_quotes(value);
                        match operator {
                            "path" => query.path = Some(cleaned_value),
                            "name" => query.name = Some(cleaned_value),
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
                                remaining_text.push(&rest[..term_end]);
                            }
                        }
                    }
                }
            } else {
                debug!("No operator found, adding to remaining text: '{}'", &rest[..term_end]);
                remaining_text.push(&rest[..term_end]);
            }
            
            // Move past this term and any following whitespace
            current_pos += term_end;
            while current_pos < input.len() && input.as_bytes()[current_pos] == b' ' {
                current_pos += 1;
            }
        }
        
        query.text = remaining_text.join(" ");
        debug!("Final query state: {:?}", query);
        query
    }

    fn build_fts5_query(&self) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // Helper function to add a column-specific condition
        let mut add_column_condition = |column: &str, value: &str| {
            if column == "created_at" || column == "modified_at" {
                // For dates, use a proper FTS5 prefix match
                // The * must be outside the quotes according to the docs
                conditions.push(format!("{} : \"{}\" *", column, value));
            } else {
                conditions.push(format!("{} : \"{}\"", column, value));
            }
            params.push(value.to_string());
        };

        // Add specific field conditions
        if let Some(ref path) = self.path {
            add_column_condition("path", path);
        }
        if let Some(ref name) = self.name {
            add_column_condition("name", name);
        }
        if let Some(ref version) = self.version {
            add_column_condition("version", version);
        }
        if let Some(ref key) = self.key {
            add_column_condition("key_signature", key);
        }
        if let Some(ref bpm) = self.bpm {
            add_column_condition("tempo", bpm);
        }
        if let Some(ref ts) = self.time_signature {
            add_column_condition("time_signature", ts);
        }
        if let Some(ref plugin) = self.plugin {
            add_column_condition("plugins", plugin);
        }
        if let Some(ref sample) = self.sample {
            add_column_condition("samples", sample);
        }
        if let Some(ref tag) = self.tag {
            add_column_condition("tags", tag);
        }
        if let Some(ref created) = self.date_created {
            add_column_condition("created_at", created);
        }
        if let Some(ref modified) = self.date_modified {
            add_column_condition("modified_at", modified);
        }

        // Add full text search if present
        if !self.text.is_empty() {
            conditions.push(format!("\"{}\"", self.text));
            params.push(self.text.clone());
        }

        let fts5_query = if conditions.is_empty() {
            String::new()
        } else {
            conditions.join(" AND ")
        };

        let query = format!(
            "SELECT project_id, rank, name, path, plugins, samples 
             FROM project_search 
             WHERE project_search MATCH ? 
             ORDER BY rank"
        );

        (query, vec![fts5_query])
    }
}

impl LiveSetDatabase {
    pub fn search_fts(&mut self, query: &SearchQuery) -> Result<Vec<SearchResult>, DatabaseError> {
        debug!("Performing FTS5 search with query: {:?}", query);

        // First collect all matching paths in a transaction
        let matching_paths = {
            let tx = self.conn.transaction()?;
            
            let (sql_query, params) = query.build_fts5_query();
            debug!("FTS5 query: {}", sql_query);
            debug!("Query params: {:?}", params);

            let results = {
                let mut stmt = tx.prepare(&sql_query)?;
                let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p as &dyn ToSql).collect();

                // Collect all results into a vector
                let mut results = Vec::new();
                let mut rows = stmt.query(param_refs.as_slice())?;
                while let Some(row) = rows.next()? {
                    let plugins: String = row.get(4)?;
                    debug!("Checking row with plugins: {:?}", plugins);
                    results.push((
                        row.get::<_, String>(0)?, // project_id
                        row.get::<_, f64>(1)?,    // rank
                        row.get::<_, String>(2)?, // name
                        row.get::<_, String>(3)?, // path
                        plugins,                  // plugins
                        row.get::<_, String>(5)?, // samples
                    ));
                }
                debug!("Found {} potential matches", results.len());
                results
            };
            
            tx.commit()?;
            results
        };
        
        // Now get full project details and build search results
        let mut search_results = Vec::new();
        #[allow(unused)]
        for (project_id, rank, name, path, plugins, samples) in matching_paths {
            debug!("Processing match: {} ({})", name, path);
            if let Ok(Some(project)) = self.get_project_by_path(&path) {
                let mut match_reason = Vec::new();
                
                // Add match reasons based on what matched
                if let Some(plugin_query) = &query.plugin {
                    let plugin_query = plugin_query.to_lowercase();
                    let plugins_lower = plugins.to_lowercase();
                    debug!("  Checking if '{}' exists in '{}'", plugin_query, plugins_lower);
                    if plugins_lower.contains(&plugin_query) {
                        debug!("  Found plugin match!");
                        match_reason.push(MatchReason::Plugin(plugin_query.clone()));
                    }
                }
                if let Some(bpm) = &query.bpm {
                    match_reason.push(MatchReason::Tempo(bpm.clone()));
                }
                if let Some(date_created) = &query.date_created {
                    match_reason.push(MatchReason::DateCreated(date_created.clone()));
                }
                if let Some(date_modified) = &query.date_modified {
                    match_reason.push(MatchReason::DateModified(date_modified.clone()));
                }

                search_results.push(SearchResult {
                    project,
                    rank,
                    match_reason,
                });
            }
        }

        debug!("Successfully built {} search results", search_results.len());
        Ok(search_results)
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
            file_name: String::from("EDM Project.als"),
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
            file_name: String::from("Rock Band.als"),
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
    fn test_search_plugins() {
        setup();
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
        setup();
        let (mut db, _, _, _, _) = setup_test_projects();

        let tempo_query = SearchQuery::parse("bpm:140");
        let results = db.search_fts(&tempo_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.tempo, 140.0);
        assert!(results[0].match_reason.iter().any(|r| matches!(r, MatchReason::Tempo(t) if t == "140")));
    }

    #[test]
    fn test_search_exact_creation_date() {
        setup();
        let (mut db, edm_created, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:2024-01-01");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.created_time.date_naive(), edm_created.date_naive());
    }

    #[test]
    fn test_search_exact_modified_date() {
        setup();
        let (mut db, _, _, _, rock_modified) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dm:2024-01-04");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.modified_time.date_naive(), rock_modified.date_naive());
    }

    #[test]
    fn test_search_full_timestamp() {
        setup();
        let (mut db, edm_created, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:2024-01-01 08:00:00");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.created_time, edm_created);
    }

    #[test]
    fn test_search_partial_date_match() {
        setup();
        let (mut db, _, edm_modified, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dm:2024-01-02");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project.modified_time.date_naive(), edm_modified.date_naive());
    }

    #[test]
    fn test_search_nonexistent_date() {
        setup();
        let (mut db, _, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:2023-12-31");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 0, "Should not find any projects for non-existent date");
    }

    #[test]
    fn test_search_invalid_date_format() {
        setup();
        let (mut db, _, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:not-a-date");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 0, "Should not find any projects for invalid date format");
    }

    #[test]
    fn test_search_year_month_only() {
        setup();
        let (mut db, _, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:2024-01");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 2, "Should find both projects from January 2024");
    }

    #[test]
    fn test_search_year_only() {
        setup();
        let (mut db, _, _, _, _) = setup_test_projects();
        
        let date_query = SearchQuery::parse("dc:2024");
        let results = db.search_fts(&date_query).expect("Search failed");
        assert_eq!(results.len(), 2, "Should find both projects from 2024");
    }
}
