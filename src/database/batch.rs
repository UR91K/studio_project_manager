use std::collections::HashMap;
use std::sync::Arc;
use log::{debug, info};
use rusqlite::{params, Connection, Transaction};


use super::models::SqlDateTime;
use crate::error::DatabaseError;
use crate::live_set::LiveSet;
use crate::models::{Plugin, Sample};

struct BatchTransaction<'a> {
    tx: Transaction<'a>,
    unique_plugins: HashMap<String, Plugin>,
    unique_samples: HashMap<String, Sample>,
    stats: BatchStats,
}

impl<'a> BatchTransaction<'a> {
    fn new(conn: &'a mut Connection) -> Result<Self, DatabaseError> {
        Ok(Self {
            tx: conn.transaction()?,
            unique_plugins: HashMap::new(),
            unique_samples: HashMap::new(),
            stats: BatchStats::default(),
        })
    }

    fn collect_items(&mut self, live_sets: &[LiveSet]) {
        for live_set in live_sets {
            // Collect unique plugins
            for plugin in &live_set.plugins {
                self.unique_plugins
                    .entry(plugin.dev_identifier.clone())
                    .or_insert_with(|| plugin.clone());
            }
            
            // Collect unique samples
            for sample in &live_set.samples {
                self.unique_samples
                    .entry(sample.path.to_string_lossy().to_string())
                    .or_insert_with(|| sample.clone());
            }
        }
        
        debug!(
            "Found {} unique plugins and {} unique samples",
            self.unique_plugins.len(),
            self.unique_samples.len()
        );
    }

    fn insert_plugins(&mut self) -> Result<(), DatabaseError> {
        debug!("Inserting {} unique plugins", self.unique_plugins.len());
        
        for plugin in self.unique_plugins.values() {
            let plugin_id = plugin.id.to_string();
            self.tx.execute(
                "INSERT OR IGNORE INTO plugins (
                    id, name, dev_identifier, format,
                    vendor, version, sdk_version, flags,
                    scanstate, enabled, installed
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    plugin_id,
                    plugin.name,
                    plugin.dev_identifier,
                    plugin.plugin_format.to_string(),
                    plugin.vendor,
                    plugin.version,
                    plugin.sdk_version,
                    plugin.flags,
                    plugin.scanstate,
                    plugin.enabled,
                    plugin.installed,
                ],
            )?;
            self.stats.plugins_inserted += 1;
        }
        Ok(())
    }

    fn insert_samples(&mut self) -> Result<(), DatabaseError> {
        debug!("Inserting {} unique samples", self.unique_samples.len());
        
        for sample in self.unique_samples.values() {
            let sample_id = sample.id.to_string();
            self.tx.execute(
                "INSERT OR IGNORE INTO samples (
                    id, name, path, is_present
                ) VALUES (?, ?, ?, ?)",
                params![
                    sample_id,
                    sample.name,
                    sample.path.to_string_lossy().to_string(),
                    sample.is_present,
                ],
            )?;
            self.stats.samples_inserted += 1;
        }
        Ok(())
    }

    fn insert_projects(&mut self, live_sets: &[LiveSet]) -> Result<(), DatabaseError> {
        for live_set in live_sets {
            let project_id = live_set.id.to_string();
            
            // Insert project
            self.tx.execute(
                "INSERT OR REPLACE INTO projects (
                    id, name, path, hash, created_at, modified_at,
                    last_parsed_at, tempo, time_signature_numerator,
                    time_signature_denominator, key_signature_tonic,
                    key_signature_scale, furthest_bar, duration_seconds,
                    ableton_version_major, ableton_version_minor,
                    ableton_version_patch, ableton_version_beta,
                    notes
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    project_id,
                    live_set.name,
                    live_set.file_path.to_string_lossy().to_string(),
                    live_set.file_hash,
                    SqlDateTime::from(live_set.created_time),
                    SqlDateTime::from(live_set.modified_time),
                    SqlDateTime::from(live_set.last_parsed_timestamp),
                    live_set.tempo,
                    live_set.time_signature.numerator,
                    live_set.time_signature.denominator,
                    live_set.key_signature.as_ref().map(|k| k.tonic.to_string()),
                    live_set.key_signature.as_ref().map(|k| k.scale.to_string()),
                    live_set.furthest_bar,
                    live_set.estimated_duration.map(|d| d.num_seconds()),
                    live_set.ableton_version.major,
                    live_set.ableton_version.minor,
                    live_set.ableton_version.patch,
                    live_set.ableton_version.beta,
                    None::<String>,
                ],
            )?;
            
            // Link plugins
            for plugin in &live_set.plugins {
                self.tx.execute(
                    "INSERT OR IGNORE INTO project_plugins (project_id, plugin_id)
                     VALUES (?, ?)",
                    params![project_id, plugin.id.to_string()],
                )?;
            }
            
            // Link samples
            for sample in &live_set.samples {
                self.tx.execute(
                    "INSERT OR IGNORE INTO project_samples (project_id, sample_id)
                     VALUES (?, ?)",
                    params![project_id, sample.id.to_string()],
                )?;
            }
            
            self.stats.projects_inserted += 1;
        }
        Ok(())
    }

    fn update_search_indexes(&self, live_sets: &[LiveSet]) -> Result<(), DatabaseError> {
        debug!("Updating search indexes for {} projects", live_sets.len());
        
        for live_set in live_sets {
            let project_id = live_set.id.to_string();
            
            self.tx.execute(
                "UPDATE project_search SET
                    plugins = (
                        SELECT GROUP_CONCAT(pl.name || ' ' || COALESCE(pl.vendor, ''), ' ')
                        FROM plugins pl
                        JOIN project_plugins pp ON pp.plugin_id = pl.id
                        WHERE pp.project_id = ?
                    ),
                    samples = (
                        SELECT GROUP_CONCAT(s.name, ' ')
                        FROM samples s
                        JOIN project_samples ps ON ps.sample_id = s.id
                        WHERE ps.project_id = ?
                    ),
                    tags = (
                        SELECT GROUP_CONCAT(t.name, ' ')
                        FROM tags t
                        JOIN project_tags pt ON pt.tag_id = t.id
                        WHERE pt.project_id = ?
                    )
                WHERE project_id = ?",
                params![project_id, project_id, project_id, project_id],
            )?;
        }
        Ok(())
    }

    fn commit(self) -> Result<BatchStats, DatabaseError> {
        self.tx.commit()?;
        Ok(self.stats)
    }
}

/// Manages batch insertion of LiveSets into the database
pub struct BatchInsertManager<'a> {
    conn: &'a mut Connection,
    live_sets: Arc<Vec<LiveSet>>,
}

impl<'a> BatchInsertManager<'a> {
    pub fn new(conn: &'a mut Connection, live_sets: Arc<Vec<LiveSet>>) -> Self {
        Self {
            conn,
            live_sets,
        }
    }

    /// Execute the batch insert operation
    pub fn execute(&mut self) -> Result<BatchStats, DatabaseError> {
        debug!("Starting batch insert of {} projects", self.live_sets.len());
        
        // Create transaction and execute all operations
        let mut batch = BatchTransaction::new(self.conn)?;
        
        // Collect all unique items
        batch.collect_items(&self.live_sets);
        
        // Insert all items
        batch.insert_plugins()?;
        batch.insert_samples()?;
        batch.insert_projects(&self.live_sets)?;
        batch.update_search_indexes(&self.live_sets)?;
        
        // Commit and get stats
        let stats = batch.commit()?;
        
        info!(
            "Batch insert complete: {} projects, {} plugins, {} samples",
            stats.projects_inserted,
            stats.plugins_inserted,
            stats.samples_inserted
        );
        
        Ok(stats)
    }
}

#[derive(Debug, Default)]
pub struct BatchStats {
    pub projects_inserted: usize,
    pub plugins_inserted: usize,
    pub samples_inserted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::test_utils::generate_test_live_sets_arc;
    use std::sync::Once;
    use crate::database::LiveSetDatabase;
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

    #[test]
    fn test_batch_insert() {
        setup();
        // Create a temporary database
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        
        // Initialize database with schema from LiveSetDatabase
        let mut live_set_db = LiveSetDatabase::new(db_path.clone())
            .expect("Failed to create database");
        
        // Get connection for batch insert
        let mut conn = &mut live_set_db.conn;

        // Generate test data
        let test_sets = generate_test_live_sets_arc(3);
        let expected_projects = test_sets.len();
        let expected_plugins: usize = test_sets.iter()
            .flat_map(|ls| &ls.plugins)
            .map(|p| &p.dev_identifier)
            .collect::<HashSet<_>>()
            .len();
        let expected_samples: usize = test_sets.iter()
            .flat_map(|ls| &ls.samples)
            .map(|s| s.path.to_string_lossy().to_string())
            .collect::<HashSet<_>>()
            .len();

        // Execute batch insert
        let mut batch_manager = BatchInsertManager::new(&mut conn, test_sets.clone());
        let stats = batch_manager.execute().expect("Batch insert failed");

        // Verify stats
        assert_eq!(stats.projects_inserted, expected_projects, "Should insert all projects");
        assert_eq!(stats.plugins_inserted, expected_plugins, "Should insert unique plugins");
        assert_eq!(stats.samples_inserted, expected_samples, "Should insert unique samples");

        // Verify database contents
        let project_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .expect("Failed to count projects");
        assert_eq!(project_count as usize, expected_projects);

        let plugin_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM plugins", [], |row| row.get(0))
            .expect("Failed to count plugins");
        assert_eq!(plugin_count as usize, expected_plugins);

        let sample_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples", [], |row| row.get(0))
            .expect("Failed to count samples");
        assert_eq!(sample_count as usize, expected_samples);

        // Verify relationships
        for live_set in test_sets.iter() {
            let project_id = live_set.id.to_string();
            
            // Check plugins
            let plugin_links: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM project_plugins WHERE project_id = ?",
                    [&project_id],
                    |row| row.get(0),
                )
                .expect("Failed to count plugin links");
            assert_eq!(plugin_links as usize, live_set.plugins.len());

            // Check samples
            let sample_links: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM project_samples WHERE project_id = ?",
                    [&project_id],
                    |row| row.get(0),
                )
                .expect("Failed to count sample links");
            assert_eq!(sample_links as usize, live_set.samples.len());
        }
    }
} 