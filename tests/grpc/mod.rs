//! gRPC server tests
//!
//! This module contains all tests for the gRPC server functionality
//! Previously located in src/grpc/server.rs tests module (lines ~795-1630)

pub mod projects;
pub mod collections;
pub mod tags;
pub mod search;
pub mod server_setup;

// Common imports for gRPC tests
use studio_project_manager::grpc::proto::*;
use studio_project_manager::grpc::server::StudioProjectManagerServer;
use studio_project_manager::database::LiveSetDatabase;
use studio_project_manager::test_utils;  // Will be updated to use crate::common
use tonic::{Request, Response, Status, Code};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;

// Re-export commonly used test utilities
pub use server_setup::*; 