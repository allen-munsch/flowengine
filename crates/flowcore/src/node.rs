use crate::{NodeError, Value, events::EventEmitter};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub type NodeId = Uuid;

/// Core trait that all executable nodes implement
#[async_trait]
pub trait Node: Send + Sync {
    /// Unique type identifier (e.g., "http.request", "transform.json")
    fn node_type(&self) -> &str;
    
    /// Execute the node with given context
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError>;
    
    /// Optional: Initialize stateful resources (DB connections, etc.)
    async fn initialize(&mut self) -> Result<(), NodeError> {
        Ok(())
    }
    
    /// Optional: Cleanup resources
    async fn shutdown(&mut self) -> Result<(), NodeError> {
        Ok(())
    }
    
    /// Optional: Validate configuration at workflow load time
    fn validate_config(&self, _config: &HashMap<String, Value>) -> Result<(), NodeError> {
        Ok(())
    }
}

/// Execution context passed to each node
#[derive(Clone)]
pub struct NodeContext {
    /// Unique node instance ID
    pub node_id: NodeId,
    
    /// Input values from connected nodes
    pub inputs: HashMap<String, Value>,
    
    /// Static configuration for this node
    pub config: HashMap<String, Value>,
    
    /// Persistent state (survives across executions in same workflow run)
    pub state: Arc<RwLock<NodeState>>,
    
    /// Event emitter for real-time updates
    pub events: EventEmitter,
    
    /// Cancellation token for graceful shutdown
    pub cancellation: tokio_util::sync::CancellationToken,
}

impl NodeContext {
    pub fn new(node_id: NodeId, events: EventEmitter) -> Self {
        Self {
            node_id,
            inputs: HashMap::new(),
            config: HashMap::new(),
            state: Arc::new(RwLock::new(NodeState::default())),
            events,
            cancellation: tokio_util::sync::CancellationToken::new(),
        }
    }
    
    /// Get required input or return error
    pub fn require_input(&self, name: &str) -> Result<&Value, NodeError> {
        self.inputs.get(name)
            .ok_or_else(|| NodeError::MissingInput(name.to_string()))
    }
    
    /// Get config value or return error
    pub fn require_config(&self, name: &str) -> Result<&Value, NodeError> {
        self.config.get(name)
            .ok_or_else(|| NodeError::Configuration(format!("Missing config: {}", name)))
    }
    
    /// Get config with default
    pub fn get_config_or(&self, name: &str, default: Value) -> Value {
        self.config.get(name).cloned().unwrap_or(default)
    }
}

/// Persistent state for a node instance
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub data: HashMap<String, Value>,
}

/// Output from node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    /// Output port values
    pub outputs: HashMap<String, Value>,
    
    /// Execution metadata
    pub metadata: NodeMetadata,
}

impl NodeOutput {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            metadata: NodeMetadata::default(),
        }
    }
    
    pub fn with_output(mut self, port: impl Into<String>, value: impl Into<Value>) -> Self {
        self.outputs.insert(port.into(), value.into());
        self
    }
}

impl Default for NodeOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub execution_time_ms: u64,
    pub memory_used_bytes: Option<u64>,
    pub custom: HashMap<String, Value>,
}

impl Default for NodeMetadata {
    fn default() -> Self {
        Self {
            execution_time_ms: 0,
            memory_used_bytes: None,
            custom: HashMap::new(),
        }
    }
}
