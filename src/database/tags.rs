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

    pub fn update_tag(&mut self, tag_id: &str, name: &str) -> Result<(), DatabaseError> {
        debug!("Updating tag {} to name: {}", tag_id, name);
        let rows_affected = self.conn.execute(
            "UPDATE tags SET name = ? WHERE id = ?",
            params![name, tag_id],
        )?;

        if rows_affected == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Tag with id {} not found",
                tag_id
            )));
        }

        debug!("Successfully updated tag: {}", tag_id);
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

    /// Get tag IDs for a project (for gRPC responses)
    pub fn get_project_tag_ids(&mut self, project_id: &str) -> Result<Vec<String>, DatabaseError> {
        debug!("Getting tag IDs for project: {}", project_id);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id 
            FROM tags t
            JOIN project_tags pt ON pt.tag_id = t.id
            WHERE pt.project_id = ?
            "#,
        )?;

        let tag_ids: Vec<String> = stmt
            .query_map([project_id], |row| {
                let tag_id: String = row.get(0)?;
                debug!("Found tag ID: {}", tag_id);
                Ok(tag_id)
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved {} tag IDs for project", tag_ids.len());
        Ok(tag_ids)
    }

    /// Get tag data with creation timestamps for a project (for gRPC responses)
    pub fn get_project_tag_data(
        &mut self,
        project_id: &str,
    ) -> Result<Vec<(String, String, i64)>, DatabaseError> {
        debug!("Getting tag data for project: {}", project_id);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.name, t.created_at
            FROM tags t
            JOIN project_tags pt ON pt.tag_id = t.id
            WHERE pt.project_id = ?
            ORDER BY t.created_at
            "#,
        )?;

        let tag_data: Vec<(String, String, i64)> = stmt
            .query_map([project_id], |row| {
                let tag_id: String = row.get(0)?;
                let tag_name: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                debug!(
                    "Found tag: {} ({}) created at {}",
                    tag_name, tag_id, created_at
                );
                Ok((tag_id, tag_name, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved {} tag data entries for project", tag_data.len());
        Ok(tag_data)
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
                        id, path, name, hash, created_at, modified_at, last_parsed_at,
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
                            .timestamp_opt(scanned_timestamp, 0)
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
                debug!("Retrieved project: {}", live_set.name);
                results.push(live_set);
            }
        }

        tx.commit()?;
        debug!("Found {} projects with tag", results.len());
        Ok(results)
    }

    pub fn list_tags(&mut self) -> Result<Vec<(String, String, i64)>, DatabaseError> {
        debug!("Listing all tags");
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM tags ORDER BY name")?;

        let tags = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                debug!("Found tag: {} ({}) created at {}", name, id, created_at);
                Ok((id, name, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved all tags");
        Ok(tags)
    }

    pub fn get_tag_by_id(
        &mut self,
        tag_id: &str,
    ) -> Result<Option<(String, String, i64)>, DatabaseError> {
        debug!("Getting tag by ID: {}", tag_id);
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM tags WHERE id = ?")?;

        let tag = stmt
            .query_row([tag_id], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                debug!("Found tag: {} ({}) created at {}", name, id, created_at);
                Ok((id, name, created_at))
            })
            .optional()?;

        debug!("Retrieved tag by ID");
        Ok(tag)
    }

    /// Search tags by name
    pub fn search_tags(
        &mut self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<(Vec<(String, String, i64)>, i32), DatabaseError> {
        debug!("Searching tags with query: {}", query);

        // Get total count
        let count_query = "SELECT COUNT(*) FROM tags WHERE name LIKE ?";
        let mut count_stmt = self.conn.prepare(count_query)?;
        let search_param = format!("%{}%", query);
        let total_count: i32 = count_stmt.query_row([&search_param], |row| row.get(0))?;

        // Get results with pagination
        let main_query = "SELECT id, name, created_at FROM tags WHERE name LIKE ? ORDER BY name LIMIT ? OFFSET ?";
        let mut stmt = self.conn.prepare(main_query)?;
        
        let limit_val = limit.unwrap_or(50);
        let offset_val = offset.unwrap_or(0);
        
        let tags: Vec<(String, String, i64)> = stmt
            .query_map([&search_param, &limit_val.to_string(), &offset_val.to_string()], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                Ok((id, name, created_at))
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Found {} tags matching search criteria", tags.len());
        Ok((tags, total_count))
    }

    /// Get tag statistics for analytics
    pub fn get_tag_statistics(&mut self) -> Result<TagStatistics, DatabaseError> {
        debug!("Getting tag statistics");

        // Get basic counts
        let total_tags: i32 = self.conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        
        let tags_in_use: i32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT tag_id) FROM project_tags", 
            [], 
            |row| row.get(0)
        )?;
        
        let unused_tags = total_tags - tags_in_use;

        // Get projects with/without tags
        let projects_with_tags: i32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT project_id) FROM project_tags", 
            [], 
            |row| row.get(0)
        )?;
        
        let total_projects: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE is_active = true", 
            [], 
            |row| row.get(0)
        )?;
        
        let projects_with_no_tags = total_projects - projects_with_tags;

        // Calculate average tags per project
        let total_tag_associations: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_tags", 
            [], 
            |row| row.get(0)
        )?;
        
        let average_tags_per_project = if total_projects > 0 {
            total_tag_associations as f64 / total_projects as f64
        } else {
            0.0
        };

        // Get most used tags (top 10)
        let most_used_tags = self.get_tag_usage_ranking(10, false)?;
        
        // Get least used tags (bottom 10, but only those that are actually used)
        let least_used_tags = self.get_tag_usage_ranking(10, true)?;

        Ok(TagStatistics {
            total_tags,
            tags_in_use,
            unused_tags,
            average_tags_per_project,
            most_used_tags,
            least_used_tags,
            projects_with_no_tags,
            projects_with_tags,
        })
    }

    /// Get tag usage ranking for statistics
    fn get_tag_usage_ranking(&mut self, limit: i32, least_used: bool) -> Result<Vec<TagUsageInfo>, DatabaseError> {
        let order = if least_used { "ASC" } else { "DESC" };
        let query = format!(
            r#"
            SELECT t.id, t.name, COUNT(pt.project_id) as usage_count,
                   CAST(COUNT(pt.project_id) AS REAL) / (SELECT COUNT(*) FROM projects WHERE is_active = true) * 100.0 as usage_percentage
            FROM tags t
            LEFT JOIN project_tags pt ON pt.tag_id = t.id
            GROUP BY t.id, t.name
            HAVING usage_count > 0
            ORDER BY usage_count {} 
            LIMIT ?
            "#,
            order
        );

        let mut stmt = self.conn.prepare(&query)?;
        let tags = stmt
            .query_map([limit], |row| {
                let tag_id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let project_count: i32 = row.get(2)?;
                let usage_percentage: f64 = row.get(3)?;
                Ok(TagUsageInfo {
                    tag_id,
                    name,
                    project_count,
                    usage_percentage,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tags)
    }

    /// Get all tags with usage information
    pub fn get_all_tags_with_usage(
        &mut self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
        min_usage_count: Option<i32>,
    ) -> Result<(Vec<TagUsageInfo>, i32), DatabaseError> {
        debug!("Getting all tags with usage information");

        // Build WHERE clause for filtering
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(min_usage) = min_usage_count {
            conditions.push("usage_count >= ?");
            params.push(Box::new(min_usage));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("HAVING {}", conditions.join(" AND "))
        };

        // Determine sort column and order
        let sort_column = match sort_by.as_deref() {
            Some("name") => "t.name",
            Some("usage_count") => "usage_count",
            Some("created_at") => "t.created_at",
            _ => "t.name", // default sort
        };

        let sort_order = if sort_desc.unwrap_or(false) { "DESC" } else { "ASC" };

        // Get total count
        let count_query = format!(
            r#"
            SELECT COUNT(*) FROM (
                SELECT t.id
                FROM tags t
                LEFT JOIN project_tags pt ON pt.tag_id = t.id
                GROUP BY t.id, t.name
                {}
            )
            "#,
            where_clause
        );
        let mut count_stmt = self.conn.prepare(&count_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let total_count: i32 = count_stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;

        // Get results with pagination
        let main_query = format!(
            r#"
            SELECT t.id, t.name, COUNT(pt.project_id) as usage_count,
                   CAST(COUNT(pt.project_id) AS REAL) / (SELECT COUNT(*) FROM projects WHERE is_active = true) * 100.0 as usage_percentage
            FROM tags t
            LEFT JOIN project_tags pt ON pt.tag_id = t.id
            GROUP BY t.id, t.name
            {}
            ORDER BY {} {}
            LIMIT ? OFFSET ?
            "#,
            where_clause, sort_column, sort_order
        );

        let limit_val = limit.unwrap_or(50);
        let offset_val = offset.unwrap_or(0);
        params.push(Box::new(limit_val));
        params.push(Box::new(offset_val));

        let mut stmt = self.conn.prepare(&main_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let tags: Vec<TagUsageInfo> = stmt
            .query_map(param_refs.as_slice(), |row| {
                let tag_id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let project_count: i32 = row.get(2)?;
                let usage_percentage: f64 = row.get(3)?;
                Ok(TagUsageInfo {
                    tag_id,
                    name,
                    project_count,
                    usage_percentage,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        debug!("Retrieved {} tags with usage information", tags.len());
        Ok((tags, total_count))
    }

    // Batch Tag Operations
    pub fn batch_tag_projects(
        &mut self,
        project_ids: &[String],
        tag_ids: &[String],
    ) -> Result<Vec<(String, Result<(), DatabaseError>)>, DatabaseError> {
        debug!(
            "Batch tagging {} projects with {} tags",
            project_ids.len(),
            tag_ids.len()
        );
        let tx = self.conn.transaction()?;
        let mut results = Vec::new();
        let now = Local::now();

        for project_id in project_ids {
            for tag_id in tag_ids {
                let result = tx.execute(
                    "INSERT OR IGNORE INTO project_tags (project_id, tag_id, created_at) VALUES (?, ?, ?)",
                    params![project_id, tag_id, SqlDateTime::from(now)],
                );

                match result {
                    Ok(_) => {
                        debug!(
                            "Successfully tagged project {} with tag {}",
                            project_id, tag_id
                        );
                        results.push((format!("{}:{}", project_id, tag_id), Ok(())));
                    }
                    Err(e) => {
                        debug!(
                            "Failed to tag project {} with tag {}: {}",
                            project_id, tag_id, e
                        );
                        results.push((
                            format!("{}:{}", project_id, tag_id),
                            Err(DatabaseError::from(e)),
                        ));
                    }
                }
            }
        }

        tx.commit()?;
        debug!(
            "Batch tag operation completed with {} results",
            results.len()
        );
        Ok(results)
    }

    pub fn batch_untag_projects(
        &mut self,
        project_ids: &[String],
        tag_ids: &[String],
    ) -> Result<Vec<(String, Result<(), DatabaseError>)>, DatabaseError> {
        debug!(
            "Batch untagging {} projects from {} tags",
            project_ids.len(),
            tag_ids.len()
        );
        let tx = self.conn.transaction()?;
        let mut results = Vec::new();

        for project_id in project_ids {
            for tag_id in tag_ids {
                let result = tx.execute(
                    "DELETE FROM project_tags WHERE project_id = ? AND tag_id = ?",
                    params![project_id, tag_id],
                );

                match result {
                    Ok(_) => {
                        debug!(
                            "Successfully untagged project {} from tag {}",
                            project_id, tag_id
                        );
                        results.push((format!("{}:{}", project_id, tag_id), Ok(())));
                    }
                    Err(e) => {
                        debug!(
                            "Failed to untag project {} from tag {}: {}",
                            project_id, tag_id, e
                        );
                        results.push((
                            format!("{}:{}", project_id, tag_id),
                            Err(DatabaseError::from(e)),
                        ));
                    }
                }
            }
        }

        tx.commit()?;
        debug!(
            "Batch untag operation completed with {} results",
            results.len()
        );
        Ok(results)
    }
}

// Database structs for tag analytics
pub struct TagStatistics {
    pub total_tags: i32,
    pub tags_in_use: i32,
    pub unused_tags: i32,
    pub average_tags_per_project: f64,
    pub most_used_tags: Vec<TagUsageInfo>,
    pub least_used_tags: Vec<TagUsageInfo>,
    pub projects_with_no_tags: i32,
    pub projects_with_tags: i32,
}

pub struct TagUsageInfo {
    pub tag_id: String,
    pub name: String,
    pub project_count: i32,
    pub usage_percentage: f64,
}
