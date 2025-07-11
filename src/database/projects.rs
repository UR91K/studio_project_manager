use super::models::SqlDateTime;
use super::helpers::{insert_plugin, insert_sample, link_project_plugin, link_project_sample, row_to_live_set};
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use crate::utils::metadata::load_file_hash;
use chrono::{Local, TimeZone, Utc};
use log::{debug, info};
use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {

    pub fn get_project_by_id(&mut self, id: &str) -> Result<Option<LiveSet>, DatabaseError> {
        debug!("Retrieving project by ID: {}", id);
        let tx = self.conn.transaction()?;

        // Get project
        let mut stmt = tx.prepare(
            r#"
            SELECT 
                id, path, name, hash, created_at, modified_at, last_parsed_at,
                tempo, time_signature_numerator, time_signature_denominator,
                key_signature_tonic, key_signature_scale, duration_seconds, furthest_bar,
                ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta
            FROM projects 
            WHERE id = ? AND is_active = true
            "#,
        )?;

        let project = stmt
            .query_row([id], |row| {
                let project_id: String = row.get(0)?;
                debug!("Found project with ID: {}", project_id);

                let duration_secs: Option<i64> = row.get(12)?;
                let created_timestamp: i64 = row.get(4)?;
                let modified_timestamp: i64 = row.get(5)?;
                let parsed_timestamp: i64 = row.get(6)?;

                // Create LiveSet instance
                let live_set = LiveSet {
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

                    tempo: row.get(7)?,
                    time_signature: TimeSignature {
                        numerator: row.get(8)?,
                        denominator: row.get(9)?,
                    },
                    key_signature: match (
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ) {
                        (Some(tonic), Some(scale)) => {
                            debug!("Found key signature: {} {}", tonic, scale);
                            Some(KeySignature {
                                tonic: tonic
                                    .parse()
                                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                scale: scale
                                    .parse()
                                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                            })
                        }
                        _ => None,
                    },
                    furthest_bar: row.get(13)?,

                    ableton_version: AbletonVersion {
                        major: row.get(14)?,
                        minor: row.get(15)?,
                        patch: row.get(16)?,
                        beta: row.get(17)?,
                    },

                    estimated_duration: duration_secs.map(chrono::Duration::seconds),
                    plugins: HashSet::new(),
                    samples: HashSet::new(),
                    tags: HashSet::new(),
                };

                Ok(live_set)
            })
            .optional()?;

        let mut project = match project {
            Some(p) => {
                debug!("Found project: {}", p.name);
                p
            }
            None => {
                debug!("No project found with ID: {}", id);
                return Ok(None);
            }
        };

        // Get plugins
        debug!("Retrieving plugins for project");
        let mut stmt = tx.prepare(
            r#"
            SELECT p.* 
            FROM plugins p
            JOIN project_plugins pp ON pp.plugin_id = p.id
            WHERE pp.project_id = ?
            "#,
        )?;

        let plugins = stmt
            .query_map([id], |row| {
                let name: String = row.get(4)?;
                debug!("Found plugin: {}", name);
                Ok(Plugin {
                    id: Uuid::new_v4(),
                    plugin_id: row.get(1)?,
                    module_id: row.get(2)?,
                    dev_identifier: row.get(3)?,
                    name,
                    plugin_format: row
                        .get::<_, String>(5)?
                        .parse()
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                    installed: row.get(6)?,
                    vendor: row.get(7)?,
                    version: row.get(8)?,
                    sdk_version: row.get(9)?,
                    flags: row.get(10)?,
                    scanstate: row.get(11)?,
                    enabled: row.get(12)?,
                })
            })?
            .collect::<SqliteResult<HashSet<_>>>()?;

        debug!("Retrieved {} plugins", plugins.len());
        project.plugins = plugins;

        // Get samples
        debug!("Retrieving samples for project");
        let mut stmt = tx.prepare(
            r#"
            SELECT s.* 
            FROM samples s
            JOIN project_samples ps ON ps.sample_id = s.id
            WHERE ps.project_id = ?
            "#,
        )?;

        let samples = stmt
            .query_map([id], |row| {
                let name: String = row.get(1)?;
                debug!("Found sample: {}", name);
                Ok(Sample {
                    id: Uuid::new_v4(),
                    name,
                    path: PathBuf::from(row.get::<_, String>(2)?),
                    is_present: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<HashSet<_>>>()?;

        debug!("Retrieved {} samples", samples.len());
        project.samples = samples;

        info!(
            "Successfully retrieved project {} with {} plugins and {} samples",
            project.name,
            project.plugins.len(),
            project.samples.len()
        );
        Ok(Some(project))
    }

    pub fn get_project_by_path(&mut self, path: &str) -> Result<Option<LiveSet>, DatabaseError> {
        debug!("Retrieving project by path: {}", path);
        let tx = self.conn.transaction()?;

        // Get project
        let mut stmt = tx.prepare(
            r#"
            SELECT 
                id, path, name, hash, created_at, modified_at, last_parsed_at,
                tempo, time_signature_numerator, time_signature_denominator,
                key_signature_tonic, key_signature_scale, duration_seconds, furthest_bar,
                ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta
            FROM projects 
            WHERE path = ?
            "#,
        )?;

        let project = stmt
            .query_row([path], |row| {
                let project_id: String = row.get(0)?;
                debug!("Found project with ID: {}", project_id);

                let duration_secs: Option<i64> = row.get(12)?;
                let created_timestamp: i64 = row.get(4)?;
                let modified_timestamp: i64 = row.get(5)?;
                let parsed_timestamp: i64 = row.get(6)?;

                // Create LiveSet instance
                let live_set = LiveSet {
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

                    tempo: row.get(7)?,
                    time_signature: TimeSignature {
                        numerator: row.get(8)?,
                        denominator: row.get(9)?,
                    },
                    key_signature: match (
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ) {
                        (Some(tonic), Some(scale)) => {
                            debug!("Found key signature: {} {}", tonic, scale);
                            Some(KeySignature {
                                tonic: tonic
                                    .parse()
                                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                scale: scale
                                    .parse()
                                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                            })
                        }
                        _ => None,
                    },
                    furthest_bar: row.get(13)?,

                    ableton_version: AbletonVersion {
                        major: row.get(14)?,
                        minor: row.get(15)?,
                        patch: row.get(16)?,
                        beta: row.get(17)?,
                    },

                    estimated_duration: duration_secs.map(chrono::Duration::seconds),
                    plugins: HashSet::new(),
                    samples: HashSet::new(),
                    tags: HashSet::new(),
                };

                Ok(live_set)
            })
            .optional()?;

        let mut project = match project {
            Some(p) => {
                debug!("Found project: {}", p.name);
                p
            }
            None => {
                debug!("No project found at path: {}", path);
                return Ok(None);
            }
        };

        // Get plugins
        debug!("Retrieving plugins for project");
        let mut stmt = tx.prepare(
            r#"
            SELECT p.* 
            FROM plugins p
            JOIN project_plugins pp ON pp.plugin_id = p.id
            WHERE pp.project_id = (SELECT id FROM projects WHERE path = ?)
            "#,
        )?;

        let plugins = stmt
            .query_map([path], |row| {
                let name: String = row.get(4)?;
                debug!("Found plugin: {}", name);
                Ok(Plugin {
                    id: Uuid::new_v4(),
                    plugin_id: row.get(1)?,
                    module_id: row.get(2)?,
                    dev_identifier: row.get(3)?,
                    name,
                    plugin_format: row
                        .get::<_, String>(5)?
                        .parse()
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                    installed: row.get(6)?,
                    vendor: row.get(7)?,
                    version: row.get(8)?,
                    sdk_version: row.get(9)?,
                    flags: row.get(10)?,
                    scanstate: row.get(11)?,
                    enabled: row.get(12)?,
                })
            })?
            .collect::<SqliteResult<HashSet<_>>>()?;

        debug!("Retrieved {} plugins", plugins.len());
        project.plugins = plugins;

        // Get samples
        debug!("Retrieving samples for project");
        let mut stmt = tx.prepare(
            r#"
            SELECT s.* 
            FROM samples s
            JOIN project_samples ps ON ps.sample_id = s.id
            WHERE ps.project_id = (SELECT id FROM projects WHERE path = ?)
            "#,
        )?;

        let samples = stmt
            .query_map([path], |row| {
                let name: String = row.get(1)?;
                debug!("Found sample: {}", name);
                Ok(Sample {
                    id: Uuid::new_v4(),
                    name,
                    path: PathBuf::from(row.get::<_, String>(2)?),
                    is_present: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<HashSet<_>>>()?;

        debug!("Retrieved {} samples", samples.len());
        project.samples = samples;

        info!(
            "Successfully retrieved project {} with {} plugins and {} samples",
            project.name,
            project.plugins.len(),
            project.samples.len()
        );
        Ok(Some(project))
    }

    pub fn insert_project(&mut self, live_set: &LiveSet) -> Result<(), DatabaseError> {
        debug!(
            "Inserting project: {} ({})",
            live_set.name,
            live_set.file_path.display()
        );
        let tx = self.conn.transaction()?;

        // Insert project
        let project_id = live_set.id.to_string();
        debug!("Using project UUID: {}", project_id);

        tx.execute(
            "INSERT INTO projects (
                id, name, path, hash, created_at, modified_at,
                last_parsed_at, tempo, time_signature_numerator,
                time_signature_denominator, key_signature_tonic,
                key_signature_scale, furthest_bar, duration_seconds,
                ableton_version_major, ableton_version_minor,
                ableton_version_patch, ableton_version_beta,
                notes
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )",
            params![
                project_id,
                live_set.name,
                live_set.file_path.to_string_lossy().to_string(),
                live_set.file_hash,
                SqlDateTime::from(live_set.created_time),
                SqlDateTime::from(live_set.modified_time),
                SqlDateTime::from(live_set.last_parsed_timestamp),
                live_set.tempo,
                live_set.time_signature.numerator,
                live_set.time_signature.denominator,
                live_set.key_signature.as_ref().map(|k| k.tonic.to_string()),
                live_set.key_signature.as_ref().map(|k| k.scale.to_string()),
                live_set.furthest_bar,
                live_set.estimated_duration.map(|d| d.num_seconds()),
                live_set.ableton_version.major,
                live_set.ableton_version.minor,
                live_set.ableton_version.patch,
                live_set.ableton_version.beta,
                None::<String>, // notes starts as NULL
            ],
        )?;

        // Insert plugins
        debug!("Inserting {} plugins", live_set.plugins.len());
        for plugin in &live_set.plugins {
            let plugin_id = plugin.id.to_string();
            debug!("Inserted plugin: {} ({})", plugin.name, plugin_id);
            insert_plugin(&tx, plugin)?;
            link_project_plugin(&tx, &project_id, &plugin_id)?;
        }

        // Insert samples
        debug!("Inserting {} samples", live_set.samples.len());
        for sample in &live_set.samples {
            let sample_id = sample.id.to_string();
            debug!("Inserted sample: {} ({})", sample.name, sample_id);
            insert_sample(&tx, sample)?;
            link_project_sample(&tx, &project_id, &sample_id)?;
        }

        // Now update the FTS index with all relations set
        tx.execute(
            "UPDATE project_search SET
                plugins = (
                    SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                    FROM plugins pl
                    JOIN project_plugins pp ON pp.plugin_id = pl.id
                    WHERE pp.project_id = ?
                ),
                samples = (
                    SELECT GROUP_CONCAT(s.name, ' ')
                    FROM samples s
                    JOIN project_samples ps ON ps.sample_id = s.id
                    WHERE ps.project_id = ?
                ),
                tags = (
                    SELECT GROUP_CONCAT(t.name, ' ')
                    FROM tags t
                    JOIN project_tags pt ON pt.tag_id = t.id
                    WHERE pt.project_id = ?
                )
            WHERE project_id = ?",
            params![project_id, project_id, project_id, project_id],
        )?;

        // Debug: Inspect FTS index content
        debug!("Inspecting FTS5 index for project {}", live_set.name);
        #[allow(unused)]
        if let Ok(Some(row)) = tx.query_row(
            "SELECT * FROM project_search WHERE project_id = ?",
            params![project_id],
            |row| {
                debug!("FTS5 index content:");
                debug!("  project_id: {}", row.get::<_, String>(0)?);
                debug!("  name: {}", row.get::<_, String>(1)?);
                debug!("  path: {}", row.get::<_, String>(2)?);
                debug!("  plugins: {:?}", row.get::<_, Option<String>>(3)?);
                debug!("  samples: {:?}", row.get::<_, Option<String>>(4)?);
                debug!("  tags: {:?}", row.get::<_, Option<String>>(5)?);
                debug!("  notes: {:?}", row.get::<_, Option<String>>(6)?);
                debug!("  created_at: {:?}", row.get::<_, Option<String>>(7)?);
                debug!("  modified_at: {:?}", row.get::<_, Option<String>>(8)?);
                debug!("  tempo: {:?}", row.get::<_, Option<String>>(9)?);
                debug!("  key_signature: {:?}", row.get::<_, Option<String>>(10)?);
                debug!("  time_signature: {:?}", row.get::<_, Option<String>>(11)?);
                debug!("  version: {:?}", row.get::<_, Option<String>>(12)?);
                Ok(Some(()))
            },
        ) {
            debug!("Successfully inspected FTS5 index");
        }

        tx.commit()?;
        info!(
            "Successfully inserted project {} with {} plugins and {} samples",
            live_set.name,
            live_set.plugins.len(),
            live_set.samples.len()
        );

        Ok(())
    }

    pub fn mark_project_deleted(&mut self, project_id: &Uuid) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE projects SET is_active = false WHERE id = ?",
            params![project_id.to_string()],
        ).map_err(DatabaseError::from)?;
        Ok(())
    }

    pub fn reactivate_project(
        &mut self, 
        project_id: &Uuid,
        new_path: &Path,
    ) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE projects SET 
                is_active = true,
                path = ?,
                modified_at = ?
             WHERE id = ?",
            params![
                new_path.to_string_lossy().to_string(),
                Utc::now().timestamp(),
                project_id.to_string(),
            ],
        ).map_err(DatabaseError::from)?;
        Ok(())
    }

    pub fn find_deleted_project_by_hash(&mut self, path: &Path) -> Result<Option<LiveSet>, DatabaseError> {
        let hash = load_file_hash(&path.to_path_buf())?;
        
        self.conn.query_row(
            "SELECT * FROM projects 
             WHERE is_active = false AND hash = ?",
            params![hash],
            |row| row_to_live_set(row),
        ).optional().map_err(DatabaseError::from)
    }

    pub fn get_all_projects_with_status(
        &self,
        is_active: Option<bool>
    ) -> Result<Vec<LiveSet>, DatabaseError> {
        let mut results = Vec::new();
        
        let query = match is_active {
            Some(status) => {
                format!("SELECT * FROM projects WHERE is_active = {}", status)
            }
            None => {
                "SELECT * FROM projects".to_string()
            }
        };
        
        let mut stmt = self.conn.prepare(&query).map_err(DatabaseError::from)?;
        
        let mut rows = stmt.query([]).map_err(DatabaseError::from)?;
        while let Some(row) = rows.next().map_err(DatabaseError::from)? {
            let mut live_set = row_to_live_set(row)?;
            let project_id_str = live_set.id.to_string();
            
            // Load plugins for this project
            {
                let mut plugin_stmt = self.conn.prepare(
                    "SELECT p.* FROM plugins p JOIN project_plugins pp ON pp.plugin_id = p.id WHERE pp.project_id = ?"
                ).map_err(DatabaseError::from)?;
                live_set.plugins = plugin_stmt.query_map([&project_id_str], |row| {
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
                }).map_err(DatabaseError::from)?.filter_map(|r| r.ok()).collect();
            }
            
            // Load samples for this project
            {
                let mut sample_stmt = self.conn.prepare(
                    "SELECT s.* FROM samples s JOIN project_samples ps ON ps.sample_id = s.id WHERE ps.project_id = ?"
                ).map_err(DatabaseError::from)?;
                live_set.samples = sample_stmt.query_map([&project_id_str], |row| {
                    Ok(Sample {
                        id: Uuid::new_v4(),
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        is_present: row.get(3)?,
                    })
                }).map_err(DatabaseError::from)?.filter_map(|r| r.ok()).collect();
            }
            
            // Load tags for this project
            {
                let mut tag_stmt = self.conn.prepare(
                    "SELECT t.name FROM tags t JOIN project_tags pt ON pt.tag_id = t.id WHERE pt.project_id = ?"
                ).map_err(DatabaseError::from)?;
                live_set.tags = tag_stmt.query_map([&project_id_str], |row| row.get(0))
                    .map_err(DatabaseError::from)?.filter_map(|r| r.ok()).collect();
            }
            
            results.push(live_set);
        }
        
        Ok(results)
    }

    pub fn permanently_delete_project(&mut self, project_id: &Uuid) -> Result<(), DatabaseError> {
        let tx = self.conn.transaction().map_err(DatabaseError::from)?;
        
        // Only allow deletion of inactive projects
        let rows_affected = tx.execute(
            "DELETE FROM projects WHERE id = ? AND is_active = false",
            params![project_id.to_string()],
        ).map_err(DatabaseError::from)?;
        
        if rows_affected == 0 {
            return Err(DatabaseError::InvalidOperation("Cannot permanently delete an active project".to_string()));
        }
        
        tx.commit().map_err(DatabaseError::from)?;
        Ok(())
    }

    /// Get projects that use a specific sample
    pub fn get_projects_by_sample_id(
        &self,
        sample_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<(Vec<LiveSet>, i32), DatabaseError> {
        // Get total count first
        let total_count: i32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT p.id) FROM projects p 
             JOIN project_samples ps ON ps.project_id = p.id 
             WHERE ps.sample_id = ? AND p.is_active = true",
            params![sample_id],
            |row| row.get(0),
        )?;

        // Get the projects with pagination
        let query = "
            SELECT DISTINCT p.* FROM projects p 
            JOIN project_samples ps ON ps.project_id = p.id 
            WHERE ps.sample_id = ? AND p.is_active = true
            ORDER BY p.name ASC
            LIMIT ? OFFSET ?
        ";

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map(
            params![
                sample_id,
                limit.unwrap_or(1000),
                offset.unwrap_or(0)
            ],
            |row| row_to_live_set(row),
        )?;

        let mut projects = Vec::new();
        for row_result in rows {
            let mut project = row_result?;
            let project_id_str = project.id.to_string();
            
            // Load plugins for this project
            {
                let mut plugin_stmt = self.conn.prepare(
                    "SELECT p.* FROM plugins p JOIN project_plugins pp ON pp.plugin_id = p.id WHERE pp.project_id = ?"
                )?;
                project.plugins = plugin_stmt.query_map([&project_id_str], |row| {
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
            
            // Load samples for this project
            {
                let mut sample_stmt = self.conn.prepare(
                    "SELECT s.* FROM samples s JOIN project_samples ps ON ps.sample_id = s.id WHERE ps.project_id = ?"
                )?;
                project.samples = sample_stmt.query_map([&project_id_str], |row| {
                    Ok(Sample {
                        id: Uuid::new_v4(),
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        is_present: row.get(3)?,
                    })
                })?.filter_map(|r| r.ok()).collect();
            }
            
            // Load tags for this project
            {
                let mut tag_stmt = self.conn.prepare(
                    "SELECT t.name FROM tags t JOIN project_tags pt ON pt.tag_id = t.id WHERE pt.project_id = ?"
                )?;
                project.tags = tag_stmt.query_map([&project_id_str], |row| row.get(0))
                    ?.filter_map(|r| r.ok()).collect();
            }
            
            projects.push(project);
        }

        Ok((projects, total_count))
    }

    /// Get projects that use a specific plugin
    pub fn get_projects_by_plugin_id(
        &self,
        plugin_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<(Vec<LiveSet>, i32), DatabaseError> {
        // Get total count first
        let total_count: i32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT p.id) FROM projects p 
             JOIN project_plugins pp ON pp.project_id = p.id 
             WHERE pp.plugin_id = ? AND p.is_active = true",
            params![plugin_id],
            |row| row.get(0),
        )?;

        // Get the projects with pagination
        let query = "
            SELECT DISTINCT p.* FROM projects p 
            JOIN project_plugins pp ON pp.project_id = p.id 
            WHERE pp.plugin_id = ? AND p.is_active = true
            ORDER BY p.name ASC
            LIMIT ? OFFSET ?
        ";

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map(
            params![
                plugin_id,
                limit.unwrap_or(1000),
                offset.unwrap_or(0)
            ],
            |row| row_to_live_set(row),
        )?;

        let mut projects = Vec::new();
        for row_result in rows {
            let mut project = row_result?;
            let project_id_str = project.id.to_string();
            
            // Load plugins for this project
            {
                let mut plugin_stmt = self.conn.prepare(
                    "SELECT p.* FROM plugins p JOIN project_plugins pp ON pp.plugin_id = p.id WHERE pp.project_id = ?"
                )?;
                project.plugins = plugin_stmt.query_map([&project_id_str], |row| {
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
            
            // Load samples for this project
            {
                let mut sample_stmt = self.conn.prepare(
                    "SELECT s.* FROM samples s JOIN project_samples ps ON ps.sample_id = s.id WHERE ps.project_id = ?"
                )?;
                project.samples = sample_stmt.query_map([&project_id_str], |row| {
                    Ok(Sample {
                        id: Uuid::new_v4(),
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        is_present: row.get(3)?,
                    })
                })?.filter_map(|r| r.ok()).collect();
            }
            
            // Load tags for this project
            {
                let mut tag_stmt = self.conn.prepare(
                    "SELECT t.name FROM tags t JOIN project_tags pt ON pt.tag_id = t.id WHERE pt.project_id = ?"
                )?;
                project.tags = tag_stmt.query_map([&project_id_str], |row| row.get(0))
                    ?.filter_map(|r| r.ok()).collect();
            }
            
            projects.push(project);
        }

        Ok((projects, total_count))
    }
} 