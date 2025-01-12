use rusqlite::{params, Connection, Result as SqliteResult, types::ToSql, OptionalExtension};
use std::path::PathBuf;
use chrono::{DateTime, Local, TimeZone};
use uuid::Uuid;
use std::collections::HashSet;
use std::str::FromStr;
#[allow(unused_imports)]
use log::{debug, info, warn};
use crate::error::DatabaseError;
use crate::models::{Plugin, Sample, PluginFormat, Scale, Tonic, KeySignature, TimeSignature, AbletonVersion};
use crate::live_set::LiveSet;

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

impl FromStr for Tonic {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Empty" => Ok(Tonic::Empty),
            "C" => Ok(Tonic::C),
            "CSharp" => Ok(Tonic::CSharp),
            "D" => Ok(Tonic::D),
            "DSharp" => Ok(Tonic::DSharp),
            "E" => Ok(Tonic::E),
            "F" => Ok(Tonic::F),
            "FSharp" => Ok(Tonic::FSharp),
            "G" => Ok(Tonic::G),
            "GSharp" => Ok(Tonic::GSharp),
            "A" => Ok(Tonic::A),
            "ASharp" => Ok(Tonic::ASharp),
            "B" => Ok(Tonic::B),
            _ => Err(format!("Invalid tonic: {}", s)),
        }
    }
}

impl FromStr for Scale {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Empty" => Ok(Scale::Empty),
            "Major" => Ok(Scale::Major),
            "Minor" => Ok(Scale::Minor),
            "Dorian" => Ok(Scale::Dorian),
            "Mixolydian" => Ok(Scale::Mixolydian),
            "Aeolian" => Ok(Scale::Aeolian),
            "Phrygian" => Ok(Scale::Phrygian),
            "Locrian" => Ok(Scale::Locrian),
            "WholeTone" => Ok(Scale::WholeTone),
            "HalfWholeDim" => Ok(Scale::HalfWholeDim),
            "WholeHalfDim" => Ok(Scale::WholeHalfDim),
            "MinorBlues" => Ok(Scale::MinorBlues),
            "MinorPentatonic" => Ok(Scale::MinorPentatonic),
            "MajorPentatonic" => Ok(Scale::MajorPentatonic),
            "HarmonicMinor" => Ok(Scale::HarmonicMinor),
            "MelodicMinor" => Ok(Scale::MelodicMinor),
            "Dorian4" => Ok(Scale::Dorian4),
            "PhrygianDominant" => Ok(Scale::PhrygianDominant),
            "LydianDominant" => Ok(Scale::LydianDominant),
            "LydianAugmented" => Ok(Scale::LydianAugmented),
            "HarmonicMajor" => Ok(Scale::HarmonicMajor),
            "SuperLocrian" => Ok(Scale::SuperLocrian),
            "BToneSpanish" => Ok(Scale::BToneSpanish),
            "HungarianMinor" => Ok(Scale::HungarianMinor),
            "Hirajoshi" => Ok(Scale::Hirajoshi),
            "Iwato" => Ok(Scale::Iwato),
            "PelogSelisir" => Ok(Scale::PelogSelisir),
            "PelogTembung" => Ok(Scale::PelogTembung),
            "Messiaen1" => Ok(Scale::Messiaen1),
            "Messiaen2" => Ok(Scale::Messiaen2),
            "Messiaen3" => Ok(Scale::Messiaen3),
            "Messiaen4" => Ok(Scale::Messiaen4),
            "Messiaen5" => Ok(Scale::Messiaen5),
            "Messiaen6" => Ok(Scale::Messiaen6),
            "Messiaen7" => Ok(Scale::Messiaen7),
            _ => Err(format!("Invalid scale: {}", s)),
        }
    }
}

impl FromStr for PluginFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "VST2Instrument" | "VST2 Instrument" => Ok(PluginFormat::VST2Instrument),
            "VST2AudioFx" | "VST2 Effect" => Ok(PluginFormat::VST2AudioFx),
            "VST3Instrument" | "VST3 Instrument" => Ok(PluginFormat::VST3Instrument),
            "VST3AudioFx" | "VST3 Effect" => Ok(PluginFormat::VST3AudioFx),
            _ => Err(format!("Invalid plugin format: {}", s)),
        }
    }
}

pub struct LiveSetDatabase {
    conn: Connection,
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
            -- Core project data
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                hash TEXT NOT NULL,
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
            "#,
        )?;

        debug!("Database schema initialized successfully");
        Ok(())
    }

    pub fn insert_project(&mut self, live_set: &LiveSet) -> Result<(), DatabaseError> {
        debug!("Inserting project: {} ({})", live_set.file_name, live_set.file_path.display());
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
        info!("Successfully inserted project {} with {} plugins and {} samples", 
            live_set.file_name, live_set.plugins.len(), live_set.samples.len());
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

            let paths: Vec<String> = stmt.query_map([&pattern], |row| {
                row.get(0)
            })?.filter_map(|r| r.ok()).collect();
            
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
                        id: Uuid::parse_str(&project_id).map_err(|_| rusqlite::Error::InvalidParameterName("Invalid UUID".into()))?,
                        file_path: PathBuf::from(row.get::<_, String>(1)?),
                        file_name: row.get(2)?,
                        file_hash: row.get(3)?,
                        created_time: Local.timestamp_opt(created_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        modified_time: Local.timestamp_opt(modified_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        last_scan_timestamp: Local.timestamp_opt(scanned_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        xml_data: Vec::new(),
                        
                        tempo: row.get(7)?,
                        time_signature: TimeSignature {
                            numerator: row.get(8)?,
                            denominator: row.get(9)?,
                        },
                        key_signature: match (row.get::<_, Option<String>>(10)?, row.get::<_, Option<String>>(11)?) {
                            (Some(tonic), Some(scale)) => {
                                debug!("Found key signature: {} {}", tonic, scale);
                                Some(KeySignature {
                                    tonic: tonic.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                    scale: scale.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                })
                            },
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

                        let plugins = stmt.query_map([&project_id], |row| {
                            let name: String = row.get(4)?;
                            debug!("Found plugin: {}", name);
                            Ok(Plugin {
                                id: Uuid::new_v4(),
                                plugin_id: row.get(1)?,
                                module_id: row.get(2)?,
                                dev_identifier: row.get(3)?,
                                name,
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
                        })?.filter_map(|r| r.ok()).collect::<HashSet<_>>();

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

                        let samples = stmt.query_map([&project_id], |row| {
                            let name: String = row.get(1)?;
                            debug!("Found sample: {}", name);
                            Ok(Sample {
                                id: Uuid::new_v4(),
                                name,
                                path: PathBuf::from(row.get::<_, String>(2)?),
                                is_present: row.get(3)?,
                            })
                        })?.filter_map(|r| r.ok()).collect::<HashSet<_>>();

                        debug!("Retrieved {} samples", samples.len());
                        live_set.samples = samples;
                    }

                    Ok(live_set)
                }).optional()?
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

        let project = stmt.query_row([path], |row| {
            let project_id: String = row.get(0)?;
            debug!("Found project with ID: {}", project_id);
            
            let duration_secs: Option<i64> = row.get(12)?;
            let created_timestamp: i64 = row.get(4)?;
            let modified_timestamp: i64 = row.get(5)?;
            let scanned_timestamp: i64 = row.get(6)?;
            
            // Create LiveSet instance
            let live_set = LiveSet {
                id: Uuid::parse_str(&project_id).map_err(|_| rusqlite::Error::InvalidParameterName("Invalid UUID".into()))?,
                file_path: PathBuf::from(row.get::<_, String>(1)?),
                file_name: row.get(2)?,
                file_hash: row.get(3)?,
                created_time: Local.timestamp_opt(created_timestamp, 0)
                    .single()
                    .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                modified_time: Local.timestamp_opt(modified_timestamp, 0)
                    .single()
                    .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                last_scan_timestamp: Local.timestamp_opt(scanned_timestamp, 0)
                    .single()
                    .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                xml_data: Vec::new(),
                
                tempo: row.get(7)?,
                time_signature: TimeSignature {
                    numerator: row.get(8)?,
                    denominator: row.get(9)?,
                },
                key_signature: match (row.get::<_, Option<String>>(10)?, row.get::<_, Option<String>>(11)?) {
                    (Some(tonic), Some(scale)) => {
                        debug!("Found key signature: {} {}", tonic, scale);
                        Some(KeySignature {
                            tonic: tonic.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                            scale: scale.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                        })
                    },
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
        }).optional()?;

        let mut project = match project {
            Some(p) => {
                debug!("Found project: {}", p.file_name);
                p
            },
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

        let plugins = stmt.query_map([path], |row| {
            let name: String = row.get(4)?;
            debug!("Found plugin: {}", name);
            Ok(Plugin {
                id: Uuid::new_v4(),
                plugin_id: row.get(1)?,
                module_id: row.get(2)?,
                dev_identifier: row.get(3)?,
                name,
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
        })?.collect::<SqliteResult<HashSet<_>>>()?;

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

        let samples = stmt.query_map([path], |row| {
            let name: String = row.get(1)?;
            debug!("Found sample: {}", name);
            Ok(Sample {
                id: Uuid::new_v4(),
                name,
                path: PathBuf::from(row.get::<_, String>(2)?),
                is_present: row.get(3)?,
            })
        })?.collect::<SqliteResult<HashSet<_>>>()?;

        debug!("Retrieved {} samples", samples.len());
        project.samples = samples;

        info!("Successfully retrieved project {} with {} plugins and {} samples", 
            project.file_name, project.plugins.len(), project.samples.len());
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
        self.conn.execute("DELETE FROM tags WHERE id = ?", [tag_id])?;
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

        let tags = stmt.query_map([project_id], |row| {
            let name: String = row.get(0)?;
            debug!("Found tag: {}", name);
            Ok(name)
        })?.filter_map(|r| r.ok()).collect();

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

            let paths: Vec<String> = stmt.query_map([tag_id], |row| {
                let path: String = row.get(0)?;
                Ok(path)
            })?.filter_map(|r| r.ok()).collect();
            
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
                        id: Uuid::parse_str(&project_id).map_err(|_| rusqlite::Error::InvalidParameterName("Invalid UUID".into()))?,
                        file_path: PathBuf::from(row.get::<_, String>(1)?),
                        file_name: row.get(2)?,
                        file_hash: row.get(3)?,
                        created_time: Local.timestamp_opt(created_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        modified_time: Local.timestamp_opt(modified_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        last_scan_timestamp: Local.timestamp_opt(scanned_timestamp, 0)
                            .single()
                            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Invalid timestamp".into()))?,
                        xml_data: Vec::new(),
                        
                        tempo: row.get(7)?,
                        time_signature: TimeSignature {
                            numerator: row.get(8)?,
                            denominator: row.get(9)?,
                        },
                        key_signature: match (row.get::<_, Option<String>>(10)?, row.get::<_, Option<String>>(11)?) {
                            (Some(tonic), Some(scale)) => {
                                debug!("Found key signature: {} {}", tonic, scale);
                                Some(KeySignature {
                                    tonic: tonic.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                    scale: scale.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                                })
                            },
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

                        let plugins = stmt.query_map([&project_id], |row| {
                            let name: String = row.get(4)?;
                            debug!("Found plugin: {}", name);
                            Ok(Plugin {
                                id: Uuid::new_v4(),
                                plugin_id: row.get(1)?,
                                module_id: row.get(2)?,
                                dev_identifier: row.get(3)?,
                                name,
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
                        })?.filter_map(|r| r.ok()).collect::<HashSet<_>>();

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

                        let samples = stmt.query_map([&project_id], |row| {
                            let name: String = row.get(1)?;
                            debug!("Found sample: {}", name);
                            Ok(Sample {
                                id: Uuid::new_v4(),
                                name,
                                path: PathBuf::from(row.get::<_, String>(2)?),
                                is_present: row.get(3)?,
                            })
                        })?.filter_map(|r| r.ok()).collect::<HashSet<_>>();

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

                        let tags = stmt.query_map([&project_id], |row| {
                            let name: String = row.get(0)?;
                            debug!("Found tag: {}", name);
                            Ok(name)
                        })?.filter_map(|r| r.ok()).collect::<HashSet<_>>();

                        debug!("Retrieved {} tags", tags.len());
                        live_set.tags = tags;
                    }

                    Ok(live_set)
                }).optional()?
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
        let mut stmt = self.conn.prepare("SELECT id, name FROM tags ORDER BY name")?;
        
        let tags = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            debug!("Found tag: {} ({})", name, id);
            Ok((id, name))
        })?.filter_map(|r| r.ok()).collect();

        debug!("Retrieved all tags");
        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use crate::{scan::ScanResult, test_utils::LiveSetBuilder};

    use super::*;
    use std::sync::Once;
    use chrono::Local;
    use std::collections::HashSet;
    
    static INIT: Once = Once::new();
    fn setup() {
        let _ = INIT.call_once(|| {
            let _ = std::env::set_var("RUST_LOG", "debug");
            if let Err(_) = env_logger::try_init() {
                // Logger already initialized, that's fine
            }
        });
    }

    fn create_test_live_set() -> LiveSet {
        let now = Local::now();
        let mut plugins = HashSet::new();
        let mut samples = HashSet::new();

        // Add a test plugin
        plugins.insert(Plugin {
            id: Uuid::new_v4(),
            plugin_id: Some(1),
            module_id: Some(2),
            dev_identifier: "device:vst3:audiofx:test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            vendor: Some("Test Vendor".to_string()),
            version: Some("1.0.0".to_string()),
            sdk_version: Some("1.0".to_string()),
            flags: Some(0),
            scanstate: Some(1),
            enabled: Some(1),
            plugin_format: PluginFormat::VST3AudioFx,
            installed: true,
        });

        // Add a test sample
        samples.insert(Sample {
            id: Uuid::new_v4(),
            name: "test_sample.wav".to_string(),
            path: PathBuf::from("C:/test/test_sample.wav"),
            is_present: true,
        });

        LiveSet {
            id: Uuid::new_v4(),
            file_path: PathBuf::from("C:/test/test_project.als"),
            file_name: "test_project.als".to_string(),
            file_hash: "test_hash".to_string(),
            created_time: now,
            modified_time: now,
            last_scan_timestamp: now,
            xml_data: Vec::new(),
            
            ableton_version: AbletonVersion {
                major: 11,
                minor: 1,
                patch: 0,
                beta: false,
            },

            key_signature: Some(KeySignature {
                tonic: Tonic::C,
                scale: Scale::Major,
            }),
            tempo: 120.0,
            time_signature: TimeSignature {
                numerator: 4,
                denominator: 4,
            },
            furthest_bar: Some(16.0),
            plugins,
            samples,
            estimated_duration: Some(chrono::Duration::seconds(60)),
            tags: HashSet::new(),
        }
    }

    fn create_test_live_set_from_scan(name: &str, scan_result: ScanResult) -> LiveSet {
        let now = Local::now();
        LiveSet {
            id: Uuid::new_v4(),
            file_path: PathBuf::from(format!("C:/test/{}", name)),
            file_name: name.to_string(),
            file_hash: format!("test_hash_{}", name),
            created_time: now,
            modified_time: now,
            last_scan_timestamp: now,
            xml_data: Vec::new(),
            
            ableton_version: scan_result.version,
            key_signature: scan_result.key_signature,
            tempo: scan_result.tempo,
            time_signature: scan_result.time_signature,
            furthest_bar: scan_result.furthest_bar,
            plugins: scan_result.plugins,
            samples: scan_result.samples,
            
            estimated_duration: Some(chrono::Duration::seconds(60)),
            tags: HashSet::new(),
        }
    }

    #[test]
    fn test_database_initialization() {
        setup();
        let db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");
        
        // Verify tables exist
        let tables = db
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"plugins".to_string()));
        assert!(tables.contains(&"samples".to_string()));
        assert!(tables.contains(&"project_plugins".to_string()));
        assert!(tables.contains(&"project_samples".to_string()));
    }

    #[test]
    fn test_insert_and_retrieve_project() {
        setup();
        let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");
        
        // Create and insert a test project
        let original_live_set = create_test_live_set();
        db.insert_project(&original_live_set).expect("Failed to insert project");

        // Retrieve the project by path
        let path = original_live_set.file_path.to_string_lossy().to_string();
        let retrieved_live_set = db.get_project_by_path(&path).expect("Failed to retrieve project")
            .expect("Project not found");

        // Compare relevant fields
        assert_eq!(retrieved_live_set.file_name, original_live_set.file_name);
        assert_eq!(retrieved_live_set.file_hash, original_live_set.file_hash);
        assert_eq!(retrieved_live_set.tempo, original_live_set.tempo);
        assert_eq!(retrieved_live_set.time_signature.numerator, original_live_set.time_signature.numerator);
        assert_eq!(retrieved_live_set.time_signature.denominator, original_live_set.time_signature.denominator);
        assert_eq!(retrieved_live_set.key_signature, original_live_set.key_signature);
        assert_eq!(retrieved_live_set.furthest_bar, original_live_set.furthest_bar);
        assert_eq!(retrieved_live_set.ableton_version, original_live_set.ableton_version);
        
        // Compare collections
        assert_eq!(retrieved_live_set.plugins.len(), original_live_set.plugins.len());
        assert_eq!(retrieved_live_set.samples.len(), original_live_set.samples.len());

        // Compare first plugin
        let original_plugin = original_live_set.plugins.iter().next().unwrap();
        let retrieved_plugin = retrieved_live_set.plugins.iter().next().unwrap();
        assert_eq!(retrieved_plugin.name, original_plugin.name);
        assert_eq!(retrieved_plugin.plugin_format, original_plugin.plugin_format);
        assert_eq!(retrieved_plugin.installed, original_plugin.installed);

        // Compare first sample
        let original_sample = original_live_set.samples.iter().next().unwrap();
        let retrieved_sample = retrieved_live_set.samples.iter().next().unwrap();
        assert_eq!(retrieved_sample.name, original_sample.name);
        assert_eq!(retrieved_sample.path, original_sample.path);
        assert_eq!(retrieved_sample.is_present, original_sample.is_present);
    }

    #[test]
    fn test_multiple_projects() {
        setup();
        let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");
        
        // Create three different projects with distinct characteristics
        let edm_project = create_test_live_set_from_scan(
            "EDM Project.als",
            LiveSetBuilder::new()
                .with_plugin("Serum")
                .with_plugin("Massive")
                .with_installed_plugin("Pro-Q 3", Some("FabFilter".to_string()))
                .with_sample("kick.wav")
                .with_sample("snare.wav")
                .with_tempo(140.0)
                .build()
        );

        let rock_project = create_test_live_set_from_scan(
            "Rock Band.als",
            LiveSetBuilder::new()
                .with_plugin("Guitar Rig 6")
                .with_installed_plugin("Pro-R", Some("FabFilter".to_string()))
                .with_sample("guitar_riff.wav")
                .with_sample("drums.wav")
                .with_tempo(120.0)
                .build()
        );

        let ambient_project = create_test_live_set_from_scan(
            "Ambient Soundscape.als",
            LiveSetBuilder::new()
                .with_plugin("Omnisphere")
                .with_installed_plugin("Pro-L 2", Some("FabFilter".to_string()))
                .with_sample("pad.wav")
                .with_sample("atmosphere.wav")
                .with_tempo(80.0)
                .build()
        );

        
        // Store the IDs before inserting
        let edm_id = edm_project.id;
        // let rock_id = rock_project.id;
        // let ambient_id = ambient_project.id;

        // Insert all projects
        db.insert_project(&edm_project).expect("Failed to insert EDM project");
        db.insert_project(&rock_project).expect("Failed to insert rock project");
        db.insert_project(&ambient_project).expect("Failed to insert ambient project");

        // Test retrieval by path
        let retrieved_edm = db.get_project_by_path(&edm_project.file_path.to_string_lossy())
            .expect("Failed to retrieve EDM project")
            .expect("EDM project not found");

        // Verify project details
        assert_eq!(retrieved_edm.id, edm_id);
        assert_eq!(retrieved_edm.tempo, 140.0);
        assert_eq!(retrieved_edm.plugins.len(), 3);
        assert_eq!(retrieved_edm.samples.len(), 2);
        
        // Verify specific plugin exists
        assert!(retrieved_edm.plugins.iter().any(|p| p.name == "Serum"));
        assert!(retrieved_edm.plugins.iter().any(|p| p.name == "Pro-Q 3" && p.vendor == Some("FabFilter".to_string())));

        // Test basic search functionality
        let fab_filter_results = db.search("FabFilter").expect("Search failed");
        assert_eq!(fab_filter_results.len(), 3); // All projects have a FabFilter plugin

        let edm_results = db.search("kick.wav").expect("Search failed");
        assert_eq!(edm_results.len(), 1); // Only EDM project has kick.wav

        let serum_results = db.search("Serum").expect("Search failed");
        assert_eq!(serum_results.len(), 1); // Only EDM project has Serum
    }

    #[test]
    fn test_tags() {
        setup();
        let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");
        
        // Create test project
        let live_set = create_test_live_set();
        db.insert_project(&live_set).expect("Failed to insert project");

        // Test adding tags
        let tag1_id = db.add_tag("work-in-progress").expect("Failed to add tag");
        let tag2_id = db.add_tag("favorite").expect("Failed to add tag");
        
        // Test listing tags
        let tags = db.list_tags().expect("Failed to list tags");
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|(_, name)| name == "work-in-progress"));
        assert!(tags.iter().any(|(_, name)| name == "favorite"));

        // Test tagging project
        db.tag_project(&live_set.id.to_string(), &tag1_id).expect("Failed to tag project");
        db.tag_project(&live_set.id.to_string(), &tag2_id).expect("Failed to tag project");

        // Test getting project tags
        let project_tags = db.get_project_tags(&live_set.id.to_string()).expect("Failed to get project tags");
        assert_eq!(project_tags.len(), 2);
        assert!(project_tags.contains("work-in-progress"));
        assert!(project_tags.contains("favorite"));

        // Test getting projects by tag
        let tagged_projects = db.get_projects_by_tag(&tag1_id).expect("Failed to get projects by tag");
        assert_eq!(tagged_projects.len(), 1);
        assert_eq!(tagged_projects[0].id, live_set.id);

        // Test untagging project
        db.untag_project(&live_set.id.to_string(), &tag1_id).expect("Failed to untag project");
        let project_tags = db.get_project_tags(&live_set.id.to_string()).expect("Failed to get project tags");
        assert_eq!(project_tags.len(), 1);
        assert!(project_tags.contains("favorite"));

        // Test removing tag
        db.remove_tag(&tag2_id).expect("Failed to remove tag");
        let tags = db.list_tags().expect("Failed to list tags");
        assert_eq!(tags.len(), 1);
        assert!(tags.iter().any(|(_, name)| name == "work-in-progress"));
    }
} 