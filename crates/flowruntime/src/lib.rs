//! Workflow execution runtime
//! 
//! This crate provides the actual execution engine that runs workflows,
//! manages the node registry, and handles DAG-based parallel execution.

mod executor;
mod executor_cache;
mod dyn_loader;
mod loader;
mod registry;
mod runtime;
pub mod tracker;

pub use executor::{WorkflowExecutor, ExecutionResult, ExecutionHandle};
pub use executor_cache::ExecutorCache;
pub use dyn_loader::DynamicLoader;
pub use loader::CustomNodeLoader;
pub use registry::{NodeFactory, NodeMetadata, PortDefinition, NodeRegistry};
pub use runtime::{FlowRuntime, RuntimeConfig};
pub use tracker::DependencyTracker;
