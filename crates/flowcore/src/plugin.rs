use crate::{Node, NodeError};
use async_trait::async_trait;
use std::collections::HashMap;
use crate::Value;
use crate::node::PortDefinition;

/// Metadata describing a plugin node type.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
}

/// Trait implemented by dynamically-loaded node plugins.
/// Each plugin .so must export a `plugin_create()` function
/// returning a `Box<dyn NodePlugin>`.
#[async_trait]
pub trait NodePlugin: Send + Sync {
    /// Create a new instance of the node for a workflow execution.
    async fn create_node(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError>;

    /// Returns metadata about this plugin node type.
    fn metadata(&self) -> PluginMetadata;

    /// The node type string used in workflow definitions.
    fn node_type(&self) -> &str;

    /// The port definitions for this node type.
    fn port_definitions(&self) -> Vec<PortDefinition>;
}
