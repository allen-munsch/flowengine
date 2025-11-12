# Flow Engine 🚀

A Rust-based event-driven workflow engine with real-time execution monitoring.

## Features

- ✨ **Event-Driven Architecture** - Built for reactive, real-time workflows
- ⚡ **Parallel Execution** - DAG-based execution with configurable parallelism
- 🔌 **Extensible** - Trait-based plugin system for custom nodes
- 📡 **Real-Time Events** - Subscribe to execution events via broadcast channels
- 🎯 **Type-Safe** - Leverages Rust's type system for reliability
- 🔄 **Retry Policies** - Node-level and workflow-level error handling
- 📊 **Observability** - Detailed execution metrics and logging

## Architecture

```
flowengine/
├── flowcore      - Core abstractions (Node trait, Value type, Events)
├── flowruntime   - Execution engine (DAG executor, Registry)
├── flownodes     - Standard node library (HTTP, JSON, Debug, etc.)
├── flowserver    - HTTP/WebSocket API server (Actix-based)
└── flowcli       - Command-line interface
```

## Quick Start

### Installation

```bash
cargo build --release
```

### Create an Example Workflow

```bash
./target/release/flow init --output my_workflow.json
```

### Run a Workflow

```bash
./target/release/flow run \
  --file my_workflow.json \
  --input '{"url": "https://api.github.com/zen"}' \
  --verbose

cargo run --bin flow -- \
run --file examples/data_pipeline.json \
--input '{"url": "https://api.github.com/repos/rust-lang/rust"}'
```

### Start the HTTP Server

```bash
# Start the API server
./target/release/flowserver

# Server runs on http://localhost:3000
# WebSocket events: ws://localhost:3000/api/events
```

See [API Documentation](docs/api.md) for HTTP endpoints.

## Workflow Definition

Workflows are defined as JSON files:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Example Workflow",
  "description": "Fetches and processes data",
  "nodes": [
    {
      "id": "node-1",
      "node_type": "http.request",
      "name": "Fetch Data",
      "config": {
        "type": "String",
        "value": "GET"
      },
      "position": { "x": 100, "y": 100 }
    },
    {
      "id": "node-2",
      "node_type": "transform.json_parse",
      "name": "Parse Response",
      "position": { "x": 300, "y": 100 }
    },
    {
      "id": "node-3",
      "node_type": "debug.log",
      "name": "Log Result",
      "position": { "x": 500, "y": 100 }
    }
  ],
  "connections": [
    {
      "from_node": "node-1",
      "from_port": "body",
      "to_node": "node-2",
      "to_port": "json"
    },
    {
      "from_node": "node-2",
      "from_port": "parsed",
      "to_node": "node-3",
      "to_port": "message"
    }
  ],
  "triggers": [],
  "settings": {
    "max_parallel_nodes": 10,
    "on_error": "StopWorkflow"
  }
}
```

## Creating Custom Nodes

### 1. Implement the Node Trait

```rust
use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};

pub struct MyCustomNode {
    // Node state
}

#[async_trait]
impl Node for MyCustomNode {
    fn node_type(&self) -> &str {
        "custom.my_node"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        // Get inputs
        let input = ctx.require_input("data")?;
        
        // Emit progress events
        ctx.events.info("Processing data...");
        ctx.events.progress(50.0, Some("Halfway done".to_string()));
        
        // Do work
        let result = process_data(input)?;
        
        // Return outputs
        Ok(NodeOutput::new()
            .with_output("result", result))
    }
}
```

### 2. Create a Factory

```rust
use flowruntime::{NodeFactory, NodeMetadata};

pub struct MyCustomNodeFactory;

impl NodeFactory for MyCustomNodeFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(MyCustomNode::new(config)?))
    }
    
    fn node_type(&self) -> &str {
        "custom.my_node"
    }
    
    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Does custom processing".to_string(),
            category: "custom".to_string(),
            inputs: vec![
                PortDefinition {
                    name: "data".to_string(),
                    description: "Input data".to_string(),
                    required: true,
                }
            ],
            outputs: vec![
                PortDefinition {
                    name: "result".to_string(),
                    description: "Processed result".to_string(),
                    required: false,
                }
            ],
        }
    }
}
```

### 3. Register the Node

```rust
let mut registry = NodeRegistry::new();
registry.register(Arc::new(MyCustomNodeFactory));
```

## Built-in Nodes

### HTTP Nodes

- **`http.request`** - Make HTTP requests
  - Config: `method` (GET/POST/PUT/DELETE)
  - Inputs: `url`, `body` (optional), `headers` (optional)
  - Outputs: `status`, `body`, `headers`

### Transform Nodes

- **`transform.json_parse`** - Parse JSON strings
  - Inputs: `json` (string)
  - Outputs: `parsed` (JSON value)

- **`transform.json_stringify`** - Convert values to JSON
  - Inputs: `value` (any)
  - Outputs: `json` (string)

### Time Nodes

- **`time.delay`** - Delay execution
  - Config: `delay_ms` (number)
  - Inputs: any (passed through)
  - Outputs: same as inputs

### Debug Nodes

- **`debug.log`** - Log values
  - Inputs: `message` (any)
  - Outputs: `message` (passthrough)

## Real-Time Event Streaming

Subscribe to workflow execution events:

```rust
let runtime = FlowRuntime::new();
let mut events = runtime.subscribe_events();

tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match event {
            ExecutionEvent::NodeStarted { node_id, .. } => {
                println!("Node {} started", node_id);
            }
            ExecutionEvent::NodeCompleted { node_id, duration_ms, .. } => {
                println!("Node {} completed in {}ms", node_id, duration_ms);
            }
            ExecutionEvent::NodeEvent { event, .. } => {
                // Handle node-specific events (info, warnings, progress)
            }
            _ => {}
        }
    }
});
```

## CLI Commands

```bash
# Run a workflow
flow run --file workflow.json --input '{"key": "value"}'

# Validate workflow syntax
flow validate --file workflow.json

# List available node types
flow nodes

# Create example workflow
flow init --output my_workflow.json
```

## Roadmap

- [x] Core execution engine
- [x] Event-driven architecture
- [x] Standard node library
- [x] CLI interface
- [x] HTTP/WebSocket API server
- [ ] Bevy-based visual editor
- [ ] Workflow persistence (SQLite)
- [ ] Scheduling & triggers (cron, webhooks)
- [ ] Distributed execution
- [ ] WASM plugin support
- [ ] Streaming data support
- [ ] Process-based node isolation
- [ ] Monitoring dashboard

## Performance

- Async/await throughout (Tokio runtime)
- Parallel node execution with configurable limits
- Zero-copy value passing where possible
- Efficient DAG traversal with petgraph

## Contributing

Contributions welcome! Areas of interest:

1. **New Nodes** - Add common integrations (databases, APIs, file systems)
2. **Error Handling** - Improve retry logic and error recovery
3. **Testing** - Add integration tests and benchmarks
4. **Documentation** - Improve examples and guides
5. **Bevy UI** - Help build the visual editor

## License

MIT OR Apache-2.0

## Acknowledgments

Inspired by:
- **n8n** - Node-based automation
- **Apache Airflow** - Workflow orchestration
- **Pure Data** - Visual dataflow programming
- **Bevy** - ECS architecture
