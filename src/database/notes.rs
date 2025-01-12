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
        
        let tx = self.conn.transaction()?;
        
        // First verify the project exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?)",
            [project_id],
            |row| row.get(0),
        )?;

        if !exists {
            debug!("Project {} not found", project_id);
            return Ok(());
        }

        // Update the notes
        tx.execute(
            "UPDATE projects SET notes = ? WHERE id = ?",
            params![notes, project_id],
        )?;

        tx.commit()?;
        debug!("Successfully set notes for project {}", project_id);
        Ok(())
    }

    pub fn get_project_notes(&mut self, project_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for project {}", project_id);
        
        let notes = self.conn.query_row(
            "SELECT notes FROM projects WHERE id = ?",
            [project_id],
            |row| row.get::<_, Option<String>>(0),
        )?;

        debug!("Retrieved notes for project {}: {:?}", project_id, notes);
        Ok(notes)
    }

    pub fn set_collection_notes(&mut self, collection_id: &str, notes: &str) -> Result<(), DatabaseError> {
        debug!("Setting notes for collection {}", collection_id);
        
        let tx = self.conn.transaction()?;
        
        // First verify the collection exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM collections WHERE id = ?)",
            [collection_id],
            |row| row.get(0),
        )?;

        if !exists {
            debug!("Collection {} not found", collection_id);
            return Ok(());
        }

        // Update the notes
        tx.execute(
            "UPDATE collections SET notes = ? WHERE id = ?",
            params![notes, collection_id],
        )?;

        tx.commit()?;
        debug!("Successfully set notes for collection {}", collection_id);
        Ok(())
    }

    pub fn get_collection_notes(&mut self, collection_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for collection {}", collection_id);
        
        let notes = self.conn.query_row(
            "SELECT notes FROM collections WHERE id = ?",
            [collection_id],
            |row| row.get::<_, Option<String>>(0),
        )?;

        debug!("Retrieved notes for collection {}: {:?}", collection_id, notes);
        Ok(notes)
    }
}
