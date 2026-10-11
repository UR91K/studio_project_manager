//! Shared state for the HTTP router (ADR-0024).
//!
//! `AppState` bundles `Services` with `SystemService` because `SystemService` is
//! not a field of the `Services` aggregator -- it is constructed separately in
//! `src/main.rs`. See ADR-0024's Consequences section: folding `SystemService` into
//! `Services` is a service-layer change, not an HTTP one (ADR-0046 does it).

use super::shutdown::ShutdownTrigger;
use crate::services::{Services, SystemService};

#[derive(Clone)]
pub struct AppState {
    pub services: Services,
    pub system: SystemService,
    /// Fired by `POST /system/shutdown`; the same switch the tray's Quit uses.
    pub shutdown: ShutdownTrigger,
}

impl AppState {
    pub fn new(services: Services, system: SystemService) -> Self {
        Self {
            services,
            system,
            shutdown: ShutdownTrigger::new(),
        }
    }

    /// Shares `shutdown` with whoever else can stop the daemon (the tray).
    pub fn with_shutdown(mut self, shutdown: ShutdownTrigger) -> Self {
        self.shutdown = shutdown;
        self
    }
}
