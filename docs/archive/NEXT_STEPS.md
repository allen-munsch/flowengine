# Next Steps: Implementation Guide

This guide shows exactly how to implement the next features for Flow Engine.

## Priority 1: More Standard Nodes

### Database Node (PostgreSQL)

Create `flownodes/src/database.rs`:

```rust
use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use sqlx::{PgPool, Row};

pub struct PostgresNode {
    pool: Option<PgPool>,
}

impl PostgresNode {
    pub fn new() -> Self {
        Self { pool: None }
    }
}

#[async_trait]
impl Node for PostgresNode {
    fn node_type(&self) -> &str {
        "database.postgres"
    }
    
    async fn initialize(&mut self) -> Result<(), NodeError> {
        let connection_string = std::env::var("DATABASE_URL")
            .map_err(|_| NodeError::InitializationFailed(
                "DATABASE_URL not set".to_string()
            ))?;
        
        self.pool = Some(
            PgPool::connect(&connection_string)
                .await
                .map_err(|e| NodeError::InitializationFailed(e.to_string()))?
        );
        
        Ok(())
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let pool = self.pool.as_ref()
            .ok_or_else(|| NodeError::ExecutionFailed("Not initialized".to_string()))?;
        
        let query = ctx.require_input("query")?.as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "query".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        
        ctx.events.info(format!("Executing query: {}", query));
        
        let rows = sqlx::query(query)
            .fetch_all(pool)
            .await
            .map_err(|e| NodeError::ExecutionFailed(e.to_string()))?;
        
        // Convert to JSON array
        let results: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, col) in row.columns().iter().enumerate() {
                    if let Ok(val) = row.try_get::<String, _>(i) {
                        obj.insert(col.name().to_string(), serde_json::json!(val));
                    }
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        
        Ok(NodeOutput::new()
            .with_output("rows", Value::Json(serde_json::Value::Array(results))))
    }
    
    async fn shutdown(&mut self) -> Result<(), NodeError> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(())
    }
}
```

Add to `Cargo.toml`:
```toml
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "postgres"] }
```

### Conditional Node (If/Else)

Create `flownodes/src/control.rs`:

```rust
pub struct IfNode;

#[async_trait]
impl Node for IfNode {
    fn node_type(&self) -> &str {
        "control.if"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let condition = ctx.require_input("condition")?
            .as_bool()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "condition".to_string(),
                expected: "bool".to_string(),
                actual: "other".to_string(),
            })?;
        
        let output_port = if condition { "true" } else { "false" };
        
        // Pass through all inputs to the appropriate output
        let mut outputs = HashMap::new();
        for (key, value) in ctx.inputs.iter() {
            if key != "condition" {
                outputs.insert(format!("{}.{}", output_port, key), value.clone());
            }
        }
        
        Ok(NodeOutput {
            outputs,
            metadata: flowcore::NodeMetadata::default(),
        })
    }
}
```

## Priority 2: HTTP API Server

Create `crates/flowserver/`:

### Server Implementation

```rust
// flowserver/src/main.rs

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use flowruntime::FlowRuntime;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    runtime: Arc<FlowRuntime>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    // Create runtime with registered nodes
    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);
    
    let runtime = FlowRuntime::with_registry(
        Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );
    
    let state = AppState {
        runtime: Arc::new(runtime),
    };
    
    let app = Router::new()
        .route("/api/workflows", post(create_workflow))
        .route("/api/workflows/:id", get(get_workflow))
        .route("/api/workflows/:id/execute", post(execute_workflow))
        .route("/api/health", get(health_check))
        .with_state(state);
    
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("🚀 Server listening on http://localhost:3000");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(workflow): Json<flowcore::Workflow>,
) -> impl IntoResponse {
    state.runtime.register_workflow(workflow.clone()).await;
    
    (StatusCode::CREATED, Json(serde_json::json!({
        "id": workflow.id,
        "message": "Workflow registered",
    })))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // TODO: Implement workflow retrieval
    StatusCode::NOT_IMPLEMENTED
}

async fn execute_workflow(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(inputs): Json<std::collections::HashMap<String, flowcore::Value>>,
) -> impl IntoResponse {
    match state.runtime.execute_workflow(id, inputs).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "execution_id": result.execution_id,
            "completed_nodes": result.completed_nodes,
            "total_nodes": result.total_nodes,
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": e.to_string(),
        }))),
    }
}
```

### WebSocket Event Streaming

```rust
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut events = state.runtime.subscribe_events();
    
    while let Ok(event) = events.recv().await {
        let json = serde_json::to_string(&event).unwrap();
        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}
```

Add to router:
```rust
.route("/api/events", get(websocket_handler))
```

## Priority 3: Workflow Persistence

### SQLite Backend

Create `crates/flowruntime/src/storage.rs`:

```rust
use flowcore::{Workflow, WorkflowId};
use sqlx::{SqlitePool, Row};

pub struct WorkflowStorage {
    pool: SqlitePool,
}

impl WorkflowStorage {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                definition TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                completed_at INTEGER,
                status TEXT NOT NULL,
                result TEXT,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id)
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool })
    }
    
    pub async fn save_workflow(&self, workflow: &Workflow) -> Result<(), sqlx::Error> {
        let definition = serde_json::to_string(workflow).unwrap();
        let now = chrono::Utc::now().timestamp();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO workflows (id, name, definition, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(workflow.id.to_string())
        .bind(&workflow.name)
        .bind(definition)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn load_workflow(&self, id: WorkflowId) -> Result<Option<Workflow>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT definition FROM workflows WHERE id = ?"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.and_then(|r| {
            let json: String = r.get(0);
            serde_json::from_str(&json).ok()
        }))
    }
    
    pub async fn list_workflows(&self) -> Result<Vec<Workflow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT definition FROM workflows ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.iter()
            .filter_map(|r| {
                let json: String = r.get(0);
                serde_json::from_str(&json).ok()
            })
            .collect())
    }
}
```

## Priority 4: Scheduling

### Cron Trigger Support

```rust
use tokio_cron_scheduler::{JobScheduler, Job};

pub struct Scheduler {
    scheduler: JobScheduler,
    runtime: Arc<FlowRuntime>,
}

impl Scheduler {
    pub async fn new(runtime: Arc<FlowRuntime>) -> Result<Self, anyhow::Error> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self { scheduler, runtime })
    }
    
    pub async fn add_cron_trigger(
        &self,
        workflow_id: WorkflowId,
        cron_expression: &str,
    ) -> Result<(), anyhow::Error> {
        let runtime = self.runtime.clone();
        
        let job = Job::new_async(cron_expression, move |_uuid, _lock| {
            let runtime = runtime.clone();
            Box::pin(async move {
                let _ = runtime.execute_workflow(workflow_id, HashMap::new()).await;
            })
        })?;
        
        self.scheduler.add(job).await?;
        Ok(())
    }
    
    pub async fn start(&self) -> Result<(), anyhow::Error> {
        self.scheduler.start().await?;
        Ok(())
    }
}
```

## Priority 5: Visual Editor (Bevy)

### Basic Node Canvas

Create `crates/flowui/`:

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

#[derive(Component)]
struct NodeEntity {
    node_id: uuid::Uuid,
    node_type: String,
}

#[derive(Component)]
struct Draggable {
    dragging: bool,
    offset: Vec2,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            render_nodes,
            handle_dragging,
            render_ui,
        ))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    
    // Spawn some example nodes
    commands.spawn((
        NodeEntity {
            node_id: uuid::Uuid::new_v4(),
            node_type: "http.request".to_string(),
        },
        Draggable {
            dragging: false,
            offset: Vec2::ZERO,
        },
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.3, 0.5, 0.8),
                custom_size: Some(Vec2::new(150.0, 80.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
    ));
}

fn render_nodes(
    query: Query<(&NodeEntity, &Transform)>,
    mut contexts: EguiContexts,
) {
    egui::Window::new("Nodes").show(contexts.ctx_mut(), |ui| {
        for (node, transform) in query.iter() {
            ui.label(format!("{}: {}", node.node_type, node.node_id));
            ui.label(format!("Position: {:.1}, {:.1}", 
                transform.translation.x, 
                transform.translation.y
            ));
            ui.separator();
        }
    });
}

fn handle_dragging(
    mut query: Query<(&mut Transform, &mut Draggable)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    // Implement drag-and-drop logic
}

fn render_ui(mut contexts: EguiContexts) {
    egui::TopBottomPanel::top("menu").show(contexts.ctx_mut(), |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Workflow").clicked() {
                    // Create new workflow
                }
                if ui.button("Open").clicked() {
                    // Open workflow
                }
                if ui.button("Save").clicked() {
                    // Save workflow
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Add Node").clicked() {
                    // Show node picker
                }
            });
        });
    });
}
```

## Testing Implementation

### Integration Test Template

```rust
// tests/integration_test.rs

use flowcore::{Workflow, NodeSpec, Value};
use flowruntime::FlowRuntime;
use std::collections::HashMap;

#[tokio::test]
async fn test_http_to_debug_workflow() {
    // Setup
    let mut registry = flowruntime::NodeRegistry::new();
    flownodes::register_all(&mut registry);
    
    let runtime = FlowRuntime::with_registry(
        std::sync::Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );
    
    // Create workflow
    let mut workflow = Workflow::new("test");
    
    let http_node = NodeSpec::new("http.request")
        .with_config("method", "GET");
    let debug_node = NodeSpec::new("debug.log");
    
    let http_id = workflow.add_node(http_node);
    let debug_id = workflow.add_node(debug_node);
    
    workflow.connect(http_id, "body", debug_id, "message");
    
    // Execute
    let mut inputs = HashMap::new();
    inputs.insert("url".to_string(), Value::String("https://api.github.com/zen".to_string()));
    
    let result = runtime.execute(&workflow, inputs).await.unwrap();
    
    // Assert
    assert_eq!(result.completed_nodes, 2);
    assert_eq!(result.total_nodes, 2);
}
```

## Deployment

### Docker Container

```dockerfile
# Dockerfile

FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/flowserver /usr/local/bin/
EXPOSE 3000
CMD ["flowserver"]
```

### Kubernetes Deployment

```yaml
# deployment.yaml

apiVersion: apps/v1
kind: Deployment
metadata:
  name: flowengine
spec:
  replicas: 3
  selector:
    matchLabels:
      app: flowengine
  template:
    metadata:
      labels:
        app: flowengine
    spec:
      containers:
      - name: flowengine
        image: flowengine:latest
        ports:
        - containerPort: 3000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: flowengine-secrets
              key: database-url
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: flowengine
spec:
  selector:
    app: flowengine
  ports:
  - port: 80
    targetPort: 3000
  type: LoadBalancer
```

---

Each of these features can be implemented independently and incrementally added to the system.

Start with the highest priority items and build from there!
