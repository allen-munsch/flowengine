// crates/flownodes/src/docker_v2.rs
// Enhanced Docker Node with IOMode for better flexibility

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::json;

/// Enhanced Docker node with flexible I/O modes
pub struct DockerNodeV2;

#[derive(Debug, Clone)]
struct DockerConfig {
    image: String,
    command: Option<Vec<String>>,
    entrypoint: Option<Vec<String>>,
    env: HashMap<String, String>,
    volumes: Vec<VolumeMount>,
    working_dir: Option<String>,
    user: Option<String>,
    network: Option<String>,
    cpu_limit: Option<String>,
    memory_limit: Option<String>,
    stdin_mode: StdinMode,
    output_mode: OutputMode,
    io_mode: IOMode,  // NEW!
    auto_pull: bool,
    detached: bool,
    remove: bool,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
struct VolumeMount {
    host_path: String,
    container_path: String,
    read_only: bool,
}

#[derive(Debug, Clone)]
enum StdinMode {
    None,
    Raw,
    Json,
    Text,
}

#[derive(Debug, Clone)]
enum OutputMode {
    Auto,
    Json,
    Text,
}

/// NEW: Controls how Value enums are serialized/deserialized
#[derive(Debug, Clone)]
enum IOMode {
    /// Automatic - smart detection based on context
    Auto,
    
    /// Flat - extract actual values, no Value enum wrapping
    /// Input: {"value": 21} instead of {"value": {"type": "Number", "value": 21}}
    Flat,
    
    /// Wrapped - full Value enum structure (backward compatible)
    Wrapped,
}

impl DockerNodeV2 {
    fn parse_config(ctx: &NodeContext) -> Result<DockerConfig, NodeError> {
        let image = ctx.require_config("image")?
            .as_str()
            .ok_or_else(|| NodeError::Configuration("image must be a string".to_string()))?
            .to_string();
        
        let command = ctx.config.get("command")
            .and_then(|v| match v {
                Value::String(s) => {
                    Some(shell_words::split(s).unwrap_or_else(|_| vec![s.clone()]))
                }
                Value::Array(arr) => {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                }
                _ => None,
            });
        
        let entrypoint = ctx.config.get("entrypoint")
            .and_then(|v| match v {
                Value::String(s) => Some(vec![s.clone()]),
                Value::Array(arr) => {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                }
                _ => None,
            });
        
        let mut env = HashMap::new();
        if let Some(Value::Object(env_obj)) = ctx.config.get("env") {
            for (key, value) in env_obj {
                if let Some(val_str) = value.as_str() {
                    env.insert(key.clone(), val_str.to_string());
                } else if let Value::Json(json_val) = value {
                    env.insert(key.clone(), json_val.to_string());
                }
            }
        }
        
        let mut volumes = Vec::new();
        if let Some(Value::Array(vols)) = ctx.config.get("volumes") {
            for vol in vols {
                if let Some(vol_str) = vol.as_str() {
                    if let Some(mount) = Self::parse_volume(vol_str) {
                        volumes.push(mount);
                    }
                }
            }
        }
        
        let working_dir = ctx.config.get("workdir")
            .and_then(|v| v.as_str())
            .map(String::from);
        
        let user = ctx.config.get("user")
            .and_then(|v| v.as_str())
            .map(String::from);
        
        let network = ctx.config.get("network")
            .and_then(|v| v.as_str())
            .map(String::from);
        
        let cpu_limit = ctx.config.get("cpu_limit")
            .and_then(|v| v.as_str())
            .map(String::from);
        
        let memory_limit = ctx.config.get("memory_limit")
            .and_then(|v| v.as_str())
            .map(String::from);
        
        let stdin_mode = ctx.config.get("stdin_mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "none" => Some(StdinMode::None),
                "raw" => Some(StdinMode::Raw),
                "json" => Some(StdinMode::Json),
                "text" => Some(StdinMode::Text),
                _ => None,
            })
            .unwrap_or(StdinMode::Json);
        
        let output_mode = ctx.config.get("output_mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "auto" => Some(OutputMode::Auto),
                "json" => Some(OutputMode::Json),
                "text" => Some(OutputMode::Text),
                _ => None,
            })
            .unwrap_or(OutputMode::Auto);
        
        // NEW: Parse IO mode
        let io_mode = ctx.config.get("io_mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "auto" => Some(IOMode::Auto),
                "flat" => Some(IOMode::Flat),
                "wrapped" => Some(IOMode::Wrapped),
                _ => None,
            })
            .unwrap_or(IOMode::Auto);  // Default to Auto for smart behavior
        
        let auto_pull = ctx.config.get("auto_pull")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        
        let detached = ctx.config.get("detached")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let remove = ctx.config.get("remove")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        
        let timeout_seconds = ctx.config.get("timeout")
            .and_then(|v| v.as_f64())
            .map(|f| f as u64);
        
        Ok(DockerConfig {
            image,
            command,
            entrypoint,
            env,
            volumes,
            working_dir,
            user,
            network,
            cpu_limit,
            memory_limit,
            stdin_mode,
            output_mode,
            io_mode,
            auto_pull,
            detached,
            remove,
            timeout_seconds,
        })
    }
    
    fn parse_volume(volume_str: &str) -> Option<VolumeMount> {
        let parts: Vec<&str> = volume_str.split(':').collect();
        
        match parts.len() {
            2 => Some(VolumeMount {
                host_path: parts[0].to_string(),
                container_path: parts[1].to_string(),
                read_only: false,
            }),
            3 => Some(VolumeMount {
                host_path: parts[0].to_string(),
                container_path: parts[1].to_string(),
                read_only: parts[2] == "ro",
            }),
            _ => None,
        }
    }
    
    /// NEW: Convert Value to flat JSON (extract actual values)
    fn value_to_json_flat(value: &Value) -> serde_json::Value {
        match value {
            Value::Null => json!(null),
            Value::Bool(b) => json!(b),
            Value::Number(n) => json!(n),
            Value::String(s) => json!(s),
            Value::Json(j) => j.clone(),
            Value::Array(arr) => {
                json!(arr.iter().map(|v| Self::value_to_json_flat(v)).collect::<Vec<_>>())
            }
            Value::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::value_to_json_flat(v));
                }
                json!(map)
            }
            Value::Bytes(_) => json!(null), // Can't represent bytes in JSON
        }
    }
    
    /// NEW: Serialize inputs based on IO mode
    fn serialize_inputs(
        inputs: &HashMap<String, Value>,
        io_mode: &IOMode,
    ) -> Result<serde_json::Value, NodeError> {
        match io_mode {
            IOMode::Flat | IOMode::Auto => {
                // Extract actual values from Value enum
                let mut flat = serde_json::Map::new();
                for (key, value) in inputs {
                    flat.insert(key.clone(), Self::value_to_json_flat(value));
                }
                Ok(json!(flat))
            }
            IOMode::Wrapped => {
                // Use full Value enum structure (backward compatible)
                serde_json::to_value(inputs)
                    .map_err(|e| NodeError::ExecutionFailed(format!("Failed to serialize inputs: {}", e)))
            }
        }
    }
    
    async fn pull_image_if_needed(image: &str, ctx: &NodeContext) -> Result<(), NodeError> {
        ctx.events.info(format!("Checking for image: {}", image));
        
        let check_result = Command::new("docker")
            .args(&["image", "inspect", image])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| NodeError::ExecutionFailed(format!("Failed to check image: {}", e)))?;
        
        if !check_result.success() {
            ctx.events.info(format!("Pulling image: {}", image));
            
            let pull_result = Command::new("docker")
                .args(&["pull", image])
                .status()
                .await
                .map_err(|e| NodeError::ExecutionFailed(format!("Failed to pull image: {}", e)))?;
            
            if !pull_result.success() {
                return Err(NodeError::ExecutionFailed(format!("Failed to pull image: {}", image)));
            }
            
            ctx.events.info("Image pulled successfully");
        }
        
        Ok(())
    }
    
    async fn prepare_stdin_data(
        ctx: &NodeContext,
        stdin_mode: &StdinMode,
        io_mode: &IOMode,
    ) -> Result<Vec<u8>, NodeError> {
        match stdin_mode {
            StdinMode::None => Ok(Vec::new()),
            StdinMode::Raw => {
                ctx.inputs.get("data")
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.as_bytes().to_vec()),
                        Value::Bytes(b) => Some(b.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| NodeError::MissingInput("data".to_string()))
            }
            StdinMode::Json => {
                // NEW: Use io_mode to control serialization
                let json_value = Self::serialize_inputs(&ctx.inputs, io_mode)?;
                
                serde_json::to_vec(&json_value)
                    .map_err(|e| NodeError::ExecutionFailed(format!("Failed to serialize JSON: {}", e)))
            }
            StdinMode::Text => {
                ctx.inputs.get("data")
                    .and_then(|v| v.as_str())
                    .map(|s| s.as_bytes().to_vec())
                    .ok_or_else(|| NodeError::MissingInput("data".to_string()))
            }
        }
    }
}

#[async_trait]
impl Node for DockerNodeV2 {
    fn node_type(&self) -> &str {
        "docker.run"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let config = Self::parse_config(&ctx)?;
        
        ctx.events.info(format!("🐳 Running Docker image: {}", config.image));
        
        if config.auto_pull {
            Self::pull_image_if_needed(&config.image, &ctx).await?;
        }
        
        let mut cmd = Command::new("docker");
        cmd.arg("run");
        
        if config.remove {
            cmd.arg("--rm");
        }
        
        if config.detached {
            cmd.arg("-d");
        } else {
            cmd.arg("-i");
        }
        
        for (key, value) in &config.env {
            cmd.arg("-e").arg(format!("{}={}", key, value));
        }
        
        for volume in &config.volumes {
            let mount_str = if volume.read_only {
                format!("{}:{}:ro", volume.host_path, volume.container_path)
            } else {
                format!("{}:{}", volume.host_path, volume.container_path)
            };
            ctx.events.info(format!("  📂 Volume: {}", mount_str));
            cmd.arg("-v").arg(mount_str);
        }
        
        if let Some(ref workdir) = config.working_dir {
            cmd.arg("-w").arg(workdir);
        }
        
        if let Some(ref user) = config.user {
            cmd.arg("-u").arg(user);
        }
        
        if let Some(ref network) = config.network {
            cmd.arg("--network").arg(network);
        }
        
        if let Some(ref cpu_limit) = config.cpu_limit {
            cmd.arg("--cpus").arg(cpu_limit);
            ctx.events.info(format!("  💻 CPU limit: {}", cpu_limit));
        }
        
        if let Some(ref memory_limit) = config.memory_limit {
            cmd.arg("--memory").arg(memory_limit);
            ctx.events.info(format!("  🧠 Memory limit: {}", memory_limit));
        }
        
        if let Some(ref entrypoint) = config.entrypoint {
            if !entrypoint.is_empty() {
                cmd.arg("--entrypoint");
                cmd.arg(&entrypoint[0]);
            }
        }
        
        cmd.arg(&config.image);
        
        if let Some(ref command) = config.command {
            for part in command {
                cmd.arg(part);
            }
        }
        
        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        ctx.events.info("  ▶️  Starting container...");
        
        let mut child = cmd.spawn()
            .map_err(|e| NodeError::ExecutionFailed(format!("Failed to spawn docker: {}", e)))?;
        
        let input_data = Self::prepare_stdin_data(&ctx, &config.stdin_mode, &config.io_mode).await?;
        
        if !input_data.is_empty() {
            ctx.events.info(format!("  📥 Sending {} bytes to stdin", input_data.len()));
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&input_data).await
                    .map_err(|e| NodeError::ExecutionFailed(format!("Failed to write stdin: {}", e)))?;
                drop(stdin);
            }
        }
        
        let mut stdout_opt = child.stdout.take();
        let mut stderr_opt = child.stderr.take();
        
        let stdout_future = async move {
            let mut data = Vec::new();
            if let Some(ref mut stdout) = stdout_opt {
                let _ = stdout.read_to_end(&mut data).await;
            }
            data
        };
        
        let stderr_future = async move {
            let mut data = Vec::new();
            if let Some(ref mut stderr) = stderr_opt {
                let _ = stderr.read_to_end(&mut data).await;
            }
            data
        };
        
        let (status, stdout_data, stderr_data) = if let Some(timeout_secs) = config.timeout_seconds {
            let duration = tokio::time::Duration::from_secs(timeout_secs);
            
            let result = tokio::time::timeout(
                duration,
                async {
                    let (stdout, stderr) = tokio::join!(stdout_future, stderr_future);
                    let status = child.wait().await
                        .map_err(|e| NodeError::ExecutionFailed(format!("Process wait failed: {}", e)))?;
                    Ok::<_, NodeError>((status, stdout, stderr))
                }
            ).await;
            
            match result {
                Ok(Ok(data)) => data,
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    ctx.events.warn(format!("Container timeout after {}s - attempting to kill", timeout_secs));
                    let _ = child.kill().await;
                    return Err(NodeError::Timeout { seconds: timeout_secs });
                }
            }
        } else {
            let (stdout, stderr) = tokio::join!(stdout_future, stderr_future);
            let status = child.wait().await
                .map_err(|e| NodeError::ExecutionFailed(format!("Failed to wait for process: {}", e)))?;
            (status, stdout, stderr)
        };
        
        let stdout_str = String::from_utf8_lossy(&stdout_data).to_string();
        let stderr_str = String::from_utf8_lossy(&stderr_data).to_string();
        
        if !stderr_str.is_empty() {
            for line in stderr_str.lines().take(10) {
                ctx.events.warn(format!("  stderr: {}", line));
            }
        }
        
        let exit_code = status.code().unwrap_or(-1);
        let success = status.success();
        
        if success {
            ctx.events.info(format!("  ✅ Container completed (exit code: {})", exit_code));
        } else {
            ctx.events.warn(format!("  ⚠️  Container exited with code: {}", exit_code));
        }
        
        let output_value = match config.output_mode {
            OutputMode::Auto => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
                    ctx.events.info("  📊 Output parsed as JSON");
                    Value::Json(json)
                } else {
                    Value::String(stdout_str.clone())
                }
            }
            OutputMode::Json => {
                let json = serde_json::from_str::<serde_json::Value>(&stdout_str)
                    .map_err(|e| NodeError::ExecutionFailed(format!("Failed to parse JSON output: {}", e)))?;
                ctx.events.info("  📊 Output parsed as JSON");
                Value::Json(json)
            }
            OutputMode::Text => {
                Value::String(stdout_str.clone())
            }
        };
        
        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout_str)
            .with_output("stderr", stderr_str)
            .with_output("exit_code", exit_code as f64)
            .with_output("success", success))
    }
}

pub struct DockerNodeV2Factory;

impl NodeFactory for DockerNodeV2Factory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(DockerNodeV2))
    }
    
    fn node_type(&self) -> &str {
        "docker.run"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Execute a Docker container with flexible I/O modes and extensive configuration".to_string(),
            category: "docker".to_string(),
            inputs: vec![
                PortDefinition {
                    name: "data".to_string(),
                    description: "Data to pass to container (mode depends on stdin_mode config)".to_string(),
                    required: false,
                }
            ],
            outputs: vec![
                PortDefinition {
                    name: "output".to_string(),
                    description: "Container output (parsed based on output_mode)".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stdout".to_string(),
                    description: "Raw stdout from container".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stderr".to_string(),
                    description: "Raw stderr from container".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "exit_code".to_string(),
                    description: "Container exit code".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "success".to_string(),
                    description: "Boolean indicating if container exited successfully (exit code 0)".to_string(),
                    required: false,
                }
            ],
        }
    }
}

mod shell_words {
    pub fn split(s: &str) -> Result<Vec<String>, ()> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut escape = false;
        
        for c in s.chars() {
            if escape {
                current.push(c);
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_quotes = !in_quotes;
            } else if c.is_whitespace() && !in_quotes {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
            } else {
                current.push(c);
            }
        }
        
        if !current.is_empty() {
            words.push(current);
        }
        
        Ok(words)
    }
}
