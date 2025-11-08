use flowcore::{
    ExecutionEvent, EventBus, FlowError, Node, NodeContext, NodeId, 
    Value, Workflow, WorkflowError, ExecutionId,
};
use crate::registry::NodeRegistry;
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{timeout, Duration};

/// Executes workflows as DAGs with parallel execution
pub struct WorkflowExecutor {
    max_parallel: usize,
}

impl WorkflowExecutor {
    pub fn new(max_parallel: usize) -> Self {
        Self { max_parallel }
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
        graph: DiGraph<NodeId, ()>,
        mut node_instances: HashMap<NodeId, Box<dyn Node>>,
        event_bus: &EventBus,
        execution_id: ExecutionId,
        initial_inputs: HashMap<String, Value>,
    ) -> Result<ExecutionResult, FlowError> {
        let mut completed = HashSet::new();
        let mut node_outputs: HashMap<NodeId, HashMap<String, Value>> = HashMap::new();
        let mut running = FuturesUnordered::new();
        let node_to_index: HashMap<NodeId, NodeIndex> = graph
            .node_indices()
            .map(|idx| (*graph.node_weight(idx).unwrap(), idx))
            .collect();
        
        // Store initial inputs for nodes without dependencies
        let mut initial_map = HashMap::new();
        for (key, value) in initial_inputs {
            initial_map.insert(key, value);
        }
        if !initial_map.is_empty() {
            node_outputs.insert(NodeId::nil(), initial_map);
        }
        
        loop {
            // Find nodes ready to execute (all dependencies completed)
            let ready_nodes = self.find_ready_nodes(&graph, &node_to_index, &completed);
            
            // Spawn tasks for ready nodes up to parallel limit
            for node_id in ready_nodes {
                if running.len() >= self.max_parallel {
                    break;
                }
                
                let node_spec = workflow.find_node(node_id)
                    .ok_or_else(|| WorkflowError::NodeNotFound(node_id.to_string()))?;
                
                let node = node_instances.remove(&node_id)
                    .ok_or_else(|| WorkflowError::NodeNotFound(node_id.to_string()))?;
                
                // Collect inputs from predecessor nodes
                let inputs = self.collect_node_inputs(
                    node_id,
                    workflow,
                    &graph,
                    &node_to_index,
                    &node_outputs,
                );
                
                let ctx = NodeContext {
                    node_id,
                    inputs,
                    config: node_spec.config.clone(),
                    state: Arc::new(tokio::sync::RwLock::new(flowcore::NodeState::default())),
                    events: event_bus.create_emitter(execution_id, node_id),
                    cancellation: tokio_util::sync::CancellationToken::new(),
                };
                
                // Emit node started event
                event_bus.emit(ExecutionEvent::NodeStarted {
                    execution_id,
                    node_id,
                    node_type: node_spec.node_type.clone(),
                    timestamp: Utc::now(),
                });
                
                // Spawn execution task
                let task = async move {
                    let start = Instant::now();
                    let result = node.execute(ctx).await;
                    let duration_ms = start.elapsed().as_millis() as u64;
                    (node_id, result, duration_ms)
                };
                
                // Apply timeout if specified
                if let Some(timeout_ms) = workflow.settings.max_execution_time_ms {
                    let duration = Duration::from_millis(timeout_ms);
                    let task_with_timeout = async move {
                        match timeout(duration, task).await {
                            Ok(result) => result,
                            Err(_) => {
                                // Timeout occurred
                                (node_id, Err(flowcore::NodeError::Timeout { 
                                    seconds: timeout_ms / 1000 
                                }), timeout_ms)
                            }
                        }
                    };
                    
                    running.push(tokio::spawn(task_with_timeout));
                } else {
                    running.push(tokio::spawn(task));
                }
            }
            
            // If nothing is running and nothing is ready, we're done
            if running.is_empty() {
                break;
            }
            
            // Wait for next task to complete
            if let Some(result) = running.next().await {
                let (node_id, exec_result, duration_ms) = result
                    .map_err(|e| FlowError::Execution(format!("Task join error: {}", e)))?;
                
                match exec_result {
                    Ok(output) => {
                        tracing::info!("Node {} completed in {}ms", node_id, duration_ms);
                        
                        event_bus.emit(ExecutionEvent::NodeCompleted {
                            execution_id,
                            node_id,
                            outputs: output.outputs.clone(),
                            duration_ms,
                            timestamp: Utc::now(),
                        });
                        
                        node_outputs.insert(node_id, output.outputs);
                        completed.insert(node_id);
                    }
                    Err(e) => {
                        tracing::error!("Node {} failed: {}", node_id, e);
                        
                        event_bus.emit(ExecutionEvent::NodeFailed {
                            execution_id,
                            node_id,
                            error: e.to_string(),
                            timestamp: Utc::now(),
                        });
                        
                        // Handle error based on workflow settings
                        match workflow.settings.on_error {
                            flowcore::ErrorHandling::StopWorkflow => {
                                return Err(FlowError::Execution(format!(
                                    "Node {} failed: {}",
                                    node_id, e
                                )));
                            }
                            flowcore::ErrorHandling::ContinueOnError => {
                                completed.insert(node_id);
                            }
                            flowcore::ErrorHandling::RetryWorkflow { .. } => {
                                // TODO: Implement workflow retry logic
                                return Err(FlowError::Execution(format!(
                                    "Node {} failed: {}",
                                    node_id, e
                                )));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(ExecutionResult {
            execution_id,
            outputs: node_outputs,
            completed_nodes: completed.len(),
            total_nodes: workflow.nodes.len(),
        })
    }
    
    /// Find nodes that are ready to execute
    fn find_ready_nodes(
        &self,
        graph: &DiGraph<NodeId, ()>,
        node_to_index: &HashMap<NodeId, NodeIndex>,
        completed: &HashSet<NodeId>,
    ) -> Vec<NodeId> {
        let mut ready = Vec::new();
        
        for (node_id, idx) in node_to_index {
            if completed.contains(node_id) {
                continue;
            }
            
            // Check if all dependencies are completed
            let dependencies_met = graph
                .neighbors_directed(*idx, petgraph::Direction::Incoming)
                .all(|dep_idx| {
                    let dep_node_id = graph.node_weight(dep_idx).unwrap();
                    completed.contains(dep_node_id)
                });
            
            if dependencies_met {
                ready.push(*node_id);
            }
        }
        
        ready
    }
    
    /// Collect inputs for a node from its predecessors
    fn collect_node_inputs(
        &self,
        node_id: NodeId,
        workflow: &Workflow,
        _graph: &DiGraph<NodeId, ()>,
        _node_to_index: &HashMap<NodeId, NodeIndex>,
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
