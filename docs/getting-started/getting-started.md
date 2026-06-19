# Getting Started with Flow Engine

## Prerequisites

- Rust 1.70+ (`rustc --version`)
- Python 3.10+ (for the Python SDK)
- Cargo package manager

## Installation

```bash
# Build all crates
cargo build --release

# Optional: install Python SDK in development mode
cd python && pip install -e .
```

## Your First Workflow

### 1. Create a workflow file

```bash
./target/release/flow init --output my_first_workflow.json
```

This creates an example workflow with HTTP request → Debug log.

### 2. Run the workflow

```bash
./target/release/flow run \
  --file my_first_workflow.json \
  --input '{"url": "https://api.github.com/zen"}' \
  --verbose
```

You should see output like:

```
🚀 Loading workflow from: my_first_workflow.json
📋 Workflow: Example HTTP Workflow
   Nodes: 2
   Connections: 1

▶️  Workflow started
  ⚡ Starting node: a1b2c3d4... (http.request)
     ℹ️  [a1b2c3d4...] GET https://api.github.com/zen
     ℹ️  [a1b2c3d4...] Response status: 200
  ✅ Node a1b2c3d4... completed in 234ms
  ⚡ Starting node: b2c3d4e5... (debug.log)
     ℹ️  [b2c3d4e5...] DEBUG: Design is not just what it looks like...
  ✅ Node b2c3d4e5... completed in 2ms
✨ Workflow completed successfully in 236ms
```

### 3. Try JSON structured output

```bash
./target/release/flow run \
  --file my_first_workflow.json \
  --input '{"url": "https://api.github.com/zen"}' \
  --output json
```

This emits one JSON line per execution event — useful for scripting and monitoring.

### 4. Use YAML workflows

Create a YAML file `workflow.yaml`:

```yaml
name: Hello YAML
nodes:
  - id: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
    node_type: http.request
    name: Fetch Quote
    config:
      method: GET
  - id: "b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e"
    node_type: debug.log
    name: Print Quote
connections:
  - from_node: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
    from_port: body
    to_node: "b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e"
    to_port: message
```

Then run it:

```bash
./target/release/flow run \
  --file workflow.yaml \
  --input '{"url": "https://api.github.com/zen"}'
```

Format detection is automatic: `.yaml`/`.yml` files use the YAML parser, everything else uses JSON. See [workflow-format.md](workflow-format.md) for full details.

## Understanding the Workflow

Open `my_first_workflow.json` to see the structure:

```json
{
  "name": "Example HTTP Workflow",
  "nodes": [
    {
      "id": "node-1",
      "node_type": "http.request",
      "config": { "method": "GET" }
    },
    {
      "id": "node-2",
      "node_type": "debug.log"
    }
  ],
  "connections": [
    {
      "from_node": "node-1",
      "from_port": "body",
      "to_node": "node-2",
      "to_port": "message"
    }
  ]
}
```

Key concepts:

- **Nodes**: Executable units with inputs and outputs. Each has a `node_type` and optional `config`.
- **Connections**: Data flows from one node's output port to another's input port.
- **Config**: Static configuration for nodes — plain values, not tagged `{"type": "String", "value": ...}` wrappers.
- **Inputs**: Dynamic data passed at runtime via `--input`.

## Available Commands

```bash
# Run a workflow
flow run --file workflow.json --input '{"key": "value"}'

# Run with structured JSON event output
flow run --file workflow.json --output json

# Validate workflow syntax
flow validate --file workflow.json

# List available node types
flow nodes

# Create example workflow
flow init --output new_workflow.json
```

All four commands: `run`, `validate`, `nodes`, `init`. See [cli-reference.md](cli-reference.md) for all flags and exit codes.

## Available Node Types

Run `flow nodes` to see all available types. The engine ships with 14 built-in nodes:

| Category | Node Types |
|----------|-----------|
| HTTP | `http.request` |
| Transform | `transform.json_parse`, `transform.json_stringify` |
| Shell | `shell.exec` |
| Debug | `debug.log` |
| Time | `time.delay` |
| API | `api.call` |
| Browser | `browser.render` |
| Docker | `docker.exec`, `docker.v2.exec` |
| Zypi | `zypi.exec`, `zypi.session_create` |
| Flow Control | `flow.branch`, `flow.merge` |

Full reference at [nodes/index.md](../using/nodes/index.md).

## Python SDK Quickstart

```python
from flowengine import task, Flow

@task(retry=3, timeout=30)
def fetch_user(url: str) -> dict:
    import requests
    r = requests.get(url)
    r.raise_for_status()
    return r.json()

flow = Flow("user-pipeline")
flow >> fetch_user
result = flow.run(url="https://jsonplaceholder.typicode.com/users/1")
```

For local execution without a server, use `FlowBuilder.build_async()`:

```python
from flowengine import FlowBuilder, task

@task()
def greet(name: str) -> str:
    return f"Hello, {name}!"

builder = FlowBuilder("local-greeter")
greet_task = builder.add(greet)
result = await builder.build_async(name="World")
```

This serializes the workflow and pipes it through the `flow` CLI. Set the `FLOW_BIN` environment variable if `flow` is not on your `PATH`.

See [python-sdk.md](../using/python-sdk.md) for the full API reference.

## Creating a Custom Workflow

### Example: Multi-Step API Processing

```json
{
  "name": "User Data Processor",
  "nodes": [
    {
      "id": "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d",
      "node_type": "http.request",
      "name": "Fetch User",
      "config": { "method": "GET" }
    },
    {
      "id": "b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e",
      "node_type": "transform.json_parse",
      "name": "Parse JSON"
    },
    {
      "id": "c3d4e5f6-a7b8-4c5d-9e0f-1a2b3c4d5e6f",
      "node_type": "debug.log",
      "name": "Log Result"
    }
  ],
  "connections": [
    {
      "from_node": "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d",
      "from_port": "body",
      "to_node": "b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e",
      "to_port": "json"
    },
    {
      "from_node": "b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e",
      "from_port": "parsed",
      "to_node": "c3d4e5f6-a7b8-4c5d-9e0f-1a2b3c4d5e6f",
      "to_port": "message"
    }
  ],
  "settings": {
    "max_parallel_nodes": 10,
    "on_error": "StopWorkflow"
  }
}
```

Save and run:

```bash
./target/release/flow run \
  --file user_processor.json \
  --input '{"url": "https://jsonplaceholder.typicode.com/users/1"}'
```

## Next Steps

1. **Read the workflow format reference** — [workflow-format.md](workflow-format.md)
2. **Explore node types** — [nodes/index.md](../using/nodes/index.md)
3. **Use the Python SDK** — [python-sdk.md](../using/python-sdk.md)
4. **Build custom nodes** — [node-development.md](../developing/node-development.md)
5. **Understand the architecture** — [architecture.md](../developing/architecture.md)

## Troubleshooting

### Build errors

```bash
cargo clean
cargo build --release
```

### Workflow validation errors

```bash
flow validate --file workflow.json
```

### Need help?

- Check `README.md` for feature overview
- See `docs/INDEX.md` for the full documentation index
- Look at `examples/` for working workflows
