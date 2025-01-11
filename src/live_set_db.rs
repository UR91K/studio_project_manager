use rusqlite::{params, Connection, Result as SqliteResult, types::ToSql, OptionalExtension};
use std::path::PathBuf;
use chrono::{DateTime, Local, TimeZone};
use uuid::Uuid;
use std::collections::HashSet;
use std::str::FromStr;
use log::{debug, info, warn};
use crate::error::DatabaseError;
use crate::models::{Plugin, Sample, PluginFormat, Scale, Tonic, KeySignature, TimeSignature, AbletonVersion, Id};
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
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                installed BOOLEAN NOT NULL,
                UNIQUE(name, format)
            );

            -- Sample catalog
            CREATE TABLE IF NOT EXISTS samples (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                is_present BOOLEAN NOT NULL
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
            "#,
        )?;

        debug!("Database schema initialized successfully");
        Ok(())
    }

    pub fn insert_project(&mut self, live_set: &LiveSet) -> Result<(), DatabaseError> {
        debug!("Inserting project: {} ({})", live_set.file_name, live_set.file_path.display());
        let tx = self.conn.transaction()?;
        
        // Generate UUIDs for new entries
        let project_id = Uuid::new_v4().to_string();
        debug!("Generated project UUID: {}", project_id);

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
            "INSERT OR IGNORE INTO plugins (id, name, format, installed) VALUES (?, ?, ?, ?)",
            params![
                id,
                plugin.name,
                format!("{:?}", plugin.plugin_format),
                plugin.installed,
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

    pub fn get_project(&mut self, id: &str) -> Result<Option<LiveSet>, DatabaseError> {
        // TODO: Implement
        Ok(None)
    }

    pub fn search(&mut self, query: &str) -> Result<Vec<LiveSet>, DatabaseError> {
        debug!("Performing search with query: {}", query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT p.* 
            FROM projects p
            LEFT JOIN project_plugins pp ON pp.project_id = p.id
            LEFT JOIN plugins pl ON pl.id = pp.plugin_id
            LEFT JOIN project_samples ps ON ps.project_id = p.id
            LEFT JOIN samples s ON s.id = ps.sample_id
            WHERE 
                p.name LIKE ?1 OR
                pl.name LIKE ?1 OR
                s.name LIKE ?1
            "#,
        )?;

        let pattern = format!("%{}%", query);
        debug!("Using search pattern: {}", pattern);

        let projects = stmt
            .query_map([pattern], |_row| {
                warn!("Search result mapping not yet implemented");
                Err(rusqlite::Error::InvalidColumnType(
                    0,
                    "Not implemented".to_string(),
                    rusqlite::types::Type::Null,
                ))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        debug!("Found {} matching projects", projects.len());
        Ok(projects)
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
                id: Id::default(),
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
            let name: String = row.get(1)?;
            debug!("Found plugin: {}", name);
            Ok(Plugin {
                name,
                plugin_format: row.get::<_, String>(2)?.parse()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                installed: row.get(3)?,
                plugin_id: None,
                module_id: None,
                dev_identifier: String::new(),
                vendor: None,
                version: None,
                sdk_version: None,
                flags: None,
                scanstate: None,
                enabled: None,
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
                id: Id::default(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use chrono::Local;
    use std::collections::HashSet;
    
    static INIT: Once = Once::new();
    fn setup() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "debug");
            env_logger::builder()
                .is_test(true)
                .filter_level(log::LevelFilter::Debug)
                .try_init()
                .expect("Failed to initialize logger");
        });
    }

    fn create_test_live_set() -> LiveSet {
        let now = Local::now();
        let mut plugins = HashSet::new();
        let mut samples = HashSet::new();

        // Add a test plugin
        plugins.insert(Plugin {
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
            id: Id::default(),
            name: "test_sample.wav".to_string(),
            path: PathBuf::from("C:/test/test_sample.wav"),
            is_present: true,
        });

        LiveSet {
            id: Id::default(),
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
        }
    }

    #[test]
    fn test_database_initialization() {
        setup();
        let mut db = LiveSetDatabase::new(PathBuf::from(":memory:")).expect("Failed to create database");
        
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
} 