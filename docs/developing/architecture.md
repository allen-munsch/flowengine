# Architecture

## Core Design Principles

### Separation of Concerns

```
                          flowcore
                  (Abstractions & Types)

    ┌─────────────────────┼─────────────────────┐
    │                     │                     │
 flowruntime         flownodes           flowcore_macros
 (Execution)         (14 built-in nodes) (NodeConfig derive)

    │
    ├── flowcli (CLI binary: run, validate, nodes, init)
    ├── flowpersist (SQLite result cache)
    └── flowserver (HTTP + WebSocket server)
```

### Event-Driven Architecture

All runtime state changes broadcast as events:

```rust
pub enum ExecutionEvent {
    WorkflowStarted { execution_id: String },
    NodeStarted { execution_id: String, node_id: String, node_type: String },
    NodeLog { execution_id: String, node_id: String, event: NodeEvent },
    NodeCompleted { execution_id: String, node_id: String, duration_ms: u64 },
    NodeFailed { execution_id: String, node_id: String, error: String },
    WorkflowCompleted { execution_id: String, success: bool, duration_ms: u64 },
}
```

Benefits: real-time monitoring without polling, decoupled UI from state, replayable executions, multiple listeners.

### Trait-Based Extensibility

```rust
#[async_trait]
pub trait Node: Send + Sync {
    fn node_type(&self) -> &str;
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError>;
    async fn initialize(&mut self) -> Result<(), NodeError>;
    async fn shutdown(&mut self) -> Result<(), NodeError>;
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<(), NodeError>;
}
```

## Crate Structure

### `flowcore` — Abstractions

Zero runtime dependencies, pure data structures and traits.

| Module | Contents |
|--------|----------|
| `node.rs` | `Node` trait, `NodeContext`, `NodeOutput`, `NodeState`, `PortDefinition` |
| `value.rs` | `Value` enum (12 variants), conversion methods, `.to_arc()` |
| `workflow.rs` | `Workflow`, `NodeSpec`, `Connection`, `RetryPolicy`, `TriggerSpec`, `WorkflowSettings` |
| `parser.rs` | `WorkflowParser` trait, `JsonParser`, `YamlParser`, format auto-detection |
| `plugin.rs` | `NodePlugin` trait, `PluginMetadata` |
| `events/` | `ExecutionEvent`, `NodeEvent`, `EventEmitter`, `EventBus` trait, `IggyEventBus` |
| `error.rs` | `NodeError`, `WorkflowError`, `FlowError` |

### `flowcore_macros` — Derive Macros

| Macro | File | What it generates |
|-------|------|-------------------|
| `#[derive(NodeConfig)]` | `lib.rs` | `TryFrom<&HashMap<String, Value>>` for typed config extraction with `#[config(default, rename, skip)]` |

Supported field types: `String`, `f64`, `bool`, `u64`, `usize`, and any type implementing `FromStr`.

### `flowruntime` — Execution Engine

| Module | Responsibility |
|--------|---------------|
| `executor.rs` | `WorkflowExecutor` — DAG-based parallel execution via `FuturesUnordered` |
| `executor_cache.rs` | `ExecutorCache` — SQLite-backed result caching (via `flowpersist`) |
| `registry.rs` | `NodeRegistry` — factory-based node instantiation, `NodeMetadata` |
| `runtime.rs` | `FlowRuntime` — main entry point, coordinates registry + executor + event bus |
| `tracker.rs` | `DependencyTracker` — incremental dependency tracking replacing O(N) scan |
| `dyn_loader.rs` | `DynamicLoader` — `libloading`-based `.so` plugin loading |
| `loader.rs` | `CustomNodeLoader` — JSON node definitions + `.so` plugin scanning from watch dir |

### `flownodes` — Built-In Node Library

14 node types across 8 categories. Source layout:

```
flownodes/src/
  lib.rs           — register_all() wires 14 factories into the registry
  http.rs          — HttpRequestNode
  transform.rs     — JsonParseNode, JsonStringifyNode
  shell.rs         — ShellNode
  debug.rs         — DebugNode
  time.rs          — DelayNode
  api_call.rs      — ApiCallNode
  browser.rs       — BrowserRenderNode
  docker.rs        — DockerNode (v1)
  docker_v2.rs     — DockerNodeV2
  zypi.rs          — ZypiExecNode, ZypiSessionCreateNode
  flow/
    branch.rs      — BranchNode
    merge.rs       — MergeNode
```

### `flowpersist` — Result Cache

SQLite-backed persistent cache for node execution results. Used by `ExecutorCache` to skip re-execution of nodes with identical inputs + config.

### `flowcli` — Command-Line Interface

Single `main.rs` binary: `run`, `validate`, `nodes`, `init`. Uses `clap` for argument parsing. Emits events as text or JSON (newline-delimited).

## Value Type System

See [workflow-format.md](../getting-started/workflow-format.md) for the full reference. Key points:

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
    // Arc-backed variants for zero-copy sharing
    StringArc(Arc<String>),
    BytesArc(Arc<Vec<u8>>),
    JsonArc(Arc<serde_json::Value>),
}
```

Arc variants enable O(1) clone via atomic reference count. The executor calls `.to_arc()` on node outputs before fanning out to downstream connections.

## Execution Model

### DAG Execution Algorithm

1. Parse workflow definition (JSON or YAML, auto-detected).
2. Build directed acyclic graph with `petgraph::DiGraph`.
3. Validate — topological sort detects cycles.
4. Build `DependencyTracker` — incremental dependency state per node.
5. Execute loop:

```
while not_all_complete:
    ready_nodes = tracker.find_ready()   // O(K) where K = nodes with newly-satisfied deps
    for node in ready_nodes[..max_parallel]:
        spawn execute_node(node)
    await any_task_completion()
    tracker.mark_completed(node_id)      // unlocks dependents
    handle_result()
```

6. Cleanup — call `shutdown()` on nodes that implemented it.

### Dependency Tracker

Replaces the original O(N) scan per event with incremental tracking:

- On execution start: each node gets a counter of pending upstream dependencies.
- On node completion: only direct dependents are decremented.
- Ready nodes: those whose dependency count reaches zero.

This eliminates the per-event full-graph scan, making DAG evaluation O(edges) total instead of O(edges × events).

## Parallelism Strategy

- Uses `FuturesUnordered` for concurrent execution.
- Respects `WorkflowSettings.max_parallel_nodes`.
- Dependencies prevent premature execution.
- `OnError::StopWorkflow` kills remaining tasks on first failure.
- `OnError::ContinueOnError` skips failed nodes and continues.

## Format Detection

Workflow files are parsed by extension:

- `.yaml` / `.yml` → `YamlParser`
- Everything else → `JsonParser`
- Stdin (`--file -`): first non-whitespace char `{` → JSON, else YAML

The `WorkflowParser` trait is implemented by `JsonParser` and `YamlParser`. See [workflow-format.md](../getting-started/workflow-format.md) for serialization details.

## Executor Cache

`ExecutorCache` stores node execution results keyed by `(node_id, hash(config), hash(inputs))` in SQLite via `flowpersist`. On re-execution of a workflow with unchanged inputs, cached nodes are skipped. Cache entries include:

- Node ID
- Input hash
- Config hash
- Output (serialized)
- Execution timestamp
- Duration

## Error Handling

### Node-Level Retry

```rust
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
    pub max_delay_ms: Option<u64>,
    pub retry_on_timeout: bool,
}
```

Per-node. Executor retries failed nodes with exponential backoff.

### Workflow-Level Handling

```rust
pub enum ErrorHandling {
    StopWorkflow,
    ContinueOnError,
    RetryWorkflow { max_attempts: u32 },
}
```

## State Management

### Per-Execution State

```rust
pub struct NodeState {
    pub data: HashMap<String, Value>,
}
```

Shared across a single workflow execution via `Arc<RwLock<NodeState>>`. Use for counters, accumulators, and caching within a run.

### Persistent State via `initialize()`

Nodes that need long-lived resources (DB pools, HTTP clients) implement `initialize()`:

```rust
impl Node for DatabaseNode {
    async fn initialize(&mut self) -> Result<(), NodeError> {
        self.pool = create_connection_pool().await?;
        Ok(())
    }
}
```

## Concurrency Model

- All async execution uses Tokio multi-threaded runtime.
- Nodes execute as independent `tokio::spawn` tasks, bounded by `max_parallel_nodes`.
- Event broadcast uses Tokio's `broadcast` channel: multiple subscribers, bounded buffer, lock-free.
- `CancellationToken` enables graceful shutdown.

## Performance

### Arc-Backed Values

`.to_arc()` converts `String` → `StringArc`, `Bytes` → `BytesArc`, `Json` → `JsonArc`. Cloning becomes O(1) atomic refcount increment.

### Dependency Tracker

Eliminates per-event full DAG scans. Ready-node discovery is O(K) where K = nodes with newly-satisfied dependencies.

### Executor Caching

SQLite-backed result cache skips re-execution of nodes with identical (node_id, config, inputs).

## Plugin System

Dynamic `.so` loading via `libloading`. See [plugin-development.md](plugin-development.md) for the full plugin API.

Key points:
- `NodePlugin` trait with `create_node()`, `metadata()`, `node_type()`, `port_definitions()`
- `extern "C" fn plugin_create()` and `plugin_version()` exports
- `DynamicLoader` scans directory for `.so` files
- Plugins are wrapped as `NodeFactory` and registered in `NodeRegistry`
- Libraries are intentionally leaked (`std::mem::forget`) for process-lifetime validity

## Security

- Nodes run in the same process as the executor (trust required).
- External commands (`shell.exec`, `docker.run`) are gated by Docker daemon permissions.
- Python sandbox nodes (`api.call`, `browser.render`) execute in Firecracker microVMs via Zypi.
- No network isolation between nodes within a workflow.

## Testing

### Unit Tests

`NodeContext` can be constructed with test emitters:

```rust
let ctx = NodeContext::new(node_id, EventEmitter::new_test());
ctx.inputs.insert("text".to_string(), Value::String("hello".to_string()));
```

### Integration Tests

Use `FlowRuntime` to execute full workflows in-memory and assert on `ExecutionResult`:

```rust
let runtime = FlowRuntime::new();
registry.register_all();  // Register all built-in nodes
let result = runtime.execute(&workflow, inputs).await?;
assert_eq!(result.completed_nodes, 3);
```

### Benchmarks

Measure DAG execution overhead and per-node performance with `criterion`.

## Monitoring

- Structured logging via `tracing` with `node_id`, `duration_ms` spans.
- `IggyEventBus` for streaming events to Iggy message server.
- CLI `--output json` for machine-readable event streams (newline-delimited JSON).
