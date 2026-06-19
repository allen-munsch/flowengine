# Node Development

How to build custom nodes for the Flow Engine.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Simple Stateless Node](#simple-stateless-node)
3. [Config-Driven Node with `#[derive(NodeConfig)]`](#config-driven-node)
4. [Stateful Node](#stateful-node)
5. [Emitting Events](#emitting-events)
6. [Arc-Backed Values and Performance](#arc-backed-values)
7. [Error Handling](#error-handling)
8. [Testing Nodes](#testing)
9. [Plugin Nodes](#plugin-nodes)

## Architecture Overview

Every node has three parts:

1. **Node struct** — implements the `Node` trait (business logic)
2. **NodeFactory** — creates node instances, provides metadata and port definitions
3. **Registration** — adding the factory to the `NodeRegistry`

The `Node` trait from `flowcore`:

```rust
use flowcore::{Node, NodeContext, NodeOutput, NodeError, Value};

#[async_trait]
pub trait Node: Send + Sync {
    fn node_type(&self) -> &str;
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError>;
    async fn initialize(&mut self) -> Result<(), NodeError> { Ok(()) }
    async fn shutdown(&mut self) -> Result<(), NodeError> { Ok(()) }
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<(), NodeError> { Ok(()) }
}
```

`NodeContext` provides:

| Field | Description |
|--------|-------------|
| `node_id: NodeId` | Unique UUID for this node instance |
| `inputs: HashMap<String, Value>` | Input values from connected upstream nodes |
| `config: HashMap<String, Value>` | Static configuration from the workflow definition |
| `state: Arc<RwLock<NodeState>>` | Per-node persistent state (survives across executions in same workflow run) |
| `events: EventEmitter` | Emit progress, logs, and data to the event bus |
| `cancellation: CancellationToken` | Check for graceful shutdown |

## Simple Stateless Node

A node that transforms input text to uppercase:

```rust
use flowcore::{Node, NodeContext, NodeOutput, NodeError, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;
use async_trait::async_trait;

pub struct UppercaseNode;

#[async_trait]
impl Node for UppercaseNode {
    fn node_type(&self) -> &str {
        "text.uppercase"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let text = ctx.require_input("text")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "text".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;

        let uppercase = text.to_uppercase();

        Ok(NodeOutput::new()
            .with_output("result", uppercase))
    }
}

pub struct UppercaseNodeFactory;

impl NodeFactory for UppercaseNodeFactory {
    fn create(&self, _config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(UppercaseNode))
    }

    fn node_type(&self) -> &str {
        "text.uppercase"
    }

    fn metadata(&self) -> NodeMetadata {
        NodeMetadata {
            description: "Converts input text to uppercase".to_string(),
            category: "text".to_string(),
            inputs: vec![
                PortDefinition {
                    name: "text".to_string(),
                    description: "Input text to convert".to_string(),
                    required: true,
                }
            ],
            outputs: vec![
                PortDefinition {
                    name: "result".to_string(),
                    description: "Uppercased text".to_string(),
                    required: false,
                }
            ],
        }
    }
}
```

### Registration

```rust
// In your main.rs or lib.rs
registry.register(Arc::new(UppercaseNodeFactory));
```

### Usage in Workflow

```json
{
  "node_type": "text.uppercase",
  "config": {}
}
```

## Config-Driven Node

Use `#[derive(NodeConfig)]` from `flowcore_macros` to auto-generate config extraction. This eliminates manual `config.get(...)` boilerplate.

```rust
use flowcore_macros::NodeConfig;

#[derive(Debug, Clone, NodeConfig)]
pub struct FilterConfig {
    #[config(default = "0.0")]
    pub threshold: f64,

    #[config(default = "\"gt\"".to_string())]
    pub operator: String,

    #[config(rename = "field_name")]
    pub field: Option<String>,

    #[config(skip)]
    pub runtime_state: String,
}
```

### Config Attribute Options

| Attribute | Description |
|-----------|-------------|
| `#[config(default = "...")]` | Provides a default value if config key is missing |
| `#[config(rename = "alt_name")]` | Reads from a different config key than the field name |
| `#[config(skip)]` | Ignores this field — uses `Default::default()` |

### Full Example with NodeConfig

```rust
use flowcore::{Node, NodeContext, NodeOutput, NodeError, Value};
use flowcore_macros::NodeConfig;
use std::collections::HashMap;

#[derive(Debug, Clone, NodeConfig)]
pub struct FilterConfig {
    #[config(default = "0.0")]
    pub threshold: f64,

    #[config(default = "\"gt\"".to_string())]
    pub operator: String,
}

pub struct FilterNode {
    config: FilterConfig,
}

impl FilterNode {
    pub fn from_config(config: &HashMap<String, Value>) -> Result<Self, NodeError> {
        let cfg = FilterConfig::try_from(config)
            .map_err(|e| NodeError::Configuration(e))?;
        Ok(Self { config: cfg })
    }
}

#[async_trait]
impl Node for FilterNode {
    fn node_type(&self) -> &str {
        "data.filter"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let value = ctx.require_input("value")?
            .as_f64()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "value".to_string(),
                expected: "number".to_string(),
                actual: "other".to_string(),
            })?;

        let passes = match self.config.operator.as_str() {
            "gt" => value > self.config.threshold,
            "lt" => value < self.config.threshold,
            "eq" => (value - self.config.threshold).abs() < f64::EPSILON,
            _ => return Err(NodeError::Configuration(
                format!("Unknown operator: {}", self.config.operator)
            )),
        };

        Ok(NodeOutput::new()
            .with_output("passes", passes)
            .with_output("value", value))
    }
}
```

### Config in Workflow

With `NodeConfig`, config values use the inferred format (no type tags):

```json
{
  "node_type": "data.filter",
  "config": {
    "threshold": 10.0,
    "operator": "gt"
  }
}
```

## Stateful Node

A node that counts executions using shared mutable state:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CounterNode {
    count: Arc<AtomicU64>,
}

impl CounterNode {
    pub fn new() -> Self {
        Self { count: Arc::new(AtomicU64::new(0)) }
    }
}

#[async_trait]
impl Node for CounterNode {
    fn node_type(&self) -> &str {
        "util.counter"
    }

    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let count = self.count.fetch_add(1, Ordering::SeqCst);

        ctx.events.info(format!("Execution count: {}", count + 1));

        Ok(NodeOutput::new()
            .with_output("count", (count + 1) as f64))
    }
}
```

For state shared within a single workflow execution (not across runs), use `ctx.state`:

```rust
// Write state
let mut state = ctx.state.write().await;
state.data.insert("count".to_string(), Value::Number(42.0));

// Read state later
let state = ctx.state.read().await;
let count = state.data.get("count").and_then(|v| v.as_f64()).unwrap_or(0.0);
```

## Emitting Events

Use `ctx.events` to emit real-time updates:

```rust
ctx.events.info("Starting data processing...".to_string());
ctx.events.warn("Unexpected format detected".to_string());
ctx.events.progress(50.0, "Halfway done".to_string());
ctx.events.data("intermediate", Value::Number(42.0));
ctx.events.stdout_line("output line".to_string());
ctx.events.stderr_line("error line".to_string());
```

See [events.md](../using/events.md) for the full event system documentation.

## Arc-Backed Values and Performance

When a node's output connects to many downstream nodes, clone overhead matters. Use `.to_arc()` on large outputs:

```rust
// Expensive: clones the entire JSON value per downstream connection
let json_value = Value::Json(serde_json::json!({"data": [1, 2, 3, /* ... */]}));
output.with_output("result", json_value);

// Efficient: clones an Arc (atomic reference count increment)
let json_value = Value::Json(serde_json::json!({"data": [1, 2, 3, /* ... */]}));
output.with_output("result", json_value.to_arc());
```

### Arc Variants

| Method | Source | Result |
|--------|--------|--------|
| `value.to_arc()` | `Value::String(s)` | `Value::StringArc(Arc<String>)` |
| `value.to_arc()` | `Value::Bytes(b)` | `Value::BytesArc(Arc<Vec<u8>>)` |
| `value.to_arc()` | `Value::Json(j)` | `Value::JsonArc(Arc<serde_json::Value>)` |
| `value.to_arc()` | Other variants | No-op (not applicable) |

The executor calls `.to_arc()` on node outputs before fanning out — downstream connections get cheap Arc clones instead of full data copies.

## Error Handling

The `NodeError` enum provides structured error reporting:

```rust
// Missing required input
return Err(NodeError::MissingInput("text".to_string()));

// Invalid input type
return Err(NodeError::InvalidInputType {
    field: "value".to_string(),
    expected: "number".to_string(),
    actual: "string".to_string(),
});

// Configuration error
return Err(NodeError::Configuration("threshold is required".to_string()));

// Execution failure
return Err(NodeError::ExecutionFailed("External service timeout".to_string()));

// Initialization failure
return Err(NodeError::InitializationFailed("DB connection refused".to_string()));
```

## Testing Nodes

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_uppercase_node() {
        let node = UppercaseNode;
        let mut ctx = NodeContext::new(
            Uuid::new_v4(),
            EventEmitter::new_test(),  // Use test emitter
        );
        ctx.inputs.insert("text".to_string(), Value::String("hello".to_string()));

        let output = node.execute(ctx).await.unwrap();
        let result = output.outputs.get("result").unwrap();

        assert_eq!(result.as_str().unwrap(), "HELLO");
    }

    #[tokio::test]
    async fn test_filter_passes() {
        let config = {
            let mut m = HashMap::new();
            m.insert("threshold".to_string(), Value::Number(10.0));
            m.insert("operator".to_string(), Value::String("gt".to_string()));
            m
        };
        let node = FilterNode::from_config(&config).unwrap();
        let mut ctx = NodeContext::new(Uuid::new_v4(), EventEmitter::new_test());
        ctx.inputs.insert("value".to_string(), Value::Number(15.0));

        let output = node.execute(ctx).await.unwrap();
        let passes = output.outputs.get("passes").unwrap();

        assert!(passes.as_bool().unwrap());
    }
}
```

## Plugin Nodes

Nodes can be distributed as dynamically-loaded shared libraries (`.so` / `.dylib`) with `NodePlugin` trait. See [plugin-development.md](plugin-development.md) for complete details.

### Quick Plugin Skeleton

```rust
// Exported from cdylib
#[no_mangle]
pub extern "C" fn plugin_create() -> Box<dyn NodePlugin> {
    Box::new(MyPlugin::new())
}

#[no_mangle]
pub extern "C" fn plugin_version() -> u32 {
    1
}
```

## Real-World Examples

See the built-in node library for production examples:

| Node | Source | Highlights |
|------|--------|------------|
| HTTP Request | `flownodes/src/http.rs` | Config extraction, error mapping, duration tracking |
| Shell Exec | `flownodes/src/shell.rs` | Stdout/stderr capture, timeout, environment setup |
| API Call | `flownodes/src/api_call.rs` | Python sandbox, pip packages, session reuse |
| Browser Render | `flownodes/src/browser.rs` | Screenshot capture, CSS selector waiting |
| Docker Run | `flownodes/src/docker.rs` | Container lifecycle, volume mounts |
| Branch | `flownodes/src/flow/branch.rs` | Conditional routing, boolean evaluation |

The engine ships with **14 built-in node types** — see [nodes/index.md](../using/nodes/index.md) for the full list.
