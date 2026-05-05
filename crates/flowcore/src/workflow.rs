use crate::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type WorkflowId = Uuid;
pub type NodeId = Uuid;

/// Complete workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: WorkflowId,
    pub name: String,
    pub description: Option<String>,
    pub nodes: Vec<NodeSpec>,
    pub connections: Vec<Connection>,
    pub triggers: Vec<TriggerSpec>,
    pub settings: WorkflowSettings,
}

impl Workflow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            nodes: Vec::new(),
            connections: Vec::new(),
            triggers: Vec::new(),
            settings: WorkflowSettings::default(),
        }
    }
    
    pub fn add_node(&mut self, node: NodeSpec) -> NodeId {
        let id = node.id;
        self.nodes.push(node);
        id
    }
    
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_port: impl Into<String>,
        to_node: NodeId,
        to_port: impl Into<String>,
    ) {
        self.connections.push(Connection {
            from_node,
            from_port: from_port.into(),
            to_node,
            to_port: to_port.into(),
        });
    }
    
    pub fn find_node(&self, id: NodeId) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Node specification in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: NodeId,
    pub node_type: String,
    pub name: Option<String>,
    pub config: HashMap<String, Value>,
    pub position: Option<Position>,
    pub retry_policy: Option<RetryPolicy>,
}

impl NodeSpec {
    pub fn new(node_type: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type: node_type.into(),
            name: None,
            config: HashMap::new(),
            position: None,
            retry_policy: None,
        }
    }
    
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }
    
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = Some(Position { x, y });
        self
    }
    
    pub fn with_retry(mut self, max_attempts: u32, delay_ms: u64) -> Self {
        self.retry_policy = Some(RetryPolicy {
            max_attempts,
            delay_ms,
            backoff_multiplier: 2.0,
            max_delay_ms: Some(60000),
            retry_on_timeout: true,
        });
        self
    }
}

/// Connection between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_port: String,
    pub to_node: NodeId,
    pub to_port: String,
}

/// Node position in visual editor
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Retry policy for node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
    pub max_delay_ms: Option<u64>,
    pub retry_on_timeout: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: Some(60000),
            retry_on_timeout: true,
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for a given attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let delay = self.delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay = delay as u64;
        if let Some(max) = self.max_delay_ms {
            delay.min(max)
        } else {
            delay
        }
    }
}

/// Workflow trigger specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerSpec {
    pub id: Uuid,
    pub trigger_type: TriggerType,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerType {
    Manual,
    Cron { expression: String },
    Webhook { path: String },
    Event { event_type: String },
}

/// Global workflow settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSettings {
    pub max_execution_time_ms: Option<u64>,
    pub max_parallel_nodes: usize,
    pub on_error: ErrorHandling,
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            max_execution_time_ms: None,
            max_parallel_nodes: 10,
            on_error: ErrorHandling::StopWorkflow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorHandling {
    StopWorkflow,
    ContinueOnError,
    RetryWorkflow { max_attempts: u32 },
}
