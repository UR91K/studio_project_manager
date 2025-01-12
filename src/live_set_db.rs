#![allow(unused_imports)]
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{AbletonVersion, KeySignature, Plugin, Sample, TimeSignature};
use chrono::{DateTime, Local, TimeZone};
use log::{debug, info, warn};
use rusqlite::{params, types::ToSql, Connection, OptionalExtension, Result as SqliteResult};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

// Wrapper type for DateTime
struct SqlDateTime(DateTime<Local>);

impl From<DateTime<Local>> for SqlDateTime {
    fn from(dt: DateTime<Local>) -> Self {
        SqlDateTime(dt)
    }
}

impl ToSql for SqlDateTime {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.timestamp()))
    }
}

pub struct LiveSetDatabase {
    pub conn: Connection,
}

impl LiveSetDatabase {
    pub fn new(db_path: PathBuf) -> Result<Self, DatabaseError> {
        debug!("Opening database at {:?}", db_path);
        let conn = Connection::open(&db_path)?;
        let mut db = Self { conn };
        db.initialize()?;
        info!("Database initialized successfully at {:?}", db_path);
        Ok(db)
    }

    fn initialize(&mut self) -> Result<(), DatabaseError> {
        debug!("Initializing database tables and indexes");
        self.conn.execute_batch(
            r#"
            -- Collections system
            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                notes TEXT,
                created_at DATETIME NOT NULL,
                modified_at DATETIME NOT NULL
            );

            -- Core project data
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                hash TEXT NOT NULL,
                notes TEXT,
                created_at DATETIME NOT NULL,
                modified_at DATETIME NOT NULL,
                last_scanned_at DATETIME NOT NULL,
                
                tempo REAL NOT NULL,
                time_signature_numerator INTEGER NOT NULL,
                time_signature_denominator INTEGER NOT NULL,
                key_signature_tonic TEXT,
                key_signature_scale TEXT,
                duration_seconds INTEGER,
                furthest_bar REAL,
                
                ableton_version_major INTEGER NOT NULL,
                ableton_version_minor INTEGER NOT NULL,
                ableton_version_patch INTEGER NOT NULL,
                ableton_version_beta BOOLEAN NOT NULL
            );

            -- Tasks system
            CREATE TABLE IF NOT EXISTS project_tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                description TEXT NOT NULL,
                completed BOOLEAN NOT NULL DEFAULT FALSE,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            -- Plugin catalog
            CREATE TABLE IF NOT EXISTS plugins (
                id TEXT PRIMARY KEY,
                ableton_plugin_id INTEGER,
                ableton_module_id INTEGER,
                dev_identifier TEXT NOT NULL,
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                installed BOOLEAN NOT NULL,
                vendor TEXT,
                version TEXT,
                sdk_version TEXT,
                flags INTEGER,
                scanstate INTEGER,
                enabled INTEGER,
                UNIQUE(dev_identifier)
            );

            -- Sample catalog
            CREATE TABLE IF NOT EXISTS samples (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                is_present BOOLEAN NOT NULL
            );

            -- Tags system
            CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                created_at DATETIME NOT NULL
            );

            CREATE TABLE IF NOT EXISTS project_tags (
                project_id TEXT NOT NULL,
                tag_id TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                PRIMARY KEY (project_id, tag_id),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            -- Collections system
            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at DATETIME NOT NULL,
                modified_at DATETIME NOT NULL
            );

            CREATE TABLE IF NOT EXISTS collection_projects (
                collection_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                position INTEGER NOT NULL,  -- For maintaining order
                added_at DATETIME NOT NULL,
                PRIMARY KEY (collection_id, project_id),
                FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            -- Junction tables
            CREATE TABLE IF NOT EXISTS project_plugins (
                project_id TEXT NOT NULL,
                plugin_id TEXT NOT NULL,
                PRIMARY KEY (project_id, plugin_id),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                FOREIGN KEY (plugin_id) REFERENCES plugins(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS project_samples (
                project_id TEXT NOT NULL,
                sample_id TEXT NOT NULL,
                PRIMARY KEY (project_id, sample_id),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                FOREIGN KEY (sample_id) REFERENCES samples(id) ON DELETE CASCADE
            );

            -- Basic indexes
            CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);
            CREATE INDEX IF NOT EXISTS idx_plugins_name ON plugins(name);
            CREATE INDEX IF NOT EXISTS idx_samples_path ON samples(path);
            CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
            CREATE INDEX IF NOT EXISTS idx_collection_projects_position ON collection_projects(collection_id, position);
            "#,
        )?;

        debug!("Database schema initialized successfully");
        Ok(())
    }

    pub fn insert_project(&mut self, live_set: &LiveSet) -> Result<(), DatabaseError> {
        debug!(
            "Inserting project: {} ({})",
            live_set.file_name,
            live_set.file_path.display()
        );
        let tx = self.conn.transaction()?;

        // Use the LiveSet's UUID
        let project_id = live_set.id.to_string();
        debug!("Using project UUID: {}", project_id);

        // Insert project
        tx.execute(
            r#"
            INSERT OR REPLACE INTO projects (
                id, path, name, hash, created_at, modified_at, last_scanned_at,
                tempo, time_signature_numerator, time_signature_denominator,
                key_signature_tonic, key_signature_scale, duration_seconds, furthest_bar,
                ableton_version_major, ableton_version_minor, ableton_version_patch, ableton_version_beta
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?
            )
            "#,
            params![
                project_id,
                live_set.file_path.to_string_lossy().to_string(),
                live_set.file_name,
                live_set.file_hash,
                SqlDateTime::from(live_set.created_time),
                SqlDateTime::from(live_set.modified_time),
                SqlDateTime::from(live_set.last_scan_timestamp),
                live_set.tempo,
                live_set.time_signature.numerator,
                live_set.time_signature.denominator,
                live_set.key_signature.as_ref().map(|k| k.tonic.to_string()),
                live_set.key_signature.as_ref().map(|k| k.scale.to_string()),
                live_set.estimated_duration.map(|d| d.num_seconds()),
                live_set.furthest_bar,
                live_set.ableton_version.major,
                live_set.ableton_version.minor,
                live_set.ableton_version.patch,
                live_set.ableton_version.beta,
            ],
        )?;

        debug!("Inserting {} plugins", live_set.plugins.len());
        // Insert plugins and link them
        for plugin in &live_set.plugins {
            let plugin_id = Uuid::new_v4().to_string();
            Self::insert_plugin(&tx, plugin, &plugin_id)?;
            Self::link_project_plugin(&tx, &project_id, &plugin_id)?;
            debug!("Inserted plugin: {} ({})", plugin.name, plugin_id);
        }

        debug!("Inserting {} samples", live_set.samples.len());
        // Insert samples and link them
        for sample in &live_set.samples {
            let sample_id = Uuid::new_v4().to_string();
            Self::insert_sample(&tx, sample, &sample_id)?;
            Self::link_project_sample(&tx, &project_id, &sample_id)?;
            debug!("Inserted sample: {} ({})", sample.name, sample_id);
        }

        tx.commit()?;
        info!(
            "Successfully inserted project {} with {} plugins and {} samples",
            live_set.file_name,
            live_set.plugins.len(),
            live_set.samples.len()
        );
        Ok(())
    }

    fn insert_plugin(
        tx: &rusqlite::Transaction,
        plugin: &Plugin,
        id: &str,
    ) -> Result<(), DatabaseError> {
        tx.execute(
            r#"
            INSERT OR IGNORE INTO plugins (
                id, ableton_plugin_id, ableton_module_id, dev_identifier, name, format, installed,
                vendor, version, sdk_version, flags, scanstate, enabled
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                id,
                plugin.plugin_id,
                plugin.module_id,
                plugin.dev_identifier,
                plugin.name,
                format!("{:?}", plugin.plugin_format),
                plugin.installed,
                plugin.vendor,
                plugin.version,
                plugin.sdk_version,
                plugin.flags,
                plugin.scanstate,
                plugin.enabled,
            ],
        )?;
        Ok(())
    }

    fn insert_sample(
        tx: &rusqlite::Transaction,
        sample: &Sample,
        id: &str,
    ) -> Result<(), DatabaseError> {
        tx.execute(
            "INSERT OR IGNORE INTO samples (id, name, path, is_present) VALUES (?, ?, ?, ?)",
            params![
                id,
                sample.name,
                sample.path.to_string_lossy().to_string(),
                sample.is_present,
            ],
        )?;
        Ok(())
    }

    fn link_project_plugin(
        tx: &rusqlite::Transaction,
        project_id: &str,
        plugin_id: &str,
    ) -> Result<(), DatabaseError> {
        tx.execute(
            "INSERT OR IGNORE INTO project_plugins (project_id, plugin_id) VALUES (?, ?)",
            params![project_id, plugin_id],
        )?;
        Ok(())
    }

    fn link_project_sample(
        tx: &rusqlite::Transaction,
        project_id: &str,
        sample_id: &str,
    ) -> Result<(), DatabaseError> {
        tx.execute(
            "INSERT OR IGNORE INTO project_samples (project_id, sample_id) VALUES (?, ?)",
            params![project_id, sample_id],
        )?;
        Ok(())
    }

    #[allow(unused_variables, dead_code)] // TODO: Implement
    pub fn get_project(&mut self, id: &str) -> Result<Option<LiveSet>, DatabaseError> {
        Ok(None)
    }

    pub fn search(&mut self, query: &str) -> Result<Vec<LiveSet>, DatabaseError> {
        debug!("Performing search with query: {}", query);
        let tx = self.conn.transaction()?;

        // First, get all matching project paths
        let project_paths = {
            let mut stmt = tx.prepare(
                r#"
                SELECT DISTINCT p.path 
                FROM projects p
                LEFT JOIN project_plugins pp ON pp.project_id = p.id
                LEFT JOIN plugins pl ON pl.id = pp.plugin_id
                LEFT JOIN project_samples ps ON ps.project_id = p.id
                LEFT JOIN samples s ON s.id = ps.sample_id
                WHERE 
                    p.name LIKE ?1 OR
                    pl.name LIKE ?1 OR
                    s.name LIKE ?1 OR
                    pl.vendor LIKE ?1
                "#,
            )?;

            let pattern = format!("%{}%", query);
            debug!("Using search pattern: {}", pattern);

            let paths: Vec<String> = stmt
                .query_map([&pattern], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            debug!("Found {} matching project paths", paths.len());
            paths
        };

        let mut results = Vec::new();

        // For each path, get the full project details
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
        debug!("Successfully retrieved {} matching projects", results.len());
        Ok(results)
    }

    pub fn get_project_by_path(&mut self, path: &str) -> Result<Option<LiveSet>, DatabaseError> {
        debug!("Retrieving project by path: {}", path);
        let tx = self.conn.transaction()?;

        // Get project
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

        let project = stmt
            .query_row([path], |row| {
                let project_id: String = row.get(0)?;
                debug!("Found project with ID: {}", project_id);

                let duration_secs: Option<i64> = row.get(12)?;
                let created_timestamp: i64 = row.get(4)?;
                let modified_timestamp: i64 = row.get(5)?;
                let scanned_timestamp: i64 = row.get(6)?;

                // Create LiveSet instance
                let live_set = LiveSet {
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

                Ok(live_set)
            })
            .optional()?;

        let mut project = match project {
            Some(p) => {
                debug!("Found project: {}", p.file_name);
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
            project.file_name,
            project.plugins.len(),
            project.samples.len()
        );
        Ok(Some(project))
    }

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

    pub fn create_collection(&mut self, name: &str, description: Option<&str>) -> Result<String, DatabaseError> {
        debug!("Creating collection: {}", name);
        let collection_id = Uuid::new_v4().to_string();
        let now = Local::now();

        self.conn.execute(
            "INSERT INTO collections (id, name, description, created_at, modified_at) VALUES (?, ?, ?, ?, ?)",
            params![
                collection_id,
                name,
                description,
                SqlDateTime::from(now),
                SqlDateTime::from(now)
            ],
        )?;

        debug!("Successfully created collection: {} ({})", name, collection_id);
        Ok(collection_id)
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
                SELECT p.id, p.path, p.name, p.hash, p.notes, p.created_at, p.modified_at, p.last_scanned_at,
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
                let scanned_timestamp: i64 = row.get(7)?;

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

                debug!("Adding project to results: {}", live_set.file_name);
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

    // Notes methods
    pub fn set_project_notes(&mut self, project_id: &str, notes: &str) -> Result<(), DatabaseError> {
        debug!("Setting notes for project {}", project_id);
        self.conn.execute(
            "UPDATE projects SET notes = ? WHERE id = ?",
            params![notes, project_id],
        )?;
        debug!("Successfully set project notes");
        Ok(())
    }

    pub fn get_project_notes(&mut self, project_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for project {}", project_id);
        let notes = self.conn.query_row(
            "SELECT notes FROM projects WHERE id = ?",
            [project_id],
            |row| row.get(0),
        )?;
        debug!("Successfully retrieved project notes");
        Ok(notes)
    }

    pub fn set_collection_notes(&mut self, collection_id: &str, notes: &str) -> Result<(), DatabaseError> {
        debug!("Setting notes for collection {}", collection_id);
        let now = Local::now();
        self.conn.execute(
            "UPDATE collections SET notes = ?, modified_at = ? WHERE id = ?",
            params![notes, SqlDateTime::from(now), collection_id],
        )?;
        debug!("Successfully set collection notes");
        Ok(())
    }

    pub fn get_collection_notes(&mut self, collection_id: &str) -> Result<Option<String>, DatabaseError> {
        debug!("Getting notes for collection {}", collection_id);
        let notes = self.conn.query_row(
            "SELECT notes FROM collections WHERE id = ?",
            [collection_id],
            |row| row.get(0),
        )?;
        debug!("Successfully retrieved collection notes");
        Ok(notes)
    }

    // Tasks methods
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
