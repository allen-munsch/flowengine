use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::AsyncReadExt;

/// Node that executes Docker containers
pub struct DockerNode;

#[async_trait]
impl Node for DockerNode {
    fn node_type(&self) -> &str {
        "docker.run"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        // Get configuration
        let image = ctx.require_config("image")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "image".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        
        // Optional: command to run (default is container's CMD)
        let command = ctx.config.get("command")
            .and_then(|v| v.as_str());
        
        // Optional: environment variables
        let mut env_vars = Vec::new();
        if let Some(Value::Object(env)) = ctx.config.get("env") {
            for (key, value) in env {
                if let Some(val_str) = value.as_str() {
                    env_vars.push(format!("{}={}", key, val_str));
                }
            }
        }
        
        // Get input data to pass to container (via stdin)
        let input_data = ctx.inputs.get("data")
            .and_then(|v| match v {
                Value::String(s) => Some(s.as_bytes().to_vec()),
                Value::Json(j) => serde_json::to_vec(j).ok(),
                _ => None,
            })
            .unwrap_or_default();
        
        ctx.events.info(format!("Running Docker image: {}", image));
        
        // Build docker command
        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")  // Remove container after execution
            .arg("-i");   // Interactive (for stdin)
        
        // Add environment variables
        for env in &env_vars {
            cmd.arg("-e").arg(env);
        }
        
        // Add image
        cmd.arg(image);
        
        // Add command if specified
        if let Some(cmd_str) = command {
            for part in cmd_str.split_whitespace() {
                cmd.arg(part);
            }
        }
        
        // Configure stdio
        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        ctx.events.info("Starting container...");
        
        // Spawn the process
        let mut child = cmd.spawn()
            .map_err(|e| NodeError::ExecutionFailed(format!("Failed to spawn docker: {}", e)))?;
        
        // Write input data to stdin
        if !input_data.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(&input_data).await
                    .map_err(|e| NodeError::ExecutionFailed(format!("Failed to write stdin: {}", e)))?;
                drop(stdin); // Close stdin
            }
        }
        
        // Read stdout and stderr
        let mut stdout_data = Vec::new();
        let mut stderr_data = Vec::new();
        
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_end(&mut stdout_data).await
                .map_err(|e| NodeError::ExecutionFailed(format!("Failed to read stdout: {}", e)))?;
        }
        
        if let Some(mut stderr) = child.stderr.take() {
            stderr.read_to_end(&mut stderr_data).await
                .map_err(|e| NodeError::ExecutionFailed(format!("Failed to read stderr: {}", e)))?;
        }
        
        // Wait for process to complete
        let status = child.wait().await
            .map_err(|e| NodeError::ExecutionFailed(format!("Failed to wait for process: {}", e)))?;
        
        let stdout_str = String::from_utf8_lossy(&stdout_data).to_string();
        let stderr_str = String::from_utf8_lossy(&stderr_data).to_string();
        
        if !stderr_str.is_empty() {
            ctx.events.warn(format!("Container stderr: {}", stderr_str));
        }
        
        if !status.success() {
            return Err(NodeError::ExecutionFailed(
                format!("Container exited with status: {}. stderr: {}", status, stderr_str)
            ));
        }
        
        ctx.events.info(format!("Container completed successfully (exit code: {})", status.code().unwrap_or(0)));
        
        // Try to parse output as JSON, fallback to string
        let output_value = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
            Value::Json(json)
        } else {
            Value::String(stdout_str)
        };
        
        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stderr", stderr_str)
            .with_output("exit_code", status.code().unwrap_or(0) as f64))
    }
}

pub struct DockerNodeFactory;

impl NodeFactory for DockerNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(DockerNode))
    }
    
    fn node_type(&self) -> &str {
        "docker.run"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Execute a Docker container".to_string(),
            category: "docker".to_string(),
            inputs: vec![
                PortDefinition {
                    name: "data".to_string(),
                    description: "Data to pass to container via stdin".to_string(),
                    required: false,
                }
            ],
            outputs: vec![
                PortDefinition {
                    name: "output".to_string(),
                    description: "Container stdout (parsed as JSON if possible)".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stderr".to_string(),
                    description: "Container stderr".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "exit_code".to_string(),
                    description: "Container exit code".to_string(),
                    required: false,
                }
            ],
        }
    }
}