//! HTTP adapter tests. See `tests/http_tests.rs`.

pub mod adapters;
pub mod daemon;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use seula::http::server::build_router;
use seula::http::state::AppState;
use tower::ServiceExt;

use crate::common::fixtures::{test_env, TestEnv};

/// A router over a fresh in-memory library, plus the environment behind it for
/// arranging and checking state directly.
pub fn app() -> (Router, TestEnv) {
    let env = test_env();
    let state = AppState::new(env.services.clone(), env.system.clone());
    (build_router(state), env)
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| {
            panic!(
                "body is not JSON ({e}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }
}

pub async fn send(app: &Router, method: Method, uri: &str) -> Reply {
    send_with(app, method, uri, Body::empty()).await
}

pub async fn send_json(app: &Router, method: Method, uri: &str, body: serde_json::Value) -> Reply {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    reply(app, request).await
}

pub async fn send_with(app: &Router, method: Method, uri: &str, body: Body) -> Reply {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .unwrap();
    reply(app, request).await
}

async fn reply(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.unwrap();

    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    Reply {
        status,
        headers,
        body: bytes,
    }
}
