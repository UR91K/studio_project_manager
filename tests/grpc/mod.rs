//! gRPC server tests
//!
//! This module contains all tests for the gRPC server functionality
//! Previously located in src/grpc/server.rs tests module (lines ~795-1630)

pub mod batch;
pub mod collections;
pub mod media;
pub mod plugins;
pub mod projects;
pub mod samples;
pub mod scanning;
pub mod search;
pub mod server_setup;
pub mod stats;
pub mod tags;
pub mod tasks;

// Common imports for gRPC tests
use seula::database::LiveSetDatabase;
use seula::grpc::projects::*;
use seula::grpc::collections::*;
use seula::grpc::tags::*;
use seula::grpc::search::*;
use seula::grpc::media::*;
use seula::grpc::system::*;
use seula::grpc::plugins::*;
use seula::grpc::samples::*;
use seula::grpc::scanning::*;

// Import all service traits
use seula::grpc::collections::collection_service_server::CollectionService;
use seula::grpc::tags::tag_service_server::TagService;
use seula::grpc::scanning::scanning_service_server::ScanningService;
use seula::grpc::plugins::plugin_service_server::PluginService;
use seula::grpc::samples::sample_service_server::SampleService;
// use crate::common;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request};
use uuid::Uuid;

// Re-export commonly used test utilities
pub use server_setup::*;
