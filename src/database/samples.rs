use crate::error::DatabaseError;
use crate::models::Sample;
use rusqlite::params;
use std::path::PathBuf;
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    /// Get all samples with pagination and sorting
    pub fn get_all_samples(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
        present_only: Option<bool>,
        missing_only: Option<bool>,
        extension_filter: Option<String>,
        min_usage_count: Option<i32>,
        max_usage_count: Option<i32>,
    ) -> Result<(Vec<Sample>, i32), DatabaseError> {
        let sort_column = match sort_by.as_deref() {
            Some("name") => "s.name",
            Some("path") => "s.path",
            Some("present") => "s.is_present",
            Some("usage_count") => "usage_count",
            _ => "s.name", // default sort
        };

        let sort_order = if sort_desc.unwrap_or(false) {
            "DESC"
        } else {
            "ASC"
        };

        // Build WHERE conditions for filtering
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Presence filters (mutually exclusive)
        if let Some(present) = present_only {
            conditions.push("s.is_present = ?");
            params.push(Box::new(present));
        } else if let Some(missing) = missing_only {
            conditions.push("s.is_present = ?");
            params.push(Box::new(!missing));
        }

        // Extension filter
        if let Some(extension) = extension_filter {
            conditions.push("s.path LIKE ?");
            params.push(Box::new(format!("%.{}", extension)));
        }

        // Determine if we need to join with project_samples for usage count
        let needs_usage_join = min_usage_count.is_some() || max_usage_count.is_some() || sort_by.as_deref() == Some("usage_count");

        // Usage count filters - only apply when we're using the join approach
        if needs_usage_join && (min_usage_count.is_some() || max_usage_count.is_some()) {
            if let Some(min_count) = min_usage_count {
                conditions.push("COALESCE(usage_count, 0) >= ?");
                params.push(Box::new(min_count));
            }
            
            if let Some(max_count) = max_usage_count {
                conditions.push("COALESCE(usage_count, 0) <= ?");
                params.push(Box::new(max_count));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Build the base query
        let base_query = if needs_usage_join {
            r#"
            SELECT s.*, COALESCE(usage_stats.usage_count, 0) as usage_count
            FROM samples s
            LEFT JOIN (
                SELECT sample_id, COUNT(*) as usage_count
                FROM project_samples
                GROUP BY sample_id
            ) usage_stats ON s.id = usage_stats.sample_id
            "#
        } else {
            "SELECT s.*, 0 as usage_count FROM samples s"
        };

        // Get total count with filters
        let count_query = if needs_usage_join {
            format!("SELECT COUNT(*) FROM ({}) {}", base_query, where_clause)
        } else {
            format!("SELECT COUNT(*) FROM samples s {}", where_clause)
        };
        let mut count_stmt = self.conn.prepare(&count_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let total_count: i32 = count_stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;

        // Build main query with pagination
        let main_query = format!(
            "{} {} ORDER BY {} {} LIMIT ? OFFSET ?",
            base_query, where_clause, sort_column, sort_order
        );

        // Debug logging
        if min_usage_count.is_some() || max_usage_count.is_some() {
            log::debug!("SQL Query: {}", main_query);
        }

        // Add pagination parameters
        params.push(Box::new(limit.unwrap_or(1000)));
        params.push(Box::new(offset.unwrap_or(0)));

        let mut stmt = self.conn.prepare(&main_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let id_str: String = row.get("id")?;
            Ok(Sample {
                id: Uuid::parse_str(&id_str).map_err(|_e| rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text))?,
                name: row.get("name")?,
                path: PathBuf::from(row.get::<_, String>("path")?),
                is_present: row.get("is_present")?,
            })
        })?;

        let samples: Result<Vec<Sample>, _> = rows.collect();
        Ok((samples?, total_count))
    }

    /// Get a single sample by ID
    pub fn get_sample_by_id(&self, sample_id: &str) -> Result<Option<Sample>, DatabaseError> {
        let mut stmt = self.conn.prepare("SELECT * FROM samples WHERE id = ?")?;
        let result = stmt.query_row(params![sample_id], |row| {
            let id_str: String = row.get("id")?;
            Ok(Sample {
                id: Uuid::parse_str(&id_str).map_err(|_e| rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text))?,
                name: row.get("name")?,
                path: PathBuf::from(row.get::<_, String>("path")?),
                is_present: row.get("is_present")?,
            })
        });

        match result {
            Ok(sample) => Ok(Some(sample)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    /// Get samples filtered by presence status
    pub fn get_samples_by_presence(
        &self,
        is_present: bool,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
    ) -> Result<(Vec<Sample>, i32), DatabaseError> {
        let sort_column = match sort_by.as_deref() {
            Some("name") => "name",
            Some("path") => "path",
            Some("present") => "is_present",
            _ => "name", // default sort
        };

        let sort_order = if sort_desc.unwrap_or(false) {
            "DESC"
        } else {
            "ASC"
        };

        // Get total count
        let total_count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM samples WHERE is_present = ?",
            params![is_present],
            |row| row.get(0),
        )?;

        // Build query with pagination
        let query = format!(
            "SELECT * FROM samples WHERE is_present = ? ORDER BY {} {} LIMIT ? OFFSET ?",
            sort_column, sort_order
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            params![is_present, limit.unwrap_or(1000), offset.unwrap_or(0)],
            |row| {
                let id_str: String = row.get("id")?;
                Ok(Sample {
                    id: Uuid::parse_str(&id_str).map_err(|_e| rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text))?,
                    name: row.get("name")?,
                    path: PathBuf::from(row.get::<_, String>("path")?),
                    is_present: row.get("is_present")?,
                })
            },
        )?;

        let samples: Result<Vec<Sample>, _> = rows.collect();
        Ok((samples?, total_count))
    }

    /// Search samples by name or path
    pub fn search_samples(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        present_only: Option<bool>,
        extension_filter: Option<String>,
    ) -> Result<(Vec<Sample>, i32), DatabaseError> {
        let mut conditions = vec!["(name LIKE ? OR path LIKE ?)"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(format!("%{}%", query)),
            Box::new(format!("%{}%", query)),
        ];

        if let Some(present) = present_only {
            conditions.push("is_present = ?");
            params.push(Box::new(present));
        }

        if let Some(extension) = extension_filter {
            conditions.push("path LIKE ?");
            params.push(Box::new(format!("%.{}", extension)));
        }

        let where_clause = conditions.join(" AND ");

        // Get total count
        let count_query = format!("SELECT COUNT(*) FROM samples WHERE {}", where_clause);
        let mut count_stmt = self.conn.prepare(&count_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let total_count: i32 = count_stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;

        // Build main query
        let main_query = format!(
            "SELECT * FROM samples WHERE {} ORDER BY name ASC LIMIT ? OFFSET ?",
            where_clause
        );

        params.push(Box::new(limit.unwrap_or(1000)));
        params.push(Box::new(offset.unwrap_or(0)));

        let mut stmt = self.conn.prepare(&main_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let id_str: String = row.get("id")?;
            Ok(Sample {
                id: Uuid::parse_str(&id_str).map_err(|_e| rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text))?,
                name: row.get("name")?,
                path: PathBuf::from(row.get::<_, String>("path")?),
                is_present: row.get("is_present")?,
            })
        })?;

        let samples: Result<Vec<Sample>, _> = rows.collect();
        Ok((samples?, total_count))
    }

    /// Get sample statistics for status bar
    pub fn get_sample_stats(&self) -> Result<SampleStats, DatabaseError> {
        let total_samples: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))?;

        let present_samples: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM samples WHERE is_present = true",
            [],
            |row| row.get(0),
        )?;

        let missing_samples = total_samples - present_samples;

        let unique_paths: i32 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT path) FROM samples", [], |row| {
                    row.get(0)
                })?;

        // Get samples by extension
        let mut samples_by_extension = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN path LIKE '%.wav' THEN 'wav'
                    WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 'aiff'
                    WHEN path LIKE '%.mp3' THEN 'mp3'
                    WHEN path LIKE '%.flac' THEN 'flac'
                    WHEN path LIKE '%.ogg' THEN 'ogg'
                    WHEN path LIKE '%.m4a' THEN 'm4a'
                    ELSE 'other'
                END as extension,
                COUNT(*) as count
            FROM samples 
            GROUP BY extension
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        for row in rows {
            let (extension, count) = row?;
            samples_by_extension.insert(extension, count);
        }

        // Estimate total size (this is a rough estimate based on typical file sizes)
        let total_estimated_size_bytes = self.conn.query_row(
            r#"
            SELECT SUM(
                CASE 
                    WHEN path LIKE '%.wav' THEN 5000000  -- ~5MB avg for WAV
                    WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 5000000  -- ~5MB avg for AIFF
                    WHEN path LIKE '%.mp3' THEN 500000   -- ~500KB avg for MP3
                    WHEN path LIKE '%.flac' THEN 2500000 -- ~2.5MB avg for FLAC
                    WHEN path LIKE '%.ogg' THEN 500000   -- ~500KB avg for OGG
                    WHEN path LIKE '%.m4a' THEN 500000   -- ~500KB avg for M4A
                    ELSE 1000000  -- ~1MB for other formats
                END
            )
            FROM samples WHERE is_present = true
            "#,
            [],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        Ok(SampleStats {
            total_samples,
            present_samples,
            missing_samples,
            unique_paths,
            samples_by_extension,
            total_estimated_size_bytes: total_estimated_size_bytes.unwrap_or(0),
        })
    }

    /// Get sample usage numbers
    pub fn get_all_sample_usage_numbers(&self) -> Result<Vec<SampleUsageInfo>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                s.id,
                s.name,
                s.path,
                COUNT(ps.project_id) as usage_count,
                COUNT(DISTINCT ps.project_id) as project_count
            FROM samples s
            LEFT JOIN project_samples ps ON ps.sample_id = s.id
            GROUP BY s.id, s.name, s.path
            ORDER BY usage_count DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SampleUsageInfo {
                sample_id: row.get("id")?,
                name: row.get("name")?,
                path: row.get("path")?,
                usage_count: row.get("usage_count")?,
                project_count: row.get("project_count")?,
            })
        })?;

        let usage_info: Result<Vec<SampleUsageInfo>, _> = rows.collect();
        Ok(usage_info?)
    }

    /// Refresh sample presence status by checking if files still exist
    pub fn refresh_sample_presence_status(&mut self) -> Result<SampleRefreshResult, DatabaseError> {
        let mut total_checked = 0;
        let mut now_present = 0;
        let mut now_missing = 0;
        let mut unchanged = 0;

        // Get all samples from our database
        let mut stmt = self.conn.prepare("SELECT id, name, path, is_present FROM samples")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("name")?,
                row.get::<_, String>("path")?,
                row.get::<_, bool>("is_present")?,
            ))
        })?;

        for row_result in rows {
            let (sample_id, _name, path_str, current_present) = row_result?;
            total_checked += 1;

            // Check if file exists
            let path = PathBuf::from(path_str);
            let is_present = path.exists();

            if current_present != is_present {
                // Status changed, update it
                self.conn.execute(
                    "UPDATE samples SET is_present = ? WHERE id = ?",
                    params![is_present, sample_id]
                )?;

                if is_present {
                    now_present += 1;
                } else {
                    now_missing += 1;
                }
            } else {
                unchanged += 1;
            }
        }

        Ok(SampleRefreshResult {
            total_samples_checked: total_checked,
            samples_now_present: now_present,
            samples_now_missing: now_missing,
            samples_unchanged: unchanged,
        })
    }

    /// Get comprehensive sample analytics
    pub fn get_sample_analytics(&self) -> Result<SampleAnalytics, DatabaseError> {
        // Get usage distribution
        let usage_distribution = self.get_usage_distribution()?;
        
        // Get extension analytics
        let extensions = self.get_extension_analytics()?;
        
        // Get missing vs present percentages
        let (missing_percentage, present_percentage) = self.get_presence_percentages()?;
        
        // Get storage usage
        let (total_storage, present_storage, missing_storage) = self.get_storage_usage()?;
        
        // Get top used samples
        let top_used_samples = self.get_top_used_samples(10)?;
        
        // Get recently added samples (last 30 days)
        let recently_added = self.get_recently_added_samples()?;

        Ok(SampleAnalytics {
            most_used_samples_count: usage_distribution.most_used,
            moderately_used_samples_count: usage_distribution.moderately_used,
            rarely_used_samples_count: usage_distribution.rarely_used,
            unused_samples_count: usage_distribution.unused,
            extensions,
            missing_samples_percentage: missing_percentage,
            present_samples_percentage: present_percentage,
            total_storage_bytes: total_storage,
            present_storage_bytes: present_storage,
            missing_storage_bytes: missing_storage,
            top_used_samples,
            recently_added_samples: recently_added,
        })
    }

    /// Get usage distribution statistics
    fn get_usage_distribution(&self) -> Result<UsageDistribution, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN usage_count >= 5 THEN 'most_used'
                    WHEN usage_count >= 2 THEN 'moderately_used'
                    WHEN usage_count = 1 THEN 'rarely_used'
                    ELSE 'unused'
                END as usage_category,
                COUNT(*) as count
            FROM (
                SELECT s.id, COALESCE(usage_stats.usage_count, 0) as usage_count
                FROM samples s
                LEFT JOIN (
                    SELECT sample_id, COUNT(*) as usage_count
                    FROM project_samples
                    GROUP BY sample_id
                ) usage_stats ON s.id = usage_stats.sample_id
            )
            GROUP BY usage_category
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut distribution = UsageDistribution {
            most_used: 0,
            moderately_used: 0,
            rarely_used: 0,
            unused: 0,
        };

        for row in rows {
            let (category, count) = row?;
            match category.as_str() {
                "most_used" => distribution.most_used = count,
                "moderately_used" => distribution.moderately_used = count,
                "rarely_used" => distribution.rarely_used = count,
                "unused" => distribution.unused = count,
                _ => {}
            }
        }

        Ok(distribution)
    }

    /// Get extension analytics with detailed statistics
    fn get_extension_analytics(&self) -> Result<std::collections::HashMap<String, ExtensionAnalytics>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN path LIKE '%.wav' THEN 'wav'
                    WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 'aiff'
                    WHEN path LIKE '%.mp3' THEN 'mp3'
                    WHEN path LIKE '%.flac' THEN 'flac'
                    WHEN path LIKE '%.ogg' THEN 'ogg'
                    WHEN path LIKE '%.m4a' THEN 'm4a'
                    ELSE 'other'
                END as extension,
                COUNT(*) as count,
                SUM(CASE WHEN is_present THEN 1 ELSE 0 END) as present_count,
                SUM(CASE WHEN NOT is_present THEN 1 ELSE 0 END) as missing_count,
                AVG(COALESCE(usage_count, 0)) as avg_usage_count,
                SUM(
                    CASE 
                        WHEN path LIKE '%.wav' THEN 5000000  -- ~5MB avg for WAV
                        WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 5000000  -- ~5MB avg for AIFF
                        WHEN path LIKE '%.mp3' THEN 500000   -- ~500KB avg for MP3
                        WHEN path LIKE '%.flac' THEN 2500000 -- ~2.5MB avg for FLAC
                        WHEN path LIKE '%.ogg' THEN 500000   -- ~500KB avg for OGG
                        WHEN path LIKE '%.m4a' THEN 500000   -- ~500KB avg for M4A
                        ELSE 1000000  -- ~1MB for other formats
                    END
                ) as total_size_bytes
            FROM (
                SELECT s.*, COALESCE(usage_stats.usage_count, 0) as usage_count
                FROM samples s
                LEFT JOIN (
                    SELECT sample_id, COUNT(*) as usage_count
                    FROM project_samples
                    GROUP BY sample_id
                ) usage_stats ON s.id = usage_stats.sample_id
            )
            GROUP BY extension
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;

        let mut extensions = std::collections::HashMap::new();
        for row in rows {
            let (extension, count, present_count, missing_count, avg_usage_count, total_size_bytes) = row?;
            extensions.insert(extension, ExtensionAnalytics {
                count,
                total_size_bytes,
                present_count,
                missing_count,
                average_usage_count: avg_usage_count,
            });
        }

        Ok(extensions)
    }

    /// Get missing vs present percentages
    fn get_presence_percentages(&self) -> Result<(i32, i32), DatabaseError> {
        let total_samples: i32 = self.conn.query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))?;
        let present_samples: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM samples WHERE is_present = true",
            [],
            |row| row.get(0),
        )?;

        let missing_samples = total_samples - present_samples;
        let missing_percentage = if total_samples > 0 {
            ((missing_samples as f64 / total_samples as f64) * 100.0) as i32
        } else {
            0
        };
        let present_percentage = if total_samples > 0 {
            ((present_samples as f64 / total_samples as f64) * 100.0) as i32
        } else {
            0
        };

        Ok((missing_percentage, present_percentage))
    }

    /// Get storage usage statistics
    fn get_storage_usage(&self) -> Result<(i64, i64, i64), DatabaseError> {
        let (total_storage, present_storage) = self.conn.query_row(
            r#"
            SELECT 
                SUM(
                    CASE 
                        WHEN path LIKE '%.wav' THEN 5000000  -- ~5MB avg for WAV
                        WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 5000000  -- ~5MB avg for AIFF
                        WHEN path LIKE '%.mp3' THEN 500000   -- ~500KB avg for MP3
                        WHEN path LIKE '%.flac' THEN 2500000 -- ~2.5MB avg for FLAC
                        WHEN path LIKE '%.ogg' THEN 500000   -- ~500KB avg for OGG
                        WHEN path LIKE '%.m4a' THEN 500000   -- ~500KB avg for M4A
                        ELSE 1000000  -- ~1MB for other formats
                    END
                ) as total_storage,
                SUM(
                    CASE 
                        WHEN is_present THEN
                            CASE 
                                WHEN path LIKE '%.wav' THEN 5000000
                                WHEN path LIKE '%.aif' OR path LIKE '%.aiff' THEN 5000000
                                WHEN path LIKE '%.mp3' THEN 500000
                                WHEN path LIKE '%.flac' THEN 2500000
                                WHEN path LIKE '%.ogg' THEN 500000
                                WHEN path LIKE '%.m4a' THEN 500000
                                ELSE 1000000
                            END
                        ELSE 0
                    END
                ) as present_storage
            FROM samples
            "#,
            [],
            |row| Ok((row.get::<_, Option<i64>>(0)?.unwrap_or(0), row.get::<_, Option<i64>>(1)?.unwrap_or(0))),
        )?;

        let missing_storage = total_storage - present_storage;
        Ok((total_storage, present_storage, missing_storage))
    }

    /// Get top used samples
    fn get_top_used_samples(&self, limit: i32) -> Result<Vec<SampleUsageInfo>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                s.id,
                s.name,
                s.path,
                COUNT(ps.project_id) as usage_count,
                COUNT(DISTINCT ps.project_id) as project_count
            FROM samples s
            LEFT JOIN project_samples ps ON ps.sample_id = s.id
            GROUP BY s.id, s.name, s.path
            ORDER BY usage_count DESC
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok(SampleUsageInfo {
                sample_id: row.get("id")?,
                name: row.get("name")?,
                path: row.get("path")?,
                usage_count: row.get("usage_count")?,
                project_count: row.get("project_count")?,
            })
        })?;

        let usage_info: Result<Vec<SampleUsageInfo>, _> = rows.collect();
        Ok(usage_info?)
    }

    /// Get recently added samples (last 30 days)
    fn get_recently_added_samples(&self) -> Result<i32, DatabaseError> {
        // Since we don't have a created_at field in samples table, we'll estimate
        // based on the assumption that samples are added when projects are scanned
        // For now, we'll return 0 as a placeholder
        // TODO: Add created_at field to samples table in future migration
        Ok(0)
    }
}

pub struct SampleStats {
    pub total_samples: i32,
    pub present_samples: i32,
    pub missing_samples: i32,
    pub unique_paths: i32,
    pub samples_by_extension: std::collections::HashMap<String, i32>,
    pub total_estimated_size_bytes: i64,
}

pub struct SampleUsageInfo {
    pub sample_id: String,
    pub name: String,
    pub path: String,
    pub usage_count: i32,
    pub project_count: i32,
}

pub struct SampleRefreshResult {
    pub total_samples_checked: i32,
    pub samples_now_present: i32,
    pub samples_now_missing: i32,
    pub samples_unchanged: i32,
}

pub struct SampleAnalytics {
    pub most_used_samples_count: i32,
    pub moderately_used_samples_count: i32,
    pub rarely_used_samples_count: i32,
    pub unused_samples_count: i32,
    pub extensions: std::collections::HashMap<String, ExtensionAnalytics>,
    pub missing_samples_percentage: i32,
    pub present_samples_percentage: i32,
    pub total_storage_bytes: i64,
    pub present_storage_bytes: i64,
    pub missing_storage_bytes: i64,
    pub top_used_samples: Vec<SampleUsageInfo>,
    pub recently_added_samples: i32,
}

pub struct UsageDistribution {
    pub most_used: i32,
    pub moderately_used: i32,
    pub rarely_used: i32,
    pub unused: i32,
}

pub struct ExtensionAnalytics {
    pub count: i32,
    pub total_size_bytes: i64,
    pub present_count: i32,
    pub missing_count: i32,
    pub average_usage_count: f64,
}
