//! gRPC server tests
//!
//! This module contains all tests for the gRPC server functionality
//! Previously located in src/grpc/server.rs tests module (lines ~795-1630)

pub mod batch;
pub mod collections;
pub mod media;
pub mod plugins;
pub mod projects;
pub mod search;
pub mod server_setup;
pub mod stats;
pub mod tags;

// Common imports for gRPC tests
use studio_project_manager::database::LiveSetDatabase;
use studio_project_manager::grpc::projects::*;
use studio_project_manager::grpc::collections::*;
use studio_project_manager::grpc::tags::*;
use studio_project_manager::grpc::search::*;
use studio_project_manager::grpc::media::*;
use studio_project_manager::grpc::system::*;
use studio_project_manager::grpc::plugins::*;

// Import all service traits
use studio_project_manager::grpc::collections::collection_service_server::CollectionService;
use studio_project_manager::grpc::tags::tag_service_server::TagService;
use studio_project_manager::grpc::scanning::scanning_service_server::ScanningService;
use studio_project_manager::grpc::plugins::plugin_service_server::PluginService;
// use crate::common;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request};
use uuid::Uuid;

// Re-export commonly used test utilities
pub use server_setup::*;
