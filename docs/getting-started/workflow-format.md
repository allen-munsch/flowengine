# Workflow Format Reference

## Top-Level Structure

```json
{
  "id": "aaaaaaaa-bbbb-4ccc-8ddd-111111111111",
  "name": "My Workflow",
  "description": "Optional description",
  "nodes": [...],
  "connections": [...],
  "triggers": [...],
  "settings": {...}
}
```

| Field | Type | Required | Description |
|--------|------|----------|-------------|
| `id` | UUID string | Yes | Unique workflow identifier |
| `name` | String | Yes | Human-readable name |
| `description` | String or null | No | Optional description |
| `nodes` | Array of NodeSpec | Yes | Node definitions |
| `connections` | Array of Connection | Yes | Edge definitions |
| `triggers` | Array of TriggerSpec | No | Auto-trigger definitions |
| `settings` | WorkflowSettings | No | Global execution settings (defaults apply) |

## NodeSpec

```json
{
  "id": "aaaaaaaa-bbbb-4ccc-8ddd-111111111111",
  "node_type": "http.request",
  "name": "Fetch Data",
  "config": { "method": "GET" },
  "position": { "x": 100.0, "y": 200.0 },
  "retry_policy": { "max_attempts": 3, "delay_ms": 1000 }
}
```

| Field | Type | Required | Description |
|--------|------|----------|-------------|
| `id` | UUID string | Yes | Unique node identifier |
| `node_type` | String | Yes | Node type key (e.g. `http.request`, `debug.log`) |
| `name` | String or null | No | Human-readable label |
| `config` | Map<String, Value> | No | Static configuration keyed by parameter name |
| `position` | Position or null | No | Visual editor coordinates |
| `retry_policy` | RetryPolicy or null | No | Per-node retry behavior |

## Connection

```json
{
  "from_node": "aaaaaaaa-bbbb-...",
  "from_port": "body",
  "to_node": "cccccccc-dddd-...",
  "to_port": "message"
}
```

| Field | Type | Description |
|--------|------|-------------|
| `from_node` | UUID string | Source node ID |
| `from_port` | String | Source output port name |
| `to_node` | UUID string | Target node ID |
| `to_port` | String | Target input port name |

## RetryPolicy

```json
{
  "max_attempts": 3,
  "delay_ms": 1000,
  "backoff_multiplier": 2.0,
  "max_delay_ms": 60000,
  "retry_on_timeout": true
}
```

| Field | Type | Default | Description |
|--------|------|---------|-------------|
| `max_attempts` | u32 | 3 | Maximum execution attempts |
| `delay_ms` | u64 | 1000 | Initial delay in milliseconds |
| `backoff_multiplier` | f64 | 2.0 | Multiplier applied to each retry |
| `max_delay_ms` | u64 or null | 60000 | Delay cap in milliseconds |
| `retry_on_timeout` | bool | true | Whether to retry on timeout |

## Position

```json
{ "x": 100.0, "y": 200.0 }
```

| Field | Type | Description |
|--------|------|-------------|
| `x` | f32 | Horizontal coordinate |
| `y` | f32 | Vertical coordinate |

## WorkflowSettings

```json
{
  "max_execution_time_ms": 300000,
  "max_parallel_nodes": 10,
  "on_error": "StopWorkflow"
}
```

| Field | Type | Default | Description |
|--------|------|---------|-------------|
| `max_execution_time_ms` | u64 or null | null (no limit) | Global execution timeout |
| `max_parallel_nodes` | usize | 10 | Max concurrent node executions |
| `on_error` | ErrorHandling | `"StopWorkflow"` | Error behavior |

### ErrorHandling variants

- `"StopWorkflow"` — halt all execution on first error
- `"ContinueOnError"` — skip errors and continue
- `{"RetryWorkflow": {"max_attempts": 3}}` — retry the entire workflow

## TriggerSpec

```json
{
  "id": "aaaaaaaa-bbbb-...",
  "trigger_type": { "type": "Cron", "expression": "0 */6 * * *" },
  "enabled": true
}
```

| Field | Type | Description |
|--------|------|-------------|
| `id` | UUID string | Unique trigger identifier |
| `trigger_type` | TriggerType | Trigger kind and configuration |
| `enabled` | bool | Whether the trigger is active |

### TriggerType variants

- `"Manual"` — user-invoked only
- `{"Cron": {"expression": "0 */6 * * *"}}` — cron schedule
- `{"Webhook": {"path": "/my-hook"}}` — HTTP webhook endpoint
- `{"Event": {"event_type": "user.created"}}` — internal event bus trigger

## Value Type System

Values are the universal data type flowing through node ports. Every config value and runtime input/output is a `Value`.

### Primitive variants

- `Null` — explicit null
- `Bool(bool)` — `true` or `false`
- `Number(f64)` — floating-point number
- `String(String)` — owned string

### Composite variants

- `Bytes(Vec<u8>)` — raw byte buffer
- `Json(serde_json::Value)` — arbitrary JSON value (object, array, etc.)
- `Array(Vec<Value>)` — ordered list of values
- `Object(HashMap<String, Value>)` — string-keyed map of values

### Arc-backed variants for zero-copy sharing

- `StringArc(Arc<String>)` — shared string (cheap clone via atomic reference count)
- `BytesArc(Arc<Vec<u8>>)` — shared byte buffer
- `JsonArc(Arc<serde_json::Value>)` — shared JSON value

The Arc variants enable efficient fan-out: when a node's output connects to multiple downstream nodes, the executor calls `.to_arc()` on the output once, then each connection receives an Arc clone (atomic increment, no data copy).

## Deserialization: Tagged vs Inferred

The `Value` type supports two deserialization strategies:

**Tagged format** (legacy, still accepted):
```json
{ "type": "String", "value": "GET" }
```

**Inferred format** (preferred):
```json
"GET"
```

The deserializer always tries the tagged format first to preserve backward compatibility. If that fails, it falls back to type inference from the JSON shape. New workflows should use the inferred format exclusively.

## YAML vs JSON

Workflows can be written in either format. The parser auto-detects:
- Files with `.yaml` or `.yml` extension → YAML parser
- Everything else → JSON parser
- When reading from stdin (`--file -`), the first non-whitespace character decides: `{` = JSON, anything else = YAML

### Tradeoffs

**JSON**: Machine-friendly, strict syntax, no comments. Good for programmatic generation.
**YAML**: Human-friendly, supports comments, less quoting. Good for hand-written workflows.

### Same workflow in both formats

#### JSON

```json
{
  "id": "abcdef01-1234-4abc-8def-0123456789ab",
  "name": "GitHub Zen",
  "nodes": [
    {
      "id": "11111111-1111-4111-8111-111111111111",
      "node_type": "http.request",
      "name": "Fetch Zen",
      "config": { "method": "GET" }
    },
    {
      "id": "22222222-2222-4222-8222-222222222222",
      "node_type": "debug.log",
      "name": "Print Zen"
    }
  ],
  "connections": [
    {
      "from_node": "11111111-1111-4111-8111-111111111111",
      "from_port": "body",
      "to_node": "22222222-2222-4222-8222-222222222222",
      "to_port": "message"
    }
  ]
}
```

#### YAML

```yaml
id: "abcdef01-1234-4abc-8def-0123456789ab"
name: "GitHub Zen"
nodes:
  - id: "11111111-1111-4111-8111-111111111111"
    node_type: http.request
    name: "Fetch Zen"
    config:
      method: GET
  - id: "22222222-2222-4222-8222-222222222222"
    node_type: debug.log
    name: "Print Zen"
connections:
  - from_node: "11111111-1111-4111-8111-111111111111"
    from_port: body
    to_node: "22222222-2222-4222-8222-222222222222"
    to_port: message
```
