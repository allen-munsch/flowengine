# FlowEngine Python SDK

Fast, lightweight workflow orchestration. Like Prefect but Rust-fast and Firecracker-sandboxed.

## Install

```bash
pip install -e python/
```

## Quick Example

```python
from flowengine import task, Flow

@task(retry=3, timeout=30)
def fetch_data(url: str) -> dict:
    import requests
    return requests.get(url).json()

flow = Flow("my-flow")
flow >> fetch_data
result = flow.run(url="https://api.github.com/zen")
```

## Full Documentation

See [docs/using/python-sdk.md](../docs/using/python-sdk.md) for the complete reference: `@task` decorator, `Flow`, `FlowBuilder`, `FlowClient`, `Sandbox`, and more.

## Why FlowEngine?

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
