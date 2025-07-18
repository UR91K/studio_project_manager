//! gRPC server tests
//!
//! This module contains all tests for the gRPC server functionality
//! Previously located in src/grpc/server.rs tests module (lines ~795-1630)

pub mod projects;
pub mod collections;
pub mod tags;
pub mod search;
pub mod server_setup;
pub mod media;
pub mod stats;
pub mod batch;

// Common imports for gRPC tests
use studio_project_manager::grpc::proto::*;
use studio_project_manager::database::LiveSetDatabase;
// use crate::common;
use tonic::{Request, Code};
use tokio::sync::Mutex;
use std::sync::Arc;
use uuid::Uuid;

// Re-export commonly used test utilities
pub use server_setup::*; 

