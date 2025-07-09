use crate::error::DatabaseError;
use chrono::{DateTime, Local, TimeZone};
use log::{debug, info};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

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
            r#"--sql
            -- Core tables
            CREATE TABLE IF NOT EXISTS projects (
                is_active BOOLEAN NOT NULL DEFAULT true,

                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                hash TEXT NOT NULL,
                notes TEXT,
                created_at DATETIME NOT NULL,
                modified_at DATETIME NOT NULL,
                last_parsed_at DATETIME NOT NULL,
                
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
                ableton_version_beta BOOLEAN NOT NULL,
                audio_file_id TEXT,
                FOREIGN KEY (audio_file_id) REFERENCES media_files(id) ON DELETE SET NULL
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

            CREATE TABLE IF NOT EXISTS media_files (
                id TEXT PRIMARY KEY,
                original_filename TEXT NOT NULL,
                file_extension TEXT NOT NULL,
                media_type TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                uploaded_at DATETIME NOT NULL,
                checksum TEXT NOT NULL
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
                modified_at DATETIME NOT NULL,
                cover_art_id TEXT,
                FOREIGN KEY (cover_art_id) REFERENCES media_files(id) ON DELETE SET NULL
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
            CREATE INDEX IF NOT EXISTS idx_projects_is_active ON projects(is_active);
            CREATE INDEX IF NOT EXISTS idx_media_files_type ON media_files(media_type);

            -- Full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS project_search USING fts5(
                project_id UNINDEXED,  -- Reference to projects table
                name,                  -- Project name
                path,                 -- Project path
                plugins,              -- Plugin list
                samples,              -- Sample list
                tags,                 -- Tags list
                notes,                -- Project notes
                created_at,           -- Creation timestamp
                modified_at,          -- Modification timestamp
                tempo,                -- Project tempo
                tokenize='porter unicode61'
            );

            -- FTS5 triggers for maintaining the search index
            CREATE TRIGGER IF NOT EXISTS projects_au AFTER UPDATE ON projects BEGIN
                DELETE FROM project_search WHERE project_id = old.id;
                INSERT INTO project_search (
                    project_id, name, path, plugins, samples, tags, notes, created_at, modified_at, tempo
                )
                SELECT 
                    p.id,
                    p.name,
                    p.path,
                    COALESCE((SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                     FROM plugins pl
                     JOIN project_plugins pp ON pp.plugin_id = pl.id
                     WHERE pp.project_id = p.id), ''),
                    COALESCE((SELECT GROUP_CONCAT(s.name, ' ')
                     FROM samples s
                     JOIN project_samples ps ON ps.sample_id = s.id
                     WHERE ps.project_id = p.id), ''),
                    COALESCE((SELECT GROUP_CONCAT(t.name, ' ')
                     FROM tags t
                     JOIN project_tags pt ON pt.tag_id = t.id
                     WHERE pt.project_id = p.id), ''),
                    COALESCE(p.notes, ''),
                    strftime('%Y-%m-%d %H:%M:%S', datetime(p.created_at, 'unixepoch')),
                    strftime('%Y-%m-%d %H:%M:%S', datetime(p.modified_at, 'unixepoch')),
                    CAST(p.tempo AS TEXT)
                FROM projects p
                WHERE p.id = new.id;
            END;

            CREATE TRIGGER IF NOT EXISTS projects_ad AFTER DELETE ON projects BEGIN
                DELETE FROM project_search WHERE project_id = old.id;
            END;

            -- Update FTS index after project insert (done manually to ensure all relations are set)
            CREATE TRIGGER IF NOT EXISTS projects_ai AFTER INSERT ON projects BEGIN
                INSERT INTO project_search (
                    project_id, name, path, plugins, samples, tags, notes, created_at, modified_at, tempo
                )
                SELECT 
                    p.id,
                    p.name,
                    p.path,
                    '',  -- Empty plugins (will be updated after linking)
                    '',  -- Empty samples (will be updated after linking)
                    '',  -- Empty tags (will be updated after linking)
                    COALESCE(p.notes, ''),
                    strftime('%Y-%m-%d %H:%M:%S', datetime(p.created_at, 'unixepoch')),
                    strftime('%Y-%m-%d %H:%M:%S', datetime(p.modified_at, 'unixepoch')),
                    CAST(p.tempo AS TEXT)
                FROM projects p
                WHERE p.id = new.id;
            END;
            "#,
        )?;

        debug!("Database schema initialized successfully");
        
        // Rebuild FTS5 table to fix any NULL values in existing data
        self.rebuild_fts5_table()?;
        
        Ok(())
    }

    pub fn get_last_scanned_time(&self, path: &Path) -> Result<Option<DateTime<Local>>, DatabaseError> {
        let path_str = path.to_string_lossy().to_string();
        
        let last_parsed: Option<i64> = self.conn
            .query_row(
                "SELECT last_parsed_at FROM projects WHERE path = ? AND is_active = true",
                params![path_str],
                |row| row.get(0)
            )
            .optional()?;
            
        Ok(last_parsed.map(|timestamp| {
            Local.timestamp_opt(timestamp, 0)
                .single()
                .expect("Invalid timestamp in database")
        }))
    }

    pub fn rebuild_fts5_table(&mut self) -> Result<(), DatabaseError> {
        debug!("Rebuilding FTS5 table to fix NULL values");
        
        // Clear and rebuild the FTS5 table
        self.conn.execute("DELETE FROM project_search", [])?;
        
        // Repopulate with corrected data
        self.conn.execute(
            r#"
            INSERT INTO project_search (
                project_id, name, path, plugins, samples, tags, notes, created_at, modified_at, tempo
            )
            SELECT 
                p.id,
                p.name,
                p.path,
                COALESCE((SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                 FROM plugins pl
                 JOIN project_plugins pp ON pp.plugin_id = pl.id
                 WHERE pp.project_id = p.id), ''),
                COALESCE((SELECT GROUP_CONCAT(s.name, ' ')
                 FROM samples s
                 JOIN project_samples ps ON ps.sample_id = s.id
                 WHERE ps.project_id = p.id), ''),
                COALESCE((SELECT GROUP_CONCAT(t.name, ' ')
                 FROM tags t
                 JOIN project_tags pt ON pt.tag_id = t.id
                 WHERE pt.project_id = p.id), ''),
                COALESCE(p.notes, ''),
                strftime('%Y-%m-%d %H:%M:%S', datetime(p.created_at, 'unixepoch')),
                strftime('%Y-%m-%d %H:%M:%S', datetime(p.modified_at, 'unixepoch')),
                CAST(p.tempo AS TEXT)
            FROM projects p
            WHERE p.is_active = true
            "#,
            [],
        )?;
        
        debug!("FTS5 table rebuilt successfully");
        Ok(())
    }
}