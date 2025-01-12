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

#[allow(unused)]
impl SearchQuery {
    pub fn parse(input: &str) -> Self {
        let mut query = SearchQuery::default();
        let mut remaining_text = Vec::new();
        
        for term in input.split_whitespace() {
            if let Some((operator, value)) = term.split_once(':') {
                match operator {
                    "path" => query.path = Some(value.to_string()),
                    "name" => query.name = Some(value.to_string()),
                    "dc" => query.date_created = Some(value.to_string()),
                    "dm" => query.date_modified = Some(value.to_string()),
                    "version" => query.version = Some(value.to_string()),
                    "key" => query.key = Some(value.to_string()),
                    "bpm" => query.bpm = Some(value.to_string()),
                    "ts" => query.time_signature = Some(value.to_string()),
                    "ed" => query.estimated_duration = Some(value.to_string()),
                    "plugin" => query.plugin = Some(value.to_string()),
                    "sample" => query.sample = Some(value.to_string()),
                    "tag" => query.tag = Some(value.to_string()),
                    _ => remaining_text.push(term),
                }
            } else {
                remaining_text.push(term);
            }
        }
        
        query.text = remaining_text.join(" ");
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_parser() {
        let query = SearchQuery::parse("drums bpm:120-140 plugin:serum path:\"C:/Music\"");
        
        assert_eq!(query.text, "drums");
        assert_eq!(query.bpm, Some("120-140".to_string()));
        assert_eq!(query.plugin, Some("serum".to_string()));
        assert_eq!(query.path, Some("C:/Music".to_string()));
    }

    #[test]
    fn test_multiple_operators() {
        let query = SearchQuery::parse("tag:wip ts:4/4 key:cmaj");
        
        assert!(query.text.is_empty());
        assert_eq!(query.tag, Some("wip".to_string()));
        assert_eq!(query.time_signature, Some("4/4".to_string()));
        assert_eq!(query.key, Some("cmaj".to_string()));
    }
}

