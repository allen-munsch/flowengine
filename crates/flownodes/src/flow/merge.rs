use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value, PortDefinition};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;

/// Collects all upstream inputs and emits them as a single merged output.
///
/// Inputs: all connected upstream ports
/// Outputs:
///   - `merged`: an Object containing all input key/value pairs
pub struct MergeNode;

#[async_trait]
impl Node for MergeNode {
    fn node_type(&self) -> &str {
        "flow.merge"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let mut merged = std::collections::HashMap::new();
        for (key, value) in &ctx.inputs {
            merged.insert(key.clone(), value.clone());
        }
        Ok(NodeOutput::new().with_output("merged", Value::Object(merged)))
    }
}

pub struct MergeNodeFactory;

impl NodeFactory for MergeNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(MergeNode))
    }

    fn node_type(&self) -> &str {
        "flow.merge"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Collects all upstream inputs into a merged object".to_string(),
            category: "flow".to_string(),
            inputs: vec![],
            outputs: vec![
                PortDefinition { name: "merged".to_string(), description: "Combined input values".to_string(), required: false },
            ],
        }
    }
}
