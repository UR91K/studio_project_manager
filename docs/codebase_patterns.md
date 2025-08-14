# Codebase Patterns

This document outlines common patterns and best practices used throughout the Seula codebase.

## Testing Patterns

### Sequential Test Execution for Shared Resources

**Problem**: Tests that use shared resources (environment variables, files, global state) can interfere with each other when run in parallel, causing flaky test failures.

**Solution**: Use a single `#[test]` function that calls implementation functions in sequence.

**Example**:
```rust
/// Single comprehensive test that runs all scenarios in order
/// This prevents test interference from parallel execution and shared environment variables
#[test]
fn test_all_config_startup_scenarios() {
    println!("=== Running all scenarios in sequence ===");
    
    // Run each test scenario in order
    test_scenario_1_impl();
    test_scenario_2_impl();
    test_scenario_3_impl();
    
    println!("=== All scenarios completed successfully ===");
}

/// Individual test implementations (no #[test] attribute)
fn test_scenario_1_impl() {
    println!("Running: test_scenario_1");
    // Test implementation here...
    
    // Clean up shared resources
    std::env::remove_var("SHARED_ENV_VAR");
}

fn test_scenario_2_impl() {
    println!("Running: test_scenario_2");
    // Test implementation here...
    
    // Clean up shared resources
    std::env::remove_var("SHARED_ENV_VAR");
}
```

**When to Use**:
- Tests that modify environment variables
- Tests that create/modify files in shared locations
- Tests that use global static variables
- Tests that require specific execution order

**Alternative Approaches**:
- `--test-threads=1` flag (slower, affects all tests)
- `Mutex` synchronization (more complex)
- `serial_test` crate (external dependency)

**Used In**: `tests/config_startup_tests.rs` - Config service tests that use `STUDIO_PROJECT_MANAGER_CONFIG` environment variable.

---

## gRPC Service Patterns

### Adding New gRPC Endpoints

**Overview**: The codebase follows a structured approach for adding gRPC services with clear separation of concerns between protocol definitions, handlers, and server implementation.

### Step-by-Step Process

#### 1. Define Protocol Buffer Service
**File**: `proto/services/{service_name}.proto`

```protobuf
syntax = "proto3";

package seula.{service_name};

import "common.proto";

service {ServiceName}Service {
  rpc GetItem(GetItemRequest) returns (GetItemResponse);
  rpc CreateItem(CreateItemRequest) returns (CreateItemResponse);
  rpc UpdateItem(UpdateItemRequest) returns (UpdateItemResponse);
  rpc DeleteItem(DeleteItemRequest) returns (DeleteItemResponse);
}

message GetItemRequest {
  string item_id = 1;
}

message GetItemResponse {
  optional seula.common.Item item = 1;
}

// ... other message definitions
```

#### 2. Add Service to Build System
**File**: `build.rs`

```rust
let services = [
    "projects",
    "collections", 
    "tasks",
    // ... existing services
    "{service_name}",  // Add your new service here
];
```

#### 3. Create Database Methods
**File**: `src/database/{service_name}.rs`

```rust
use crate::database::LiveSetDatabase;
use crate::error::DatabaseError;

impl LiveSetDatabase {
    pub fn get_item(&mut self, item_id: &str) -> Result<Option<ItemData>, DatabaseError> {
        // Database implementation
    }

    pub fn create_item(&mut self, item: &ItemData) -> Result<String, DatabaseError> {
        // Database implementation
    }

    // ... other database methods
}

// Database-specific structs
pub struct ItemData {
    pub id: String,
    pub name: String,
    // ... other fields
}
```

#### 4. Create gRPC Handler
**File**: `src/grpc/handlers/{service_name}.rs`

```rust
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Code, Request, Response, Status};

use crate::database::LiveSetDatabase;
use super::super::{service_name}::*;
use super::super::common::*;

#[derive(Clone)]
pub struct {ServiceName}Handler {
    pub db: Arc<Mutex<LiveSetDatabase>>,
}

impl {ServiceName}Handler {
    pub fn new(db: Arc<Mutex<LiveSetDatabase>>) -> Self {
        Self { db }
    }

    pub async fn get_item(
        &self,
        request: Request<GetItemRequest>,
    ) -> Result<Response<GetItemResponse>, Status> {
        debug!("GetItem request: {:?}", request);

        let req = request.into_inner();
        let mut db = self.db.lock().await;

        match db.get_item(&req.item_id) {
            Ok(Some(item_data)) => {
                let item = Item {
                    id: item_data.id,
                    name: item_data.name,
                    // ... convert database struct to proto
                };

                let response = GetItemResponse { item: Some(item) };
                Ok(Response::new(response))
            }
            Ok(None) => {
                debug!("Item not found: {}", req.item_id);
                let response = GetItemResponse { item: None };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get item {}: {:?}", req.item_id, e);
                Err(Status::new(
                    Code::Internal,
                    format!("Database error: {}", e),
                ))
            }
        }
    }

    // ... other handler methods
}
```

#### 5. Register Handler in Module
**File**: `src/grpc/handlers/mod.rs`

```rust
pub mod collections;
pub mod config;
// ... existing handlers
pub mod {service_name};  // Add your handler module

pub use collections::CollectionsHandler;
pub use config::ConfigHandler;
// ... existing exports
pub use {service_name}::{ServiceName}Handler;  // Export your handler
```

#### 6. Add Service to gRPC Module
**File**: `src/grpc/mod.rs`

```rust
pub mod {service_name} {
    tonic::include_proto!("seula.{service_name}");
}
```

#### 7. Integrate with Main Server
**File**: `src/grpc/server.rs`

```rust
// Add import
use super::{service_name}::*;

// Add handler to server struct
#[derive(Clone)]
pub struct StudioProjectManagerServer {
    // ... existing handlers
    pub {service_name}_handler: {ServiceName}Handler,
}

// Initialize handler in new() method
impl StudioProjectManagerServer {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // ... existing initialization
        Ok(Self {
            // ... existing handlers
            {service_name}_handler: {ServiceName}Handler::new(Arc::clone(&db)),
        })
    }
}

// Implement the service trait
#[tonic::async_trait]
impl {service_name}_service_server::{ServiceName}Service for StudioProjectManagerServer {
    async fn get_item(
        &self,
        request: Request<GetItemRequest>,
    ) -> Result<Response<GetItemResponse>, Status> {
        self.{service_name}_handler.get_item(request).await
    }

    // ... implement other service methods
}
```

#### 8. Add Tests
**File**: `tests/grpc/{service_name}.rs`

```rust
use crate::grpc::*;
use crate::grpc::server_setup::{setup_test_server, create_test_project};
use seula::grpc::{service_name}::*;
use seula::grpc::{service_name}::{service_name}_service_server::{ServiceName}Service;

#[tokio::test]
async fn test_get_item() {
    let (server, _db) = setup_test_server().await;

    let request = Request::new(GetItemRequest {
        item_id: "test-id".to_string(),
    });

    let response = server.get_item(request).await.unwrap();
    let item = response.into_inner().item;
    
    // Add assertions
}
```

**Don't forget to add the module to** `tests/grpc/mod.rs`:
```rust
pub mod {service_name};
```

### Key Patterns & Conventions

#### Error Handling
- Always use `debug!()` for request logging
- Use `error!()` for database/internal errors  
- Convert database errors to gRPC `Status::internal()`
- Return appropriate gRPC status codes

#### Database Integration
- Always acquire database lock: `let mut db = self.db.lock().await;`
- Handle `Ok(Some())`, `Ok(None)`, and `Err()` cases explicitly
- Use meaningful error messages in Status responses

#### Proto Conversion
- Convert database structs to proto structs in handlers
- Use `Some()` for optional fields when data exists
- Use `None` for optional fields when data doesn't exist

#### Testing
- Use `setup_test_server()` helper for consistent test setup
- Test happy path, not found, and error cases
- Use descriptive test names: `test_{method_name}_{scenario}`

### Examples in Codebase
- **Tags Service**: `proto/services/tags.proto`, `src/grpc/handlers/tags.rs`
- **Config Service**: `proto/services/config.proto`, `src/grpc/handlers/config.rs`
- **Samples Service**: `proto/services/samples.proto`, `src/grpc/handlers/samples.rs`

---
