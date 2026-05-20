//! Generic API call node — runs Python scripts with pip dependencies in Zypi sandbox
//!
//! The "n8n marketplace" pattern: instead of a native Rust node per API,
//! use one generic node that installs SDKs and executes scripts.
//! Adding a new integration = adding a JSON workflow template, not code.
//!
//! Config:
//!   packages     - Pip packages to install (e.g., ["google-api-python-client", "slack-sdk"])
//!   script       - Python script to execute
//!   env          - API keys/tokens as env vars (passed securely, not in script)
//!   timeout      - Execution timeout (default: 60s)
//!   memory_mb    - VM memory (default: 256, bump for heavy SDKs)

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;

pub struct ApiCallNode {
    client: reqwest::Client,
}

impl ApiCallNode {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn build_script(ctx: &NodeContext) -> Result<String, NodeError> {
        let packages: Vec<String> = ctx
            .config
            .get("packages")
            .and_then(|v| match v {
                Value::Array(arr) => Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                ),
                Value::String(s) => Some(
                    s.split(',').map(|s| s.trim().to_string()).collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        let script = ctx
            .config
            .get("script")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                NodeError::Configuration("script is required".to_string())
            })?;

        let timeout = ctx
            .config
            .get("timeout")
            .and_then(|v| v.as_f64())
            .unwrap_or(60.0) as u64;

        // Build a self-contained script that installs packages then runs the user script
        let install_block = if packages.is_empty() {
            String::new()
        } else {
            let pkg_list = packages
                .iter()
                .map(|p| format!("'{}'", p))
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "import subprocess, sys\n\
                 pkgs = [{}]\n\
                 for pkg in pkgs:\n\
                     subprocess.check_call([sys.executable, '-m', 'pip', 'install', '--quiet', pkg])\n",
                pkg_list
            )
        };

        // Wrap user script in a try/except that outputs JSON errors
        let wrapped = format!(
            r#"{}
import json, traceback
try:
{}
    print(json.dumps({{"status": "ok"}}))
except Exception as e:
    print(json.dumps({{"status": "error", "error": str(e), "traceback": traceback.format_exc()}}), file=__import__('sys').stderr)
    raise SystemExit(1)
"#,
            install_block,
            script.lines()
                .map(|l| format!("    {}", l))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Ok(wrapped)
    }
}

#[async_trait]
impl Node for ApiCallNode {
    fn node_type(&self) -> &str {
        "api.call"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let script = Self::build_script(&ctx)?;

        let zypi_url = ctx
            .config
            .get("zypi_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:4000");

        let image = ctx
            .config
            .get("image")
            .and_then(|v| v.as_str())
            .unwrap_or("ubuntu:24.04");

        let timeout = ctx
            .config
            .get("timeout")
            .and_then(|v| v.as_f64())
            .unwrap_or(60.0) as u64;

        let memory_mb = ctx
            .config
            .get("memory_mb")
            .and_then(|v| v.as_f64())
            .unwrap_or(256.0) as u64;

        let node_name = ctx
            .config
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("api.call");

        ctx.events.info(format!("🔌 {} — pip install + execute in sandbox", node_name));

        // Build Zypi payload
        let mut payload = serde_json::Map::new();
        payload.insert(
            "cmd".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("/usr/bin/python3".to_string()),
                serde_json::Value::String("-c".to_string()),
                serde_json::Value::String(script),
            ]),
        );
        payload.insert(
            "image".to_string(),
            serde_json::Value::String(image.to_string()),
        );
        payload.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(timeout)),
        );
        payload.insert(
            "memory_mb".to_string(),
            serde_json::Value::Number(serde_json::Number::from(memory_mb)),
        );

        // Pass API keys as env vars from config and inputs
        let mut env = serde_json::Map::new();
        if let Some(Value::Object(config_env)) = ctx.config.get("env") {
            for (k, v) in config_env {
                if let Some(s) = v.as_str() {
                    env.insert(k.clone(), serde_json::Value::String(s.to_string()));
                }
            }
        }
        // Also pass workflow inputs as env vars (uppercased)
        for (key, value) in &ctx.inputs {
            let env_key = key.to_uppercase();
            match value {
                Value::String(s) => {
                    env.insert(env_key, serde_json::Value::String(s.clone()));
                }
                Value::Number(n) => {
                    env.insert(env_key, serde_json::Value::String(n.to_string()));
                }
                _ => {}
            }
        }
        if !env.is_empty() {
            payload.insert("env".to_string(), serde_json::Value::Object(env));
        }

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(format!("{}/exec", zypi_url))
            .json(&serde_json::Value::Object(payload))
            .timeout(std::time::Duration::from_secs(timeout + 30))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    NodeError::Timeout {
                        seconds: timeout,
                    }
                } else {
                    NodeError::ExecutionFailed(format!("Zypi error: {}", e))
                }
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let body: serde_json::Value =
            response.json().await.map_err(|e| {
                NodeError::ExecutionFailed(format!("Bad response: {}", e))
            })?;

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

        if exit_code != 0 {
            ctx.events.warn(format!("  stderr: {}", stderr));
            return Err(NodeError::ExecutionFailed(format!(
                "{} failed (exit {}): {}",
                node_name, exit_code, stderr
            )));
        }

        ctx.events
            .info(format!("  ✅ {} completed in {}ms", node_name, duration_ms));

        let output_value =
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
                Value::Json(json)
            } else {
                Value::String(stdout.to_string())
            };

        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout.to_string())
            .with_output("stderr", stderr.to_string())
            .with_output("duration_ms", duration_ms as f64))
    }
}

pub struct ApiCallNodeFactory;

impl NodeFactory for ApiCallNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(ApiCallNode::new()))
    }

    fn node_type(&self) -> &str {
        "api.call"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description:
                "Run Python scripts with pip packages in a Zypi sandbox. Use for any REST API integration — install SDKs, call endpoints, return JSON."
                    .to_string(),
            category: "api".to_string(),
            inputs: vec![PortDefinition {
                name: "stdin".to_string(),
                description: "Data piped to script's stdin".to_string(),
                required: false,
            }],
            outputs: vec![
                PortDefinition {
                    name: "output".to_string(),
                    description: "Script output (JSON-parsed if possible)"
                        .to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stdout".to_string(),
                    description: "Raw stdout".to_string(),
                    required: false,
                },
            ],
        }
    }
}
