//! Zypi Firecracker microVM execution node
//!
//! Executes commands in Firecracker microVMs via Zypi's HTTP API.
//! Much faster than Docker: sub-second VM boot with CoW snapshots.
//!
//! API endpoints:
//!   POST /exec   — Execute command in microVM
//!   GET  /health  — Health check
//!
//! Config:
//!   url         - Zypi server URL (default: http://localhost:4000)
//!   image       - Container image to use (default: ubuntu:24.04)
//!   command     - Command to execute (string or array)
//!   env         - Environment variables
//!   workdir     - Working directory
//!   timeout     - Execution timeout in seconds
//!   files       - Files to inject into the sandbox (from Blob inputs)

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;

pub struct ZypiExecNode {
    client: reqwest::Client,
}

impl ZypiExecNode {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ZypiConfig {
    url: String,
    image: String,
    command: Vec<String>,
    env: HashMap<String, String>,
    workdir: Option<String>,
    timeout_seconds: Option<u64>,
}

impl ZypiConfig {
    fn from_ctx(ctx: &NodeContext) -> Result<Self, NodeError> {
        let url = ctx
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:4000")
            .to_string();

        let image = ctx
            .config
            .get("image")
            .and_then(|v| v.as_str())
            .unwrap_or("ubuntu:24.04")
            .to_string();

        // Parse command — can be string or array
        let command: Vec<String> = ctx
            .config
            .get("command")
            .and_then(|v| match v {
                Value::String(s) => {
                    Some(s.split_whitespace().map(String::from).collect())
                }
                Value::Array(arr) => Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        if command.is_empty() {
            return Err(NodeError::Configuration(
                "command is required".to_string(),
            ));
        }

        let mut env = HashMap::new();
        if let Some(Value::Object(env_obj)) = ctx.config.get("env") {
            for (key, value) in env_obj {
                if let Some(val_str) = value.as_str() {
                    env.insert(key.clone(), val_str.to_string());
                }
            }
        }

        let workdir = ctx
            .config
            .get("workdir")
            .and_then(|v| v.as_str())
            .map(String::from);

        let timeout_seconds = ctx
            .config
            .get("timeout")
            .and_then(|v| v.as_f64())
            .map(|f| f as u64);

        Ok(Self {
            url,
            image,
            command,
            env,
            workdir,
            timeout_seconds,
        })
    }
}

#[async_trait]
impl Node for ZypiExecNode {
    fn node_type(&self) -> &str {
        "zypi.exec"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let config = ZypiConfig::from_ctx(&ctx)?;

        ctx.events.info(format!(
            "🔥 Zypi exec: {} ({})",
            config.command.join(" "),
            config.image
        ));

        // Build request payload
        let mut payload = serde_json::Map::new();
        payload.insert(
            "cmd".to_string(),
            serde_json::Value::Array(
                config
                    .command
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
        payload.insert(
            "image".to_string(),
            serde_json::Value::String(config.image.clone()),
        );

        // Merge config env with workflow inputs (passed as env vars)
        let mut env = config.env.clone();
        for (key, value) in &ctx.inputs {
            let env_key = key.to_uppercase();
            match value {
                Value::String(s) => { env.insert(env_key, s.clone()); }
                Value::Number(n) => { env.insert(env_key, n.to_string()); }
                Value::Bool(b) => { env.insert(env_key, b.to_string()); }
                _ => {}
            }
        }

        if !env.is_empty() {
            let env_map: serde_json::Map<String, serde_json::Value> = env
                .iter()
                .map(|(k, v)| {
                    (k.clone(), serde_json::Value::String(v.clone()))
                })
                .collect();
            payload.insert(
                "env".to_string(),
                serde_json::Value::Object(env_map),
            );
        }

        if let Some(ref wd) = config.workdir {
            payload.insert(
                "workdir".to_string(),
                serde_json::Value::String(wd.clone()),
            );
        }

        // Inject files from Blob inputs
        let mut files: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::new();
        for (key, value) in &ctx.inputs {
            if key.starts_with("file:") {
                let path = key.strip_prefix("file:").unwrap();
                let content = match value {
                    Value::String(s) => {
                        serde_json::Value::String(s.clone())
                    }
                    Value::Bytes(b) => {
                        // Base64 encode binary data
                        use base64::Engine;
                        serde_json::Value::String(
                            base64::engine::general_purpose::STANDARD
                                .encode(b),
                        )
                    }
                    _ => serde_json::Value::String(value.to_string()),
                };
                files.insert(path.to_string(), content);
            }
        }

        // Also support a "files" input object
        if let Some(Value::Object(file_map)) = ctx.inputs.get("files") {
            for (path, content) in file_map {
                let content_str = match content {
                    Value::String(s) => s.clone(),
                    _ => content.to_string(),
                };
                files.insert(path.clone(), serde_json::Value::String(content_str));
            }
        }

        if !files.is_empty() {
            payload.insert(
                "files".to_string(),
                serde_json::Value::Object(files),
            );
        }

        let timeout = config.timeout_seconds.unwrap_or(300);

        // Build the request
        let mut request = self
            .client
            .post(format!("{}/exec", config.url))
            .json(&serde_json::Value::Object(payload))
            .timeout(std::time::Duration::from_secs(timeout));

        // Propagate timeout to request
        if let Some(ts) = config.timeout_seconds {
            request = request
                .timeout(std::time::Duration::from_secs(ts));
        }

        ctx.events
            .info(format!("  📡 POST {}/exec", config.url));

        let start = std::time::Instant::now();

        let response = request.send().await.map_err(|e| {
            // Distinguish timeout from other errors
            if e.is_timeout() {
                NodeError::Timeout {
                    seconds: timeout,
                }
            } else if e.is_connect() {
                NodeError::ExecutionFailed(format!(
                    "Cannot connect to Zypi at {} — is it running? ({})",
                    config.url, e
                ))
            } else {
                NodeError::ExecutionFailed(format!(
                    "Zypi request failed: {}",
                    e
                ))
            }
        })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let status_code = response.status().as_u16();
        let body: serde_json::Value = response.json().await.map_err(|e| {
            NodeError::ExecutionFailed(format!(
                "Failed to parse Zypi response: {}",
                e
            ))
        })?;

        if status_code != 200 {
            let err_msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(NodeError::ExecutionFailed(format!(
                "Zypi error ({}): {}",
                status_code, err_msg
            )));
        }

        let exit_code = body
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let stdout = body
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stderr = body
            .get("stderr")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let success = exit_code == 0;

        ctx.events.info(format!(
            "  ✅ Zypi completed in {}ms (exit: {})",
            duration_ms, exit_code
        ));

        if !stderr.is_empty() {
            ctx.events.warn(format!("  stderr: {}", stderr));
        }

        // Parse stdout as JSON if possible
        let output_value = if let Ok(json) =
            serde_json::from_str::<serde_json::Value>(stdout)
        {
            Value::Json(json)
        } else {
            Value::String(stdout.to_string())
        };

        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout.to_string())
            .with_output("stderr", stderr.to_string())
            .with_output("exit_code", exit_code as f64)
            .with_output("success", success)
            .with_output("duration_ms", duration_ms as f64))
    }
}

pub struct ZypiExecNodeFactory;

impl NodeFactory for ZypiExecNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(ZypiExecNode::new()))
    }

    fn node_type(&self) -> &str {
        "zypi.exec"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description:
                "Execute command in Zypi Firecracker microVM (sub-second boot)"
                    .to_string(),
            category: "zypi".to_string(),
            inputs: vec![
                PortDefinition {
                    name: "stdin".to_string(),
                    description: "Data to pipe to stdin".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "files".to_string(),
                    description: "Files to inject (Object of path→content)"
                        .to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "file:<path>".to_string(),
                    description:
                        "Individual file injection (e.g., file:/app/script.py)"
                            .to_string(),
                    required: false,
                },
            ],
            outputs: vec![
                PortDefinition {
                    name: "output".to_string(),
                    description:
                        "Command output (JSON-parsed if possible)"
                            .to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stdout".to_string(),
                    description: "Raw stdout".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stderr".to_string(),
                    description: "Raw stderr".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "exit_code".to_string(),
                    description: "Process exit code".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "success".to_string(),
                    description: "Whether command succeeded (exit 0)"
                        .to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "duration_ms".to_string(),
                    description: "Execution time in milliseconds"
                        .to_string(),
                    required: false,
                },
            ],
        }
    }
}
