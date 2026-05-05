# Flow Engine 🚀

A Rust-based event-driven workflow engine with real-time execution monitoring. Like Prefect, but faster, simpler, and sandboxed with Firecracker microVMs.

## Features

- ✨ **Event-Driven Architecture** — Reactive, real-time workflows with broadcast events
- ⚡ **Parallel DAG Execution** — Configurable parallelism with topological sort
- 🔥 **Firecracker Sandboxing** — Execute nodes in Zypi Firecracker microVMs (sub-second boot)
- 🐚 **Shell & Process Nodes** — Run local commands with streaming stdout/stderr
- 🐳 **Docker Nodes** — Run containers with full config (env, volumes, resource limits, I/O modes)
- 🔄 **Retry with Backoff** — Exponential backoff, max delays, per-node retry policies
- 💾 **SQLite Persistence** — Save/load workflows, execution history, node-level result caching
- 📡 **Streaming Output** — Real-time stdout/stderr line streaming via WebSocket/CLI
- 🐍 **Python SDK** — `@task` decorator, `Flow` DAG builder, `Sandbox` for Zypi
- 🎯 **Type-Safe** — Rust type system for reliability, no runtime surprises
- 📊 **Observability** — Detailed execution metrics, tracing, event bus

## vs Prefect

| | FlowEngine | Prefect |
|---|---|---|
| **Runtime** | Rust (fast, single binary) | Python (GIL-bound, heavy env) |
| **Sandbox** | Firecracker μVMs (sub-second) | Docker only |
| **Latency per node** | <10ms | 100–500ms |
| **Streaming** | Native WebSocket events | Polling |
| **Python API** | `@task`, `Flow`, `Sandbox` | `@task`, `@flow` |
| **Retry** | Exponential backoff | Exponential backoff |
| **Caching** | Content-fingerprint (SQLite) | Result persistence |
| **Deployment** | Single binary (`flow` + `flowserver`) | Python env + server |

## Architecture

```
flowengine/
├── flowcore      - Core abstractions (Node trait, Value type, Events, RetryPolicy)
├── flowruntime   - Execution engine (DAG executor, Registry, Runtime)
├── flownodes     - Standard node library (shell, zypi, docker, http, transform, debug)
├── flowpersist   - SQLite-backed persistence & result caching
├── flowserver    - HTTP/WebSocket API server (Actix-based)
├── flowcli       - Command-line interface
└── python/       - Python SDK (flowengine package)
```

## Quick Start

### Installation

```bash
cargo build --release
```

### Run a Shell Pipeline

```bash
./target/release/flow run \
  --file examples/shell_pipeline.json \
  --verbose
```

### Run a Zypi-Sandboxed Pipeline

```bash
# Start Zypi first
cd ../../exs/zypi && docker compose up -d

# Run sandboxed
./target/release/flow run \
  --file examples/zypi_sandbox.json \
  --input '{"value": 42, "text": "hello from firecracker"}' \
  --verbose
```

### Start the HTTP Server

```bash
./target/release/flowserver
# → http://localhost:3000
# → WebSocket: ws://localhost:3000/api/events
```

See [API Documentation](docs/api.md) for full HTTP endpoints.

## Python SDK

```bash
pip install -e python/
```

### Decorator-based Workflows

```python
from flowengine import Flow, task
import requests

@task(retry=3, timeout=30)
def fetch_data(url: str) -> dict:
    return requests.get(url).json()

@task()
def process(data: dict) -> dict:
    return {"summary": data["title"]}

flow = Flow("data-pipeline")
flow >> fetch_data >> process
result = flow.run(url="https://api.github.com/zen")
```

### Zypi Sandbox (drop-in for `bubbleproc.py`)

```python
from flowengine import Sandbox

sandbox = Sandbox(image="ubuntu:24.04")

# Execute commands in Firecracker microVMs
exit_code, stdout, stderr = sandbox.exec(["python3", "script.py"])

# File injection
result = sandbox.exec(
    ["python3", "/app/analyze.py"],
    files={"/app/analyze.py": "print('sandboxed!')"},
)

# check_output — raises on failure
output = sandbox.check_output(["echo", "hello"])

# Context manager
with Sandbox() as s:
    s.exec(["ls", "-la"])
```

## Workflow Definition

### JSON Format

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Example Workflow",
  "nodes": [
    {
      "id": "node-1",
      "node_type": "http.request",
      "name": "Fetch Data",
      "config": {
        "method": {"type": "String", "value": "GET"}
      },
      "retry_policy": {
        "max_attempts": 3,
        "delay_ms": 1000,
        "backoff_multiplier": 2.0,
        "max_delay_ms": 60000,
        "retry_on_timeout": true
      }
    },
    {
      "id": "node-2",
      "node_type": "debug.log",
      "name": "Log Result"
    }
  ],
  "connections": [
    {
      "from_node": "node-1",
      "from_port": "body",
      "to_node": "node-2",
      "to_port": "message"
    }
  ],
  "settings": {
    "max_parallel_nodes": 10,
    "on_error": "StopWorkflow"
  }
}
```

### Python Builder API

```python
from flowengine import FlowBuilder, task

builder = FlowBuilder("explicit-pipeline")
fetch = builder.add(fetch_data)
proc = builder.add(process)
builder.connect(fetch, "output", proc, "input")
flow = builder.build()
flow.save("workflow.json")
```

## Built-in Nodes

### Shell & Process

- **`shell.exec`** — Execute local commands
  - Config: `command`, `args`, `env`, `workdir`, `timeout`, `shell`, `stream_output`, `env_passthrough`
  - Inputs: `stdin` (piped to process)
  - Outputs: `output`, `stdout`, `stderr`, `exit_code`, `success`
  - Events: real-time `StdoutLine` / `StderrLine` streaming

- **`zypi.exec`** — Execute in Firecracker microVM via Zypi API
  - Config: `url`, `image`, `command`, `env`, `workdir`, `timeout`
  - Inputs: `stdin`, `files` (object), `file:<path>` (individual files)
  - Outputs: `output`, `stdout`, `stderr`, `exit_code`, `success`, `duration_ms`

### Docker

- **`docker.run`** — Run Docker containers with full configuration
  - Config: `image`, `command`, `entrypoint`, `env`, `volumes`, `workdir`, `user`, `network`, `cpu_limit`, `memory_limit`, `stdin_mode`, `output_mode`, `io_mode`, `auto_pull`, `detached`, `remove`, `timeout`
  - I/O modes: `flat` (plain values), `wrapped` (Value enum), `auto`
  - Inputs: `data` (stdin)
  - Outputs: `output`, `stdout`, `stderr`, `exit_code`, `success`

### HTTP

- **`http.request`** — Make HTTP requests
  - Config: `method` (GET/POST/PUT/DELETE), `headers`
  - Inputs: `url`, `body` (optional)
  - Outputs: `status`, `body`, `headers`

### Transform

- **`transform.json_parse`** — Parse JSON strings
  - Inputs: `json` (string)
  - Outputs: `parsed` (JSON value)

- **`transform.json_stringify`** — Convert values to JSON
  - Inputs: `value` (any)
  - Outputs: `json` (string)

### Utility

- **`time.delay`** — Delay execution (passthrough inputs)
  - Config: `delay_ms`

- **`debug.log`** — Log values for debugging
  - Inputs: `message` (any)
  - Outputs: `message` (passthrough)

## Retry Policies

Every node supports exponential backoff retry:

```json
{
  "retry_policy": {
    "max_attempts": 5,
    "delay_ms": 1000,
    "backoff_multiplier": 2.0,
    "max_delay_ms": 60000,
    "retry_on_timeout": true
  }
}
```

Delays: 1s → 2s → 4s → 8s → 16s (capped at 60s max).

## Streaming Events

Nodes emit real-time events streamed to CLI, WebSocket, or programmatic subscribers:

```rust
let mut events = runtime.subscribe_events();

while let Ok(event) = events.recv().await {
    match event {
        ExecutionEvent::NodeStarted { node_id, node_type, .. } => { }
        ExecutionEvent::NodeCompleted { node_id, duration_ms, .. } => { }
        ExecutionEvent::NodeFailed { node_id, error, .. } => { }
        ExecutionEvent::NodeEvent { event, .. } => match event {
            NodeEvent::Info { message } => { }
            NodeEvent::Warning { message } => { }
            NodeEvent::Progress { percent, message } => { }
            NodeEvent::StdoutLine { line } => { }    // streaming!
            NodeEvent::StderrLine { line } => { }    // streaming!
            _ => { }
        }
        _ => { }
    }
}
```

## Persistence & Caching

```rust
use flowpersist::PersistentStore;

let store = PersistentStore::open("flowengine.db")?;

// Save a workflow
store.save_workflow(&workflow)?;

// Load it back
let wf = store.load_workflow(id)?;

// Record execution history
store.record_execution(&ExecutionRecord { ... })?;

// Cache node results with content fingerprint
let config_hash = PersistentStore::compute_hash(&config);
let input_hash = PersistentStore::compute_hash(&inputs);
store.cache_result("shell.exec", &config_hash, &input_hash, &outputs, Some(3600))?;

// Check cache before re-executing
if let Some(cached) = store.get_cached_result("shell.exec", &config_hash, &input_hash)? {
    return Ok(cached); // cache hit!
}
```

## Creating Custom Nodes

### 1. Implement the Node Trait

```rust
use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};

pub struct MyCustomNode;

#[async_trait]
impl Node for MyCustomNode {
    fn node_type(&self) -> &str { "custom.my_node" }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let input = ctx.require_input("data")?;

        ctx.events.info("Processing...");
        ctx.events.progress(50.0, Some("Halfway".to_string()));

        // Stream output to subscribers
        ctx.events.stdout_line("processing item 1");
        ctx.events.stdout_line("processing item 2");

        Ok(NodeOutput::new()
            .with_output("result", "done"))
    }
}
```

### 2. Create a Factory

```rust
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};

pub struct MyCustomNodeFactory;

impl NodeFactory for MyCustomNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(MyCustomNode))
    }

    fn node_type(&self) -> &str { "custom.my_node" }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "My custom node".to_string(),
            category: "custom".to_string(),
            inputs: vec![PortDefinition {
                name: "data".to_string(),
                description: "Input data".to_string(),
                required: true,
            }],
            outputs: vec![PortDefinition {
                name: "result".to_string(),
                description: "Processed result".to_string(),
                required: false,
            }],
        }
    }
}
```

### 3. Register

```rust
registry.register(Arc::new(MyCustomNodeFactory));
```

## CLI Commands

```bash
# Run a workflow
flow run --file workflow.json --input '{"key": "value"}' --verbose

# Validate workflow
flow validate workflow.json

# List available node types
flow nodes

# Create example workflow
flow init --output my_workflow.json
```

## Roadmap

- [x] Core execution engine
- [x] Event-driven architecture
- [x] Standard node library
- [x] Shell & process execution with streaming
- [x] Zypi Firecracker microVM integration
- [x] Exponential backoff retry policies
- [x] CLI interface
- [x] HTTP/WebSocket API server
- [x] Python SDK (`@task`, `Flow`, `Sandbox`)
- [x] SQLite persistence & result caching
- [ ] Bevy-based visual editor
- [ ] Scheduling & triggers (cron, webhooks)
- [ ] Distributed execution
- [ ] WASM plugin support
- [ ] Monitoring dashboard
- [ ] Prefect-compatible API layer

## Performance

- **448ms** — 3-node shell pipeline (curl → python → debug)
- Async/await throughout (Tokio runtime)
- Parallel node execution with configurable limits
- Zero-copy value passing where possible
- Efficient DAG traversal with petgraph

## Contributing

Contributions welcome! Priority areas:

1. **New Nodes** — Databases, message queues, file systems, cloud APIs
2. **Bevy UI** — Visual workflow editor
3. **Scheduling** — Cron triggers, webhooks
4. **Distributed** — Multi-node execution, work stealing
5. **Documentation** — Guides, tutorials, video demos

## License

MIT OR Apache-2.0

## Acknowledgments

Inspired by:
- **Prefect** — Python workflow orchestration
- **n8n** — Node-based automation
- **Apache Airflow** — DAG scheduling
- **Firecracker** — MicroVM sandboxing
- **Bevy** — ECS architecture
