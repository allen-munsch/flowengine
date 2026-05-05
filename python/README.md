# FlowEngine Python SDK

Fast, lightweight workflow orchestration. Like Prefect but Rust-fast and Firecracker-sandboxed.

## Install

```bash
pip install -e python/
```

## Quick Start

### Define and run a workflow

```python
from flowengine import Flow, task
import requests

@task(retry=3, timeout=30)
def fetch_data(url: str) -> dict:
    return requests.get(url).json()

@task()
def process(data: dict) -> str:
    return f"Got: {data}"

flow = Flow("my-first-flow")
flow >> fetch_data >> process
result = flow.run(url="https://api.github.com/zen")
```

### Sandboxed execution (Zypi/Firecracker)

```python
from flowengine import Sandbox

sandbox = Sandbox(image="ubuntu:24.04")
exit_code, stdout, stderr = sandbox.exec(
    ["python3", "-c", "print('hello from firecracker VM!')"]
)
```

### Without a server — use the CLI

```python
from flowengine import Flow, task

@task()
def hello(name: str) -> str:
    return f"Hello, {name}!"

flow = Flow("hello-flow")
flow >> hello
flow.save("hello_workflow.json")
```

Then run:
```bash
flow run --file hello_workflow.json --input '{"name": "World"}'
```

## Why FlowEngine over Prefect?

| | FlowEngine | Prefect |
|---|---|---|
| **Runtime** | Rust (fast, no GIL) | Python (GIL-bound) |
| **Sandbox** | Firecracker microVMs (sub-second boot) | Docker only |
| **Latency** | <10ms per node | 100-500ms per task |
| **Deployment** | Single binary | Heavy Python env |
| **Streaming** | Native WebSocket events | Polling-based |
| **Python API** | Yes (this SDK) | Yes (native) |
| **Retry** | Exponential backoff | Exponential backoff |
| **Caching** | Content-fingerprint | Result persistence |
