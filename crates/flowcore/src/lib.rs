//! Core abstractions for the flow engine
//! 
//! This crate provides the fundamental types and traits that all other
//! components depend on. It has no runtime dependencies.

mod error;
pub mod events;
mod node;
pub mod parser;
pub mod plugin;
mod value;
mod workflow;

pub use error::{FlowError, NodeError, WorkflowError};
pub use node::{Node, NodeContext, NodeOutput, NodeMetadata, NodeState, PortDefinition};
pub use workflow::{
    Workflow, WorkflowId, NodeId, NodeSpec, Connection, 
    TriggerSpec, TriggerType, ErrorHandling
};
pub use value::Value;
pub use parser::{WorkflowParser, JsonParser, YamlParser, parse_workflow, parse_workflow_file};
pub use plugin::{NodePlugin, PluginMetadata};
pub use events::*;

/// Result type for flow operations
pub type Result<T> = std::result::Result<T, FlowError>;
