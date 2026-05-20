use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

/// Delay execution for a specified duration
pub struct DelayNode;

#[async_trait]
impl Node for DelayNode {
    fn node_type(&self) -> &str {
        "time.delay"
    }
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let delay_ms = ctx.config.get("delay_ms")
            .and_then(|v| v.as_f64())
            .unwrap_or(1000.0) as u64;  // Default to 1 second if not specified
        
        ctx.events.info(format!("Delaying for {}ms", delay_ms));
        
        sleep(Duration::from_millis(delay_ms)).await;
        
        // Pass through any inputs
        let outputs = ctx.inputs.clone();
        
        Ok(NodeOutput {
            outputs,
            metadata: flowcore::NodeMetadata::default(),
        })
    }
}

pub struct DelayNodeFactory;

impl NodeFactory for DelayNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(DelayNode))
    }
    
    fn node_type(&self) -> &str {
        "time.delay"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Delay execution for specified milliseconds".to_string(),
            category: "time".to_string(),
            inputs: vec![],
            outputs: vec![],
        }
    }
}
