use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;

/// Parse JSON string to Value
pub struct JsonParseNode;

#[async_trait]
impl Node for JsonParseNode {
    fn node_type(&self) -> &str {
        "transform.json_parse"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let input = ctx.require_input("json")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "json".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        
        let parsed: serde_json::Value = serde_json::from_str(input)
            .map_err(|e| NodeError::ExecutionFailed(format!("JSON parse error: {}", e)))?;
        
        Ok(NodeOutput::new()
            .with_output("parsed", Value::Json(parsed)))
    }
}

pub struct JsonParseNodeFactory;

impl NodeFactory for JsonParseNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(JsonParseNode))
    }
    
    fn node_type(&self) -> &str {
        "transform.json_parse"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Parse JSON string".to_string(),
            category: "transform".to_string(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}

/// Stringify Value to JSON
pub struct JsonStringifyNode;

#[async_trait]
impl Node for JsonStringifyNode {
    fn node_type(&self) -> &str {
        "transform.json_stringify"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let value = ctx.require_input("value")?;
        
        let json_str = serde_json::to_string_pretty(value)
            .map_err(|e| NodeError::ExecutionFailed(format!("JSON stringify error: {}", e)))?;
        
        Ok(NodeOutput::new()
            .with_output("json", json_str))
    }
}

pub struct JsonStringifyNodeFactory;

impl NodeFactory for JsonStringifyNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(JsonStringifyNode))
    }
    
    fn node_type(&self) -> &str {
        "transform.json_stringify"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Convert value to JSON string".to_string(),
            category: "transform".to_string(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}
