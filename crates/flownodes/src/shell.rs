//! Shell/process execution node
//!
//! Runs local processes with full configuration:
//! - command, args, env, workdir, stdin, timeout
//! - Streaming stdout/stderr via events
//! - File injection from Blob inputs

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct ShellExecNode;

#[derive(Debug, Clone)]
struct ShellConfig {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    workdir: Option<String>,
    timeout_seconds: Option<u64>,
    shell: bool,
    capture_stdout: bool,
    capture_stderr: bool,
    stream_output: bool,
    strip_trailing_newline: bool,
}

impl ShellConfig {
    fn from_ctx(ctx: &NodeContext) -> Result<Self, NodeError> {
        let command = ctx
            .require_config("command")?
            .as_str()
            .ok_or_else(|| {
                NodeError::Configuration("command must be a string".to_string())
            })?
            .to_string();

        let args: Vec<String> = ctx
            .config
            .get("args")
            .and_then(|v| match v {
                Value::Array(arr) => Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                ),
                Value::String(s) => Some(
                    s.split_whitespace().map(String::from).collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        let mut env = HashMap::new();
        if let Some(Value::Object(env_obj)) = ctx.config.get("env") {
            for (key, value) in env_obj {
                if let Some(val_str) = value.as_str() {
                    env.insert(key.clone(), val_str.to_string());
                }
            }
        }

        // Passthrough env vars
        if let Some(Value::Array(pass)) = ctx.config.get("env_passthrough") {
            for var in pass {
                if let Some(var_name) = var.as_str() {
                    if let Ok(val) = std::env::var(var_name) {
                        env.insert(var_name.to_string(), val);
                    }
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

        let shell = ctx
            .config
            .get("shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let capture_stdout = ctx
            .config
            .get("capture_stdout")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let capture_stderr = ctx
            .config
            .get("capture_stderr")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let stream_output = ctx
            .config
            .get("stream_output")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let strip_trailing_newline = ctx
            .config
            .get("strip_trailing_newline")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(Self {
            command,
            args,
            env,
            workdir,
            timeout_seconds,
            shell,
            capture_stdout,
            capture_stderr,
            stream_output,
            strip_trailing_newline,
        })
    }
}

#[async_trait]
impl Node for ShellExecNode {
    fn node_type(&self) -> &str {
        "shell.exec"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let config = ShellConfig::from_ctx(&ctx)?;

        ctx.events.info(format!(
            "🖥️  Running: {} {}",
            config.command,
            config.args.join(" ")
        ));

        let mut cmd = if config.shell {
            let mut c = Command::new("sh");
            c.arg("-c");
            let full_cmd = if config.args.is_empty() {
                config.command.clone()
            } else {
                format!("{} {}", config.command, config.args.join(" "))
            };
            c.arg(full_cmd);
            c
        } else {
            let mut c = Command::new(&config.command);
            for arg in &config.args {
                c.arg(arg);
            }
            c
        };

        // Environment
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Working directory
        if let Some(ref wd) = config.workdir {
            cmd.current_dir(wd);
        }

        // Stdio
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Kill on drop
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            NodeError::ExecutionFailed(format!("Failed to spawn process: {}", e))
        })?;

        // Write stdin from input
        if let Some(stdin_data) = ctx.inputs.get("stdin") {
            let data = match stdin_data {
                Value::String(s) => s.as_bytes().to_vec(),
                Value::Bytes(b) => b.clone(),
                Value::Json(j) => j.to_string().as_bytes().to_vec(),
                other => other.to_string().as_bytes().to_vec(),
            };
            if !data.is_empty() {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin
                        .write_all(&data)
                        .await
                        .map_err(|e| NodeError::ExecutionFailed(format!("Stdin write: {}", e)))?;
                    drop(stdin);
                }
            }
        }

        // Take stdout/stderr handles
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Read stdout with optional streaming
        let ctx_for_stdout = ctx.clone();
        let stream_stdout = config.stream_output;
        let stdout_task = async move {
            let mut all_data = Vec::new();
            if let Some(stdout) = stdout_handle {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if stream_stdout {
                        ctx_for_stdout.events.stdout_line(&line);
                    }
                    all_data.extend_from_slice(line.as_bytes());
                    all_data.push(b'\n');
                }
            }
            all_data
        };

        let ctx_for_stderr = ctx.clone();
        let stream_stderr = config.stream_output;
        let stderr_task = async move {
            let mut all_data = Vec::new();
            if let Some(stderr) = stderr_handle {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if stream_stderr {
                        ctx_for_stderr.events.stderr_line(&line);
                    }
                    all_data.extend_from_slice(line.as_bytes());
                    all_data.push(b'\n');
                }
            }
            all_data
        };

        // Wait for process with optional timeout
        let (status, stdout_data, stderr_data) =
            if let Some(timeout_secs) = config.timeout_seconds {
                let duration = tokio::time::Duration::from_secs(timeout_secs);
                let result = tokio::time::timeout(
                    duration,
                    async {
                        let (stdout, stderr) =
                            tokio::join!(stdout_task, stderr_task);
                        let status = child.wait().await.map_err(|e| {
                            NodeError::ExecutionFailed(format!(
                                "Process wait failed: {}",
                                e
                            ))
                        })?;
                        Ok::<_, NodeError>((status, stdout, stderr))
                    },
                )
                .await;

                match result {
                    Ok(Ok(data)) => data,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        ctx.events.warn(format!(
                            "Process timeout after {}s, killing...",
                            timeout_secs
                        ));
                        let _ = child.kill().await;
                        return Err(NodeError::Timeout {
                            seconds: timeout_secs,
                        });
                    }
                }
            } else {
                let (stdout, stderr) = tokio::join!(stdout_task, stderr_task);
                let status = child.wait().await.map_err(|e| {
                    NodeError::ExecutionFailed(format!(
                        "Process wait failed: {}",
                        e
                    ))
                })?;
                (status, stdout, stderr)
            };

        let mut stdout_str = String::from_utf8_lossy(&stdout_data).to_string();
        let stderr_str = String::from_utf8_lossy(&stderr_data).to_string();

        if config.strip_trailing_newline {
            while stdout_str.ends_with('\n') {
                stdout_str.pop();
            }
        }

        let exit_code = status.code().unwrap_or(-1);
        let success = status.success();

        if success {
            ctx.events
                .info(format!("  ✅ Completed (exit: {})", exit_code));
        } else {
            ctx.events
                .warn(format!("  ⚠️  Exited with code: {}", exit_code));
        }

        // Try parsing stdout as JSON
        let output_value = if let Ok(json) =
            serde_json::from_str::<serde_json::Value>(&stdout_str)
        {
            Value::Json(json)
        } else {
            Value::String(stdout_str.clone())
        };

        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout_str)
            .with_output("stderr", stderr_str)
            .with_output("exit_code", exit_code as f64)
            .with_output("success", success))
    }
}

pub struct ShellExecNodeFactory;

impl NodeFactory for ShellExecNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(ShellExecNode))
    }

    fn node_type(&self) -> &str {
        "shell.exec"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Execute a local shell command with streaming output"
                .to_string(),
            category: "shell".to_string(),
            inputs: vec![
                flowruntime::PortDefinition {
                    name: "stdin".to_string(),
                    description: "Data to pipe to stdin".to_string(),
                    required: false,
                },
            ],
            outputs: vec![
                flowruntime::PortDefinition {
                    name: "output".to_string(),
                    description: "Command output (JSON-parsed if possible)"
                        .to_string(),
                    required: false,
                },
                flowruntime::PortDefinition {
                    name: "stdout".to_string(),
                    description: "Raw stdout".to_string(),
                    required: false,
                },
                flowruntime::PortDefinition {
                    name: "stderr".to_string(),
                    description: "Raw stderr".to_string(),
                    required: false,
                },
                flowruntime::PortDefinition {
                    name: "exit_code".to_string(),
                    description: "Process exit code".to_string(),
                    required: false,
                },
                flowruntime::PortDefinition {
                    name: "success".to_string(),
                    description: "Whether process succeeded (exit 0)"
                        .to_string(),
                    required: false,
                },
            ],
        }
    }
}
