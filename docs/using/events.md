# Events

The flow engine uses an event-driven architecture for execution monitoring and inter-component communication.

## Event System Architecture

```
Node.execute()
    └── EventEmitter.emit(NodeEvent::Info(...))
        └── EventBus.dispatch(ExecutionEvent::NodeLog(...))
            └── Subscribers receive event
                ├── CLI text output
                ├── CLI JSON output
                └── Iggy message bus (optional)
```

## `ExecutionEvent` Enum

Top-level execution events emitted by the engine. Six variants:

| Variant | Fields | When |
|---------|--------|------|
| `WorkflowStarted` | `execution_id: String` | Workflow execution begins |
| `NodeStarted` | `execution_id: String`, `node_id: String`, `node_type: String` | A node starts executing |
| `NodeLog` | `execution_id: String`, `node_id: String`, `event: NodeEvent` | A node emits a sub-event |
| `NodeCompleted` | `execution_id: String`, `node_id: String`, `duration_ms: u64` | A node completes successfully |
| `NodeFailed` | `execution_id: String`, `node_id: String`, `error: String` | A node fails |
| `WorkflowCompleted` | `execution_id: String`, `success: bool`, `duration_ms: u64` | Workflow execution ends |

## `NodeEvent` Enum

Sub-events emitted by individual nodes during execution. Carried inside `ExecutionEvent::NodeLog`.

| Variant | Fields | Description |
|---------|--------|-------------|
| `Info` | `message: String` | Informational log message |
| `Warning` | `message: String` | Warning message |
| `Progress` | `percent: f64, message: String` | Progress update (0-100) |
| `Data` | `key: String, value: Value` | Structured data payload |
| `StdoutLine` | `line: String` | Line from process stdout |
| `StderrLine` | `line: String` | Line from process stderr |

## `EventEmitter`

Each node receives an `EventEmitter` via the execution context for emitting `NodeEvent`s:

```rust
use flowcore::events::{EventEmitter, NodeEvent};

impl Node for MyNode {
    fn execute(
        &self,
        config: &HashMap<String, Value>,
        inputs: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<NodeOutput, NodeError> {
        ctx.events.info("Starting work...".to_string());

        // Report progress
        ctx.events.progress(50.0, "Halfway done".to_string());

        // Emit structured data
        ctx.events.data("intermediate", Value::Number(42.0));
    }
}
```

### EventEmitter Methods

| Method | Emits |
|--------|-------|
| `info(msg: String)` | `NodeEvent::Info` |
| `warn(msg: String)` | `NodeEvent::Warning` |
| `progress(percent: f64, msg: String)` | `NodeEvent::Progress` |
| `data(key: String, value: Value)` | `NodeEvent::Data` |
| `stdout_line(line: String)` | `NodeEvent::StdoutLine` |
| `stderr_line(line: String)` | `NodeEvent::StderrLine` |

## `EventBus` Trait

The `EventBus` trait defines the interface for event dispatch:

```rust
pub trait EventBus {
    fn subscribe(&mut self, receiver: tokio::sync::broadcast::Sender<ExecutionEvent>);
    fn emit(&self, event: ExecutionEvent);
}
```

Implementations:
- **In-memory broadcast channel** — the default, sends events to all subscribers
- **`IggyEventBus`** — publishes events to an Iggy message streaming server

## CLI Event Output

### `--output text` (default)

Human-readable emoji-marker output. Uses `NodeEvent` metadata to format lines:

```
  ⚡ Starting node: a1b2c3d4... (http.request)
     ℹ️  [a1b2c3d4...] GET https://example.com
     📊 [a1b2c3d4...] 50% - Processing...
     📤 [a1b2c3d4...] <stdout line>
  ✅ Node a1b2c3d4... completed in 234ms
```

### `--output json`

One JSON object per `ExecutionEvent`, newline-delimited:

```json
{"type":"workflow_started","execution_id":"exec-001"}
{"type":"node_started","execution_id":"exec-001","node_id":"a1b2...","node_type":"http.request"}
{"type":"node_log","execution_id":"exec-001","node_id":"a1b2...","event":{"type":"Info","message":"GET https://example.com"}}
{"type":"node_log","execution_id":"exec-001","node_id":"a1b2...","event":{"type":"Progress","percent":50.0,"message":"Processing..."}}
{"type":"node_completed","execution_id":"exec-001","node_id":"a1b2...","duration_ms":234}
{"type":"workflow_completed","execution_id":"exec-001","success":true,"duration_ms":236}
```

### Consuming the JSON stream

```bash
# Run and pipe to jq
flow run --file workflow.json --output json | while IFS= read -r line; do
    type=$(echo "$line" | jq -r '.type')
    case "$type" in
        node_completed)
            duration=$(echo "$line" | jq -r '.duration_ms')
            echo "  Done in ${duration}ms"
            ;;
        node_failed)
            error=$(echo "$line" | jq -r '.error')
            echo "  FAILED: $error"
            ;;
    esac
done
```

## `IggyEventBus`

Optional integration with the [Iggy](https://github.com/iggy-rs/iggy) message streaming server for distributed event consumption.

```rust
use flowcore::events::{IggyEventBus, IggyEventBusConfig};

let config = IggyEventBusConfig {
    server_address: "127.0.0.1:8090".to_string(),
    stream_name: "flow_events".to_string(),
    topic_name: "executions".to_string(),
    ..Default::default()
};

let bus = IggyEventBus::new(config).await?;
// Wiring the IggyEventBus into the executor subscribes all execution events
```

### Configuration

| Config Field | Type | Default | Description |
|-------------|------|---------|-------------|
| `server_address` | String | `"127.0.0.1:8090"` | Iggy server address |
| `stream_name` | String | `"flow_events"` | Iggy stream name |
| `topic_name` | String | `"executions"` | Iggy topic name |
| `auto_create` | bool | `true` | Auto-create stream/topic if absent |
| `client_id` | String | auto-generated | Client identifier |

## Subscribing in Rust

```rust
use tokio::sync::broadcast;
use flowcore::events::{EventBus, ExecutionEvent};

// Create a channel
let (tx, mut rx) = broadcast::channel::<ExecutionEvent>(256);

// Subscribe to the event bus
event_bus.subscribe(tx);

// Consume events
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            ExecutionEvent::NodeCompleted { node_id, duration_ms, .. } => {
                println!("Node {} done in {}ms", node_id, duration_ms);
            }
            ExecutionEvent::NodeFailed { node_id, error, .. } => {
                eprintln!("Node {} failed: {}", node_id, error);
            }
            _ => {}
        }
    }
});
```
