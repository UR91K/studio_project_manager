//! The one switch that stops the daemon (ADR-0076).
//!
//! The tray's Quit and `POST /api/v1/system/shutdown` both fire it. The tray loop polls it,
//! and the HTTP server's graceful shutdown waits on it, so either way the tray icon is
//! removed by the tray loop ending and the server finishes the response it is sending.

use std::sync::Arc;

use tokio::sync::watch;

#[derive(Clone)]
pub struct ShutdownTrigger {
    tx: Arc<watch::Sender<bool>>,
}

impl ShutdownTrigger {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(watch::channel(false).0),
        }
    }

    /// Asks the daemon to stop. Safe to call more than once.
    pub fn trigger(&self) {
        self.tx.send_replace(true);
    }

    pub fn is_triggered(&self) -> bool {
        *self.tx.borrow()
    }

    /// Resolves once `trigger` has been called (immediately if it already was).
    pub async fn wait(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.wait_for(|stop| *stop).await;
    }
}

/// Serves `router` on `listener` until `shutdown` fires, then lets in-flight responses
/// finish. A stream (SSE) that never ends is cut off after a few seconds rather than
/// blocking the exit.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shutdown: ShutdownTrigger,
) -> std::io::Result<()> {
    let drained = axum::serve(listener, router).with_graceful_shutdown({
        let shutdown = shutdown.clone();
        async move { shutdown.wait().await }
    });
    tokio::select! {
        result = drained => result,
        _ = async {
            shutdown.wait().await;
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        } => Ok(()),
    }
}

impl Default for ShutdownTrigger {
    fn default() -> Self {
        Self::new()
    }
}
