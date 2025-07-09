use crate::error::DatabaseError;
use rusqlite::OptionalExtension;

use super::LiveSetDatabase;

impl LiveSetDatabase {

    // Statistics methods
    pub fn get_basic_counts(&self) -> Result<(i32, i32, i32, i32, i32, i32), DatabaseError> {
        let total_projects: i32 = self.conn.query_row("SELECT COUNT(*) FROM projects WHERE is_active = true", [], |row| row.get(0))?;
        let total_plugins: i32 = self.conn.query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))?;
        let total_samples: i32 = self.conn.query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))?;
        let total_collections: i32 = self.conn.query_row("SELECT COUNT(*) FROM collections", [], |row| row.get(0))?;
        let total_tags: i32 = self.conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_tasks: i32 = self.conn.query_row("SELECT COUNT(*) FROM project_tasks", [], |row| row.get(0))?;
        
        Ok((total_projects, total_plugins, total_samples, total_collections, total_tags, total_tasks))
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
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?
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
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?
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
             ORDER BY tempo_range"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, f64>(0)?,
                row.get::<_, i32>(1)?
            ))
        })?;
        
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
             ORDER BY count DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?
            ))
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
             ORDER BY count DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?
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
             ORDER BY year DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?
            ))
        })?;
        
        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }
    
    pub fn get_projects_per_month(&self, limit: i32) -> Result<Vec<(i32, i32, i32)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                CAST(strftime('%Y', datetime(created_at, 'unixepoch')) AS INTEGER) as year,
                CAST(strftime('%m', datetime(created_at, 'unixepoch')) AS INTEGER) as month,
                COUNT(*) as count
             FROM projects
             WHERE is_active = true AND created_at IS NOT NULL
             GROUP BY year, month
             ORDER BY year DESC, month DESC
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?
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
        let avg_plugins: f64 = self.conn.query_row(
            "SELECT AVG(plugin_count) FROM (
                SELECT COUNT(*) as plugin_count
                FROM project_plugins pp
                JOIN projects p ON pp.project_id = p.id
                WHERE p.is_active = true
                GROUP BY pp.project_id
            )",
            [],
            |row| row.get(0)
        ).unwrap_or(0.0);
        
        let avg_samples: f64 = self.conn.query_row(
            "SELECT AVG(sample_count) FROM (
                SELECT COUNT(*) as sample_count
                FROM project_samples ps
                JOIN projects p ON ps.project_id = p.id
                WHERE p.is_active = true
                GROUP BY ps.project_id
            )",
            [],
            |row| row.get(0)
        ).unwrap_or(0.0);
        
        Ok((avg_plugins, avg_samples))
    }
    
    pub fn get_most_complex_projects(&self, limit: i32) -> Result<Vec<(String, i32, i32, i32)>, DatabaseError> {
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
                row.get::<_, i32>(3)?
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
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?
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
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?
            ))
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
            |row| row.get(0)
        )?;
        
        let pending_tasks: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM project_tasks WHERE completed = false",
            [],
            |row| row.get(0)
        )?;
        
        let total_tasks = completed_tasks + pending_tasks;
        let completion_rate = if total_tasks > 0 {
            (completed_tasks as f64 / total_tasks as f64) * 100.0
        } else {
            0.0
        };
        
        Ok((completed_tasks, pending_tasks, completion_rate))
    }
    
    pub fn get_recent_activity(&self, days: i32) -> Result<Vec<(i32, i32, i32, i32, i32)>, DatabaseError> {
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
                row.get::<_, i32>(4)?
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
             ORDER BY count DESC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?
            ))
        })?;
        
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row?);
        }
        Ok(versions)
    }
    
    pub fn get_collection_analytics(&self) -> Result<(f64, Option<String>), DatabaseError> {
        let avg_projects_per_collection: f64 = self.conn.query_row(
            "SELECT AVG(project_count) FROM (
                SELECT COUNT(*) as project_count
                FROM collection_projects cp
                JOIN collections c ON cp.collection_id = c.id
                JOIN projects p ON cp.project_id = p.id
                WHERE p.is_active = true
                GROUP BY cp.collection_id
            )",
            [],
            |row| row.get(0)
        ).unwrap_or(0.0);
        
        let largest_collection: Option<String> = self.conn.query_row(
            "SELECT c.id
             FROM collections c
             JOIN collection_projects cp ON c.id = cp.collection_id
             JOIN projects p ON cp.project_id = p.id
             WHERE p.is_active = true
             GROUP BY c.id
             ORDER BY COUNT(*) DESC
             LIMIT 1",
            [],
            |row| row.get(0)
        ).optional()?;
        
        Ok((avg_projects_per_collection, largest_collection))
    }
}
