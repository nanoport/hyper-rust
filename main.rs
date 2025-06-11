use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tracing::{info, warn, error};
use uuid::Uuid;

mod vm;
mod function;
mod pool;
mod types;

use vm::VmManager;
use function::FunctionStore;
use pool::VmPool;
use types::*;

#[derive(Clone)]
pub struct AppState {
    vm_manager: Arc<VmManager>,
    function_store: Arc<FunctionStore>,
    vm_pool: Arc<VmPool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting Hyperdrive Rust");

    // Initialize components
    let vm_manager = Arc::new(VmManager::new().await?);
    let function_store = Arc::new(FunctionStore::new());
    let vm_pool = Arc::new(VmPool::new(vm_manager.clone()).await?);

    let state = AppState {
        vm_manager,
        function_store,
        vm_pool,
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/functions", get(list_functions))
        .route("/api/v1/functions", post(create_function))
        .route("/api/v1/functions/:name/invoke", post(invoke_function))
        .route("/api/v1/advanced/vms", get(list_vms))
        .with_state(state);

    // Start server
    let listener = TcpListener::bind("0.0.0.0:8090").await?;
    info!("Hyperdrive Rust listening on :8090");
    
    axum::serve(listener, app).await?;
    Ok(())
}

// Health check endpoint
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

// List functions
async fn list_functions(State(state): State<AppState>) -> Json<FunctionListResponse> {
    let functions = state.function_store.list().await;
    Json(FunctionListResponse { functions })
}

// Create function
async fn create_function(
    State(state): State<AppState>,
    Json(request): Json<CreateFunctionRequest>,
) -> Result<Json<CreateFunctionResponse>, StatusCode> {
    match state.function_store.create(request).await {
        Ok(function) => Ok(Json(CreateFunctionResponse { 
            name: function.name,
            created: true,
        })),
        Err(e) => {
            error!("Failed to create function: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

// Invoke function
async fn invoke_function(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<InvokeResponse>, StatusCode> {
    info!("Invoking function: {}", name);

    // Get function
    let function = match state.function_store.get(&name).await {
        Some(f) => f,
        None => {
            warn!("Function not found: {}", name);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Get VM from pool
    let vm = match state.vm_pool.acquire().await {
        Ok(vm) => vm,
        Err(e) => {
            error!("Failed to acquire VM: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Execute function
    match vm.execute_function(&function, payload).await {
        Ok(result) => {
            // Return VM to pool
            state.vm_pool.release(vm).await;
            Ok(Json(InvokeResponse { result }))
        }
        Err(e) => {
            error!("Function execution failed: {}", e);
            // VM might be corrupted, don't return to pool
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// List active VMs
async fn list_vms(State(state): State<AppState>) -> Json<VmListResponse> {
    let vms = state.vm_manager.list_active_vms().await;
    Json(VmListResponse { vms })
}