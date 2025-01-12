use crate::database::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use chrono::{Local, TimeZone};
use log::debug;
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    pub fn add_tag(&mut self, name: &str) -> Result<String, DatabaseError> {
        debug!("Adding tag: {}", name);
        let tag_id = Uuid::new_v4().to_string();
        let now = Local::now();

        self.conn.execute(
            "INSERT INTO tags (id, name, created_at) VALUES (?, ?, ?)",
            params![tag_id, name, SqlDateTime::from(now)],
        )?;

        debug!("Successfully added tag: {} ({})", name, tag_id);
        Ok(tag_id)
    }

    pub fn remove_tag(&mut self, tag_id: &str) -> Result<(), DatabaseError> {
        debug!("Removing tag: {}", tag_id);
        self.conn
            .execute("DELETE FROM tags WHERE id = ?", [tag_id])?;
        debug!("Successfully removed tag: {}", tag_id);
        Ok(())
    }

    pub fn tag_project(&mut self, project_id: &str, tag_id: &str) -> Result<(), DatabaseError> {
        debug!("Tagging project {} with tag {}", project_id, tag_id);
        let now = Local::now();

        self.conn.execute(
            "INSERT OR IGNORE INTO project_tags (project_id, tag_id, created_at) VALUES (?, ?, ?)",
            params![project_id, tag_id, SqlDateTime::from(now)],
        )?;

        debug!("Successfully tagged project");
        Ok(())
    }

    pub fn untag_project(&mut self, project_id: &str, tag_id: &str) -> Result<(), DatabaseError> {
        debug!("Removing tag {} from project {}", tag_id, project_id);
        self.conn.execute(
            "DELETE FROM project_tags WHERE project_id = ? AND tag_id = ?",
            params![project_id, tag_id],
        )?;
        debug!("Successfully untagged project");
        Ok(())
    }

    pub fn get_project_tags(&mut self, project_id: &str) -> Result<HashSet<String>, DatabaseError> {
        debug!("Getting tags for project: {}", project_id);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.name 
            FROM tags t
            JOIN project_tags pt ON pt.tag_id = t.id
            WHERE pt.project_id = ?
            "#,
        )?;

        let tags = stmt
            .query_map([project_id], |row| {
                let name: String = row.get(0)?;
                debug!("Found tag: {}", name);
                Ok(name)
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved tags for project");
        Ok(tags)
    }

    pub fn get_projects_by_tag(&mut self, tag_id: &str) -> Result<Vec<LiveSet>, DatabaseError> {
        debug!("Getting projects with tag: {}", tag_id);
        let tx = self.conn.transaction()?;

        let project_paths = {
            let mut stmt = tx.prepare(
                r#"
                SELECT DISTINCT p.path 
                FROM projects p
                JOIN project_tags pt ON pt.project_id = p.id
                WHERE pt.tag_id = ?
                "#,
            )?;

            let paths: Vec<String> = stmt
                .query_map([tag_id], |row| {
                    let path: String = row.get(0)?;
                    Ok(path)
                })?
                .filter_map(|r| r.ok())
                .collect();

            debug!("Found {} project paths with tag", paths.len());
            paths
        };

        let mut results = Vec::new();
        for path in project_paths {
            let project = {
                let mut stmt = tx.prepare(
                    r#"
                    SELECT 
                        id, path, name, hash, created_at, modified_at, last_scanned_at,
                        tempo, time_signature_numerator, time_signature_denominator,
                        key_signature_tonic, key_signature_scale, duration_seconds, furthest_bar,
                        ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta
                    FROM projects 
                    WHERE path = ?
                    "#,
                )?;

                stmt.query_row([&path], |row| {
                    let project_id: String = row.get(0)?;
                    debug!("Found project with ID: {}", project_id);

                    let duration_secs: Option<i64> = row.get(12)?;
                    let created_timestamp: i64 = row.get(4)?;
                    let modified_timestamp: i64 = row.get(5)?;
                    let scanned_timestamp: i64 = row.get(6)?;

                    let mut live_set = LiveSet {
                        id: Uuid::parse_str(&project_id).map_err(|_| {
                            rusqlite::Error::InvalidParameterName("Invalid UUID".into())
                        })?,
                        file_path: PathBuf::from(row.get::<_, String>(1)?),
                        file_name: row.get(2)?,
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
                        last_scan_timestamp: Local
                            .timestamp_opt(scanned_timestamp, 0)
                            .single()
                            .ok_or_else(|| {
                                rusqlite::Error::InvalidParameterName("Invalid timestamp".into())
                            })?,
                        xml_data: Vec::new(),

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

                    // Get plugins in a new scope
                    {
                        let mut stmt = tx.prepare(
                            r#"
                            SELECT p.* 
                            FROM plugins p
                            JOIN project_plugins pp ON pp.plugin_id = p.id
                            WHERE pp.project_id = ?
                            "#,
                        )?;

                        let plugins = stmt
                            .query_map([&project_id], |row| {
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
                            .filter_map(|r| r.ok())
                            .collect::<HashSet<_>>();

                        debug!("Retrieved {} plugins", plugins.len());
                        live_set.plugins = plugins;
                    }

                    // Get samples in a new scope
                    {
                        let mut stmt = tx.prepare(
                            r#"
                            SELECT s.* 
                            FROM samples s
                            JOIN project_samples ps ON ps.sample_id = s.id
                            WHERE ps.project_id = ?
                            "#,
                        )?;

                        let samples = stmt
                            .query_map([&project_id], |row| {
                                let name: String = row.get(1)?;
                                debug!("Found sample: {}", name);
                                Ok(Sample {
                                    id: Uuid::new_v4(),
                                    name,
                                    path: PathBuf::from(row.get::<_, String>(2)?),
                                    is_present: row.get(3)?,
                                })
                            })?
                            .filter_map(|r| r.ok())
                            .collect::<HashSet<_>>();

                        debug!("Retrieved {} samples", samples.len());
                        live_set.samples = samples;
                    }

                    // Get tags in a new scope
                    {
                        let mut stmt = tx.prepare(
                            r#"
                            SELECT t.name 
                            FROM tags t
                            JOIN project_tags pt ON pt.tag_id = t.id
                            WHERE pt.project_id = ?
                            "#,
                        )?;

                        let tags = stmt
                            .query_map([&project_id], |row| {
                                let name: String = row.get(0)?;
                                debug!("Found tag: {}", name);
                                Ok(name)
                            })?
                            .filter_map(|r| r.ok())
                            .collect::<HashSet<_>>();

                        debug!("Retrieved {} tags", tags.len());
                        live_set.tags = tags;
                    }

                    Ok(live_set)
                })
                .optional()?
            };

            if let Some(live_set) = project {
                debug!("Retrieved project: {}", live_set.file_name);
                results.push(live_set);
            }
        }

        tx.commit()?;
        debug!("Found {} projects with tag", results.len());
        Ok(results)
    }

    pub fn list_tags(&mut self) -> Result<Vec<(String, String)>, DatabaseError> {
        debug!("Listing all tags");
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM tags ORDER BY name")?;

        let tags = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                debug!("Found tag: {} ({})", name, id);
                Ok((id, name))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved all tags");
        Ok(tags)
    }
}
