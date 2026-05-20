# Building Custom Nodes

This guide walks through creating custom nodes for the Flow Engine.

## Table of Contents
1. [Simple Stateless Node](#simple-stateless-node)
2. [Stateful Node with Configuration](#stateful-node)
3. [Node with External Dependencies](#external-dependencies)
4. [Emitting Events](#emitting-events)
5. [Error Handling](#error-handling)
6. [Testing Nodes](#testing)

## Simple Stateless Node

Let's create a node that transforms text to uppercase:

```rust
use async_trait::async_trait;
use flowcore::{Node, NodeContext, NodeError, NodeOutput, Value};
use flowruntime::{NodeFactory, NodeMetadata, PortDefinition};
use std::collections::HashMap;

/// Converts input text to uppercase
pub struct UppercaseNode;

#[async_trait]
impl Node for UppercaseNode {
    fn node_type(&self) -> &str {
        "text.uppercase"
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        // Get input
        let text = ctx.require_input("text")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "text".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        
        // Transform
        let uppercase = text.to_uppercase();
        
        // Return output
        Ok(NodeOutput::new()
            .with_output("result", uppercase))
    }
}

/// Factory for creating UppercaseNode instances
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
            description: "Converts text to uppercase".to_string(),
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
  "nodes": [
    {
      "id": "node-1",
      "node_type": "text.uppercase",
      "config": {}
    }
  ]
}
```

## Stateful Node

A node that counts how many times it's been executed:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CounterNode {
    count: Arc<AtomicU64>,
}

impl CounterNode {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicU64::new(0)),
        }
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

### Using Node State

For state that's shared within a single workflow execution:

```rust
async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
    // Read state
    let state = ctx.state.read().await;
    let previous_count = state.data.get("count")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    
    drop(state); // Release read lock
    
    // Write state
    let mut state = ctx.state.write().await;
    state.data.insert("count".to_string(), Value::Number(previous_count + 1.0));
    
    Ok(NodeOutput::new()
        .with_output("count", previous_count + 1.0))
}
```

## Node with Configuration

A node that has configurable behavior:

```rust
pub struct FilterNode {
    threshold: f64,
    operator: FilterOperator,
}

enum FilterOperator {
    GreaterThan,
    LessThan,
    Equals,
}

impl FilterNode {
    pub fn from_config(config: &HashMap<String, Value>) -> Result<Self, NodeError> {
        let threshold = config.get("threshold")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| NodeError::Configuration("Missing threshold".to_string()))?;
        
        let operator_str = config.get("operator")
            .and_then(|v| v.as_str())
            .ok_or_else(|| NodeError::Configuration("Missing operator".to_string()))?;
        
        let operator = match operator_str {
            "gt" => FilterOperator::GreaterThan,
            "lt" => FilterOperator::LessThan,
            "eq" => FilterOperator::Equals,
            _ => return Err(NodeError::Configuration(
                format!("Invalid operator: {}", operator_str)
            )),
        };
        
        Ok(Self { threshold, operator })
    }
}

#[async_trait]
impl Node for FilterNode {
    fn node_type(&self) -> &str {
        "data.filter"
    }
    
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<(), NodeError> {
        // Called at workflow load time
        if !config.contains_key("threshold") {
            return Err(NodeError::Configuration("threshold is required".to_string()));
        }
        Ok(())
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let value = ctx.require_input("value")?
            .as_f64()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "value".to_string(),
                expected: "number".to_string(),
                actual: "other".to_string(),
            })?;
        
        let passes = match self.operator {
            FilterOperator::GreaterThan => value > self.threshold,
            FilterOperator::LessThan => value < self.threshold,
            FilterOperator::Equals => (value - self.threshold).abs() < f64::EPSILON,
        };
        
        Ok(NodeOutput::new()
            .with_output("passes", passes)
            .with_output("value", value))
    }
}
```

### Factory with Configuration

```rust
impl NodeFactory for FilterNodeFactory {
    fn create(&self, config: &HashMap<String, Value>) -> Result<Box<dyn Node>, NodeError> {
        Ok(Box::new(FilterNode::from_config(config)?))
    }
    
    fn node_type(&self) -> &str {
        "data.filter"
    }
}
```

### Workflow Configuration

```json
{
  "nodes": [
    {
      "id": "filter-1",
      "node_type": "data.filter",
      "config": {
        "threshold": {
          "type": "Number",
          "value": 10.0
        },
        "operator": {
          "type": "String",
          "value": "gt"
        }
      }
    }
  ]
}
```

## External Dependencies

A node that makes database queries:

```rust
use sqlx::{PgPool, Row};

pub struct DatabaseQueryNode {
    pool: PgPool,
}

impl DatabaseQueryNode {
    pub async fn new(connection_string: &str) -> Result<Self, NodeError> {
        let pool = PgPool::connect(connection_string)
            .await
            .map_err(|e| NodeError::InitializationFailed(e.to_string()))?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl Node for DatabaseQueryNode {
    fn node_type(&self) -> &str {
        "database.query"
    }
    
    async fn initialize(&mut self) -> Result<(), NodeError> {
        // Test connection
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| NodeError::InitializationFailed(
                format!("Database connection failed: {}", e)
            ))?;
        
        Ok(())
    }
    
    async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
        let query = ctx.require_input("query")?
            .as_str()
            .ok_or_else(|| NodeError::InvalidInputType {
                field: "query".to_string(),
                expected: "string".to_string(),
                actual: "other".to_string(),
            })?;
        
        ctx.events.info(format!("Executing query: {}", query));
        
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| NodeError::ExecutionFailed(e.to_string()))?;
        
        // Convert rows to JSON
        let results: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                // Convert row to JSON (simplified)
                serde_json::json!({})
            })
            .collect();
        
        Ok(NodeOutput::new()
            .with_output("results", Value::Json(serde_json::Value::Array(results))))
    }
    
    async fn shutdown(&mut self) -> Result<(), NodeError> {
        self.pool.close().await;
        Ok(())
    }
}
```

## Emitting Events

Nodes can emit events for real-time monitoring:

```rust
async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
    ctx.events.info("Starting data processing");
    
    // Long-running operation
    for i in 0..100 {
        // Update progress
        ctx.events.progress(i as f64, Some(format!("Processing item {}", i)));
        
        process_item(i).await?;
        
        // Check for cancellation
        if ctx.cancellation.is_cancelled() {
            return Err(NodeError::Cancelled);
        }
    }
    
    // Emit intermediate data
    ctx.events.data("partial_result", Value::Number(42.0));
    
    ctx.events.info("Processing complete");
    
    Ok(NodeOutput::new())
}
```

### Event Types

```rust
// Information message
ctx.events.info("Processing started");

// Warning message
ctx.events.warn("Large dataset detected");

// Progress update (0-100)
ctx.events.progress(50.0, Some("Halfway done".to_string()));

// Stream data on a port
ctx.events.data("intermediate", value);
```

## Error Handling

### Validation Errors

Catch configuration issues early:

```rust
fn validate_config(&self, config: &HashMap<String, Value>) -> Result<(), NodeError> {
    // Required fields
    if !config.contains_key("url") {
        return Err(NodeError::Configuration("url is required".to_string()));
    }
    
    // Type checks
    let timeout = config.get("timeout")
        .and_then(|v| v.as_f64());
    
    if let Some(t) = timeout {
        if t < 0.0 {
            return Err(NodeError::Configuration(
                "timeout must be non-negative".to_string()
            ));
        }
    }
    
    Ok(())
}
```

### Execution Errors

Handle runtime failures gracefully:

```rust
async fn execute(&self, ctx: NodeContext) -> Result<NodeOutput, NodeError> {
    // Validate inputs
    let url = ctx.require_input("url")?
        .as_str()
        .ok_or_else(|| NodeError::InvalidInputType {
            field: "url".to_string(),
            expected: "string".to_string(),
            actual: "other".to_string(),
        })?;
    
    // External operation with error handling
    let response = match reqwest::get(url).await {
        Ok(resp) => resp,
        Err(e) => {
            ctx.events.warn(format!("Request failed: {}", e));
            return Err(NodeError::ExecutionFailed(e.to_string()));
        }
    };
    
    // Check response status
    if !response.status().is_success() {
        return Err(NodeError::ExecutionFailed(
            format!("HTTP error: {}", response.status())
        ));
    }
    
    Ok(NodeOutput::new())
}
```

### Retry Logic

The runtime handles retries automatically if configured:

```json
{
  "nodes": [
    {
      "id": "node-1",
      "node_type": "http.request",
      "retry_policy": {
        "max_attempts": 3,
        "delay_ms": 1000,
        "backoff_multiplier": 2.0
      }
    }
  ]
}
```

## Testing

### Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_uppercase_node() {
        let node = UppercaseNode;
        
        let (tx, _rx) = tokio::sync::broadcast::channel(10);
        let emitter = flowcore::EventEmitter::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            tx,
        );
        
        let mut ctx = NodeContext::new(uuid::Uuid::new_v4(), emitter);
        ctx.inputs.insert("text".to_string(), Value::String("hello".to_string()));
        
        let result = node.execute(ctx).await.unwrap();
        
        assert_eq!(
            result.outputs.get("result").unwrap().as_str().unwrap(),
            "HELLO"
        );
    }
}
```

### Integration Test

```rust
#[tokio::test]
async fn test_workflow_with_custom_node() {
    use flowruntime::FlowRuntime;
    
    let mut registry = flowruntime::NodeRegistry::new();
    registry.register(Arc::new(UppercaseNodeFactory));
    
    let runtime = FlowRuntime::with_registry(
        Arc::new(registry),
        flowruntime::RuntimeConfig::default(),
    );
    
    let mut workflow = Workflow::new("test");
    let node = NodeSpec::new("text.uppercase");
    workflow.add_node(node);
    
    let mut inputs = HashMap::new();
    inputs.insert("text".to_string(), Value::String("test".to_string()));
    
    let result = runtime.execute(&workflow, inputs).await.unwrap();
    assert_eq!(result.completed_nodes, 1);
}
```

## Best Practices

### 1. Make Nodes Focused

✅ Good: `text.uppercase`, `text.lowercase`, `text.trim`  
❌ Bad: `text.all_transformations`

### 2. Validate Early

Implement `validate_config()` to catch issues at workflow load time, not execution time.

### 3. Emit Events

Help users understand what's happening:
```rust
ctx.events.info("Connecting to database");
ctx.events.progress(50.0, Some("Halfway done".to_string()));
ctx.events.info("Query returned 100 rows");
```

### 4. Handle Cancellation

```rust
if ctx.cancellation.is_cancelled() {
    return Err(NodeError::Cancelled);
}
```

### 5. Use Typed Errors

```rust
// Good
return Err(NodeError::InvalidInputType {
    field: "count".to_string(),
    expected: "number".to_string(),
    actual: "string".to_string(),
});

// Less good
return Err(NodeError::ExecutionFailed("bad input".to_string()));
```

### 6. Document Ports

Provide clear metadata:
```rust
fn metadata(&self) -> NodeMetadata {
    NodeMetadata {
        description: "Filters numeric values based on a threshold".to_string(),
        category: "data".to_string(),
        inputs: vec![
            PortDefinition {
                name: "value".to_string(),
                description: "Numeric value to test".to_string(),
                required: true,
            }
        ],
        outputs: vec![
            PortDefinition {
                name: "passes".to_string(),
                description: "Boolean indicating if filter passed".to_string(),
                required: false,
            }
        ],
    }
}
```

## Examples of Complex Nodes

See the standard library for real examples:
- `flownodes/src/http.rs` - HTTP client with headers
- `flownodes/src/transform.rs` - JSON parsing with error handling
- `flownodes/src/time.rs` - Async delays

## Next Steps

1. Create your node implementation
2. Write unit tests
3. Add to a custom crate or fork `flownodes`
4. Register in your runtime
5. Test in a workflow
6. Consider contributing back to the project!
