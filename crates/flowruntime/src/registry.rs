use flowcore::{Node, NodeError, Value, WorkflowError};
use std::collections::HashMap;
use std::sync::Arc;

/// Factory trait for creating node instances
pub trait NodeFactory: Send + Sync {
    /// Create a new instance of the node with given configuration
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError>;
    
    /// Get node type identifier
    fn node_type(&self) -> &str;
    
    /// Optional: Get node metadata (description, input/output schema, etc.)
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata::default()
    }
}

/// Metadata about a node type
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    pub description: String,
    pub category: String,
    pub inputs: Vec<PortDefinition>,
    pub outputs: Vec<PortDefinition>,
}

impl Default for NodeMetadata {
    fn default() -> Self {
        Self {
            description: String::new(),
            category: "general".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortDefinition {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// Registry of available node types
pub struct NodeRegistry {
    factories: HashMap<String, Arc<dyn NodeFactory>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }
    
    /// Register a node factory
    pub fn register(&mut self, factory: Arc<dyn NodeFactory>) {
        let node_type = factory.node_type().to_string();
        tracing::info!("Registering node type: {}", node_type);
        self.factories.insert(node_type, factory);
    }
    
    /// Create a node instance from a node type and config
    pub fn create_node(
        &self,
        node_type: &str,
        config: &HashMap<String, Value>,
    ) -> Result<Box<dyn Node>, WorkflowError> {
        let factory = self.factories.get(node_type)
            .ok_or_else(|| WorkflowError::UnknownNodeType(node_type.to_string()))?;
        
        factory.create(config)
            .map_err(|e| WorkflowError::Invalid(format!("Failed to create node: {}", e)))
    }
    
    /// Get all registered node types
    pub fn list_node_types(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }
    
    /// Get metadata for a node type
    pub fn get_metadata(&self, node_type: &str) -> Option<NodeMetadata> {
        self.factories.get(node_type).map(|f| f.metadata())
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
