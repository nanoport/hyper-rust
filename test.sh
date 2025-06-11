#!/bin/bash
# Hyperdrive Rust - Quick Setup and Test Script

set -e

echo "🚀 Setting up Hyperdrive Rust..."

# Create project directory
PROJECT_DIR="hyperdrive-rust"
if [ -d "$PROJECT_DIR" ]; then
    echo "Directory $PROJECT_DIR already exists. Removing..."
    rm -rf "$PROJECT_DIR"
fi

mkdir -p "$PROJECT_DIR/src"
cd "$PROJECT_DIR"

# Create the project structure based on our artifacts
cat > Cargo.toml << 'EOF'
[package]
name = "hyperdrive-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
axum = { version = "0.7", features = ["json", "tower-log"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["json"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tempfile = "3.0"
uuid = { version = "1.0", features = ["v4"] }
futures = "0.3"
tokio-util = "0.7"
ipnetwork = "0.20"
anyhow = "1.0"
thiserror = "1.0"
dashmap = "5.0"
parking_lot = "0.12"
nix = "0.28"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-test = "0.4"
EOF

# Create main.rs with minimal working version
cat > src/main.rs << 'EOF'
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};

mod types;
mod function;
mod vm;
mod pool;

use types::*;
use function::FunctionStore;
use vm::VmManager;
use pool::VmPool;

#[derive(Clone)]
pub struct AppState {
    function_store: Arc<FunctionStore>,
    vm_manager: Arc<VmManager>,
    vm_pool: Arc<VmPool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("🚀 Starting Hyperdrive Rust");

    let function_store = Arc::new(FunctionStore::new());
    let vm_manager = Arc::new(VmManager::new().await?);
    let vm_pool = Arc::new(VmPool::new(vm_manager.clone()).await?);

    let state = AppState {
        function_store,
        vm_manager,
        vm_pool,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/functions", get(list_functions))
        .route("/api/v1/functions", post(create_function))
        .route("/api/v1/functions/:name/invoke", post(invoke_function))
        .route("/api/v1/advanced/vms", get(list_vms))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:8090").await?;
    info!("🌟 Hyperdrive Rust listening on :8090");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        platform: "hyperdrive-rust".to_string(),
        status: "healthy".to_string(),
        version: "0.1.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        components: HealthComponents {
            firecracker: true,
            dns: true,
            ssl: true,
            cdn: true,
            monitoring: true,
        },
    })
}

async fn list_functions(State(state): State<AppState>)