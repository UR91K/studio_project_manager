//! Database batch insert tests

use super::*;
use crate::common::{generate_test_live_sets_arc, setup};
use std::collections::HashSet;
use studio_project_manager::database::batch::BatchInsertManager;
use tempfile::tempdir;

#[test]
fn test_batch_insert() {
    setup("error");
    // Create a temporary database
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    // Initialize database with schema from LiveSetDatabase
    let mut live_set_db = LiveSetDatabase::new(db_path.clone()).expect("Failed to create database");

    // Get connection for batch insert
    let mut conn = &mut live_set_db.conn;

    // Generate test data
    let test_sets = generate_test_live_sets_arc(3);
    let expected_projects = test_sets.len();
    let expected_plugins: usize = test_sets
        .iter()
        .flat_map(|ls| &ls.plugins)
        .map(|p| &p.dev_identifier)
        .collect::<HashSet<_>>()
        .len();
    let expected_samples: usize = test_sets
        .iter()
        .flat_map(|ls| &ls.samples)
        .map(|s| s.path.to_string_lossy().to_string())
        .collect::<HashSet<_>>()
        .len();

    // Execute batch insert
    let mut batch_manager = BatchInsertManager::new(&mut conn, test_sets.clone());
    let stats = batch_manager.execute().expect("Batch insert failed");

    // Verify stats
    assert_eq!(
        stats.projects_inserted, expected_projects,
        "Should insert all projects"
    );
    assert_eq!(
        stats.plugins_inserted, expected_plugins,
        "Should insert unique plugins"
    );
    assert_eq!(
        stats.samples_inserted, expected_samples,
        "Should insert unique samples"
    );

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
