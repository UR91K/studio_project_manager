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

impl LiveSetDatabase {
    pub fn set_project_notes(&mut self, project_id: &str, notes: &str) -> Result<(), DatabaseError> {
        debug!("Setting notes for project {}", project_id);
        self.conn.execute(
            "UPDATE projects SET notes = ? WHERE id = ?",
            params![notes, project_id],
        )?;
        debug!("Successfully set project notes");
        Ok(())
    }

    pub fn get_project_notes(&mut self, project_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for project {}", project_id);
        let notes = self.conn.query_row(
            "SELECT notes FROM projects WHERE id = ?",
            [project_id],
            |row| row.get(0),
        )?;
        debug!("Successfully retrieved project notes");
        Ok(notes)
    }

    pub fn set_collection_notes(&mut self, collection_id: &str, notes: &str) -> Result<(), DatabaseError> {
        debug!("Setting notes for collection {}", collection_id);
        let now = Local::now();
        self.conn.execute(
            "UPDATE collections SET notes = ?, modified_at = ? WHERE id = ?",
            params![notes, SqlDateTime::from(now), collection_id],
        )?;
        debug!("Successfully set collection notes");
        Ok(())
    }

    pub fn get_collection_notes(&mut self, collection_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for collection {}", collection_id);
        let notes = self.conn.query_row(
            "SELECT notes FROM collections WHERE id = ?",
            [collection_id],
            |row| row.get(0),
        )?;
        debug!("Successfully retrieved collection notes");
        Ok(notes)
    }
}
