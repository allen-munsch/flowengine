# Python SDK

## Installation

```bash
cd python && pip install -e .
```

## `@task` Decorator

Wraps a Python function as a flowengine task. The function's signature determines the node's input ports.

```python
from flowengine import task

@task(
    name="my-task",           # Display name (default: function name)
    retry=3,                  # Retry count (default: 0)
    timeout=30,               # Timeout in seconds (default: 30)
    node_type="api.call",     # Underlying node type (default: "api.call")
    image="python:3.11",      # Zypi image (default: "python:3.11-slim")
    **config                  # Additional node config
)
def greet(name: str, greeting: str = "Hello") -> str:
    return f"{greeting}, {name}!"
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | str | Function name | Human-readable task name |
| `retry` | int | `0` | Number of retry attempts on failure |
| `timeout` | int | `30` | Execution timeout in seconds |
| `node_type` | str | `"api.call"` | Underlying node type to execute |
| `image` | str | `"python:3.11-slim"` | Docker/Zypi image for execution |
| `**config` | Any | — | Extra config forwarded to the node |

### Function Signature → Ports

- Each parameter becomes an input port with the same name.
- Parameters with defaults become optional ports.
- Return annotation determines output type.

## `Task` Chaining with `>>`

Tasks can be connected with the `>>` operator to build pipelines:

```python
@task()
def fetch_data(url: str) -> dict:
    import requests
    return requests.get(url).json()

@task()
def process_data(data: dict) -> list:
    return [item["name"] for item in data]

pipeline = fetch_data >> process_data
flow = Flow("pipeline")
flow >> pipeline
result = flow.run(url="https://jsonplaceholder.typicode.com/users")
```

## `Flow` Class

Top-level workflow container.

```python
flow = Flow(name="my-flow")
flow >> task1 >> task2  # Add tasks to the flow
result = flow.run(**inputs)
```

### Methods

| Method | Description |
|--------|-------------|
| `run(**inputs)` | Execute synchronously (local execution) |
| `run_async(**inputs)` | Execute via FlowClient HTTP (requires server) |
| `to_dict()` | Serialize to dict |
| `to_json(path=None)` | Serialize to JSON string (write to file if path given) |
| `save(path)` | Save workflow JSON to file |
| `__rshift__(other)` | Add a task or pipeline to the flow |

## `FlowBuilder` Class

Lower-level builder for constructing workflows programmatically.

```python
from flowengine import FlowBuilder, task

@task()
def add_one(x: int) -> int:
    return x + 1

@task()
def double(x: int) -> int:
    return x * 2

builder = FlowBuilder("math-pipeline")
t1 = builder.add(add_one)
t2 = builder.add(double)
builder.connect(t1.id, "result", t2.id, "x")

# Execute locally via flow CLI
result = await builder.build_async(x=5)
```

### Methods

| Method | Description |
|--------|-------------|
| `add(task)` | Register a task, returns the created node |
| `connect(from_node, from_port, to_node, to_port)` | Add a connection |
| `build(**inputs)` | Execute locally (synchronous, via flow CLI) |
| `build_async(**inputs)` | Execute locally (async subprocess) |
| `to_flow()` | Convert to a `Flow` object |

### `build_async()` — Local Execution Without Server

Calls the `flow` CLI binary as a subprocess with `--output json` to get structured events. Set the `FLOW_BIN` environment variable to point to the flow binary:

```bash
export FLOW_BIN=./target/release/flow
```

## `FlowClient` Class

Remote client for executing workflows via the flowserver HTTP API.

```python
from flowengine import FlowClient

client = FlowClient(base_url="http://localhost:8080")

# Synchronous HTTP call
result = client.execute(workflow, inputs={"key": "value"})

# Async HTTP call
result = await client.execute_async(workflow, inputs={"key": "value"})
```

### Constructor Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `base_url` | str | `"http://localhost:8080"` | Flowserver base URL |
| `timeout` | int | `60` | Request timeout in seconds |

### Methods

| Method | Description |
|--------|-------------|
| `execute(workflow, inputs)` | Synchronous workflow execution |
| `execute_async(workflow, inputs)` | Async workflow execution |

## `Sandbox` Class

Zypi sandbox integration for running Python code in Firecracker microVMs.

```python
from flowengine import Sandbox

sandbox = Sandbox(image="python:3.11", memory_mb=512)
result = sandbox.run("print('hello from microVM')")
```

## Full Example

```python
from flowengine import task, Flow, FlowBuilder

@task(retry=3, timeout=30)
def fetch_user(url: str) -> dict:
    import requests
    r = requests.get(url)
    r.raise_for_status()
    return r.json()

@task()
def extract_name(data: dict) -> str:
    return data["name"]

@task()
def log_result(name: str) -> str:
    print(f"User: {name}")
    return name

# Using Flow
flow = Flow("user-pipeline")
flow >> fetch_user >> extract_name >> log_result

result = flow.run(
    url="https://jsonplaceholder.typicode.com/users/1"
)
print(result)

# Save for later use
flow.to_json("user_pipeline.json")
```

## See Also

- [Getting Started](../getting-started/getting-started.md) — first steps with the engine
- [Node Reference](nodes/index.md) — all built-in node types
