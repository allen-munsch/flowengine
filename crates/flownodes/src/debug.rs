use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;

/// Simple debug node that logs its inputs
pub struct DebugNode;

#[async_trait]
impl Node for DebugNode {
    fn node_type(&self) -> &str {
        "debug.log"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let message = ctx.inputs.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("(no message)");
        
        ctx.events.info(format!("DEBUG: {}", message));
        
        // Also log all inputs for visibility
        for (key, value) in &ctx.inputs {
            ctx.events.info(format!("  {}: {:?}", key, value));
        }
        
        Ok(NodeOutput::new()
            .with_output("message", message.to_string()))
    }
}

pub struct DebugNodeFactory;

impl NodeFactory for DebugNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(DebugNode))
    }
    
    fn node_type(&self) -> &str {
        "debug.log"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Logs input values for debugging".to_string(),
            category: "debug".to_string(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}
