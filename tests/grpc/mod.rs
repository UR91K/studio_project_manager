//! gRPC server tests
//!
//! This module contains all tests for the gRPC server functionality
//! Previously located in src/grpc/server.rs tests module (lines ~795-1630)

pub mod batch;
pub mod collections;
pub mod media;
pub mod projects;
pub mod search;
pub mod server_setup;
pub mod stats;
pub mod tags;

// Common imports for gRPC tests
use studio_project_manager::database::LiveSetDatabase;
use studio_project_manager::grpc::proto::*;
// use crate::common;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request};
use uuid::Uuid;

// Re-export commonly used test utilities
pub use server_setup::*;
