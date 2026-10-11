//! HTTP router adapter over the service layer (ADR-0024). A caller of
//! `src/services/`, alongside the CLI, built to the "thin handler" rule: parse the
//! request, call one `Services` method, convert the result.

pub mod dto;
pub mod error;
pub mod handlers;
pub mod server;
pub mod shutdown;
pub mod state;
