use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;

/// HTTP request node
pub struct HttpRequestNode {
    client: reqwest::Client,
}

impl HttpRequestNode {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Node for HttpRequestNode {
    fn node_type(&self) -> &str {
        "http.request"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let url = ctx.require_input("url")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "url".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        let method_value = ctx.get_config_or("method", Value::String("GET".to_string()));
        let method = method_value.as_str().unwrap_or("GET");        
        
        ctx.events.info(format!("{} {}", method, url));
        
        let request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => {
                let mut req = self.client.post(url);
                if let Some(body) = ctx.inputs.get("body") {
                    if let Some(json) = body.as_json() {
                        req = req.json(json);
                    } else if let Some(text) = body.as_str() {
                        req = req.body(text.to_string());
                    }
                }
                req
            }
            "PUT" => {
                let mut req = self.client.put(url);
                if let Some(body) = ctx.inputs.get("body") {
                    if let Some(json) = body.as_json() {
                        req = req.json(json);
                    }
                }
                req
            }
            "DELETE" => self.client.delete(url),
            _ => return Err(NodeError::Configuration(format!("Unsupported method: {}", method))),
        };
        
        // Add headers if provided
        let request = if let Some(Value::Object(headers)) = ctx.config.get("headers") {
            let mut req = request;
            for (key, value) in headers {
                if let Some(val_str) = value.as_str() {
                    req = req.header(key, val_str);
                }
            }
            req
        } else {
            request
        };
        
        let response = request
            .send()
            .await
            .map_err(|e| NodeError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;
        
        let status = response.status().as_u16();
        let headers_map: HashMap<String, Value> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), Value::String(v.to_str().unwrap_or("").to_string())))
            .collect();
        
        let body_text = response
            .text()
            .await
            .map_err(|e| NodeError::ExecutionFailed(format!("Failed to read response: {}", e)))?;
        
        ctx.events.info(format!("Response status: {}", status));
        
        Ok(NodeOutput::new()
            .with_output("status", status as f64)
            .with_output("body", body_text.clone())
            .with_output("headers", Value::Object(headers_map)))
    }
}

pub struct HttpRequestNodeFactory;

impl NodeFactory for HttpRequestNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(HttpRequestNode::new()))
    }
    
    fn node_type(&self) -> &str {
        "http.request"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Make HTTP requests".to_string(),
            category: "http".to_string(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}
