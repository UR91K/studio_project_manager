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