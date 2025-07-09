use crate::database::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use chrono::{Local, TimeZone};
use log::debug;
use rusqlite::{params, OptionalExtension};
use rusqlite::types::ToSql;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    
    // Collection methods
    pub fn create_collection(&mut self, name: &str, description: Option<&str>, notes: Option<&str>) -> Result<String, DatabaseError> {
        debug!("Creating collection: {}", name);
        let collection_id = Uuid::new_v4().to_string();
        let now = Local::now();

        self.conn.execute(
            "INSERT INTO collections (id, name, description, notes, created_at, modified_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                collection_id,
                name,
                description,
                notes,
                SqlDateTime::from(now),
                SqlDateTime::from(now)
            ],
        )?;

        debug!("Successfully created collection: {} ({})", name, collection_id);
        Ok(collection_id)
    }

    pub fn get_collection_by_id(&mut self, collection_id: &str) -> Result<Option<(String, String, Option<String>, Option<String>, i64, i64, Vec<String>, Option<String>)>, DatabaseError> {
        debug!("Getting collection by ID: {}", collection_id);
        
        let collection_data: Option<(String, String, Option<String>, Option<String>, i64, i64, Option<String>)> = self.conn.query_row(
            "SELECT id, name, description, notes, created_at, modified_at, cover_art_id FROM collections WHERE id = ?",
            [collection_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        ).optional()?;

        if let Some((id, name, description, notes, created_at, modified_at, cover_art_id)) = collection_data {
            // Get project IDs for this collection
            let mut stmt = self.conn.prepare(
                "SELECT project_id FROM collection_projects WHERE collection_id = ? ORDER BY position"
            )?;
            let project_ids: Vec<String> = stmt.query_map([collection_id], |row| {
                row.get(0)
            })?.filter_map(|r| r.ok()).collect();

            debug!("Found collection: {} with {} projects", name, project_ids.len());
            Ok(Some((id, name, description, notes, created_at, modified_at, project_ids, cover_art_id)))
        } else {
            debug!("Collection not found: {}", collection_id);
            Ok(None)
        }
    }

    pub fn update_collection(&mut self, collection_id: &str, name: Option<&str>, description: Option<&str>, notes: Option<&str>) -> Result<(), DatabaseError> {
        debug!("Updating collection: {}", collection_id);
        let now = Local::now();

        // Build the update query dynamically based on provided fields
        let mut query = String::from("UPDATE collections SET modified_at = ?");
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(SqlDateTime::from(now))];

        if let Some(name) = name {
            query.push_str(", name = ?");
            params.push(Box::new(name.to_string()));
        }

        if let Some(description) = description {
            query.push_str(", description = ?");
            params.push(Box::new(description.to_string()));
        }

        if let Some(notes) = notes {
            query.push_str(", notes = ?");
            params.push(Box::new(notes.to_string()));
        }

        query.push_str(" WHERE id = ?");
        params.push(Box::new(collection_id.to_string()));

        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        self.conn.execute(&query, param_refs.as_slice())?;

        debug!("Successfully updated collection: {}", collection_id);
        Ok(())
    }

    pub fn delete_collection(&mut self, collection_id: &str) -> Result<(), DatabaseError> {
        debug!("Deleting collection: {}", collection_id);
        self.conn.execute("DELETE FROM collections WHERE id = ?", [collection_id])?;
        debug!("Successfully deleted collection");
        Ok(())
    }

    pub fn add_project_to_collection(
        &mut self,
        collection_id: &str,
        project_id: &str,
    ) -> Result<(), DatabaseError> {
        debug!(
            "Adding project {} to collection {}",
            project_id, collection_id
        );

        // Debug: Verify project exists
        let project_exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?)",
            [project_id],
            |row| row.get(0),
        )?;
        debug!("Project exists in projects table: {}", project_exists);

        let now = Local::now();

        // Get the highest position in the collection
        let max_position: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM collection_projects WHERE collection_id = ?",
                [collection_id],
                |row| row.get(0),
            )?;

        let next_position = max_position + 1;

        self.conn.execute(
            "INSERT INTO collection_projects (collection_id, project_id, position, added_at) VALUES (?, ?, ?, ?)",
            params![
                collection_id,
                project_id,
                next_position,
                SqlDateTime::from(now)
            ],
        )?;

        // Debug: Verify insertion
        let inserted_project: Option<(String, i32)> = self.conn.query_row(
            "SELECT project_id, position FROM collection_projects WHERE collection_id = ? AND project_id = ?",
            params![collection_id, project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        
        if let Some((pid, pos)) = inserted_project {
            debug!("Verified project {} inserted at position {}", pid, pos);
        }

        // Update collection's modified timestamp
        self.conn.execute(
            "UPDATE collections SET modified_at = ? WHERE id = ?",
            params![SqlDateTime::from(now), collection_id],
        )?;

        debug!("Successfully added project to collection at position {}", next_position);
        Ok(())
    }

    pub fn remove_project_from_collection(
        &mut self,
        collection_id: &str,
        project_id: &str,
    ) -> Result<(), DatabaseError> {
        debug!(
            "Removing project {} from collection {}",
            project_id, collection_id
        );
        let now = Local::now();

        let tx = self.conn.transaction()?;
        
        // Get the position of the project being removed
        let removed_position: i32 = tx.query_row(
            "SELECT position FROM collection_projects WHERE collection_id = ? AND project_id = ?",
            params![collection_id, project_id],
            |row| row.get(0),
        )?;

        // Remove the project
        tx.execute(
            "DELETE FROM collection_projects WHERE collection_id = ? AND project_id = ?",
            params![collection_id, project_id],
        )?;

        // Update positions of remaining projects
        tx.execute(
            "UPDATE collection_projects SET position = position - 1 
             WHERE collection_id = ? AND position > ?",
            params![collection_id, removed_position],
        )?;

        // Update collection's modified timestamp
        tx.execute(
            "UPDATE collections SET modified_at = ? WHERE id = ?",
            params![SqlDateTime::from(now), collection_id],
        )?;

        tx.commit()?;
        debug!("Successfully removed project from collection");
        Ok(())
    }

    pub fn reorder_project_in_collection(
        &mut self,
        collection_id: &str,
        project_id: &str,
        new_position: i32,
    ) -> Result<(), DatabaseError> {
        debug!(
            "Moving project {} to position {} in collection {}",
            project_id, new_position, collection_id
        );
        let now = Local::now();

        let tx = self.conn.transaction()?;

        // Get the current position
        let current_position: i32 = tx.query_row(
            "SELECT position FROM collection_projects WHERE collection_id = ? AND project_id = ?",
            params![collection_id, project_id],
            |row| row.get(0),
        )?;

        if current_position == new_position {
            debug!("Project is already at position {}", new_position);
            return Ok(());
        }

        if current_position < new_position {
            // Moving down: shift intermediate items up
            tx.execute(
                "UPDATE collection_projects 
                 SET position = position - 1
                 WHERE collection_id = ? 
                 AND position > ? 
                 AND position <= ?",
                params![collection_id, current_position, new_position],
            )?;
        } else {
            // Moving up: shift intermediate items down
            tx.execute(
                "UPDATE collection_projects 
                 SET position = position + 1
                 WHERE collection_id = ? 
                 AND position >= ? 
                 AND position < ?",
                params![collection_id, new_position, current_position],
            )?;
        }

        // Set the new position
        tx.execute(
            "UPDATE collection_projects SET position = ? 
             WHERE collection_id = ? AND project_id = ?",
            params![new_position, collection_id, project_id],
        )?;

        // Update collection's modified timestamp
        tx.execute(
            "UPDATE collections SET modified_at = ? WHERE id = ?",
            params![SqlDateTime::from(now), collection_id],
        )?;

        tx.commit()?;
        debug!("Successfully moved project to new position");
        Ok(())
    }

    pub fn get_collection_projects(
        &mut self,
        collection_id: &str,
    ) -> Result<Vec<LiveSet>, DatabaseError> {
        debug!("Getting projects in collection: {}", collection_id);
        let tx = self.conn.transaction()?;
        let mut results = Vec::new();
        
        {
            let mut stmt = tx.prepare(
                r#"
                SELECT p.id, p.path, p.name, p.hash, p.notes, p.created_at, p.modified_at, p.last_parsed_at,
                       p.tempo, p.time_signature_numerator, p.time_signature_denominator,
                       p.key_signature_tonic, p.key_signature_scale, p.duration_seconds, p.furthest_bar,
                       p.ableton_version_major, p.ableton_version_minor, p.ableton_version_patch, p.ableton_version_beta
                FROM projects p
                JOIN collection_projects cp ON cp.project_id = p.id
                WHERE cp.collection_id = ?
                ORDER BY cp.position
                "#,
            )?;

            let mut rows = stmt.query([collection_id])?;
            while let Some(row) = rows.next()? {
                let project_id: String = row.get(0)?;
                debug!("Retrieved project from join query: {}", project_id);
                let duration_secs: Option<i64> = row.get(13)?;
                let created_timestamp: i64 = row.get(5)?;
                let modified_timestamp: i64 = row.get(6)?;
                let parsed_timestamp: i64 = row.get(7)?;

                let mut live_set = LiveSet {
                    is_active: true,
                    id: Uuid::parse_str(&project_id).map_err(|_| {
                        rusqlite::Error::InvalidParameterName("Invalid UUID".into())
                    })?,
                    file_path: PathBuf::from(row.get::<_, String>(1)?),
                    name: row.get(2)?,
                    file_hash: row.get(3)?,
                    created_time: Local
                        .timestamp_opt(created_timestamp, 0)
                        .single()
                        .ok_or_else(|| {
                            rusqlite::Error::InvalidParameterName("Invalid timestamp".into())
                        })?,
                    modified_time: Local
                        .timestamp_opt(modified_timestamp, 0)
                        .single()
                        .ok_or_else(|| {
                            rusqlite::Error::InvalidParameterName("Invalid timestamp".into())
                        })?,
                    last_parsed_timestamp: Local
                        .timestamp_opt(parsed_timestamp, 0)
                        .single()
                        .ok_or_else(|| {
                            rusqlite::Error::InvalidParameterName("Invalid timestamp".into())
                        })?,

                    tempo: row.get(8)?,
                    time_signature: TimeSignature {
                        numerator: row.get(9)?,
                        denominator: row.get(10)?,
                    },
                    key_signature: match (row.get::<_, Option<String>>(11)?, row.get::<_, Option<String>>(12)?) {
                        (Some(tonic), Some(scale)) => Some(KeySignature {
                            tonic: tonic.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                            scale: scale.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                        }),
                        _ => None,
                    },
                    furthest_bar: row.get(14)?,
                    ableton_version: AbletonVersion {
                        major: row.get(15)?,
                        minor: row.get(16)?,
                        patch: row.get(17)?,
                        beta: row.get(18)?,
                    },
                    estimated_duration: duration_secs.map(chrono::Duration::seconds),
                    plugins: HashSet::new(),
                    samples: HashSet::new(),
                    tags: HashSet::new(),
                };

                // Get plugins, samples, and tags in separate scopes
                {
                    let mut stmt = tx.prepare(
                        "SELECT p.* FROM plugins p JOIN project_plugins pp ON pp.plugin_id = p.id WHERE pp.project_id = ?"
                    )?;
                    live_set.plugins = stmt.query_map([&project_id], |row| {
                        Ok(Plugin {
                            id: Uuid::new_v4(),
                            plugin_id: row.get(1)?,
                            module_id: row.get(2)?,
                            dev_identifier: row.get(3)?,
                            name: row.get(4)?,
                            plugin_format: row.get::<_, String>(5)?.parse()
                                .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                            installed: row.get(6)?,
                            vendor: row.get(7)?,
                            version: row.get(8)?,
                            sdk_version: row.get(9)?,
                            flags: row.get(10)?,
                            scanstate: row.get(11)?,
                            enabled: row.get(12)?,
                        })
                    })?.filter_map(|r| r.ok()).collect();
                }

                {
                    let mut stmt = tx.prepare(
                        "SELECT s.* FROM samples s JOIN project_samples ps ON ps.sample_id = s.id WHERE ps.project_id = ?"
                    )?;
                    live_set.samples = stmt.query_map([&project_id], |row| {
                        Ok(Sample {
                            id: Uuid::new_v4(),
                            name: row.get(1)?,
                            path: PathBuf::from(row.get::<_, String>(2)?),
                            is_present: row.get(3)?,
                        })
                    })?.filter_map(|r| r.ok()).collect();
                }

                {
                    let mut stmt = tx.prepare(
                        "SELECT t.name FROM tags t JOIN project_tags pt ON pt.tag_id = t.id WHERE pt.project_id = ?"
                    )?;
                    live_set.tags = stmt.query_map([&project_id], |row| row.get(0))?.filter_map(|r| r.ok()).collect();
                }

                debug!("Adding project to results: {}", live_set.name);
                results.push(live_set);
            }
        }

        tx.commit()?;
        debug!("Retrieved {} projects from collection", results.len());
        Ok(results)
    }

    pub fn list_collections(&mut self) -> Result<Vec<(String, String, Option<String>)>, DatabaseError> {
        debug!("Listing all collections");
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description FROM collections ORDER BY name"
        )?;

        let collections = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let description: Option<String> = row.get(2)?;
                debug!("Found collection: {} ({})", name, id);
                Ok((id, name, description))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved all collections");
        Ok(collections)
    }

    /// Get all collection IDs that contain a specific project
    pub fn get_collections_for_project(&mut self, project_id: &str) -> Result<Vec<String>, DatabaseError> {
        debug!("Getting collections for project: {}", project_id);
        let mut stmt = self.conn.prepare(
            "SELECT collection_id FROM collection_projects WHERE project_id = ?"
        )?;

        let collection_ids: Vec<String> = stmt
            .query_map([project_id], |row| {
                let collection_id: String = row.get(0)?;
                debug!("Found collection: {}", collection_id);
                Ok(collection_id)
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved {} collections for project", collection_ids.len());
        Ok(collection_ids)
    }

    /// Get statistics for a specific collection (total duration and project count)
    pub fn get_collection_statistics(&mut self, collection_id: &str) -> Result<(Option<f64>, i32), DatabaseError> {
        debug!("Getting statistics for collection {}", collection_id);
        
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                SUM(COALESCE(p.duration_seconds, 0)) as total_duration,
                COUNT(p.id) as project_count
            FROM collection_projects cp
            LEFT JOIN projects p ON p.id = cp.project_id
            WHERE cp.collection_id = ?
            "#
        )?;

        let result = stmt.query_row([collection_id], |row| {
            let total_duration: Option<f64> = row.get(0)?;
            let project_count: i32 = row.get(1)?;
            Ok((total_duration, project_count))
        });

        match result {
            Ok((duration, count)) => {
                debug!("Collection statistics: {} projects, {:?} total duration", count, duration);
                Ok((duration, count))
            }
            Err(e) => Err(DatabaseError::from(e))
        }
    }
}
