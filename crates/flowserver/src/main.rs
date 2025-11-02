use actix_cors::Cors;
use actix_web::{
    get, post, web, App, HttpResponse, HttpServer, Responder, Result as ActixResult,
};
use actix_ws::Message;
use flowcore::{ExecutionEvent, Value, Workflow, WorkflowId};
use flowruntime::FlowRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

/// Application state shared across handlers
struct AppState {
    runtime: Arc<FlowRuntime>,
    workflows: Arc<RwLock<HashMap<WorkflowId, Workflow>>>,
}

/// Request body for workflow execution
#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    inputs: HashMap<String, serde_json::Value>,
}

/// Response for workflow creation
#[derive(Debug, Serialize)]
struct WorkflowResponse {
    id: Uuid,
    message: String,
}

/// Response for workflow execution
#[derive(Debug, Serialize)]
struct ExecutionResponse {
    execution_id: Uuid,
    completed_nodes: usize,
    total_nodes: usize,
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

/// Health check endpoint
#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "flowengine"
    }))
}

/// List all workflows
#[get("/api/workflows")]
async fn list_workflows(data: web::Data<AppState>) -> ActixResult<impl Responder> {
    let workflows = data.workflows.read().await;
    let workflow_list: Vec<_> = workflows
        .values()
        .map(|w| {
            serde_json::json!({
                "id": w.id,
                "name": w.name,
                "description": w.description,
                "nodes": w.nodes.len(),
                "connections": w.connections.len(),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(workflow_list))
}

/// Create a new workflow
#[post("/api/workflows")]
async fn create_workflow(
    data: web::Data<AppState>,
    workflow: web::Json<Workflow>,
) -> ActixResult<impl Responder> {
    let workflow = workflow.into_inner();
    let workflow_id = workflow.id;

    info!("Creating workflow: {} ({})", workflow.name, workflow_id);

    // Store in memory
    data.workflows.write().await.insert(workflow_id, workflow);

    // Also register with runtime
    if let Some(workflow) = data.workflows.read().await.get(&workflow_id) {
        data.runtime.register_workflow(workflow.clone()).await;
    }

    Ok(HttpResponse::Created().json(WorkflowResponse {
        id: workflow_id,
        message: "Workflow created successfully".to_string(),
    }))
}

/// Get a specific workflow
#[get("/api/workflows/{id}")]
async fn get_workflow(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<impl Responder> {
    let workflow_id = path.into_inner();
    let workflows = data.workflows.read().await;

    match workflows.get(&workflow_id) {
        Some(workflow) => Ok(HttpResponse::Ok().json(workflow)),
        None => Ok(HttpResponse::NotFound().json(ErrorResponse {
            error: format!("Workflow {} not found", workflow_id),
        })),
    }
}

/// Delete a workflow
#[actix_web::delete("/api/workflows/{id}")]
async fn delete_workflow(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<impl Responder> {
    let workflow_id = path.into_inner();
    let mut workflows = data.workflows.write().await;

    match workflows.remove(&workflow_id) {
        Some(_) => {
            info!("Deleted workflow: {}", workflow_id);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": "Workflow deleted successfully"
            })))
        }
        None => Ok(HttpResponse::NotFound().json(ErrorResponse {
            error: format!("Workflow {} not found", workflow_id),
        })),
    }
}

/// Execute a workflow
#[post("/api/workflows/{id}/execute")]
async fn execute_workflow(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: web::Json<ExecuteRequest>,
) -> ActixResult<impl Responder> {
    let workflow_id = path.into_inner();
    let inputs = req.into_inner().inputs;

    info!("Executing workflow: {}", workflow_id);

    let converted_inputs: HashMap<String, Value> = inputs
        .into_iter()
        .map(|(k, v)| (k, Value::Json(v)))
        .collect();

    match data.runtime.execute_workflow(workflow_id, converted_inputs).await {
        Ok(result) => {
            info!(
                "Workflow {} completed: {}/{} nodes",
                workflow_id, result.completed_nodes, result.total_nodes
            );

            Ok(HttpResponse::Ok().json(ExecutionResponse {
                execution_id: result.execution_id,
                completed_nodes: result.completed_nodes,
                total_nodes: result.total_nodes,
            }))
        }
        Err(e) => {
            error!("Workflow {} execution failed: {}", workflow_id, e);
            Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            }))
        }
    }
}

/// WebSocket endpoint for real-time events
#[get("/api/events")]
async fn websocket_events(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    info!("WebSocket client connected");

    // Subscribe to events
    let mut events = data.runtime.subscribe_events();

    // Spawn task to handle WebSocket
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Receive event from runtime
                event = events.recv() => {
                    match event {
                        Ok(event) => {
                            // Serialize and send event
                            if let Ok(json) = serde_json::to_string(&event) {
                                if session.text(json).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }

                // Handle incoming WebSocket messages (ping/pong)
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }

                else => break,
            }
        }

        info!("WebSocket client disconnected");
        let _ = session.close(None).await;
    });

    Ok(res)
}

/// List available node types
#[get("/api/nodes")]
async fn list_node_types(data: web::Data<AppState>) -> ActixResult<impl Responder> {
    let registry = data.runtime.registry();
    let node_types = registry.list_node_types();

    let nodes: Vec<_> = node_types
        .iter()
        .map(|node_type| {
            let metadata = registry.get_metadata(node_type);
            serde_json::json!({
                "type": node_type,
                "description": metadata.as_ref().map(|m| m.description.clone()).unwrap_or_default(),
                "category": metadata.as_ref().map(|m| m.category.clone()).unwrap_or_default(),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(nodes))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Starting Flow Engine Server");

    // Create runtime with registered nodes
    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);

    let runtime = FlowRuntime::with_registry(
        Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );

    info!("✅ Runtime initialized with standard nodes");

    // Create app state
    let app_state = web::Data::new(AppState {
        runtime: Arc::new(runtime),
        workflows: Arc::new(RwLock::new(HashMap::new())),
    });

    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    info!("🌐 Server starting on http://{}", bind_address);

    // Start HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .wrap(actix_web::middleware::Logger::default())
            .service(health_check)
            .service(list_workflows)
            .service(create_workflow)
            .service(get_workflow)
            .service(delete_workflow)
            .service(execute_workflow)
            .service(websocket_events)
            .service(list_node_types)
    })
    .bind(&bind_address)?
    .run()
    .await?;

    Ok(())
}
