use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// API Request/Response types
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub platform: String,
    pub status: String,
    pub version: String,
    pub timestamp: String,
    pub components: HealthComponents,
}

#[derive(Debug, Serialize)]
pub struct HealthComponents {
    pub firecracker: bool,
    pub dns: bool,
    pub ssl: bool,
    pub cdn: bool,
    pub monitoring: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateFunctionRequest {
    pub name: String,
    pub code: String,
    pub runtime: String, // "v8" for now
}

#[derive(Debug, Serialize)]
pub struct CreateFunctionResponse {
    pub name: String,
    pub created: bool,
}

#[derive(Debug, Serialize)]
pub struct FunctionListResponse {
    pub functions: Vec<Function>,
}

#[derive(Debug, Serialize)]
pub struct InvokeResponse {
    pub result: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct VmListResponse {
    pub vms: Option<Vec<VmInfo>>,
}

// Core domain types
#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub name: String,
    pub code: String,
    pub runtime: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VmInfo {
    pub id: String,
    pub state: VmState,
    pub ip_address: Option<String>,
    pub port: Option<u16>,
    pub created_at: String,
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum VmState {
    Starting,
    Ready,
    Busy,
    Stopping,
    Failed,
}

// VM configuration
#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub kernel_path: String,
    pub rootfs_path: String,
    pub v8_host_path: String,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            mem_size_mib: 128,
            kernel_path: "/opt/firecracker/vmlinux.bin".to_string(),
            rootfs_path: "/opt/firecracker/rootfs.ext4".to_string(),
            v8_host_path: "/opt/firecracker/v8-host".to_string(),
        }
    }
}

// VM execution context
#[derive(Debug)]
pub struct VmInstance {
    pub id: Uuid,
    pub state: VmState,
    pub ip_address: Option<String>,
    pub port: Option<u16>,
    pub process_id: Option<u32>,
    pub work_dir: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

impl VmInstance {
    pub fn new(work_dir: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: VmState::Starting,
            ip_address: None,
            port: None,
            process_id: None,
            work_dir,
            created_at: now,
            last_used: now,
        }
    }

    pub async fn execute_function(
        &mut self,
        function: &Function,
        payload: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self.last_used = chrono::Utc::now();
        self.state = VmState::Busy;

        // Execute function via HTTP call to V8 host in VM
        let result = self.call_v8_host(function, payload).await?;
        
        self.state = VmState::Ready;
        Ok(result)
    }

    async fn call_v8_host(
        &self,
        function: &Function,
        payload: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let ip = self.ip_address.as_ref()
            .ok_or_else(|| anyhow::anyhow!("VM has no IP address"))?;
        let port = self.port
            .ok_or_else(|| anyhow::anyhow!("VM has no port"))?;

        let url = format!("http://{}:{}/execute", ip, port);
        
        let request_body = serde_json::json!({
            "code": function.code,
            "payload": payload
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if response.status().is_success() {
            let result: serde_json::Value = response.json().await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Function execution failed: {}", response.status()))
        }
    }
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum HyperdriveError {
    #[error("VM creation failed: {0}")]
    VmCreationFailed(String),
    
    #[error("VM not ready: {0}")]
    VmNotReady(String),
    
    #[error("Function execution failed: {0}")]
    FunctionExecutionFailed(String),
    
    #[error("Pool exhausted")]
    PoolExhausted,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}