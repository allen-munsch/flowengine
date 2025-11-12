//! Core abstractions for the flow engine
//! 
//! This crate provides the fundamental types and traits that all other
//! components depend on. It has no runtime dependencies.

mod error;
pub mod events;
mod node;
mod value;
mod workflow;

pub use error::{FlowError, NodeError, WorkflowError};
pub use node::{Node, NodeContext, NodeOutput, NodeMetadata, NodeState};
pub use workflow::{
    Workflow, WorkflowId, NodeId, NodeSpec, Connection, 
    TriggerSpec, TriggerType, ErrorHandling  // <-- Add ErrorHandling
};
pub use value::Value;
pub use events::*;

/// Result type for flow operations
pub type Result<T> = std::result::Result<T, FlowError>;
