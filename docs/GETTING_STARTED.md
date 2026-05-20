# Getting Started with Flow Engine

## Prerequisites

- Rust 1.70+ (`rustc --version`)
- Cargo package manager

## Installation

```bash
# Clone or extract the project
cd flowengine

# Build all crates
cargo build --release

# This will take a few minutes on first build
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
  ⚡ Starting node: a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d (http.request)
     ℹ️  [a1b2c3d4...] GET https://api.github.com/zen
     ℹ️  [a1b2c3d4...] Response status: 200
  ✅ Node a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d completed in 234ms
  ⚡ Starting node: b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e (debug.log)
     ℹ️  [b2c3d4e5...] DEBUG: Design is not just what it looks like...
  ✅ Node b2c3d4e5-f6a7-4b5c-8d9e-0f1a2b3c4d5e completed in 2ms
✨ Workflow completed successfully in 236ms
```

### 3. Try the example workflows

```bash
# Simple GitHub API example
./target/release/flow run \
  --file examples/github_zen.json \
  --input '{"url": "https://api.github.com/zen"}'

# More complex data pipeline
./target/release/flow run \
  --file examples/data_pipeline.json \
  --input '{"url": "https://jsonplaceholder.typicode.com/users/1"}'
```

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
- **Nodes**: Executable units with inputs and outputs
- **Connections**: Data flows from one node's output to another's input
- **Config**: Static configuration for nodes
- **Inputs**: Dynamic data passed at runtime

## Available Commands

```bash
# Run a workflow
flow run --file workflow.json --input '{"key": "value"}'

# Validate workflow syntax
flow validate --file workflow.json

# List available node types
flow nodes

# Create example workflow
flow init --output new_workflow.json
```

## Available Node Types

Run `flow nodes` to see all available types. Currently includes:

- **`http.request`** - Make HTTP requests
- **`debug.log`** - Log values
- **`transform.json_parse`** - Parse JSON strings
- **`transform.json_stringify`** - Convert to JSON
- **`time.delay`** - Delay execution

## Creating a Custom Workflow

### Example: Multi-Step API Processing

```json
{
  "name": "User Data Processor",
  "nodes": [
    {
      "id": "fetch",
      "node_type": "http.request",
      "name": "Fetch User",
      "config": {
        "method": { "type": "String", "value": "GET" }
      }
    },
    {
      "id": "parse",
      "node_type": "transform.json_parse",
      "name": "Parse JSON"
    },
    {
      "id": "log",
      "node_type": "debug.log",
      "name": "Log Result"
    }
  ],
  "connections": [
    {
      "from_node": "fetch",
      "from_port": "body",
      "to_node": "parse",
      "to_port": "json"
    },
    {
      "from_node": "parse",
      "from_port": "parsed",
      "to_node": "log",
      "to_port": "message"
    }
  ],
  "settings": {
    "max_parallel_nodes": 10,
    "on_error": "StopWorkflow"
  }
}
```

Save this as `user_processor.json` and run:

```bash
./target/release/flow run \
  --file user_processor.json \
  --input '{"url": "https://jsonplaceholder.typicode.com/users/1"}'
```

## Next Steps

1. **Read the docs**
   - `docs/architecture.md` - Understanding the system
   - `docs/node_development.md` - Creating custom nodes

2. **Explore examples**
   - `examples/github_zen.json` - Simple HTTP workflow
   - `examples/data_pipeline.json` - Multi-step processing

3. **Build custom nodes**
   - See `flownodes/src/` for examples
   - Follow patterns in node development guide

4. **Integrate with your app**
   ```rust
   use flowruntime::FlowRuntime;
   
   let runtime = FlowRuntime::new();
   let result = runtime.execute(&workflow, inputs).await?;
   ```

## Troubleshooting

### Build errors

```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Workflow validation errors

```bash
# Validate before running
flow validate --file workflow.json
```

### Need help?

- Check `README.md` for feature overview
- See `docs/` for detailed documentation
- Look at `examples/` for working workflows

## What's Next?

The engine is designed to be extended. You can:

1. **Add more nodes** - HTTP, databases, file operations, etc.
2. **Build a UI** - The Bevy-based visual editor is planned
3. **Add persistence** - Store workflows in a database
4. **Add scheduling** - Cron-based triggers
5. **Distribute execution** - Run nodes on different machines

See the roadmap in `README.md` for planned features.

---

Happy workflow building! 🚀
