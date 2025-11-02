//! Workflow execution runtime
//! 
//! This crate provides the actual execution engine that runs workflows,
//! manages the node registry, and handles DAG-based parallel execution.

mod executor;
mod registry;
mod runtime;

pub use executor::{WorkflowExecutor, ExecutionResult, ExecutionHandle};
pub use registry::{NodeFactory, NodeMetadata, PortDefinition, NodeRegistry};
pub use runtime::{FlowRuntime, RuntimeConfig};
