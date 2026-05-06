use actix_cors::Cors;
use actix_web::{
    delete, get, post, web, App, HttpResponse, HttpServer, Responder, Result as ActixResult,
};
use actix_ws::Message;
use flowcore::{Value, Workflow, WorkflowId};
use flowruntime::FlowRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

/// Application state shared across handlers
struct AppState {
    runtime: Arc<FlowRuntime>,
    workflows: Arc<RwLock<HashMap<WorkflowId, Workflow>>>,
}

/// Request body for workflow execution
#[derive(Debug, Deserialize, ToSchema)]
struct ExecuteRequest {
    /// Input values keyed by port name (e.g., {"url": "https://api.github.com/zen"})
    inputs: HashMap<String, serde_json::Value>,
}

/// Response for workflow creation
#[derive(Debug, Serialize, ToSchema)]
struct WorkflowResponse {
    /// UUID of the created workflow
    id: Uuid,
    /// Human-readable confirmation
    #[schema(example = "Workflow created successfully")]
    message: String,
}

/// Response for workflow execution
#[derive(Debug, Serialize, ToSchema)]
struct ExecutionResponse {
    /// UUID of this execution run
    execution_id: Uuid,
    /// Number of nodes that completed successfully
    #[schema(example = 2)]
    completed_nodes: usize,
    /// Total nodes in the workflow
    #[schema(example = 2)]
    total_nodes: usize,
}

/// Error response for any 4xx/5xx
#[derive(Debug, Serialize, ToSchema)]
struct ErrorResponse {
    /// Human-readable error description
    #[schema(example = "Workflow not found")]
    error: String,
}

/// Summary of a registered node type
#[derive(Debug, Serialize, ToSchema)]
struct NodeTypeInfo {
    /// Node type identifier (e.g., "zypi.exec", "shell.exec")
    #[schema(example = "zypi.exec")]
    r#type: String,
    /// Human-readable description
    #[schema(example = "Execute command in Firecracker microVM")]
    description: String,
    /// Category for grouping (e.g., "zypi", "shell", "docker")
    #[schema(example = "zypi")]
    category: String,
}

/// Health check — returns service status
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = serde_json::Value)
    )
)]
#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "flowengine"
    }))
}

/// List all registered workflows (summary only — no full definitions)
#[utoipa::path(
    get,
    path = "/api/workflows",
    responses(
        (status = 200, description = "List of workflow summaries", body = Vec<serde_json::Value>)
    )
)]
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

/// Create a new workflow from a FlowEngine workflow definition
#[utoipa::path(
    post,
    path = "/api/workflows",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Workflow created", body = WorkflowResponse),
        (status = 400, description = "Invalid workflow JSON", body = ErrorResponse)
    )
)]
#[post("/api/workflows")]
async fn create_workflow(
    data: web::Data<AppState>,
    workflow: web::Json<Workflow>,
) -> ActixResult<impl Responder> {
    let workflow = workflow.into_inner();
    let workflow_id = workflow.id;

    info!("Creating workflow: {} ({})", workflow.name, workflow_id);

    data.workflows.write().await.insert(workflow_id, workflow);

    if let Some(workflow) = data.workflows.read().await.get(&workflow_id) {
        data.runtime.register_workflow(workflow.clone()).await;
    }

    Ok(HttpResponse::Created().json(WorkflowResponse {
        id: workflow_id,
        message: "Workflow created successfully".to_string(),
    }))
}

/// Get a specific workflow by ID (full definition including nodes and connections)
#[utoipa::path(
    get,
    path = "/api/workflows/{id}",
    params(
        ("id" = Uuid, description = "Workflow UUID")
    ),
    responses(
        (status = 200, description = "Workflow definition", body = serde_json::Value),
        (status = 404, description = "Workflow not found", body = ErrorResponse)
    )
)]
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

/// Delete a workflow by ID
#[utoipa::path(
    delete,
    path = "/api/workflows/{id}",
    params(
        ("id" = Uuid, description = "Workflow UUID")
    ),
    responses(
        (status = 200, description = "Workflow deleted", body = serde_json::Value),
        (status = 404, description = "Workflow not found", body = ErrorResponse)
    )
)]
#[delete("/api/workflows/{id}")]
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

/// Execute a previously created workflow with inputs
#[utoipa::path(
    post,
    path = "/api/workflows/{id}/execute",
    params(
        ("id" = Uuid, description = "Workflow UUID")
    ),
    request_body = ExecuteRequest,
    responses(
        (status = 200, description = "Workflow executed successfully", body = ExecutionResponse),
        (status = 404, description = "Workflow not found", body = ErrorResponse),
        (status = 500, description = "Execution failed", body = ErrorResponse)
    )
)]
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

/// WebSocket endpoint for real-time execution events.
/// NOTE: not included in OpenAPI spec (WebSocket not modeled by OpenAPI 3.x).
#[get("/api/events")]
async fn websocket_events(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    info!("WebSocket client connected");

    let mut events = data.runtime.subscribe_events();

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                event = events.recv() => {
                    match event {
                        Ok(event) => {
                            if let Ok(json) = serde_json::to_string(&event) {
                                if session.text(json).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }

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

/// List all available node types that can be used in workflow definitions
#[utoipa::path(
    get,
    path = "/api/nodes",
    responses(
        (status = 200, description = "List of available node types", body = Vec<NodeTypeInfo>)
    )
)]
#[get("/api/nodes")]
async fn list_node_types(data: web::Data<AppState>) -> ActixResult<impl Responder> {
    let registry = data.runtime.registry();
    let node_types = registry.list_node_types();

    let nodes: Vec<NodeTypeInfo> = node_types
        .iter()
        .map(|node_type| {
            let metadata = registry.get_metadata(node_type);
            NodeTypeInfo {
                r#type: node_type.clone(),
                description: metadata.as_ref().map(|m| m.description.clone()).unwrap_or_default(),
                category: metadata.as_ref().map(|m| m.category.clone()).unwrap_or_default(),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(nodes))
}

/// OpenAPI spec — generated at compile time from utoipa annotations
#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        list_workflows,
        create_workflow,
        get_workflow,
        delete_workflow,
        execute_workflow,
        list_node_types,
    ),
    components(
        schemas(
            ExecuteRequest,
            WorkflowResponse,
            ExecutionResponse,
            ErrorResponse,
            NodeTypeInfo,
        )
    ),
    info(
        title = "FlowEngine API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Event-driven DAG workflow engine with Firecracker microVM sandboxing. Supports shell.exec, zypi.exec, browser.render, docker.run, http.request, and transform nodes."
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development"),
    ),
    tags(
        (name = "workflows", description = "Workflow CRUD and execution"),
        (name = "nodes", description = "Node type discovery"),
    )
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Starting Flow Engine Server");

    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);

    let runtime = FlowRuntime::with_registry(
        Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );

    info!("✅ Runtime initialized with standard nodes");

    let app_state = web::Data::new(AppState {
        runtime: Arc::new(runtime),
        workflows: Arc::new(RwLock::new(HashMap::new())),
    });

    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    info!("🌐 Server starting on http://{}", bind_address);
    info!("📖 OpenAPI spec at http://{}/api-docs/openapi.json", bind_address);
    info!("🔍 Swagger UI at http://{}/api-docs/", bind_address);

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
            .service(
                SwaggerUi::new("/api-docs/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind(&bind_address)?
    .run()
    .await?;

    Ok(())
}
