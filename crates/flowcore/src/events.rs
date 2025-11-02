use crate::{NodeId, Value};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

pub type ExecutionId = Uuid;

/// Events emitted during workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecutionEvent {
    WorkflowStarted {
        execution_id: ExecutionId,
        workflow_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    WorkflowCompleted {
        execution_id: ExecutionId,
        success: bool,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    NodeStarted {
        execution_id: ExecutionId,
        node_id: NodeId,
        node_type: String,
        timestamp: DateTime<Utc>,
    },
    NodeCompleted {
        execution_id: ExecutionId,
        node_id: NodeId,
        outputs: std::collections::HashMap<String, Value>,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    NodeFailed {
        execution_id: ExecutionId,
        node_id: NodeId,
        error: String,
        timestamp: DateTime<Utc>,
    },
    NodeEvent {
        execution_id: ExecutionId,
        node_id: NodeId,
        event: NodeEvent,
        timestamp: DateTime<Utc>,
    },
}

/// Events specific to node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum NodeEvent {
    Info { message: String },
    Warning { message: String },
    Progress { percent: f64, message: Option<String> },
    Data { port: String, value: Value },
}

/// Event emitter for nodes to send real-time updates
#[derive(Clone)]
pub struct EventEmitter {
    execution_id: ExecutionId,
    node_id: NodeId,
    sender: broadcast::Sender<ExecutionEvent>,
}

impl EventEmitter {
    pub fn new(
        execution_id: ExecutionId,
        node_id: NodeId,
        sender: broadcast::Sender<ExecutionEvent>,
    ) -> Self {
        Self {
            execution_id,
            node_id,
            sender,
        }
    }
    
    /// Emit a node-specific event
    pub fn emit(&self, event: NodeEvent) {
        let _ = self.sender.send(ExecutionEvent::NodeEvent {
            execution_id: self.execution_id,
            node_id: self.node_id.clone(),
            event,
            timestamp: Utc::now(),
        });
    }
    
    /// Emit info message
    pub fn info(&self, message: impl Into<String>) {
        self.emit(NodeEvent::Info {
            message: message.into(),
        });
    }
    
    /// Emit warning message
    pub fn warn(&self, message: impl Into<String>) {
        self.emit(NodeEvent::Warning {
            message: message.into(),
        });
    }
    
    /// Emit progress update
    pub fn progress(&self, percent: f64, message: Option<String>) {
        self.emit(NodeEvent::Progress { percent, message });
    }
    
    /// Emit data on a specific port (for streaming)
    pub fn data(&self, port: impl Into<String>, value: Value) {
        self.emit(NodeEvent::Data {
            port: port.into(),
            value,
        });
    }
}

/// Global event bus
pub struct EventBus {
    sender: broadcast::Sender<ExecutionEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.sender.subscribe()
    }
    
    pub fn emit(&self, event: ExecutionEvent) {
        let _ = self.sender.send(event);
    }
    
    pub fn create_emitter(&self, execution_id: ExecutionId, node_id: NodeId) -> EventEmitter {
        EventEmitter::new(execution_id, node_id, self.sender.clone())
    }
}
