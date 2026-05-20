//! Zypi Firecracker microVM execution node
//!
//! Executes commands in Firecracker microVMs via Zypi's HTTP API.
//! Much faster than Docker: sub-second VM boot with CoW snapshots.
//!
//! API endpoints:
//!   POST /exec                    — One-shot command execution
//!   POST /sessions                — Create long-lived session
//!   POST /sessions/:id/exec       — Execute in existing session
//!   GET  /health                  — Health check
//!
//! Config:
//!   url          - Zypi server URL (default: http://localhost:4000)
//!   image        - Container image to use (default: ubuntu:24.04)
//!   command      - Command to execute (string or array)
//!   session_id   - Reuse an existing session (from zypi.session_create output)
//!   env          - Environment variables
//!   workdir      - Working directory
//!   timeout      - Execution timeout in seconds
//!   files        - Files to inject into the sandbox (from Blob inputs)
//!
//! Session chaining:
//!   Node 1: zypi.session_create → outputs session_id
//!   Node 2: zypi.exec {session_id: "$prev.session_id"} → reuses session
//!   Node 3: zypi.exec {session_id: "$prev.session_id"} → same warm VM
//!   Session auto-expires after 5min idle. Close explicitly for cleanup.

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
    session_id: Option<String>,
    env: HashMap<String, String>,
    workdir: Option<String>,
    timeout_seconds: Option<u64>,
    memory_mb: Option<u64>,
    vcpus: Option<u64>,
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

        // session_id: config takes priority, then input port
        let session_id = ctx
            .config
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                ctx.inputs
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            });

        let memory_mb = ctx
            .config
            .get("memory_mb")
            .and_then(|v| v.as_f64())
            .map(|f| f as u64);

        let vcpus = ctx
            .config
            .get("vcpus")
            .and_then(|v| v.as_f64())
            .map(|f| f as u64);

        Ok(Self {
            url,
            image,
            command,
            session_id,
            env,
            workdir,
            timeout_seconds,
            memory_mb,
            vcpus,
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

        // ── Try gRPC first (port 4001), fall back to REST ───────────────
        let grpc_url = config.url.replace(":4000", ":4001");
        let grpc_client = crate::zypi_grpc::ZypiGrpcClient::new(&grpc_url);

        // Merge config env with workflow inputs
        let mut env_vars = std::collections::HashMap::new();
        for (key, value) in &config.env {
            env_vars.insert(key.clone(), value.clone());
        }
        for (key, value) in &ctx.inputs {
            let env_key = key.to_uppercase();
            match value {
                Value::String(s) => { env_vars.insert(env_key, s.clone()); }
                Value::Number(n) => { env_vars.insert(env_key, n.to_string()); }
                Value::Bool(b) => { env_vars.insert(env_key, b.to_string()); }
                _ => {}
            }
        }

        let grpc_result = if let Some(ref sid) = config.session_id {
            crate::zypi_grpc::try_grpc_or_err(
                grpc_client.session_exec(
                    sid,
                    config.command.clone(),
                    config.timeout_seconds.unwrap_or(300) as u32,
                    env_vars.clone(),
                    config.workdir.as_deref(),
                )
            ).await
        } else {
            crate::zypi_grpc::try_grpc_or_err(
                grpc_client.execute(
                    config.command.clone(),
                    &config.image,
                    config.timeout_seconds.unwrap_or(300) as u32,
                    env_vars.clone(),
                    config.workdir.as_deref(),
                    config.memory_mb.map(|m| m as u32),
                    config.vcpus.map(|c| c as u32),
                )
            ).await
        };

        if let Ok(result) = grpc_result {
            if result.exit_code != 0 {
                ctx.events.warn(format!("  stderr: {}", result.stderr));
                return Err(NodeError::ExecutionFailed(format!(
                    "Zypi gRPC error (exit {}): {}",
                    result.exit_code, result.stderr
                )));
            }
            let output_value = if let Ok(json) =
                serde_json::from_str::<serde_json::Value>(&result.stdout)
            {
                Value::Json(json)
            } else {
                Value::String(result.stdout.clone())
            };
            let mut output = NodeOutput::new()
                .with_output("output", output_value)
                .with_output("stdout", result.stdout)
                .with_output("stderr", result.stderr)
                .with_output("exit_code", result.exit_code as f64)
                .with_output("success", result.exit_code == 0)
                .with_output("duration_ms", result.duration_ms as f64);
            if let Some(sid) = result.session_id {
                output = output.with_output("session_id", sid);
            }
            ctx.events.info(format!(
                "  ✅ Zypi gRPC completed in {}ms", result.duration_ms
            ));
            return Ok(output);
        }
        // ⬇ gRPC failed — fall through to REST below

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

        if let Some(mem) = config.memory_mb {
            payload.insert(
                "memory_mb".to_string(),
                serde_json::Value::Number(serde_json::Number::from(mem)),
            );
        }

        if let Some(cpu) = config.vcpus {
            payload.insert(
                "vcpus".to_string(),
                serde_json::Value::Number(serde_json::Number::from(cpu)),
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
        payload.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(timeout)),
        );

        // Route to session exec or one-shot exec based on session_id presence
        let (endpoint, log_msg) = if let Some(ref sid) = config.session_id {
            (
                format!("{}/sessions/{}/exec", config.url, sid),
                format!("  📡 POST {}/sessions/{}/exec (session reuse)", config.url, sid),
            )
        } else {
            (
                format!("{}/exec", config.url),
                format!("  📡 POST {}/exec", config.url),
            )
        };

        ctx.events.info(log_msg);

        // Build the request
        let mut request = self
            .client
            .post(&endpoint)
            .json(&serde_json::Value::Object(payload))
            .timeout(std::time::Duration::from_secs(timeout));

        // Propagate timeout to request
        if let Some(ts) = config.timeout_seconds {
            request = request
                .timeout(std::time::Duration::from_secs(ts));
        }

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

        // Extract session_id from response (present in session exec responses)
        let response_session_id = body
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Carry forward session_id: response takes priority, then config, then input
        let output_session_id = response_session_id.or(config.session_id);

        // Parse stdout as JSON if possible
        let output_value = if let Ok(json) =
            serde_json::from_str::<serde_json::Value>(stdout)
        {
            Value::Json(json)
        } else {
            Value::String(stdout.to_string())
        };

        let mut output = NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout.to_string())
            .with_output("stderr", stderr.to_string())
            .with_output("exit_code", exit_code as f64)
            .with_output("success", success)
            .with_output("duration_ms", duration_ms as f64);

        if let Some(sid) = output_session_id {
            output = output.with_output("session_id", sid);
        }

        Ok(output)
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
                    name: "session_id".to_string(),
                    description: "Reuse an existing session (from zypi.session_create output)".to_string(),
                    required: false,
                },
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
                PortDefinition {
                    name: "session_id".to_string(),
                    description: "Session ID (for chaining to next zypi.exec node)".to_string(),
                    required: false,
                },
            ],
        }
    }
}

// ── Session Create Node ───────────────────────────────────────

pub struct ZypiSessionCreateNode {
    client: reqwest::Client,
}

impl ZypiSessionCreateNode {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct SessionCreateConfig {
    url: String,
    image: String,
    agent_id: Option<String>,
    vcpus: u64,
    memory_mb: u64,
}

impl SessionCreateConfig {
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

        let agent_id = ctx
            .config
            .get("agent_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let vcpus = ctx
            .config
            .get("vcpus")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as u64;

        let memory_mb = ctx
            .config
            .get("memory_mb")
            .and_then(|v| v.as_f64())
            .unwrap_or(256.0) as u64;

        Ok(Self {
            url,
            image,
            agent_id,
            vcpus,
            memory_mb,
        })
    }
}

#[async_trait]
impl Node for ZypiSessionCreateNode {
    fn node_type(&self) -> &str {
        "zypi.session_create"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let config = SessionCreateConfig::from_ctx(&ctx)?;

        ctx.events.info(format!(
            "🔥 Zypi session create: {} ({})",
            config.image,
            if config.agent_id.is_some() { "agent" } else { "no agent" }
        ));

        // ── Try gRPC first (port 4001), fall back to REST ───────────────
        let grpc_url = config.url.replace(":4000", ":4001");
        let grpc_client = crate::zypi_grpc::ZypiGrpcClient::new(&grpc_url);

        let grpc_result = crate::zypi_grpc::try_grpc_or_err(
            grpc_client.create_session(
                &config.image,
                config.vcpus as u32,
                config.memory_mb as u32,
                config.agent_id.as_deref(),
            )
        ).await;

        if let Ok(session) = grpc_result {
            ctx.events.info(format!(
                "  ✅ Zypi gRPC session {} created (container: {}, ip: {})",
                session.session_id, session.container_id, session.ip
            ));
            return Ok(NodeOutput::new()
                .with_output("session_id", session.session_id)
                .with_output("container_id", session.container_id)
                .with_output("ip", session.ip)
                .with_output("image", session.image)
                .with_output("duration_ms", 0.0_f64));
        }
        // ⬇ gRPC failed — fall through to REST below

        let mut payload = serde_json::Map::new();
        payload.insert(
            "image".to_string(),
            serde_json::Value::String(config.image.clone()),
        );
        payload.insert(
            "vcpus".to_string(),
            serde_json::Value::Number(config.vcpus.into()),
        );
        payload.insert(
            "memory_mb".to_string(),
            serde_json::Value::Number(config.memory_mb.into()),
        );
        if let Some(ref agent_id) = config.agent_id {
            payload.insert(
                "agent_id".to_string(),
                serde_json::Value::String(agent_id.clone()),
            );
        }

        ctx.events
            .info(format!("  📡 POST {}/sessions", config.url));

        let start = std::time::Instant::now();

        let response = self
            .client
            .post(format!("{}/sessions", config.url))
            .json(&serde_json::Value::Object(payload))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    NodeError::ExecutionFailed(format!(
                        "Cannot connect to Zypi at {} — is it running? ({})",
                        config.url, e
                    ))
                } else {
                    NodeError::ExecutionFailed(format!(
                        "Zypi session create failed: {}", e
                    ))
                }
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let status_code = response.status().as_u16();
        let body: serde_json::Value = response.json().await.map_err(|e| {
            NodeError::ExecutionFailed(format!(
                "Failed to parse Zypi response: {}", e
            ))
        })?;

        if status_code != 201 {
            let err_msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(NodeError::ExecutionFailed(format!(
                "Zypi session create error ({}): {}",
                status_code, err_msg
            )));
        }

        let session_id = body
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                NodeError::ExecutionFailed(
                    "No session_id in response".to_string(),
                )
            })?;

        let container_id = body
            .get("container_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let ip = body
            .get("ip")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        ctx.events.info(format!(
            "  ✅ Session {} created in {}ms (container: {}, ip: {})",
            session_id, duration_ms, container_id, ip
        ));

        Ok(NodeOutput::new()
            .with_output("session_id", session_id.to_string())
            .with_output("container_id", container_id.to_string())
            .with_output("ip", ip.to_string())
            .with_output("image", config.image)
            .with_output("duration_ms", duration_ms as f64))
    }
}

pub struct ZypiSessionCreateNodeFactory;

impl NodeFactory for ZypiSessionCreateNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(ZypiSessionCreateNode::new()))
    }

    fn node_type(&self) -> &str {
        "zypi.session_create"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description:
                "Create a long-lived Zypi Firecracker microVM session for multi-step workflows"
                    .to_string(),
            category: "zypi".to_string(),
            inputs: vec![PortDefinition {
                name: "agent_id".to_string(),
                description: "Agent ID for memory attribution".to_string(),
                required: false,
            }],
            outputs: vec![
                PortDefinition {
                    name: "session_id".to_string(),
                    description: "Session ID — pass to zypi.exec nodes for chaining".to_string(),
                    required: true,
                },
                PortDefinition {
                    name: "container_id".to_string(),
                    description: "Container ID (for debugging)".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "ip".to_string(),
                    description: "VM IP address".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "image".to_string(),
                    description: "Image used for the session".to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "duration_ms".to_string(),
                    description: "Session creation time in milliseconds".to_string(),
                    required: false,
                },
            ],
        }
    }
}
