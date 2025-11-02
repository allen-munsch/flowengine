//! Core abstractions for the flow engine
//! 
//! This crate provides the fundamental types and traits that all other
//! components depend on. It has no runtime dependencies.

mod error;
mod events;
mod node;
mod value;
mod workflow;

pub use error::{FlowError, NodeError, WorkflowError};
pub use events::{ExecutionEvent, NodeEvent, EventEmitter, EventBus, ExecutionId};
pub use node::{Node, NodeContext, NodeOutput, NodeMetadata, NodeState};
pub use workflow::{
    Workflow, WorkflowId, NodeId, NodeSpec, Connection, 
    TriggerSpec, TriggerType, ErrorHandling  // <-- Add ErrorHandling
};
pub use value::Value;

/// Result type for flow operations
pub type Result<T> = std::result::Result<T, FlowError>;
