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
    pub fn add_task(&mut self, project_id: &str, description: &str) -> Result<String, DatabaseError> {
        debug!("Adding task to project {}: {}", project_id, description);
        let task_id = Uuid::new_v4().to_string();
        let now = Local::now();

        self.conn.execute(
            "INSERT INTO project_tasks (id, project_id, description, completed, created_at) VALUES (?, ?, ?, ?, ?)",
            params![task_id, project_id, description, false, SqlDateTime::from(now)],
        )?;

        debug!("Successfully added task: {}", task_id);
        Ok(task_id)
    }

    pub fn complete_task(&mut self, task_id: &str, completed: bool) -> Result<(), DatabaseError> {
        debug!("Setting task {} completion status to {}", task_id, completed);
        self.conn.execute(
            "UPDATE project_tasks SET completed = ? WHERE id = ?",
            params![completed, task_id],
        )?;
        debug!("Successfully updated task completion status");
        Ok(())
    }

    pub fn remove_task(&mut self, task_id: &str) -> Result<(), DatabaseError> {
        debug!("Removing task {}", task_id);
        self.conn.execute("DELETE FROM project_tasks WHERE id = ?", [task_id])?;
        debug!("Successfully removed task");
        Ok(())
    }

    pub fn get_project_tasks(&mut self, project_id: &str) -> Result<Vec<(String, String, bool)>, DatabaseError> {
        debug!("Getting tasks for project {}", project_id);
        let mut stmt = self.conn.prepare(
            "SELECT id, description, completed FROM project_tasks WHERE project_id = ? ORDER BY created_at"
        )?;

        let tasks = stmt.query_map([project_id], |row| {
            let id: String = row.get(0)?;
            let description: String = row.get(1)?;
            let completed: bool = row.get(2)?;
            debug!("Found task: {} ({})", description, id);
            Ok((id, description, completed))
        })?.filter_map(|r| r.ok()).collect();

        debug!("Successfully retrieved project tasks");
        Ok(tasks)
    }

    pub fn get_collection_tasks(&mut self, collection_id: &str) -> Result<Vec<(String, String, String, bool)>, DatabaseError> {
        debug!("Getting tasks for all projects in collection {}", collection_id);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, p.name, t.description, t.completed
            FROM project_tasks t
            JOIN projects p ON p.id = t.project_id
            JOIN collection_projects cp ON cp.project_id = p.id
            WHERE cp.collection_id = ?
            ORDER BY cp.position, t.created_at
            "#
        )?;

        let tasks = stmt.query_map([collection_id], |row| {
            let id: String = row.get(0)?;
            let project_name: String = row.get(1)?;
            let description: String = row.get(2)?;
            let completed: bool = row.get(3)?;
            debug!("Found task: {} ({}) from project {}", description, id, project_name);
            Ok((id, project_name, description, completed))
        })?.filter_map(|r| r.ok()).collect();

        debug!("Successfully retrieved collection tasks");
        Ok(tasks)
    }
}
