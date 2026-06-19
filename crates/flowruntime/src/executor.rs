use flowcore::{
    ExecutionEvent, EventBus, FlowError, Node, NodeContext, NodeId, 
    Value, Workflow, WorkflowError, ExecutionId,
};
use crate::executor_cache::ExecutorCache;
use crate::registry::NodeRegistry;
use crate::tracker::DependencyTracker;
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use flowpersist::PersistentStore;
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{timeout, Duration};

/// Executes workflows as DAGs with parallel execution
pub struct WorkflowExecutor {
    max_parallel: usize,
    cache: Option<Arc<ExecutorCache>>,
}

impl WorkflowExecutor {
    pub fn new(max_parallel: usize) -> Self {
        Self { max_parallel, cache: None }
    }

    /// Enable result caching with a persistent store.
    pub fn with_cache(mut self, store: Arc<PersistentStore>) -> Self {
        self.cache = Some(Arc::new(ExecutorCache::new(store)));
        self
    }
    
    /// Execute a workflow and return results
    pub async fn execute(
        &self,
        workflow: &Workflow,
        registry: &NodeRegistry,
        event_bus: &EventBus,
        initial_inputs: HashMap<String, Value>,
    ) -> Result<ExecutionResult, FlowError> {
        let execution_id = ExecutionId::new_v4();
        let start_time = Instant::now();
        
        // Emit workflow started event
        event_bus.emit(ExecutionEvent::WorkflowStarted {
            execution_id,
            workflow_id: workflow.id,
            timestamp: Utc::now(),
        });
        
        tracing::info!("Starting workflow execution: {}", workflow.id);
        
        // Build dependency graph
        let graph = self.build_graph(workflow)?;
        
        // Create node instances
        let mut node_instances = HashMap::new();
        for node_spec in &workflow.nodes {
            let mut node = registry.create_node(&node_spec.node_type, &node_spec.config)?;
            
            // Initialize node
            if let Err(e) = node.initialize().await {
                tracing::error!("Failed to initialize node {}: {}", node_spec.id, e);
                return Err(FlowError::Execution(format!("Node initialization failed: {}", e)));
            }
            
            node_instances.insert(node_spec.id, node);
        }
        
        // Execute the DAG
        let result = self.execute_dag(
            workflow,
            graph,
            node_instances,
            event_bus,
            execution_id,
            initial_inputs,
            self.cache.clone(),
        ).await;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = result.is_ok();
        
        // Emit workflow completed event
        event_bus.emit(ExecutionEvent::WorkflowCompleted {
            execution_id,
            success,
            duration_ms,
            timestamp: Utc::now(),
        });
        
        result
    }
    
    /// Build a dependency graph from the workflow
    fn build_graph(&self, workflow: &Workflow) -> Result<DiGraph<NodeId, ()>, WorkflowError> {
        let mut graph = DiGraph::new();
        let mut node_to_index = HashMap::new();
        
        // Add all nodes
        for node_spec in &workflow.nodes {
            let idx = graph.add_node(node_spec.id);
            node_to_index.insert(node_spec.id, idx);
        }
        
        // Add edges from connections
        for conn in &workflow.connections {
            let from_idx = node_to_index.get(&conn.from_node)
                .ok_or_else(|| WorkflowError::NodeNotFound(conn.from_node.to_string()))?;
            let to_idx = node_to_index.get(&conn.to_node)
                .ok_or_else(|| WorkflowError::NodeNotFound(conn.to_node.to_string()))?;
            
            graph.add_edge(*from_idx, *to_idx, ());
        }
        
        // Check for cycles
        if toposort(&graph, None).is_err() {
            return Err(WorkflowError::CyclicDependency);
        }
        
        Ok(graph)
    }
    
    /// Execute the DAG with parallelism
    async fn execute_dag(
        &self,
        workflow: &Workflow,
        _graph: DiGraph<NodeId, ()>,
        mut node_instances: HashMap<NodeId, Box<dyn Node>>,
        event_bus: &EventBus,
        execution_id: ExecutionId,
        initial_inputs: HashMap<String, Value>,
        cache: Option<Arc<ExecutorCache>>,
    ) -> Result<ExecutionResult, FlowError> {
        let mut node_outputs: HashMap<NodeId, HashMap<String, Value>> = HashMap::new();
        let mut running = FuturesUnordered::new();
        let mut tracker = DependencyTracker::new(workflow);
        
        let mut initial_map = HashMap::new();
        for (key, value) in initial_inputs {
            initial_map.insert(key, value);
        }
        if !initial_map.is_empty() {
            node_outputs.insert(NodeId::nil(), initial_map);
        }
        
        loop {
            let ready_nodes = tracker.ready_nodes();
            
            for node_id in ready_nodes {
                if running.len() >= self.max_parallel {
                    break;
                }
                
                let node_spec = workflow.find_node(node_id)
                    .ok_or_else(|| WorkflowError::NodeNotFound(node_id.to_string()))?;
                
                let node = node_instances.remove(&node_id)
                    .ok_or_else(|| WorkflowError::NodeNotFound(node_id.to_string()))?;
                
                let inputs = self.collect_node_inputs(node_id, workflow, &node_outputs);
                
                // Cache check
                let _cache_hit = if let Some(ref cache) = cache {
                    let config_hash = PersistentStore::compute_hash(&node_spec.config);
                    let input_hash = PersistentStore::compute_hash(&inputs);
                    let cached = cache.check(&node_spec.node_type, &config_hash, &input_hash).await;
                    if let Some(outputs) = cached {
                        tracing::debug!("Cache hit for node {}", node_id);
                        node_outputs.insert(node_id, outputs.into_iter()
                            .map(|(k, v)| (k, v.to_arc())).collect());
                        let downstream: HashSet<NodeId> = workflow.connections.iter()
                            .filter(|c| c.from_node == node_id)
                            .map(|c| c.to_node).collect();
                        for target in downstream {
                            tracker.mark_satisfied(target);
                        }
                        event_bus.emit(ExecutionEvent::NodeCompleted {
                            execution_id, node_id,
                            outputs: HashMap::new(),
                            duration_ms: 0,
                            timestamp: Utc::now(),
                        });
                        continue;
                    }
                    Some((config_hash, input_hash))
                } else {
                    None
                };
                
                let ctx = NodeContext {
                    node_id,
                    inputs,
                    config: node_spec.config.clone(),
                    state: Arc::new(tokio::sync::RwLock::new(flowcore::NodeState::default())),
                    events: event_bus.create_emitter(execution_id, node_id),
                    cancellation: tokio_util::sync::CancellationToken::new(),
                };
                
                event_bus.emit(ExecutionEvent::NodeStarted {
                    execution_id,
                    node_id,
                    node_type: node_spec.node_type.clone(),
                    timestamp: Utc::now(),
                });
                
                let retry_policy = node_spec.retry_policy.clone();
                let task = async move {
                    let mut last_error = None;
                    let max_attempts = retry_policy.as_ref()
                        .map(|r| r.max_attempts)
                        .unwrap_or(1);
                    for attempt in 0..max_attempts {
                        if attempt > 0 {
                            let delay_ms = retry_policy.as_ref()
                                .map(|r| r.delay_for_attempt(attempt))
                                .unwrap_or(1000);
                            tracing::warn!("Retrying node {} (attempt {}/{}) after {}ms",
                                node_id, attempt + 1, max_attempts, delay_ms);
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        let start = Instant::now();
                        let result = node.execute(ctx.clone()).await;
                        let duration_ms = start.elapsed().as_millis() as u64;
                        match result {
                            Ok(output) => return (node_id, Ok(output), duration_ms),
                            Err(e) => {
                                let is_timeout = matches!(e, flowcore::NodeError::Timeout { .. });
                                let retry_on_timeout = retry_policy.as_ref()
                                    .map(|r| r.retry_on_timeout).unwrap_or(true);
                                if is_timeout && !retry_on_timeout {
                                    return (node_id, Err(e), duration_ms);
                                }
                                last_error = Some(e);
                            }
                        }
                    }
                    (node_id, Err(last_error.unwrap()), 0)
                };
                
                if let Some(timeout_ms) = workflow.settings.max_execution_time_ms {
                    let duration = Duration::from_millis(timeout_ms);
                    let task_with_timeout = async move {
                        match timeout(duration, task).await {
                            Ok(result) => result,
                            Err(_) => (node_id, Err(flowcore::NodeError::Timeout {
                                seconds: timeout_ms / 1000
                            }), timeout_ms)
                        }
                    };
                    running.push(tokio::spawn(task_with_timeout));
                } else {
                    running.push(tokio::spawn(task));
                }
            }
            
            if running.is_empty() && tracker.all_scheduled() {
                break;
            }
            
            if let Some(result) = running.next().await {
                let (node_id, exec_result, duration_ms) = result
                    .map_err(|e| FlowError::Execution(format!("Task join error: {}", e)))?;
                
                match exec_result {
                    Ok(output) => {
                        tracing::info!("Node {} completed in {}ms", node_id, duration_ms);
                        event_bus.emit(ExecutionEvent::NodeCompleted {
                            execution_id, node_id,
                            outputs: output.outputs.clone(),
                            duration_ms,
                            timestamp: Utc::now(),
                        });
                        node_outputs.insert(node_id,
                            output.outputs.clone().into_iter()
                                .map(|(k, v)| (k, v.to_arc()))
                                .collect());
                        // Store in cache if enabled
                        if let Some(ref cache) = cache {
                            if let Some(node_spec) = workflow.find_node(node_id) {
                                let config_hash = PersistentStore::compute_hash(&node_spec.config);
                                let inputs = self.collect_node_inputs(node_id, workflow, &node_outputs);
                                let input_hash = PersistentStore::compute_hash(&inputs);
                                let outputs_map: HashMap<String, Value> = output.outputs.clone();
                                cache.store(
                                    &node_spec.node_type, &config_hash, &input_hash,
                                    &outputs_map, None,
                                ).await;
                            }
                        }
                        let downstream: HashSet<NodeId> = workflow.connections.iter()
                            .filter(|c| c.from_node == node_id)
                            .map(|c| c.to_node)
                            .collect();
                        for target in downstream {
                            tracker.mark_satisfied(target);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Node {} failed: {}", node_id, e);
                        event_bus.emit(ExecutionEvent::NodeFailed {
                            execution_id, node_id,
                            error: e.to_string(),
                            timestamp: Utc::now(),
                        });
                        match workflow.settings.on_error {
                            flowcore::ErrorHandling::StopWorkflow => {
                                return Err(FlowError::Execution(format!(
                                    "Node {} failed: {}", node_id, e)));
                            }
                            flowcore::ErrorHandling::ContinueOnError => {
                                let downstream: HashSet<NodeId> = workflow.connections.iter()
                                    .filter(|c| c.from_node == node_id)
                                    .map(|c| c.to_node)
                                    .collect();
                                for target in downstream {
                                    tracker.mark_satisfied(target);
                                }
                            }
                            flowcore::ErrorHandling::RetryWorkflow { .. } => {
                                return Err(FlowError::Execution(format!(
                                    "Node {} failed: {}", node_id, e)));
                            }
                        }
                    }
                }
            }
        }
        
        let total_nodes = workflow.nodes.len();
        let completed_count = total_nodes - node_instances.len();
        Ok(ExecutionResult {
            execution_id,
            outputs: node_outputs,
            completed_nodes: completed_count,
            total_nodes,
        })
    }
    
    /// Collect inputs for a node from its predecessors
    fn collect_node_inputs(
        &self,
        node_id: NodeId,
        workflow: &Workflow,
        node_outputs: &HashMap<NodeId, HashMap<String, Value>>,
    ) -> HashMap<String, Value> {
        let mut inputs = HashMap::new();
        
        // Check if this node has any incoming connections
        let has_predecessors = workflow.connections.iter()
            .any(|conn| conn.to_node == node_id);
        
        // If no predecessors, use initial inputs from NodeId::nil()
        if !has_predecessors {
            if let Some(initial_inputs) = node_outputs.get(&NodeId::nil()) {
                inputs.extend(initial_inputs.clone());
            }
        }
        
        // Find connections leading to this node
        for conn in &workflow.connections {
            if conn.to_node == node_id {
                if let Some(outputs) = node_outputs.get(&conn.from_node) {
                    if let Some(value) = outputs.get(&conn.from_port) {
                        inputs.insert(conn.to_port.clone(), value.clone());
                    }
                }
            }
        }
        
        inputs
    }
}

/// Result of workflow execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub execution_id: ExecutionId,
    pub outputs: HashMap<NodeId, HashMap<String, Value>>,
    pub completed_nodes: usize,
    pub total_nodes: usize,
}

/// Handle for monitoring execution
pub struct ExecutionHandle {
    pub execution_id: ExecutionId,
    // TODO: Add methods for cancellation, status queries, etc.
}
