use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value, PortDefinition};
use flowruntime::{NodeFactory, NodeMetadata};
use std::collections::HashMap;

/// Routes an input payload to one of two output ports based on a condition.
///
/// Inputs:
///   - `condition`: boolean (true/false) deciding which output port gets the payload
///   - `payload`:  the value to route
///
/// Outputs (only one is set per execution):
///   - `true_out`:  set when condition is truthy
///   - `false_out`: set when condition is falsy
pub struct BranchNode;

#[async_trait]
impl Node for BranchNode {
    fn node_type(&self) -> &str {
        "flow.branch"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let condition = ctx
            .inputs
            .get("condition")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let payload = ctx.inputs.get("payload").cloned();

        let mut output = NodeOutput::new();
        if condition {
            if let Some(p) = payload {
                output = output.with_output("true_out", p);
            }
        } else if let Some(p) = payload {
            output = output.with_output("false_out", p);
        }

        Ok(output)
    }
}

pub struct BranchNodeFactory;

impl NodeFactory for BranchNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(BranchNode))
    }

    fn node_type(&self) -> &str {
        "flow.branch"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Routes payload to true_out or false_out based on condition".to_string(),
            category: "flow".to_string(),
            inputs: vec![
                PortDefinition { name: "condition".to_string(), description: "Boolean condition".to_string(), required: true },
                PortDefinition { name: "payload".to_string(), description: "Value to route".to_string(), required: false },
            ],
            outputs: vec![
                PortDefinition { name: "true_out".to_string(), description: "Payload when condition is true".to_string(), required: false },
                PortDefinition { name: "false_out".to_string(), description: "Payload when condition is false".to_string(), required: false },
            ],
        }
    }
}
