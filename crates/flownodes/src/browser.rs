//! Browser rendering node — headless Chromium in a Zypi sandbox
//!
//! This is a convenience wrapper around zypi.exec that pre-configures
//! the chromium-browser command with safe headless flags.
//!
//! Config:
//!   url          - URL to render
//!   html         - Inline HTML to render (written to /tmp/page.html in sandbox)
//!   mode         - "dom" (full DOM), "text" (extracted text via lynx), "screenshot" (base64 PNG)
//!   wait_ms      - Extra wait time for JS to execute (default: 1000)
//!   memory_mb    - VM memory in MB (default: 512)
//!   timeout      - Execution timeout in seconds (default: 30)

use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;

pub struct BrowserRenderNode;

#[async_trait]
impl Node for BrowserRenderNode {
    fn node_type(&self) -> &str {
        "browser.render"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let url = ctx.config.get("url").and_then(|v| v.as_str());
        let html = ctx.config.get("html").and_then(|v| v.as_str());
        let mode = ctx
            .config
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("dom");
        let wait_ms = ctx
            .config
            .get("wait_ms")
            .and_then(|v| v.as_f64())
            .unwrap_or(1000.0) as u64;
        let memory_mb = ctx
            .config
            .get("memory_mb")
            .and_then(|v| v.as_f64())
            .unwrap_or(512.0) as u64;
        let timeout = ctx
            .config
            .get("timeout")
            .and_then(|v| v.as_f64())
            .unwrap_or(30.0) as u64;
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

        // Inject HTML file if provided
        let mut files = HashMap::new();
        if let Some(html_content) = html {
            files.insert(
                "/tmp/page.html".to_string(),
                Value::String(html_content.to_string()),
            );
        }

        // Build the target
        let target = url.unwrap_or("file:///tmp/page.html");

        ctx.events.info(format!("🌐 Rendering: {}", target));

        match mode {
            "text" => {
                // Two-step: chromium dumps DOM, then lynx/elinks extracts text
                // If lynx isn't available, just return raw DOM
                let cmd = format!(
                    "chromium-browser --headless --disable-gpu --no-sandbox --disable-dev-shm-usage --single-process --dump-dom --virtual-time-budget={} '{}' 2>/dev/null | lynx -stdin -dump -nolist 2>/dev/null || chromium-browser --headless --disable-gpu --no-sandbox --disable-dev-shm-usage --single-process --dump-dom --virtual-time-budget={} '{}' 2>/dev/null",
                    wait_ms * 1000, target, wait_ms * 1000, target
                );
                ctx.events.info("  Mode: text (DOM → lynx/elinks extraction)");
                self.exec_in_sandbox(&ctx, &zypi_url, image, &cmd, timeout, memory_mb, files).await
            }
            "screenshot" => {
                let cmd = format!(
                    "chromium-browser --headless --disable-gpu --no-sandbox --disable-dev-shm-usage --single-process --screenshot=/tmp/output.png --window-size=1280,720 --virtual-time-budget={} '{}' 2>/dev/null && base64 /tmp/output.png",
                    wait_ms * 1000, target
                );
                ctx.events.info("  Mode: screenshot (1280x720 PNG, base64 output)");
                self.exec_in_sandbox(&ctx, &zypi_url, image, &cmd, timeout, memory_mb, files).await
            }
            _ => {
                let cmd = format!(
                    "chromium-browser --headless --disable-gpu --no-sandbox --disable-dev-shm-usage --single-process --dump-dom --virtual-time-budget={} '{}'",
                    wait_ms * 1000, target
                );
                ctx.events.info("  Mode: dom (raw HTML)");
                self.exec_in_sandbox(&ctx, &zypi_url, image, &cmd, timeout, memory_mb, files).await
            }
        }
    }
}

impl BrowserRenderNode {
    async fn exec_in_sandbox(
        &self,
        ctx: &NodeContext,
        zypi_url: &str,
        image: &str,
        command: &str,
        timeout: u64,
        memory_mb: u64,
        files: HashMap<String, Value>,
    ) -> Result<NodeOutput, NodeError> {
        // Build Zypi exec payload
        let mut payload = serde_json::Map::new();
        payload.insert(
            "cmd".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("/bin/sh".to_string()),
                serde_json::Value::String("-c".to_string()),
                serde_json::Value::String(command.to_string()),
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

        if !files.is_empty() {
            let file_map: serde_json::Map<String, serde_json::Value> = files
                .iter()
                .map(|(k, v)| {
                    let content = match v {
                        Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    (k.clone(), serde_json::Value::String(content))
                })
                .collect();
            payload.insert(
                "files".to_string(),
                serde_json::Value::Object(file_map),
            );
        }

        let client = reqwest::Client::new();
        let start = std::time::Instant::now();

        let response = client
            .post(format!("{}/exec", zypi_url))
            .json(&serde_json::Value::Object(payload))
            .timeout(std::time::Duration::from_secs(timeout + 10))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    NodeError::Timeout { seconds: timeout }
                } else if e.is_connect() {
                    NodeError::ExecutionFailed(format!(
                        "Cannot connect to Zypi at {}",
                        zypi_url
                    ))
                } else {
                    NodeError::ExecutionFailed(format!("Zypi error: {}", e))
                }
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let body: serde_json::Value =
            response.json().await.map_err(|e| {
                NodeError::ExecutionFailed(format!(
                    "Bad Zypi response: {}",
                    e
                ))
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
            ctx.events.warn(format!(
                "  Chromium exited with code {}: {}",
                exit_code, stderr
            ));
            return Err(NodeError::ExecutionFailed(format!(
                "Browser render failed (exit {}): {}",
                exit_code, stderr
            )));
        }

        ctx.events
            .info(format!("  ✅ Rendered in {}ms", duration_ms));

        let output_value =
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
                Value::Json(json)
            } else {
                Value::String(stdout.to_string())
            };

        Ok(NodeOutput::new()
            .with_output("output", output_value)
            .with_output("stdout", stdout.to_string())
            .with_output("duration_ms", duration_ms as f64))
    }
}

pub struct BrowserRenderNodeFactory;

impl NodeFactory for BrowserRenderNodeFactory {
    fn create(
        &self,
        _config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(BrowserRenderNode))
    }

    fn node_type(&self) -> &str {
        "browser.render"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description:
                "Render HTML/URL in headless Chromium inside a Firecracker sandbox"
                    .to_string(),
            category: "browser".to_string(),
            inputs: vec![],
            outputs: vec![
                PortDefinition {
                    name: "output".to_string(),
                    description:
                        "Rendered content (DOM, text, or base64 screenshot)"
                            .to_string(),
                    required: false,
                },
                PortDefinition {
                    name: "stdout".to_string(),
                    description: "Raw stdout from chromium".to_string(),
                    required: false,
                },
            ],
        }
    }
}
