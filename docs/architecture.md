# Architecture Deep Dive

## Core Design Principles

### 1. Separation of Concerns

```
┌─────────────────────────────────────────────────────────┐
│  flowcore - Abstractions & Types                       │
│  (No runtime dependencies, pure data structures)       │
└──────────────────────┬──────────────────────────────────┘
                       │
          ┌────────────┴────────────┐
          │                         │
┌─────────▼──────────┐    ┌────────▼─────────┐
│  flowruntime       │    │   flownodes      │
│  (Execution)       │    │   (Implementations)│
└─────────┬──────────┘    └──────────────────┘
          │
┌─────────▼──────────┐
│  flowcli/flowserver│
│  (Interfaces)      │
└────────────────────┘
```

### 2. Event-Driven Architecture

All runtime state changes are broadcast as events:

```rust
pub enum ExecutionEvent {
    WorkflowStarted { ... },
    NodeStarted { ... },
    NodeCompleted { ... },
    NodeFailed { ... },
    NodeEvent { ... },  // Custom node events
    WorkflowCompleted { ... },
}
```

Benefits:
- **Observability**: Real-time monitoring without polling
- **Decoupling**: UI doesn't need to query state
- **Testing**: Easy to record and replay executions
- **Extensibility**: Multiple listeners can react to same events

### 3. Trait-Based Extensibility

```rust
#[async_trait]
pub trait Node: Send + Sync {
    fn node_type(&self) -> &str;
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError>;
    async fn initialize(&mut self) -> Result<(), NodeError>;
    async fn shutdown(&mut self) -> Result<(), NodeError>;
}
```

Why traits over other approaches:
- ✅ Zero-cost abstractions
- ✅ Type safety at compile time
- ✅ No reflection overhead
- ✅ Clear contracts
- ❌ Requires recompilation for new nodes (WASM planned)

## Component Breakdown

### flowcore

**Purpose**: Pure abstractions with zero runtime dependencies

**Key Types**:
- `Node` - Core trait for executable units
- `Value` - Dynamic type system for data flow
- `Workflow` - Serializable workflow definition
- `ExecutionEvent` - Event system types

**Design Decisions**:
- No async runtime (tokio) in type definitions
- All types are `Serialize + Deserialize`
- Error types are `Clone` for broadcasting

### flowruntime

**Purpose**: Workflow execution engine

**Components**:

#### NodeRegistry
```rust
pub struct NodeRegistry {
    factories: HashMap<String, Arc<dyn NodeFactory>>,
}
```

Manages node type discovery and instantiation. Thread-safe via `Arc`.

#### WorkflowExecutor
```rust
pub struct WorkflowExecutor {
    max_parallel: usize,
}
```

**Execution Algorithm**:

1. **Build DAG**: Convert workflow to `petgraph::DiGraph`
2. **Validate**: Check for cycles using topological sort
3. **Initialize Nodes**: Call `initialize()` on all nodes
4. **Execute Loop**:
   ```
   while not_all_complete:
       ready_nodes = find_nodes_with_satisfied_deps()
       for node in ready_nodes[..max_parallel]:
           spawn execute_node(node)
       await any_task_completion()
       handle_result()
   ```
5. **Cleanup**: Call `shutdown()` on all nodes

**Parallelism Strategy**:
- Uses `FuturesUnordered` for concurrent execution
- Respects `max_parallel_nodes` setting
- Dependencies prevent premature execution
- Failed nodes can stop workflow or continue (configurable)

#### FlowRuntime
```rust
pub struct FlowRuntime {
    registry: Arc<NodeRegistry>,
    executor: Arc<WorkflowExecutor>,
    event_bus: Arc<EventBus>,
    workflows: Arc<RwLock<HashMap<WorkflowId, Workflow>>>,
}
```

Main entry point. Coordinates:
- Node registry access
- Workflow storage (in-memory, can be backed by DB)
- Event subscription
- Execution lifecycle

### flownodes

**Purpose**: Standard library of nodes

**Categories**:
- **HTTP**: `http.request` - External API calls
- **Transform**: `json_parse`, `json_stringify` - Data manipulation
- **Time**: `delay` - Flow control
- **Debug**: `log` - Observability

**Adding New Nodes**:

1. Create struct implementing `Node`
2. Create `NodeFactory` implementation
3. Register in `register_all()`

Example pattern:
```rust
pub struct MyNode { /* state */ }

#[async_trait]
impl Node for MyNode {
    fn node_type(&self) -> &str { "category.mynode" }
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        // Implementation
    }
}

pub struct MyNodeFactory;
impl NodeFactory for MyNodeFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(MyNode::from_config(config)?))
    }
    fn node_type(&self) -> &str { "category.mynode" }
}
```

## Data Flow

### Input/Output Mechanism

```
Node A                  Node B
┌─────────┐            ┌─────────┐
│ outputs │ ───────►   │ inputs  │
│  {      │            │  {      │
│   "x":5 │   via      │   "y":5 │
│  }      │ connection │  }      │
└─────────┘            └─────────┘
```

Connection definition:
```rust
Connection {
    from_node: NodeId,
    from_port: "x",
    to_node: NodeId,
    to_port: "y",
}
```

Runtime maps `from_port` output to `to_port` input.

### Value Type System

```rust
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}
```

**Why not just `serde_json::Value`?**
- Need `Bytes` for binary data
- Need distinction between JSON and native types
- Future: `Stream` for real-time data

**Type Coercion**:
Currently minimal. Future: automatic conversions.

## Execution Model

### Current: Pull-Based DAG

```
Timer ──► HTTP ──► Parse ──► Log
  1       2        3       4
```

Execution order: 1 → 2 → 3 → 4 (sequential due to dependencies)

With parallelism:
```
       ┌──► HTTP1 ──┐
Timer ─┤            ├──► Merge ──► Log
       └──► HTTP2 ──┘
```

HTTP1 and HTTP2 run in parallel.

### Future: Push-Based Streams

For real-time/streaming:
```
WebSocket ──► Filter ──► Transform ──► Database
  (push)      (push)      (push)       (push)
```

Each node processes data as it arrives, not waiting for completion.

## Error Handling

### Node-Level Retry
```rust
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
}
```

Applied per-node. Executor retries failed nodes automatically.

### Workflow-Level Handling
```rust
pub enum ErrorHandling {
    StopWorkflow,
    ContinueOnError,
    RetryWorkflow { max_attempts: u32 },
}
```

Determines what happens when a node fails after retries.

## State Management

### Per-Execution State
```rust
pub struct NodeState {
    pub data: HashMap<String, Value>,
}
```

Shared across a single workflow execution via `Arc<RwLock<NodeState>>`.

Use cases:
- Counters
- Accumulation
- Caching within run

### Persistent State

Currently: Not implemented
Future: Node can implement `initialize()` to load DB connections, etc.

```rust
impl Node for DatabaseNode {
    async fn initialize(&mut self) -> Result<(), NodeError> {
        self.pool = create_connection_pool().await?;
        Ok(())
    }
}
```

Pool shared across all executions of this workflow.

## Concurrency Model

### Tokio Runtime

All async execution uses Tokio:
```rust
#[tokio::main]
async fn main() {
    // Runtime automatically created
}
```

### Task Spawning

Nodes execute as independent tasks:
```rust
tokio::spawn(async move {
    node.execute(ctx).await
})
```

Bounded by `max_parallel_nodes` semaphore.

### Event Broadcasting

```rust
pub struct EventBus {
    sender: broadcast::Sender<ExecutionEvent>,
}
```

Tokio's `broadcast` channel:
- Multiple subscribers
- Bounded buffer (drops old events if full)
- Lock-free

## Performance Considerations

### Zero-Copy Optimization Opportunities

Current: `Value` is cloned between nodes
Future: Use `Arc<Value>` for large data

### Lazy Evaluation

Current: All connected nodes receive outputs
Future: Pull model where downstream requests data

### Memory Management

- Workflows: Stored in `Arc<RwLock<HashMap>>`
- Node instances: Created per execution (cheap)
- Event buffer: Bounded, drops old events

## Future Enhancements

### 1. Streaming Data
```rust
pub enum Value {
    Stream(StreamHandle),  // Future
}

pub struct StreamHandle {
    receiver: mpsc::Receiver<Value>,
}
```

### 2. WASM Nodes
```rust
pub struct WasmNode {
    instance: wasmtime::Instance,
}
```

Sandboxed, safe, community-contributed nodes.

### 3. Distributed Execution
```rust
pub enum ExecutionLocation {
    Local,
    Remote { worker_id: String },
}
```

Nodes can execute on different machines.

### 4. Visual Editor (Bevy)

```rust
#[derive(Component)]
struct NodeEntity {
    node_id: NodeId,
}

fn sync_with_backend(
    events: Res<Events<BackendEvent>>,
    mut query: Query<&mut NodeEntity>,
) {
    // Update visual state based on execution events
}
```

Real-time visualization of execution.

## Testing Strategy

### Unit Tests
Test individual nodes in isolation.

### Integration Tests
```rust
#[tokio::test]
async fn test_http_to_debug_workflow() {
    let mut workflow = Workflow::new("test");
    // Build workflow
    let runtime = FlowRuntime::new();
    let result = runtime.execute(&workflow, inputs).await?;
    assert!(result.completed_nodes == 2);
}
```

### Property Tests
Use `proptest` for workflow validation.

### Benchmarks
```rust
#[bench]
fn bench_dag_execution(b: &mut Bencher) {
    // Measure execution overhead
}
```

## Security Considerations

### Current
- Nodes run in same process (trust required)
- No sandboxing

### Future
- WASM sandboxing for untrusted nodes
- Resource limits (CPU, memory, time)
- Network isolation per node

## Monitoring & Observability

### Structured Logging
```rust
tracing::info!(
    node_id = %node_id,
    duration_ms = duration_ms,
    "Node completed"
);
```

### Metrics (Future)
- Node execution times (histogram)
- Failure rates (counter)
- Active workflows (gauge)

### Distributed Tracing (Future)
OpenTelemetry integration for spans across nodes.
