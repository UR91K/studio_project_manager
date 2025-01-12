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
use super::models::SqlDateTime;
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
            -- Core tables
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

            CREATE TABLE IF NOT EXISTS samples (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                is_present BOOLEAN NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                created_at DATETIME NOT NULL
            );

            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                notes TEXT,
                created_at DATETIME NOT NULL,
                modified_at DATETIME NOT NULL
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

            CREATE TABLE IF NOT EXISTS project_tags (
                project_id TEXT NOT NULL,
                tag_id TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                PRIMARY KEY (project_id, tag_id),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS collection_projects (
                collection_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                added_at DATETIME NOT NULL,
                PRIMARY KEY (collection_id, project_id),
                FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            -- Additional features
            CREATE TABLE IF NOT EXISTS project_tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                description TEXT NOT NULL,
                completed BOOLEAN NOT NULL DEFAULT FALSE,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            -- Basic indexes for performance
            CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);
            CREATE INDEX IF NOT EXISTS idx_plugins_name ON plugins(name);
            CREATE INDEX IF NOT EXISTS idx_samples_path ON samples(path);
            CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
            CREATE INDEX IF NOT EXISTS idx_collection_projects_position ON collection_projects(collection_id, position);

            -- Full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS project_search USING fts5(
                project_id UNINDEXED,  -- Reference to the original project
                name,                  -- Project name
                path,                  -- Project path
                created_at UNINDEXED,  -- Timestamps are not searchable
                modified_at UNINDEXED,
                version,               -- Ableton version
                key_signature,         -- Combined key signature
                tempo,                 -- Project tempo
                time_signature,        -- Combined time signature
                plugins,               -- Concatenated plugin names and vendors
                samples,               -- Concatenated sample names
                tags,                  -- Concatenated tag names
                notes,                 -- Project notes
                tokenize = 'porter unicode61 remove_diacritics 2'
            );

            -- FTS5 triggers for maintaining the search index
            CREATE TRIGGER IF NOT EXISTS projects_au AFTER UPDATE ON projects BEGIN
                -- First delete the old record from FTS
                DELETE FROM project_search WHERE project_id = old.id;
                
                -- Then insert the updated record
                INSERT INTO project_search (
                    project_id, name, path, created_at, modified_at,
                    version, key_signature, tempo, time_signature,
                    plugins, samples, tags, notes
                )
                SELECT 
                    p.id,
                    p.name,
                    p.path,
                    p.created_at,
                    p.modified_at,
                    p.ableton_version_major || '.' || p.ableton_version_minor,
                    CASE 
                        WHEN p.key_signature_tonic IS NOT NULL 
                        THEN p.key_signature_tonic || ' ' || p.key_signature_scale 
                        ELSE NULL 
                    END,
                    p.tempo,
                    p.time_signature_numerator || '/' || p.time_signature_denominator,
                    (SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                     FROM plugins pl 
                     JOIN project_plugins pp ON pp.plugin_id = pl.id 
                     WHERE pp.project_id = p.id),
                    (SELECT GROUP_CONCAT(s.name, ' ')
                     FROM samples s 
                     JOIN project_samples ps ON ps.sample_id = s.id 
                     WHERE ps.project_id = p.id),
                    (SELECT GROUP_CONCAT(t.name, ' ')
                     FROM tags t 
                     JOIN project_tags pt ON pt.tag_id = t.id 
                     WHERE pt.project_id = p.id),
                    COALESCE(p.notes, '')
                FROM projects p
                WHERE p.id = new.id;
            END;

            CREATE TRIGGER IF NOT EXISTS projects_ad AFTER DELETE ON projects BEGIN
                DELETE FROM project_search WHERE project_id = old.id;
            END;
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

        // Update FTS5 index after everything is inserted
        debug!("Updating FTS5 index for project {}", live_set.file_name);
        self.conn.execute(
            r#"
            INSERT INTO project_search (
                project_id, name, path, created_at, modified_at,
                version, key_signature, tempo, time_signature,
                plugins, samples, tags, notes
            )
            SELECT 
                p.id,
                p.name,
                p.path,
                p.created_at,
                p.modified_at,
                p.ableton_version_major || '.' || p.ableton_version_minor,
                CASE 
                    WHEN p.key_signature_tonic IS NOT NULL 
                    THEN p.key_signature_tonic || ' ' || p.key_signature_scale 
                    ELSE NULL 
                END,
                p.tempo,
                p.time_signature_numerator || '/' || p.time_signature_denominator,
                (SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                 FROM plugins pl 
                 JOIN project_plugins pp ON pp.plugin_id = pl.id 
                 WHERE pp.project_id = p.id),
                (SELECT GROUP_CONCAT(s.name, ' ')
                 FROM samples s 
                 JOIN project_samples ps ON ps.sample_id = s.id 
                 WHERE ps.project_id = p.id),
                (SELECT GROUP_CONCAT(t.name, ' ')
                 FROM tags t 
                 JOIN project_tags pt ON pt.tag_id = t.id 
                 WHERE pt.project_id = p.id),
                COALESCE(p.notes, '')
            FROM projects p
            WHERE p.id = ?
            "#,
            [project_id],
        )?;

        info!(
            "Successfully inserted project {} with {} plugins and {} samples",
            live_set.file_name,
            live_set.plugins.len(),
            live_set.samples.len()
        );
        
        // After successful commit, let's inspect the FTS5 index
        debug!("Inspecting FTS5 index for project {}", live_set.file_name);
        match self.conn.query_row(
            "SELECT * FROM project_search WHERE project_id = ?",
            [live_set.id.to_string()],
            |row| {
                debug!("FTS5 index content:");
                debug!("  project_id: {}", row.get::<_, String>(0)?);
                debug!("  name: {}", row.get::<_, String>(1)?);
                debug!("  path: {}", row.get::<_, String>(2)?);
                let plugins: Option<String> = row.get(9)?;
                debug!("  plugins: {:?}", plugins);
                let samples: Option<String> = row.get(10)?;
                debug!("  samples: {:?}", samples);
                let tags: Option<String> = row.get(11)?;
                debug!("  tags: {:?}", tags);
                let notes: Option<String> = row.get(12)?;
                debug!("  notes: {:?}", notes);
                Ok(())
            },
        ) {
            Ok(_) => debug!("Successfully inspected FTS5 index"),
            Err(e) => warn!("Failed to inspect FTS5 index: {}", e),
        };

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
}
