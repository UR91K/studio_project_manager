//! Config changes reach the running daemon (ADR-0072).
//!
//! Every test shares the process-wide config cell, so they run one at a time, and each
//! starts by writing a known `config.toml` and reloading it. `SEULA_CONFIG` points at a
//! temporary file before anything touches the config: the save path writes wherever the
//! config was found, and on a developer's machine that is the real one.

use seula::config;
use seula::database::ProjectDatabase;
use seula::http::dto::config::ConfigDto;
use seula::media::{MediaConfig, MediaError, MediaStorageManager, MediaType};
use seula::services::{ConfigService, ScanStatus, SystemService};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const BASE_PORT: u16 = 50111;

struct Env {
    dir: PathBuf,
    config_file: PathBuf,
}

static ENV: OnceLock<Env> = OnceLock::new();
static ONE_AT_A_TIME: Mutex<()> = Mutex::const_new(());

fn toml_path(p: &Path) -> String {
    format!("'{}'", p.display())
}

/// The baseline file: no project paths, ports and folders under the scratch folder.
fn baseline_toml(env_dir: &Path) -> String {
    toml_with(env_dir, &[], BASE_PORT, "media", "info")
}

fn toml_with(dir: &Path, paths: &[&Path], port: u16, media: &str, level: &str) -> String {
    let paths: Vec<String> = paths.iter().map(|p| toml_path(p)).collect();
    format!(
        "paths = [{}]\nhttp_port = {}\nlog_level = '{}'\ndatabase_path = {}\nmedia_storage_dir = {}\nvst_search_paths = [{}]\n",
        paths.join(", "),
        port,
        level,
        toml_path(&dir.join("seula.db")),
        toml_path(&dir.join(media)),
        toml_path(&dir.join("no-plugins")),
    )
}

fn env() -> &'static Env {
    ENV.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        std::fs::create_dir_all(dir.join("no-plugins")).unwrap();
        let config_file = dir.join("config.toml");
        std::fs::write(&config_file, baseline_toml(&dir)).unwrap();
        std::env::set_var("SEULA_CONFIG", &config_file);
        std::env::remove_var("SEULA_HTTP_PORT");
        std::env::remove_var("SEULA_DATABASE_PATH");
        std::env::remove_var("SEULA_LOG_LEVEL");
        Env { dir, config_file }
    })
}

/// Takes the lock, writes the baseline file and loads it.
async fn fresh() -> (tokio::sync::MutexGuard<'static, ()>, &'static Env) {
    let guard = ONE_AT_A_TIME.lock().await;
    let env = env();
    std::fs::write(&env.config_file, baseline_toml(&env.dir)).unwrap();
    config::live::reload().expect("the baseline should load");
    (guard, env)
}

fn project_dir(env: &Env, name: &str) -> PathBuf {
    let dir = env.dir.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn until_watched(system: &SystemService, path: &Path, watched: bool) {
    let start = Instant::now();
    while system.watched_paths().await.iter().any(|p| p == path) != watched {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {} to be watched={watched}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn system_service(env: &Env) -> SystemService {
    let db = ProjectDatabase::new(env.dir.join("scan.db")).unwrap();
    SystemService::new(
        Arc::new(Mutex::new(db)),
        Arc::new(Mutex::new(ScanStatus::Unknown)),
        Arc::new(Mutex::new(None)),
        Arc::new(Mutex::new(None)),
        Arc::new(Mutex::new(None)),
        Instant::now(),
    )
}

/// Runs a scan through the service and returns every message it reported, joined.
async fn scan_message(system: &SystemService) -> String {
    let messages = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&messages);
    assert!(
        system
            .start_scan(move |p| sink.lock().unwrap().push(p.message))
            .await,
        "a scan should start"
    );
    let start = Instant::now();
    loop {
        let (status, _) = system.get_scan_status().await;
        if status.is_final() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "scan did not end"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let all = messages.lock().unwrap().join(" | ");
    all
}

#[tokio::test]
async fn an_api_path_change_reaches_the_next_scan_with_no_restart() {
    let (_guard, env) = fresh().await;
    let service = ConfigService::new();
    let system = system_service(env);

    assert!(scan_message(&system).await.contains("Setup required"));

    let folder = project_dir(env, "projects-api");
    let folder_str = folder.to_str().unwrap().to_string();
    service.add_path(folder_str.clone()).unwrap();
    assert_eq!(config::current().unwrap().paths, vec![folder_str.clone()]);
    assert!(scan_message(&system).await.contains("No projects found"));

    service.remove_path(&folder_str).unwrap();
    assert!(config::current().unwrap().paths.is_empty());
    assert!(scan_message(&system).await.contains("Setup required"));

    // The file agrees with what is in force.
    let on_disk = std::fs::read_to_string(&env.config_file).unwrap();
    assert!(!on_disk.contains("projects-api"), "{on_disk}");
}

#[tokio::test]
async fn a_file_edit_reaches_the_next_scan_with_no_restart() {
    let (_guard, env) = fresh().await;
    let _watch = config::live::watch_file().expect("the watcher should start");
    let system = system_service(env);
    assert!(scan_message(&system).await.contains("Setup required"));

    let folder = project_dir(env, "projects-edit");
    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[&folder], BASE_PORT, "media", "info"),
    )
    .unwrap();

    wait_for("the edit to be applied", || {
        !config::current().unwrap().paths.is_empty()
    })
    .await;
    assert!(scan_message(&system).await.contains("No projects found"));
}

#[tokio::test]
async fn the_daemons_own_save_does_not_reload_a_second_time() {
    let (_guard, env) = fresh().await;
    let _watch = config::live::watch_file().unwrap();
    let mut changes = config::subscribe().unwrap();
    changes.borrow_and_update();

    let folder = project_dir(env, "projects-self");
    ConfigService::new()
        .add_path(folder.to_str().unwrap().to_string())
        .unwrap();
    assert!(changes.has_changed().unwrap(), "the save itself replaces");
    changes.borrow_and_update();

    // Longer than the debounce plus a margin: the watcher has read the file by now.
    tokio::time::sleep(config::live::DEBOUNCE + Duration::from_millis(1200)).await;
    assert!(
        !changes.has_changed().unwrap(),
        "reading back our own save must not replace the config again"
    );
    assert_eq!(config::reload_error(), None);
}

#[tokio::test]
async fn a_bad_edit_keeps_the_old_config_and_reports_why() {
    let (_guard, env) = fresh().await;
    let folder = project_dir(env, "projects-bad");
    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[&folder], BASE_PORT, "media", "info"),
    )
    .unwrap();
    config::live::reload().unwrap();
    let before = config::current().unwrap();

    // Not TOML at all.
    std::fs::write(&env.config_file, "paths = [[[ not toml").unwrap();
    assert!(config::live::reload().is_err());
    assert!(Arc::ptr_eq(&before, &config::current().unwrap()));
    let reason = config::reload_error().expect("the reason should be kept");
    assert!(!reason.is_empty());
    assert_eq!(
        ConfigService::new().reload_error().as_deref(),
        Some(reason.as_str())
    );

    // Parses, but does not validate.
    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[&folder], BASE_PORT, "media", "shouting"),
    )
    .unwrap();
    assert!(config::live::reload().is_err());
    assert_eq!(config::current().unwrap().log_level, "info");
    assert!(config::reload_error().is_some());

    // A good edit clears it.
    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[&folder], BASE_PORT, "media", "warn"),
    )
    .unwrap();
    config::live::reload().unwrap();
    assert_eq!(config::reload_error(), None);
    assert_eq!(config::current().unwrap().log_level, "warn");
}

#[tokio::test]
async fn a_bad_edit_through_the_watcher_is_reported_and_a_missing_file_ignored() {
    let (_guard, env) = fresh().await;
    let _watch = config::live::watch_file().unwrap();

    std::fs::write(&env.config_file, "this is = = not toml").unwrap();
    wait_for("the error to be reported", || {
        config::reload_error().is_some()
    })
    .await;
    assert!(config::current().unwrap().paths.is_empty());

    // The file vanishing, as mid-rename, changes nothing and reports nothing new.
    std::fs::remove_file(&env.config_file).unwrap();
    tokio::time::sleep(config::live::DEBOUNCE + Duration::from_millis(800)).await;
    assert!(config::current().unwrap().paths.is_empty());

    std::fs::write(&env.config_file, baseline_toml(&env.dir)).unwrap();
    wait_for("the error to clear", || config::reload_error().is_none()).await;
}

#[tokio::test]
async fn keys_that_need_a_restart_are_listed_and_live_ones_are_not() {
    let (_guard, env) = fresh().await;
    assert!(config::restart_required().is_empty());

    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[], BASE_PORT + 1, "media-elsewhere", "debug"),
    )
    .unwrap();
    config::live::reload().unwrap();

    let keys = ConfigService::new().restart_required();
    assert_eq!(keys, vec!["http_port", "media_storage_dir"]);
    // The live key applied; the others are in the file, not in effect.
    assert_eq!(config::current().unwrap().log_level, "debug");

    std::fs::write(&env.config_file, baseline_toml(&env.dir)).unwrap();
    config::live::reload().unwrap();
    assert!(config::restart_required().is_empty());
}

#[tokio::test]
async fn the_log_level_changes_while_the_daemon_runs() {
    let (_guard, _env) = fresh().await;
    let _log = seula::logging::init("info", false);
    assert_eq!(
        seula::logging::current_max_level(),
        Some(tracing::level_filters::LevelFilter::INFO)
    );

    ConfigService::new()
        .update_settings(None, Some("debug".into()), None, None, None, None, None)
        .unwrap();
    assert_eq!(
        seula::logging::current_max_level(),
        Some(tracing::level_filters::LevelFilter::DEBUG)
    );

    // And from a file edit.
    let env = env();
    std::fs::write(
        &env.config_file,
        toml_with(&env.dir, &[], BASE_PORT, "media", "error"),
    )
    .unwrap();
    config::live::reload().unwrap();
    assert_eq!(
        seula::logging::current_max_level(),
        Some(tracing::level_filters::LevelFilter::ERROR)
    );
}

#[tokio::test]
async fn the_plugin_settings_and_media_limits_are_live() {
    let (_guard, env) = fresh().await;
    let service = ConfigService::new();
    service
        .update_settings(
            None,
            None,
            None,
            Some(Some(1)),
            None,
            Some(vec!["/some/vst".into()]),
            Some(7),
        )
        .unwrap();
    let now = config::current().unwrap();
    assert_eq!(now.vst_search_paths, vec!["/some/vst".to_string()]);
    assert_eq!(now.vst_scan_timeout_secs, 7);
    let dto = serde_json::to_value(ConfigDto::from(&*now)).unwrap();
    assert_eq!(dto["vst_search_paths"][0], "/some/vst");
    assert_eq!(dto["vst_scan_timeout_secs"], 7);
    assert_eq!(dto["http_port"], BASE_PORT);

    let media =
        MediaStorageManager::new_following_config(env.dir.join("media"), MediaConfig::from(&*now))
            .unwrap();
    let two_mb = vec![0u8; 2 * 1024 * 1024];
    let too_large = |r: Result<_, MediaError>| matches!(r, Err(MediaError::FileTooLarge { .. }));
    assert!(too_large(media.store_file(
        &two_mb,
        "a.png",
        MediaType::CoverArt
    )));

    // Raise the limit: the same manager, no restart, accepts the size now.
    service
        .update_settings(None, None, None, Some(Some(0)), None, None, None)
        .unwrap();
    assert!(!too_large(media.store_file(
        &two_mb,
        "a.png",
        MediaType::CoverArt
    )));

    // A manager built the old way keeps the limits it was given.
    let fixed = MediaStorageManager::new(
        env.dir.join("media"),
        MediaConfig {
            max_cover_art_size_mb: Some(1),
            max_audio_file_size_mb: None,
        },
    )
    .unwrap();
    assert!(too_large(fixed.store_file(
        &two_mb,
        "a.png",
        MediaType::CoverArt
    )));
}

#[tokio::test]
async fn a_running_folder_watcher_follows_paths_and_a_stopped_one_stays_off() {
    let (_guard, env) = fresh().await;
    let system = system_service(env);
    system.follow_config_paths();
    let service = ConfigService::new();

    // Not running: a change starts nothing.
    let first = project_dir(env, "watch-first");
    service.add_path(first.to_str().unwrap().into()).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(system.watched_paths().await.is_empty());

    assert!(system.start_watcher().await.unwrap());
    assert_eq!(system.watched_paths().await, vec![first.clone()]);

    let second = project_dir(env, "watch-second");
    service.add_path(second.to_str().unwrap().into()).unwrap();
    until_watched(&system, &second, true).await;

    service.remove_path(first.to_str().unwrap()).unwrap();
    until_watched(&system, &first, false).await;
    assert_eq!(system.watched_paths().await, vec![second]);
    system.stop_watcher().await;
}
