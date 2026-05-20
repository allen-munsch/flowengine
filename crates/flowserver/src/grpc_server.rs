//! gRPC server for FlowEngine (port 3001)
//!
//! Implements the FlowEngineService trait from the generated proto.
//! Runs alongside the REST server (port 3000). Both share the same AppState.
//!
//! RPCs:
//!   - CreateWorkflow — registers a new workflow
//!   - ExecuteWorkflow — runs a workflow, streams events back
//!   - GetWorkflowStatus — query execution status
//!   - CancelWorkflow — stop a running execution
//!   - Health — service health check

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{info, error};

use flowcore::{self, Workflow, Value};
use flowruntime;

use flowproto::flowengine::v1::{
    self as pb,
    flow_engine_service_server::{FlowEngineService, FlowEngineServiceServer},
};

use super::AppState;

/// Wraps the app state for the gRPC service
pub struct FlowEngineGrpcServer {
    state: Arc<AppState>,
}

impl FlowEngineGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Convert the service into a tonic router
    pub fn into_service(self) -> FlowEngineServiceServer<Self> {
        FlowEngineServiceServer::new(self)
    }
}

// ── Type Conversions ───────────────────────────────────────────────────────

/// Convert proto Workflow → flowcore Workflow
fn proto_to_workflow(pb: pb::Workflow) -> Result<Workflow, Status> {
    let id = pb
        .id
        .parse::<uuid::Uuid>()
        .map_err(|e| Status::invalid_argument(format!("invalid workflow id: {}", e)))?;

    let nodes = pb
        .nodes
        .into_iter()
        .map(|n| {
            let node_id = n.id.parse::<uuid::Uuid>().map_err(|e| {
                Status::invalid_argument(format!("invalid node id: {}", e))
            })?;
            Ok(flowcore::NodeSpec {
                id: node_id,
                node_type: n.node_type,
                name: n.name,
                config: n
                    .config
                    .into_iter()
                    .map(|(k, v)| proto_to_value(v).map(|val| (k, val)))
                    .collect::<Result<HashMap<_, _>, Status>>()?,
                position: n.position.map(|p| flowcore::Position { x: p.x, y: p.y }),
                retry_policy: n.retry_policy.map(|rp| flowcore::RetryPolicy {
                    max_attempts: rp.max_attempts,
                    delay_ms: rp.delay_ms,
                    backoff_multiplier: rp.backoff_multiplier,
                    max_delay_ms: rp.max_delay_ms,
                    retry_on_timeout: rp.retry_on_timeout,
                }),
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;

    let connections = pb
        .connections
        .into_iter()
        .map(|c| {
            Ok(flowcore::Connection {
                from_node: c.from_node.parse().map_err(|e| {
                    Status::invalid_argument(format!("invalid from_node: {}", e))
                })?,
                from_port: c.from_port,
                to_node: c.to_node.parse().map_err(|e| {
                    Status::invalid_argument(format!("invalid to_node: {}", e))
                })?,
                to_port: c.to_port,
            })
        })
        .collect::<Result<Vec<_>, Status>>()?;

    let settings = pb.settings.unwrap_or_default();
    let on_error = match settings.on_error {
        1 => flowcore::ErrorHandling::StopWorkflow,
        2 => flowcore::ErrorHandling::ContinueOnError,
        3 => flowcore::ErrorHandling::RetryWorkflow { max_attempts: 3 },
        _ => flowcore::ErrorHandling::StopWorkflow,
    };

    Ok(Workflow {
        id,
        name: pb.name,
        description: pb.description,
        nodes,
        connections,
        triggers: vec![],
        settings: flowcore::WorkflowSettings {
            max_execution_time_ms: settings.max_execution_time_ms,
            max_parallel_nodes: settings.max_parallel_nodes as usize,
            on_error,
        },
    })
}

/// Convert proto Value → flowcore Value
fn proto_to_value(pb: pb::Value) -> Result<Value, Status> {
    match pb.kind {
        Some(pb::value::Kind::NullValue(_)) => Ok(Value::Null),
        Some(pb::value::Kind::BoolValue(b)) => Ok(Value::Bool(b)),
        Some(pb::value::Kind::NumberValue(n)) => Ok(Value::Number(n)),
        Some(pb::value::Kind::StringValue(s)) => Ok(Value::String(s)),
        Some(pb::value::Kind::BytesValue(b)) => Ok(Value::Bytes(b)),
        Some(pb::value::Kind::JsonValue(j)) => {
            let json: serde_json::Value =
                serde_json::from_str(&j).map_err(|e| {
                    Status::invalid_argument(format!("invalid json value: {}", e))
                })?;
            Ok(Value::Json(json))
        }
        Some(pb::value::Kind::ArrayValue(arr)) => {
            let values: Vec<Value> = arr
                .values
                .into_iter()
                .map(proto_to_value)
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(values))
        }
        Some(pb::value::Kind::ObjectValue(obj)) => {
            let map: HashMap<String, Value> = obj
                .entries
                .into_iter()
                .map(|(k, v)| proto_to_value(v).map(|val| (k, val)))
                .collect::<Result<_, _>>()?;
            Ok(Value::Object(map))
        }
        None => Ok(Value::Null),
    }
}

/// Convert flowcore Value → proto Value
fn value_to_proto(v: &Value) -> pb::Value {
    let kind = match v {
        Value::Null => pb::value::Kind::NullValue(pb::NullValue {}),
        Value::Bool(b) => pb::value::Kind::BoolValue(*b),
        Value::Number(n) => pb::value::Kind::NumberValue(*n),
        Value::String(s) => pb::value::Kind::StringValue(s.clone()),
        Value::Bytes(b) => pb::value::Kind::BytesValue(b.clone()),
        Value::Json(j) => pb::value::Kind::JsonValue(j.to_string()),
        Value::Array(arr) => {
            let values = arr.iter().map(value_to_proto).collect();
            pb::value::Kind::ArrayValue(pb::ValueList { values })
        }
        Value::Object(obj) => {
            let entries = obj
                .iter()
                .map(|(k, v)| (k.clone(), value_to_proto(v)))
                .collect();
            pb::value::Kind::ObjectValue(pb::ValueMap { entries })
        }
    };
    pb::Value { kind: Some(kind) }
}

// ── gRPC Service Implementation ────────────────────────────────────────────

#[tonic::async_trait]
impl FlowEngineService for FlowEngineGrpcServer {
    type ExecuteWorkflowStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<pb::WorkflowEvent, Status>> + Send + 'static>>;

    /// Health check
    async fn health(
        &self,
        _request: Request<pb::HealthRequest>,
    ) -> Result<Response<pb::HealthResponse>, Status> {
        Ok(Response::new(pb::HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            service: "flowengine".to_string(),
        }))
    }

    /// Create a new workflow
    async fn create_workflow(
        &self,
        request: Request<pb::CreateWorkflowRequest>,
    ) -> Result<Response<pb::CreateWorkflowResponse>, Status> {
        let req = request.into_inner();
        let pb_workflow = req
            .workflow
            .ok_or_else(|| Status::invalid_argument("workflow is required"))?;

        let workflow = proto_to_workflow(pb_workflow)?;
        let workflow_id = workflow.id.to_string();

        info!("[gRPC] Creating workflow: {} ({})", workflow.name, workflow_id);

        self.state
            .workflows
            .write()
            .await
            .insert(workflow.id, workflow.clone());

        self.state.runtime.register_workflow(workflow).await;

        Ok(Response::new(pb::CreateWorkflowResponse {
            workflow_id,
            message: "Workflow created successfully".to_string(),
        }))
    }

    /// Execute a workflow — server-streaming events
    async fn execute_workflow(
        &self,
        request: Request<pb::ExecuteWorkflowRequest>,
    ) -> Result<Response<Self::ExecuteWorkflowStream>, Status> {
        let req = request.into_inner();

        let workflow_id = req
            .workflow_id
            .parse::<uuid::Uuid>()
            .map_err(|e| Status::invalid_argument(format!("invalid workflow id: {}", e)))?;

        let inputs: HashMap<String, Value> = req
            .inputs
            .into_iter()
            .map(|(k, v)| proto_to_value(v).map(|val| (k, val)))
            .collect::<Result<_, Status>>()?;

        info!("[gRPC] Executing workflow: {}", workflow_id);

        // Subscribe to events before executing (to avoid race)
        let mut event_rx = self.state.runtime.subscribe_events();

        // Execute the workflow
        let runtime = self.state.runtime.clone();
        let exec_result = runtime.execute_workflow(workflow_id, inputs).await;

        // Stream events
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            // Forward workflow events from the EventEmitter to the gRPC stream
            loop {
                tokio::select! {
                    event = event_rx.recv() => {
                        match event {
                            Ok(flow_event) => {
                                let pb_event = convert_event(&flow_event);
                                if tx.send(Ok(pb_event)).await.is_err() {
                                    break; // client disconnected
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    else => break,
                }
            }
        });

        // Also send the execution result as a final event
        match exec_result {
            Ok(result) => {
                info!(
                    "[gRPC] Workflow {} completed: {}/{} nodes",
                    workflow_id, result.completed_nodes, result.total_nodes
                );
            }
            Err(e) => {
                error!("[gRPC] Workflow {} execution failed: {}", workflow_id, e);
            }
        }

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::ExecuteWorkflowStream))
    }

    /// Get workflow execution status
    async fn get_workflow_status(
        &self,
        request: Request<pb::GetWorkflowStatusRequest>,
    ) -> Result<Response<pb::WorkflowStatusResponse>, Status> {
        let req = request.into_inner();
        let execution_id = req
            .execution_id
            .parse::<uuid::Uuid>()
            .map_err(|e| Status::invalid_argument(format!("invalid execution_id: {}", e)))?;

        let exec_id = flowcore::ExecutionId::from(execution_id);
        
        match self.state.runtime.get_execution_status(exec_id).await {
            Some(state) => {
                let status_str = match state.status {
                    flowruntime::ExecutionStatus::Running => "running",
                    flowruntime::ExecutionStatus::Completed => "completed",
                    flowruntime::ExecutionStatus::Failed => "failed",
                    flowruntime::ExecutionStatus::Cancelled => "cancelled",
                };
                Ok(Response::new(pb::WorkflowStatusResponse {
                    execution_id: state.execution_id.to_string(),
                    workflow_id: state.workflow_id.to_string(),
                    status: status_str.to_string(),
                    completed_nodes: 0,   // TODO: track node count in state
                    total_nodes: 0,
                    duration_ms: 0,        // TODO: track duration in state
                    started_at: state.started_at.to_rfc3339(),
                    completed_at: state.completed_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                }))
            }
            None => Err(Status::not_found(format!(
                "Execution {} not found",
                execution_id
            ))),
        }
    }

    /// Cancel a running workflow
    async fn cancel_workflow(
        &self,
        request: Request<pb::CancelWorkflowRequest>,
    ) -> Result<Response<pb::CancelWorkflowResponse>, Status> {
        let req = request.into_inner();
        let execution_id = req
            .execution_id
            .parse::<uuid::Uuid>()
            .map_err(|e| Status::invalid_argument(format!("invalid execution_id: {}", e)))?;

        let exec_id = flowcore::ExecutionId::from(execution_id);
        let cancelled = self.state.runtime.cancel_execution(exec_id).await;

        Ok(Response::new(pb::CancelWorkflowResponse {
            execution_id: execution_id.to_string(),
            status: if cancelled {
                "cancelled".to_string()
            } else {
                "not_found".to_string()
            },
        }))
    }
}

/// Convert a flowcore ExecutionEvent to a proto WorkflowEvent
fn convert_event(event: &flowcore::events::ExecutionEvent) -> pb::WorkflowEvent {
    use flowcore::events::ExecutionEvent;
    match event {
        ExecutionEvent::WorkflowStarted {
            execution_id,
            workflow_id,
            timestamp,
        } => pb::WorkflowEvent {
            event: Some(pb::workflow_event::Event::WorkflowStarted(
                pb::WorkflowStartedEvent {
                    execution_id: execution_id.to_string(),
                    workflow_id: workflow_id.to_string(),
                    timestamp: timestamp.to_rfc3339(),
                },
            )),
        },
        ExecutionEvent::WorkflowCompleted {
            execution_id,
            success,
            duration_ms,
            timestamp,
        } => pb::WorkflowEvent {
            event: Some(pb::workflow_event::Event::WorkflowCompleted(
                pb::WorkflowCompletedEvent {
                    execution_id: execution_id.to_string(),
                    success: *success,
                    duration_ms: *duration_ms,
                    timestamp: timestamp.to_rfc3339(),
                },
            )),
        },
        ExecutionEvent::NodeStarted {
            execution_id,
            node_id,
            node_type,
            timestamp,
        } => pb::WorkflowEvent {
            event: Some(pb::workflow_event::Event::NodeStarted(
                pb::NodeStartedEvent {
                    execution_id: execution_id.to_string(),
                    node_id: node_id.to_string(),
                    node_type: node_type.clone(),
                    timestamp: timestamp.to_rfc3339(),
                },
            )),
        },
        ExecutionEvent::NodeCompleted {
            execution_id,
            node_id,
            outputs,
            duration_ms,
            timestamp,
        } => {
            let proto_outputs: HashMap<String, pb::Value> = outputs
                .iter()
                .map(|(k, v)| (k.clone(), value_to_proto(v)))
                .collect();
            pb::WorkflowEvent {
                event: Some(pb::workflow_event::Event::NodeCompleted(
                    pb::NodeCompletedEvent {
                        execution_id: execution_id.to_string(),
                        node_id: node_id.to_string(),
                        outputs: proto_outputs,
                        duration_ms: *duration_ms,
                        timestamp: timestamp.to_rfc3339(),
                    },
                )),
            }
        }
        ExecutionEvent::NodeFailed {
            execution_id,
            node_id,
            error,
            timestamp,
        } => pb::WorkflowEvent {
            event: Some(pb::workflow_event::Event::NodeFailed(
                pb::NodeFailedEvent {
                    execution_id: execution_id.to_string(),
                    node_id: node_id.to_string(),
                    error: error.clone(),
                    timestamp: timestamp.to_rfc3339(),
                },
            )),
        },
        ExecutionEvent::NodeEvent {
            execution_id,
            node_id,
            event: node_event,
            timestamp,
        } => {
            use flowcore::events::NodeEvent;
            let detail = match node_event {
                NodeEvent::Info { message } => {
                    pb::node_event_message::Detail::Info(pb::NodeInfo {
                        message: message.clone(),
                    })
                }
                NodeEvent::Warning { message } => {
                    // Map Warning to Info for gRPC (proto has no Warning variant)
                    pb::node_event_message::Detail::Info(pb::NodeInfo {
                        message: format!("⚠ {}", message),
                    })
                }
                NodeEvent::StdoutLine { line } => {
                    pb::node_event_message::Detail::StdoutLine(pb::NodeStdoutLine {
                        line: line.clone(),
                    })
                }
                NodeEvent::StderrLine { line } => {
                    pb::node_event_message::Detail::StderrLine(pb::NodeStderrLine {
                        line: line.clone(),
                    })
                }
                NodeEvent::Progress { percent, message } => {
                    pb::node_event_message::Detail::Progress(pb::NodeProgress {
                        percent: *percent,
                        message: message.clone().unwrap_or_default(),
                    })
                }
                NodeEvent::Data { port, value } => {
                    // Map Data to Info with serialized value
                    pb::node_event_message::Detail::Info(pb::NodeInfo {
                        message: format!("Data[{}]: {:?}", port, value),
                    })
                }
            };
            pb::WorkflowEvent {
                event: Some(pb::workflow_event::Event::NodeEvent(
                    pb::NodeEventMessage {
                        execution_id: execution_id.to_string(),
                        node_id: node_id.to_string(),
                        detail: Some(detail),
                        timestamp: timestamp.to_rfc3339(),
                    },
                )),
            }
        }
    }
}
