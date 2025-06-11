use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::types::{CreateFunctionRequest, Function};

pub struct FunctionStore {
    functions: RwLock<HashMap<String, Function>>,
}

impl FunctionStore {
    pub fn new() -> Self {
        Self {
            functions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create(&self, request: CreateFunctionRequest) -> Result<Function> {
        // Validate function
        self.validate_function(&request)?;

        let function = Function {
            name: request.name.clone(),
            code: request.code,
            runtime: request.runtime,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        // Store function
        {
            let mut functions = self.functions.write().await;
            functions.insert(request.name.clone(), function.clone());
        }

        info!("Created function: {}", request.name);
        Ok(function)
    }

    pub async fn get(&self, name: &str) -> Option<Function> {
        let functions = self.functions.read().await;
        functions.get(name).cloned()
    }

    pub async fn list(&self) -> Vec<Function> {
        let functions = self.functions.read().await;
        functions.values().cloned().collect()
    }

    pub async fn delete(&self, name: &str) -> Result<bool> {
        let mut functions = self.functions.write().await;
        match functions.remove(name) {
            Some(_) => {
                info!("Deleted function: {}", name);
                Ok(true)
            }
            None => {
                warn!("Attempted to delete non-existent function: {}", name);
                Ok(false)
            }
        }
    }

    pub async fn update(&self, name: &str, request: CreateFunctionRequest) -> Result<Function> {
        // Validate function
        self.validate_function(&request)?;

        let function = Function {
            name: name.to_string(),
            code: request.code,
            runtime: request.runtime,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        // Update function
        {
            let mut functions = self.functions.write().await;
            functions.insert(name.to_string(), function.clone());
        }

        info!("Updated function: {}", name);
        Ok(function)
    }

    fn validate_function(&self, request: &CreateFunctionRequest) -> Result<()> {
        // Validate name
        if request.name.is_empty() {
            return Err(anyhow::anyhow!("Function name cannot be empty"));
        }

        if !request.name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(anyhow::anyhow!("Function name can only contain alphanumeric characters, hyphens, and underscores"));
        }

        if request.name.len() > 64 {
            return Err(anyhow::anyhow!("Function name cannot exceed 64 characters"));
        }

        // Validate code
        if request.code.is_empty() {
            return Err(anyhow::anyhow!("Function code cannot be empty"));
        }

        if request.code.len() > 1024 * 1024 {
            return Err(anyhow::anyhow!("Function code cannot exceed 1MB"));
        }

        // Validate runtime
        if request.runtime != "v8" {
            return Err(anyhow::anyhow!("Only 'v8' runtime is currently supported"));
        }

        // Basic JavaScript syntax validation
        self.validate_javascript_syntax(&request.code)?;

        Ok(())
    }

    fn validate_javascript_syntax(&self, code: &str) -> Result<()> {
        // Basic validation - check for export default
        if !code.contains("export default") && !code.contains("module.exports") {
            return Err(anyhow::anyhow!("Function must export a default function"));
        }

        // Check for forbidden patterns
        let forbidden_patterns = [
            "require('fs')",
            "require(\"fs\")",
            "import fs",
            "process.exit",
            "__dirname",
            "__filename",
        ];

        for pattern in &forbidden_patterns {
            if code.contains(pattern) {
                return Err(anyhow::anyhow!("Function contains forbidden pattern: {}", pattern));
            }
        }

        Ok(())
    }

    pub async fn get_function_stats(&self) -> FunctionStats {
        let functions = self.functions.read().await;
        FunctionStats {
            total_functions: functions.len(),
            total_code_size: functions.values().map(|f| f.code.len()).sum(),
            runtimes: {
                let mut runtimes = HashMap::new();
                for function in functions.values() {
                    *runtimes.entry(function.runtime.clone()).or_insert(0) += 1;
                }
                runtimes
            },
        }
    }
}

#[derive(Debug)]
pub struct FunctionStats {
    pub total_functions: usize,
    pub total_code_size: usize,
    pub runtimes: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_function_creation() {
        let store = FunctionStore::new();
        
        let request = CreateFunctionRequest {
            name: "test-function".to_string(),
            code: "export default function handler(event) { return { message: 'Hello!' }; }".to_string(),
            runtime: "v8".to_string(),
        };

        let result = store.create(request).await;
        assert!(result.is_ok());

        let function = store.get("test-function").await;
        assert!(function.is_some());
    }

    #[tokio::test]
    async fn test_function_validation() {
        let store = FunctionStore::new();
        
        // Test empty name
        let request = CreateFunctionRequest {
            name: "".to_string(),
            code: "export default function handler(event) { return {}; }".to_string(),
            runtime: "v8".to_string(),
        };
        assert!(store.create(request).await.is_err());

        // Test invalid runtime
        let request = CreateFunctionRequest {
            name: "test".to_string(),
            code: "export default function handler(event) { return {}; }".to_string(),
            runtime: "python".to_string(),
        };
        assert!(store.create(request).await.is_err());

        // Test forbidden pattern
        let request = CreateFunctionRequest {
            name: "test".to_string(),
            code: "const fs = require('fs'); export default function handler(event) { return {}; }".to_string(),
            runtime: "v8".to_string(),
        };
        assert!(store.create(request).await.is_err());
    }

    #[tokio::test]
    async fn test_function_list_and_delete() {
        let store = FunctionStore::new();
        
        // Create multiple functions
        for i in 0..3 {
            let request = CreateFunctionRequest {
                name: format!("test-function-{}", i),
                code: "export default function handler(event) { return {}; }".to_string(),
                runtime: "v8".to_string(),
            };
            store.create(request).await.unwrap();
        }

        // List functions
        let functions = store.list().await;
        assert_eq!(functions.len(), 3);

        // Delete function
        let deleted = store.delete("test-function-1").await.unwrap();
        assert!(deleted);

        let functions = store.list().await;
        assert_eq!(functions.len(), 2);

        // Try to delete non-existent function
        let deleted = store.delete("non-existent").await.unwrap();
        assert!(!deleted);
    }
}