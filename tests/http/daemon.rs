//! The daemon controls and the paths it reports (ADR-0076).

use std::time::Duration;

use axum::http::{Method, StatusCode};
use seula::http::server::build_router;
use seula::http::shutdown::{serve, ShutdownTrigger};
use seula::http::state::AppState;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{app, send};
use crate::common::fixtures::test_env;

/// Points `SEULA_CONFIG` at a throwaway file; the real one is the maintainer's.
fn temp_config() -> std::path::PathBuf {
    let dir = tempfile::tempdir().unwrap().keep();
    let file = dir.join("config.toml");
    std::fs::write(
        &file,
        format!(
            "paths = []\nhttp_port = 50994\nlog_level = 'info'\ndatabase_path = '{}'\nmedia_storage_dir = '{}'\nvst_search_paths = []\n",
            dir.join("seula.db").display(),
            dir.join("media").display()
        ),
    )
    .unwrap();
    std::env::set_var("SEULA_CONFIG", &file);
    file
}

#[tokio::test]
async fn system_info_reports_the_config_and_log_paths() {
    let file = temp_config();
    let (app, _env) = app();
    let reply = send(&app, Method::GET, "/api/v1/system/info").await;
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(
        std::path::PathBuf::from(body["config_path"].as_str().unwrap()),
        file
    );
    let log_dir = seula::logging::log_dir().unwrap();
    assert_eq!(
        body["log_dir"].as_str().unwrap(),
        log_dir.display().to_string()
    );
}

#[tokio::test]
async fn shutdown_answers_202_and_then_the_server_stops() {
    let env = test_env();
    let shutdown = ShutdownTrigger::new();
    let state =
        AppState::new(env.services.clone(), env.system.clone()).with_shutdown(shutdown.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(serve(listener, build_router(state), shutdown.clone()));

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            b"POST /api/v1/system/shutdown HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).await.unwrap();
    assert!(raw.starts_with("HTTP/1.1 202"), "got: {raw}");

    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("the server should stop after the shutdown route")
        .unwrap()
        .unwrap();
    assert!(shutdown.is_triggered());
    assert!(
        TcpStream::connect(addr).await.is_err(),
        "the port should be closed"
    );
}
