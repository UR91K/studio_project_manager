use crate::error::DatabaseError;
use rusqlite::OptionalExtension;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    // Statistics methods
    pub fn get_basic_counts(&self) -> Result<(i32, i32, i32, i32, i32, i32), DatabaseError> {
        let total_projects: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE is_active = true",
            [],
            |row| row.get(0),
        )?;
        let total_plugins: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))?;
        let total_samples: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))?;
        let total_collections: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM collections", [], |row| row.get(0))?;
        let total_tags: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_tasks: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM project_tasks", [], |row| row.get(0))?;

        Ok((
            total_projects,
            total_plugins,
            total_samples,
            total_collections,
            total_tags,
            total_tasks,
        ))
    }

    pub fn get_top_plugins(&self, limit: i32) -> Result<Vec<(String, String, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT p.name, COALESCE(p.vendor, 'Unknown'), COUNT(*) as usage_count
             FROM plugins p
             JOIN project_plugins pp ON p.id = pp.plugin_id
             JOIN projects proj ON pp.project_id = proj.id
             WHERE proj.is_active = true
             GROUP BY p.id
             ORDER BY usage_count DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;

        let mut plugins = Vec::new();
        for row in rows {
            plugins.push(row?);
        }
        Ok(plugins)
    }

    pub fn get_top_vendors(&self, limit: i32) -> Result<Vec<(String, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                COALESCE(p.vendor, 'Unknown') as vendor,
                COUNT(DISTINCT p.id) as plugin_count,
                COUNT(*) as usage_count
             FROM plugins p
             JOIN project_plugins pp ON p.id = pp.plugin_id
             JOIN projects proj ON pp.project_id = proj.id
             WHERE proj.is_active = true
             GROUP BY vendor
             ORDER BY usage_count DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;

        let mut vendors = Vec::new();
        for row in rows {
            vendors.push(row?);
        }
        Ok(vendors)
    }

    pub fn get_tempo_distribution(&self) -> Result<Vec<(f64, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CASE 
                    WHEN tempo < 90 THEN 80
                    WHEN tempo < 100 THEN 90
                    WHEN tempo < 110 THEN 100
                    WHEN tempo < 120 THEN 110
                    WHEN tempo < 130 THEN 120
                    WHEN tempo < 140 THEN 130
                    WHEN tempo < 150 THEN 140
                    WHEN tempo < 160 THEN 150
                    WHEN tempo < 170 THEN 160
                    WHEN tempo < 180 THEN 170
                    ELSE 180
                END as tempo_range,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true AND tempo > 0
             GROUP BY tempo_range
             ORDER BY tempo_range",
        )?;

        let rows = stmt.query_map([], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, i32>(1)?)))?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    pub fn get_key_distribution(&self) -> Result<Vec<(String, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CASE 
                    WHEN key_signature_tonic IS NOT NULL AND key_signature_scale IS NOT NULL 
                    THEN key_signature_tonic || ' ' || key_signature_scale
                    ELSE 'Unknown'
                END as key_sig,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true
             GROUP BY key_sig
             ORDER BY count DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    pub fn get_time_signature_distribution(&self) -> Result<Vec<(i32, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT time_signature_numerator, time_signature_denominator, COUNT(*) as count
             FROM projects
             WHERE is_active = true
             GROUP BY time_signature_numerator, time_signature_denominator
             ORDER BY count DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    pub fn get_projects_per_year(&self) -> Result<Vec<(i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CAST(strftime('%Y', datetime(created_at, 'unixepoch')) AS INTEGER) as year,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true AND created_at IS NOT NULL
             GROUP BY year
             ORDER BY year DESC",
        )?;

        let rows = stmt.query_map([], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }

    pub fn get_projects_per_month(
        &self,
        limit: i32,
    ) -> Result<Vec<(i32, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CAST(strftime('%Y', datetime(created_at, 'unixepoch')) AS INTEGER) as year,
                CAST(strftime('%m', datetime(created_at, 'unixepoch')) AS INTEGER) as month,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true AND created_at IS NOT NULL
             GROUP BY year, month
             ORDER BY year DESC, month DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }

    pub fn get_duration_analytics(&self) -> Result<(f64, i32, Option<String>), DatabaseError> {
        let avg_duration: f64 = self.conn.query_row(
            "SELECT AVG(CAST(duration_seconds AS REAL)) FROM projects WHERE is_active = true AND duration_seconds IS NOT NULL",
            [],
            |row| row.get(0)
        ).unwrap_or(0.0);

        let short_projects: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE is_active = true AND duration_seconds IS NOT NULL AND duration_seconds < 40",
            [],
            |row| row.get(0)
        )?;

        let longest_project: Option<String> = self.conn.query_row(
            "SELECT id FROM projects WHERE is_active = true AND duration_seconds IS NOT NULL ORDER BY duration_seconds DESC LIMIT 1",
            [],
            |row| row.get(0)
        ).optional()?;

        Ok((avg_duration, short_projects, longest_project))
    }

    pub fn get_complexity_metrics(&self) -> Result<(f64, f64), DatabaseError> {
        let avg_plugins: f64 = self
            .conn
            .query_row(
                "SELECT AVG(plugin_count) FROM (
                SELECT COUNT(*) as plugin_count
                FROM project_plugins pp
                JOIN projects p ON pp.project_id = p.id
                WHERE p.is_active = true
                GROUP BY pp.project_id
            )",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let avg_samples: f64 = self
            .conn
            .query_row(
                "SELECT AVG(sample_count) FROM (
                SELECT COUNT(*) as sample_count
                FROM project_samples ps
                JOIN projects p ON ps.project_id = p.id
                WHERE p.is_active = true
                GROUP BY ps.project_id
            )",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        Ok((avg_plugins, avg_samples))
    }

    pub fn get_most_complex_projects(
        &self,
        limit: i32,
    ) -> Result<Vec<(String, i32, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                p.id,
                COALESCE(plugin_counts.plugin_count, 0) as plugin_count,
                COALESCE(sample_counts.sample_count, 0) as sample_count,
                (COALESCE(plugin_counts.plugin_count, 0) + COALESCE(sample_counts.sample_count, 0)) as complexity_score
             FROM projects p
             LEFT JOIN (
                 SELECT project_id, COUNT(*) as plugin_count
                 FROM project_plugins
                 GROUP BY project_id
             ) plugin_counts ON p.id = plugin_counts.project_id
             LEFT JOIN (
                 SELECT project_id, COUNT(*) as sample_count
                 FROM project_samples
                 GROUP BY project_id
             ) sample_counts ON p.id = sample_counts.project_id
             WHERE p.is_active = true
             ORDER BY complexity_score DESC
             LIMIT ?"
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
            ))
        })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn get_top_samples(&self, limit: i32) -> Result<Vec<(String, String, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT s.name, s.path, COUNT(*) as usage_count
             FROM samples s
             JOIN project_samples ps ON s.id = ps.sample_id
             JOIN projects p ON ps.project_id = p.id
             WHERE p.is_active = true
             GROUP BY s.id
             ORDER BY usage_count DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;

        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?);
        }
        Ok(samples)
    }

    pub fn get_top_tags(&self, limit: i32) -> Result<Vec<(String, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT t.name, COUNT(*) as usage_count
             FROM tags t
             JOIN project_tags pt ON t.id = pt.tag_id
             JOIN projects p ON pt.project_id = p.id
             WHERE p.is_active = true
             GROUP BY t.id
             ORDER BY usage_count DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map([limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }
        Ok(tags)
    }

    pub fn get_task_statistics(&self) -> Result<(i32, i32, f64), DatabaseError> {
        let completed_tasks: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_tasks WHERE completed = true",
            [],
            |row| row.get(0),
        )?;

        let pending_tasks: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_tasks WHERE completed = false",
            [],
            |row| row.get(0),
        )?;

        let total_tasks = completed_tasks + pending_tasks;
        let completion_rate = if total_tasks > 0 {
            (completed_tasks as f64 / total_tasks as f64) * 100.0
        } else {
            0.0
        };

        Ok((completed_tasks, pending_tasks, completion_rate))
    }

    pub fn get_recent_activity(
        &self,
        days: i32,
    ) -> Result<Vec<(i32, i32, i32, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CAST(strftime('%Y', date) AS INTEGER) as year,
                CAST(strftime('%m', date) AS INTEGER) as month,
                CAST(strftime('%d', date) AS INTEGER) as day,
                SUM(projects_created) as projects_created,
                SUM(projects_modified) as projects_modified
             FROM (
                 SELECT 
                     DATE(datetime(created_at, 'unixepoch')) as date,
                     COUNT(*) as projects_created,
                     0 as projects_modified
                 FROM projects
                 WHERE is_active = true AND created_at IS NOT NULL AND datetime(created_at, 'unixepoch') >= DATE('now', '-' || ? || ' days')
                 GROUP BY DATE(datetime(created_at, 'unixepoch'))
                 UNION ALL
                 SELECT 
                     DATE(datetime(modified_at, 'unixepoch')) as date,
                     0 as projects_created,
                     COUNT(*) as projects_modified
                 FROM projects
                 WHERE is_active = true AND modified_at IS NOT NULL AND datetime(modified_at, 'unixepoch') >= DATE('now', '-' || ? || ' days')
                 GROUP BY DATE(datetime(modified_at, 'unixepoch'))
             )
             WHERE date IS NOT NULL
             GROUP BY date
             ORDER BY date DESC"
        )?;

        let rows = stmt.query_map([days, days], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })?;

        let mut activity = Vec::new();
        for row in rows {
            activity.push(row?);
        }
        Ok(activity)
    }

    pub fn get_ableton_version_stats(&self) -> Result<Vec<(String, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CAST(ableton_version_major AS TEXT) || '.' || 
                CAST(ableton_version_minor AS TEXT) || '.' || 
                CAST(ableton_version_patch AS TEXT) ||
                CASE WHEN ableton_version_beta = true THEN ' beta' ELSE '' END as version,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true
             GROUP BY version
             ORDER BY count DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        let mut versions = Vec::new();
        for row in rows {
            versions.push(row?);
        }
        Ok(versions)
    }

    pub fn get_collection_analytics(&self) -> Result<(f64, Option<String>), DatabaseError> {
        // Check if there are any collections first
        let collection_count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM collections",
            [],
            |row| row.get(0),
        )?;

        if collection_count == 0 {
            // No collections, return default values
            return Ok((0.0, None));
        }

        // Check if there are any collection_projects entries
        let project_count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM collection_projects",
            [],
            |row| row.get(0),
        )?;

        if project_count == 0 {
            // No projects in collections, return default values
            return Ok((0.0, None));
        }

        let average_projects_per_collection: Option<f64> = self.conn.query_row(
            "SELECT AVG(project_count) FROM (
                SELECT COUNT(*) as project_count 
                FROM collection_projects 
                GROUP BY collection_id
            )",
            [],
            |row| row.get(0),
        ).optional()?;

        let largest_collection_id: Option<String> = self.conn.query_row(
            "SELECT collection_id FROM (
                SELECT collection_id, COUNT(*) as project_count 
                FROM collection_projects 
                GROUP BY collection_id 
                ORDER BY project_count DESC 
                LIMIT 1
            )",
            [],
            |row| row.get(0),
        ).optional()?;

        Ok((average_projects_per_collection.unwrap_or(0.0), largest_collection_id))
    }

    // Project-specific statistics methods
    pub fn get_project_statistics(
        &self,
        min_tempo: Option<f64>,
        max_tempo: Option<f64>,
        key_signature_tonic: Option<String>,
        key_signature_scale: Option<String>,
        time_signature_numerator: Option<i32>,
        time_signature_denominator: Option<i32>,
        ableton_version_major: Option<i32>,
        ableton_version_minor: Option<i32>,
        ableton_version_patch: Option<i32>,
        created_after: Option<i64>,
        created_before: Option<i64>,
        has_audio_file: Option<bool>,
    ) -> Result<ProjectStatistics, DatabaseError> {
        // Build WHERE conditions for filtering
        let mut conditions = vec!["is_active = true"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(min_tempo_val) = min_tempo {
            conditions.push("tempo >= ?");
            params.push(Box::new(min_tempo_val));
        }

        if let Some(max_tempo_val) = max_tempo {
            conditions.push("tempo <= ?");
            params.push(Box::new(max_tempo_val));
        }

        if let Some(ref tonic) = key_signature_tonic {
            conditions.push("key_signature_tonic = ?");
            params.push(Box::new(tonic.clone()));
        }

        if let Some(ref scale) = key_signature_scale {
            conditions.push("key_signature_scale = ?");
            params.push(Box::new(scale.clone()));
        }

        if let Some(numerator) = time_signature_numerator {
            conditions.push("time_signature_numerator = ?");
            params.push(Box::new(numerator));
        }

        if let Some(denominator) = time_signature_denominator {
            conditions.push("time_signature_denominator = ?");
            params.push(Box::new(denominator));
        }

        if let Some(major) = ableton_version_major {
            conditions.push("ableton_version_major = ?");
            params.push(Box::new(major));
        }

        if let Some(minor) = ableton_version_minor {
            conditions.push("ableton_version_minor = ?");
            params.push(Box::new(minor));
        }

        if let Some(patch) = ableton_version_patch {
            conditions.push("ableton_version_patch = ?");
            params.push(Box::new(patch));
        }

        if let Some(after) = created_after {
            conditions.push("created_at >= ?");
            params.push(Box::new(after));
        }

        if let Some(before) = created_before {
            conditions.push("created_at <= ?");
            params.push(Box::new(before));
        }

        if let Some(has_audio) = has_audio_file {
            if has_audio {
                conditions.push("audio_file_id IS NOT NULL");
            } else {
                conditions.push("audio_file_id IS NULL");
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Basic counts
        let total_projects: i32 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM projects {}", where_clause),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let projects_with_audio_files: i32 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM projects {} AND audio_file_id IS NOT NULL", where_clause),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let projects_without_audio_files = total_projects - projects_with_audio_files;

        // Musical statistics
        let (average_tempo, min_tempo, max_tempo): (Option<f64>, Option<f64>, Option<f64>) = self.conn.query_row(
            &format!("SELECT AVG(tempo), MIN(tempo), MAX(tempo) FROM projects {}", where_clause),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Duration statistics
        let (average_duration, min_duration, max_duration): (Option<f64>, Option<f64>, Option<f64>) = self.conn.query_row(
            &format!("SELECT AVG(duration_seconds), MIN(duration_seconds), MAX(duration_seconds) FROM projects {}", where_clause),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Complexity statistics
        let average_plugins_per_project: Option<f64> = self.conn.query_row(
            &format!(
                "SELECT AVG(plugin_count) FROM (
                    SELECT COUNT(*) as plugin_count 
                    FROM project_plugins pp 
                    JOIN projects p ON pp.project_id = p.id 
                    {} 
                    GROUP BY p.id
                )",
                where_clause
            ),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let average_samples_per_project: Option<f64> = self.conn.query_row(
            &format!(
                "SELECT AVG(sample_count) FROM (
                    SELECT COUNT(*) as sample_count 
                    FROM project_samples ps 
                    JOIN projects p ON ps.project_id = p.id 
                    {} 
                    GROUP BY p.id
                )",
                where_clause
            ),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let average_tags_per_project: Option<f64> = self.conn.query_row(
            &format!(
                "SELECT AVG(tag_count) FROM (
                    SELECT COUNT(*) as tag_count 
                    FROM project_tags pt 
                    JOIN projects p ON pt.project_id = p.id 
                    {} 
                    GROUP BY p.id
                )",
                where_clause
            ),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        // Get distributions
        let tempo_distribution = self.get_project_tempo_distribution(&where_clause, &params)?;
        let key_signature_distribution = self.get_project_key_signature_distribution(&where_clause, &params)?;
        let time_signature_distribution = self.get_project_time_signature_distribution(&where_clause, &params)?;
        let ableton_version_distribution = self.get_project_ableton_version_distribution(&where_clause, &params)?;
        let projects_per_year = self.get_project_year_distribution(&where_clause, &params)?;
        let projects_per_month = self.get_project_month_distribution(&where_clause, &params)?;
        let most_complex_projects = self.get_project_complexity_statistics(&where_clause, &params)?;

        Ok(ProjectStatistics {
            total_projects,
            projects_with_audio_files,
            projects_without_audio_files,
            average_tempo: average_tempo.unwrap_or(0.0),
            min_tempo: min_tempo.unwrap_or(0.0),
            max_tempo: max_tempo.unwrap_or(0.0),
            tempo_distribution,
            key_signature_distribution,
            time_signature_distribution,
            ableton_version_distribution,
            average_duration_seconds: average_duration.unwrap_or(0.0),
            min_duration_seconds: min_duration.unwrap_or(0.0),
            max_duration_seconds: max_duration.unwrap_or(0.0),
            average_plugins_per_project: average_plugins_per_project.unwrap_or(0.0),
            average_samples_per_project: average_samples_per_project.unwrap_or(0.0),
            average_tags_per_project: average_tags_per_project.unwrap_or(0.0),
            projects_per_year,
            projects_per_month,
            most_complex_projects,
        })
    }

    fn get_project_tempo_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(String, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                CASE 
                    WHEN tempo < 80 THEN '60-80 BPM'
                    WHEN tempo < 100 THEN '80-100 BPM'
                    WHEN tempo < 120 THEN '100-120 BPM'
                    WHEN tempo < 140 THEN '120-140 BPM'
                    WHEN tempo < 160 THEN '140-160 BPM'
                    WHEN tempo < 180 THEN '160-180 BPM'
                    ELSE '180+ BPM'
                END as tempo_range,
                COUNT(*) as count
             FROM projects {}
             GROUP BY tempo_range
             ORDER BY MIN(tempo)",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_key_signature_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(String, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                CASE 
                    WHEN key_signature_tonic IS NOT NULL AND key_signature_scale IS NOT NULL 
                    THEN key_signature_tonic || ' ' || key_signature_scale
                    ELSE 'Unknown'
                END as key_signature,
                COUNT(*) as count
             FROM projects {}
             GROUP BY key_signature
             ORDER BY count DESC",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_time_signature_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(i32, i32, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                time_signature_numerator,
                time_signature_denominator,
                COUNT(*) as count
             FROM projects {}
             GROUP BY time_signature_numerator, time_signature_denominator
             ORDER BY count DESC",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)),
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_ableton_version_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(String, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                CAST(ableton_version_major AS TEXT) || '.' || 
                CAST(ableton_version_minor AS TEXT) || '.' || 
                CAST(ableton_version_patch AS TEXT) as version,
                COUNT(*) as count
             FROM projects {}
             GROUP BY version
             ORDER BY ableton_version_major DESC, ableton_version_minor DESC, ableton_version_patch DESC",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_year_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(i32, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                strftime('%Y', datetime(created_at, 'unixepoch')) as year,
                COUNT(*) as count
             FROM projects {}
             GROUP BY year
             ORDER BY year DESC",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
                let year_str: String = row.get(0)?;
                let year: i32 = year_str.parse().unwrap_or(0);
                Ok((year, row.get::<_, i32>(1)?))
            },
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_month_distribution(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(i32, i32, i32)>, DatabaseError> {
        let query = format!(
            "SELECT 
                strftime('%Y', datetime(created_at, 'unixepoch')) as year,
                strftime('%m', datetime(created_at, 'unixepoch')) as month,
                COUNT(*) as count
             FROM projects {}
             GROUP BY year, month
             ORDER BY year DESC, month DESC
             LIMIT 24",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| {
                let year_str: String = row.get(0)?;
                let month_str: String = row.get(1)?;
                let year: i32 = year_str.parse().unwrap_or(0);
                let month: i32 = month_str.parse().unwrap_or(0);
                Ok((year, month, row.get::<_, i32>(2)?))
            },
        )?;

        let mut distribution = Vec::new();
        for row in rows {
            distribution.push(row?);
        }
        Ok(distribution)
    }

    fn get_project_complexity_statistics(
        &self,
        where_clause: &str,
        params: &[Box<dyn rusqlite::ToSql>],
    ) -> Result<Vec<(String, String, i32, i32, i32, f64)>, DatabaseError> {
        let query = format!(
            "SELECT 
                p.id,
                p.name,
                COALESCE(plugin_count, 0) as plugin_count,
                COALESCE(sample_count, 0) as sample_count,
                COALESCE(tag_count, 0) as tag_count,
                (COALESCE(plugin_count, 0) + COALESCE(sample_count, 0) + COALESCE(tag_count, 0)) as complexity_score
             FROM projects p
             LEFT JOIN (
                SELECT project_id, COUNT(*) as plugin_count
                FROM project_plugins
                GROUP BY project_id
             ) pp ON pp.project_id = p.id
             LEFT JOIN (
                SELECT project_id, COUNT(*) as sample_count
                FROM project_samples
                GROUP BY project_id
             ) ps ON ps.project_id = p.id
             LEFT JOIN (
                SELECT project_id, COUNT(*) as tag_count
                FROM project_tags
                GROUP BY project_id
             ) pt ON pt.project_id = p.id
             {}
             ORDER BY complexity_score DESC
             LIMIT 10",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, f64>(5)?,
            )),
        )?;

        let mut statistics = Vec::new();
        for row in rows {
            statistics.push(row?);
        }
        Ok(statistics)
    }
}

#[derive(Debug)]
pub struct ProjectStatistics {
    pub total_projects: i32,
    pub projects_with_audio_files: i32,
    pub projects_without_audio_files: i32,
    pub average_tempo: f64,
    pub min_tempo: f64,
    pub max_tempo: f64,
    pub tempo_distribution: Vec<(String, i32)>,
    pub key_signature_distribution: Vec<(String, i32)>,
    pub time_signature_distribution: Vec<(i32, i32, i32)>,
    pub ableton_version_distribution: Vec<(String, i32)>,
    pub average_duration_seconds: f64,
    pub min_duration_seconds: f64,
    pub max_duration_seconds: f64,
    pub average_plugins_per_project: f64,
    pub average_samples_per_project: f64,
    pub average_tags_per_project: f64,
    pub projects_per_year: Vec<(i32, i32)>,
    pub projects_per_month: Vec<(i32, i32, i32)>,
    pub most_complex_projects: Vec<(String, String, i32, i32, i32, f64)>,
}
