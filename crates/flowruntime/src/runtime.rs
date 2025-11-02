use crate::{registry::NodeRegistry, WorkflowExecutor, ExecutionResult};
use flowcore::{EventBus, FlowError, Value, Workflow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main runtime for executing workflows
pub struct FlowRuntime {
    registry: Arc<NodeRegistry>,
    executor: Arc<WorkflowExecutor>,
    event_bus: Arc<EventBus>,
    workflows: Arc<RwLock<HashMap<uuid::Uuid, Workflow>>>,
}

impl FlowRuntime {
    /// Create a new runtime with default settings
    pub fn new() -> Self {
        Self::with_config(RuntimeConfig::default())
    }
    
    /// Create a new runtime with custom configuration
    pub fn with_config(config: RuntimeConfig) -> Self {
        let registry = Arc::new(NodeRegistry::new());
        Self::with_registry(registry, config)
    }
    
    /// Create a new runtime with a pre-configured registry
    pub fn with_registry(registry: Arc<NodeRegistry>, config: RuntimeConfig) -> Self {
        let executor = Arc::new(WorkflowExecutor::new(config.max_parallel_nodes));
        let event_bus = Arc::new(EventBus::new(config.event_buffer_size));
        
        Self {
            registry,
            executor,
            event_bus,
            workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get access to the node registry for registering node types
    pub fn registry(&self) -> &Arc<NodeRegistry> {
        &self.registry
    }
    
    /// Register a workflow
    pub async fn register_workflow(&self, workflow: Workflow) {
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.id, workflow);
    }
    
    /// Execute a workflow by ID
    pub async fn execute_workflow(
        &self,
        workflow_id: uuid::Uuid,
        inputs: HashMap<String, Value>,
    ) -> Result<ExecutionResult, FlowError> {
        let workflows = self.workflows.read().await;
        let workflow = workflows
            .get(&workflow_id)
            .ok_or_else(|| FlowError::Workflow(
                flowcore::WorkflowError::NotFound(workflow_id.to_string())
            ))?;
        
        self.executor
            .execute(workflow, &self.registry, &self.event_bus, inputs)
            .await
    }
    
    /// Execute a workflow directly (without registration)
    pub async fn execute(
        &self,
        workflow: &Workflow,
        inputs: HashMap<String, Value>,
    ) -> Result<ExecutionResult, FlowError> {
        self.executor
            .execute(workflow, &self.registry, &self.event_bus, inputs)
            .await
    }
    
    /// Subscribe to execution events
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<flowcore::ExecutionEvent> {
        self.event_bus.subscribe()
    }
    
    /// Get the event bus for direct access
    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }
}

impl Default for FlowRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the runtime
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub max_parallel_nodes: usize,
    pub event_buffer_size: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_parallel_nodes: 10,
            event_buffer_size: 1000,
        }
    }
}
