use crate::error::DatabaseError;
use crate::models::Plugin;
use rusqlite::params;
use uuid::Uuid;

use super::LiveSetDatabase;

impl LiveSetDatabase {
    /// Get all plugins with pagination and sorting
    pub fn get_all_plugins(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
    ) -> Result<(Vec<Plugin>, i32), DatabaseError> {
        let sort_column = match sort_by.as_deref() {
            Some("name") => "name",
            Some("vendor") => "vendor",
            Some("installed") => "installed",
            Some("format") => "format",
            _ => "name", // default sort
        };

        let sort_order = if sort_desc.unwrap_or(false) {
            "DESC"
        } else {
            "ASC"
        };

        // Get total count
        let total_count: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))?;

        // Build query with pagination
        let query = format!(
            "SELECT * FROM plugins ORDER BY {} {} LIMIT ? OFFSET ?",
            sort_column, sort_order
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(params![limit.unwrap_or(1000), offset.unwrap_or(0)], |row| {
            Ok(Plugin {
                id: Uuid::new_v4(),
                plugin_id: row.get("ableton_plugin_id")?,
                module_id: row.get("ableton_module_id")?,
                dev_identifier: row.get("dev_identifier")?,
                name: row.get("name")?,
                plugin_format: row
                    .get::<_, String>("format")?
                    .parse()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                installed: row.get("installed")?,
                vendor: row.get("vendor")?,
                version: row.get("version")?,
                sdk_version: row.get("sdk_version")?,
                flags: row.get("flags")?,
                scanstate: row.get("scanstate")?,
                enabled: row.get("enabled")?,
            })
        })?;

        let plugins: Result<Vec<Plugin>, _> = rows.collect();
        Ok((plugins?, total_count))
    }

    /// Get plugins filtered by installation status
    pub fn get_plugins_by_installed_status(
        &self,
        installed: bool,
        limit: Option<i32>,
        offset: Option<i32>,
        sort_by: Option<String>,
        sort_desc: Option<bool>,
    ) -> Result<(Vec<Plugin>, i32), DatabaseError> {
        let sort_column = match sort_by.as_deref() {
            Some("name") => "name",
            Some("vendor") => "vendor",
            Some("installed") => "installed",
            Some("format") => "format",
            _ => "name", // default sort
        };

        let sort_order = if sort_desc.unwrap_or(false) {
            "DESC"
        } else {
            "ASC"
        };

        // Get total count
        let total_count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM plugins WHERE installed = ?",
            params![installed],
            |row| row.get(0),
        )?;

        // Build query with pagination
        let query = format!(
            "SELECT * FROM plugins WHERE installed = ? ORDER BY {} {} LIMIT ? OFFSET ?",
            sort_column, sort_order
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            params![installed, limit.unwrap_or(1000), offset.unwrap_or(0)],
            |row| {
                Ok(Plugin {
                    id: Uuid::new_v4(),
                    plugin_id: row.get("ableton_plugin_id")?,
                    module_id: row.get("ableton_module_id")?,
                    dev_identifier: row.get("dev_identifier")?,
                    name: row.get("name")?,
                    plugin_format: row
                        .get::<_, String>("format")?
                        .parse()
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                    installed: row.get("installed")?,
                    vendor: row.get("vendor")?,
                    version: row.get("version")?,
                    sdk_version: row.get("sdk_version")?,
                    flags: row.get("flags")?,
                    scanstate: row.get("scanstate")?,
                    enabled: row.get("enabled")?,
                })
            },
        )?;

        let plugins: Result<Vec<Plugin>, _> = rows.collect();
        Ok((plugins?, total_count))
    }

    /// Search plugins by name, vendor, or format
    pub fn search_plugins(
        &self,
        query: &str,
        limit: Option<i32>,
        offset: Option<i32>,
        installed_only: Option<bool>,
        vendor_filter: Option<String>,
        format_filter: Option<String>,
    ) -> Result<(Vec<Plugin>, i32), DatabaseError> {
        let mut conditions = vec!["(name LIKE ? OR vendor LIKE ? OR format LIKE ?)"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(format!("%{}%", query)),
            Box::new(format!("%{}%", query)),
            Box::new(format!("%{}%", query)),
        ];

        if let Some(installed) = installed_only {
            conditions.push("installed = ?");
            params.push(Box::new(installed));
        }

        if let Some(vendor) = vendor_filter {
            conditions.push("vendor = ?");
            params.push(Box::new(vendor));
        }

        if let Some(format) = format_filter {
            conditions.push("format = ?");
            params.push(Box::new(format));
        }

        let where_clause = conditions.join(" AND ");

        // Get total count
        let count_query = format!("SELECT COUNT(*) FROM plugins WHERE {}", where_clause);
        let mut count_stmt = self.conn.prepare(&count_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let total_count: i32 = count_stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;

        // Build main query
        let main_query = format!(
            "SELECT * FROM plugins WHERE {} ORDER BY name ASC LIMIT ? OFFSET ?",
            where_clause
        );

        params.push(Box::new(limit.unwrap_or(1000)));
        params.push(Box::new(offset.unwrap_or(0)));

        let mut stmt = self.conn.prepare(&main_query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Plugin {
                id: Uuid::new_v4(),
                plugin_id: row.get("ableton_plugin_id")?,
                module_id: row.get("ableton_module_id")?,
                dev_identifier: row.get("dev_identifier")?,
                name: row.get("name")?,
                plugin_format: row
                    .get::<_, String>("format")?
                    .parse()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                installed: row.get("installed")?,
                vendor: row.get("vendor")?,
                version: row.get("version")?,
                sdk_version: row.get("sdk_version")?,
                flags: row.get("flags")?,
                scanstate: row.get("scanstate")?,
                enabled: row.get("enabled")?,
            })
        })?;

        let plugins: Result<Vec<Plugin>, _> = rows.collect();
        Ok((plugins?, total_count))
    }

    /// Get plugin statistics for status bar
    pub fn get_plugin_stats(&self) -> Result<PluginStats, DatabaseError> {
        let total_plugins: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))?;

        let installed_plugins: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM plugins WHERE installed = true",
            [],
            |row| row.get(0),
        )?;

        let missing_plugins = total_plugins - installed_plugins;

        let unique_vendors: i32 = self.conn.query_row(
            "SELECT COUNT(DISTINCT vendor) FROM plugins WHERE vendor IS NOT NULL",
            [],
            |row| row.get(0),
        )?;

        // Get plugins by format
        let mut plugins_by_format = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT format, COUNT(*) FROM plugins GROUP BY format")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        for row in rows {
            let (format, count) = row?;
            plugins_by_format.insert(format, count);
        }

        // Get plugins by vendor
        let mut plugins_by_vendor = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT vendor, COUNT(*) FROM plugins WHERE vendor IS NOT NULL GROUP BY vendor ORDER BY COUNT(*) DESC LIMIT 10"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;

        for row in rows {
            let (vendor, count) = row?;
            plugins_by_vendor.insert(vendor, count);
        }

        Ok(PluginStats {
            total_plugins,
            installed_plugins,
            missing_plugins,
            unique_vendors,
            plugins_by_format,
            plugins_by_vendor,
        })
    }

    /// Get plugin usage numbers
    pub fn get_all_plugin_usage_numbers(&self) -> Result<Vec<PluginUsageInfo>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                p.id,
                p.name,
                p.vendor,
                COUNT(pp.project_id) as usage_count,
                COUNT(DISTINCT pp.project_id) as project_count
            FROM plugins p
            LEFT JOIN project_plugins pp ON pp.plugin_id = p.id
            GROUP BY p.id, p.name, p.vendor
            ORDER BY usage_count DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PluginUsageInfo {
                plugin_id: row.get("id")?,
                name: row.get("name")?,
                vendor: row.get::<_, Option<String>>("vendor")?,
                usage_count: row.get("usage_count")?,
                project_count: row.get("project_count")?,
            })
        })?;

        let usage_info: Result<Vec<PluginUsageInfo>, _> = rows.collect();
        Ok(usage_info?)
    }
}

pub struct PluginStats {
    pub total_plugins: i32,
    pub installed_plugins: i32,
    pub missing_plugins: i32,
    pub unique_vendors: i32,
    pub plugins_by_format: std::collections::HashMap<String, i32>,
    pub plugins_by_vendor: std::collections::HashMap<String, i32>,
}

pub struct PluginUsageInfo {
    pub plugin_id: String,
    pub name: String,
    pub vendor: Option<String>,
    pub usage_count: i32,
    pub project_count: i32,
}
