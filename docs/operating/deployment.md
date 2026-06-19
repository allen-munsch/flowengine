# Deployment

How to deploy and operate Flow Engine workflows.

## Deployment Options

### CLI (Local Execution)

The simplest deployment mode. Run workflows directly from the command line:

```bash
flow run --file workflow.json
```

No server, no network. Calendars, webhooks, and events are not available in CLI mode. See [cli-reference.md](../getting-started/cli-reference.md) for all commands.

### Python SDK (Embedded)

Integrate workflow execution into a Python application:

```python
from flowengine import task, Flow, FlowBuilder

@task(retry=3, timeout=30)
def process(data: str) -> dict:
    import json
    return json.loads(data)

# Local execution via subprocess
builder = FlowBuilder("processor")
builder.add(process)
result = await builder.build_async(data='{"key": "value"}')
```

Set `FLOW_BIN` to the flow binary path. See [python-sdk.md](../using/python-sdk.md).

### Flow Server (HTTP + WebSocket)

The `flowserver` binary provides an HTTP API and WebSocket event stream. This enables remote workflow execution, shared state, and persistent workflows.

## Server Configuration

```bash
# Default: localhost:3000
flowserver --host 0.0.0.0 --port 8080
```

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `127.0.0.1` | Bind address |
| `--port` | `3000` | Listen port |
| `--plugin-dir` | `/etc/flow/plugins` | Plugin directory |

## REST API Endpoints

### `GET /health`

Health check.

```bash
curl http://localhost:3000/health
```

Response: `200 OK` with `{"status": "ok", "uptime_seconds": 3600}`.

### `POST /api/workflows/execute`

Execute a workflow from a file path or inline definition.

```bash
curl -X POST http://localhost:3000/api/workflows/execute \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_file": "examples/github_zen.json",
    "inputs": {
      "url": "https://api.github.com/zen"
    }
  }'
```

Request body:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workflow_file` | String | Yes* | Path to workflow JSON/YAML file |
| `workflow_definition` | Object | Yes* | Inline workflow definition |
| `inputs` | Object | No | Input values (inferred format — plain values, no type tags) |

*Exactly one of `workflow_file` or `workflow_definition` required.

Response:

```json
{
  "execution_id": "exec-abc123",
  "status": "completed",
  "duration_ms": 234,
  "completed_nodes": 2,
  "failed_nodes": 0,
  "outputs": {
    "node-2": {
      "message": "..."
    }
  }
}
```

### `GET /api/workflows/:id`

Get workflow definition by ID.

```bash
curl http://localhost:3000/api/workflows/wf-abc123
```

### `GET /api/workflows/:id/status`

Get execution status for a running workflow.

```bash
curl http://localhost:3000/api/workflows/wf-abc123/status
```

Response:

```json
{
  "workflow_id": "wf-abc123",
  "status": "running",
  "started_at": "2025-01-15T10:30:00Z",
  "completed_nodes": 5,
  "total_nodes": 10,
  "current_nodes": ["node-6", "node-7"]
}
```

Status values: `"running"`, `"completed"`, `"failed"`, `"not_found"`.

### `WS /api/events`

WebSocket endpoint for real-time execution event streaming.

Connect via WebSocket client:

```javascript
const ws = new WebSocket("ws://localhost:3000/api/events");
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  switch (data.type) {
    case "node_started":
      console.log(`Node ${data.node_id} started`);
      break;
    case "node_completed":
      console.log(`Node ${data.node_id} done in ${data.duration_ms}ms`);
      break;
    case "workflow_completed":
      console.log(`Workflow ${data.success ? "OK" : "FAILED"}`);
      ws.close();
      break;
  }
};
```

Events follow the same `ExecutionEvent` schema as `--output json` in the CLI. See [events.md](../using/events.md).

## Docker Deployment

```dockerfile
FROM rust:1.75 AS builder
WORKDIR /app
COPY crates/ crates/
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release -p flowserver

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/flowserver /usr/local/bin/flowserver
COPY --from=builder /app/target/release/flow /usr/local/bin/flow
COPY examples/ /etc/flow/examples/

EXPOSE 3000
CMD ["flowserver", "--host", "0.0.0.0", "--port", "3000"]
```

Build and run:

```bash
docker build -t flowengine:latest .
docker run -d -p 3000:3000 --name flowengine flowengine:latest
```

## Systemd Service

```
# /etc/systemd/system/flowserver.service
[Unit]
Description=Flow Engine Server
After=network.target docker.service

[Service]
Type=simple
User=flow
ExecStart=/usr/local/bin/flowserver --host 0.0.0.0 --port 3000
Restart=always
RestartSec=5
Environment=RUST_LOG=info
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now flowserver
sudo journalctl -u flowserver -f
```

## TLS Termination

The flowserver does not handle TLS directly. Run it behind a reverse proxy:

### Nginx

```nginx
server {
    listen 443 ssl;
    server_name flow.example.com;

    ssl_certificate /etc/letsencrypt/live/flow.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/flow.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### Caddy

```
flow.example.com {
    reverse_proxy localhost:3000
}
```

## Health Checks

### HTTP

```bash
curl -f http://localhost:3000/health || exit 1
```

### Docker

```yaml
# docker-compose.yml
services:
  flowserver:
    image: flowengine:latest
    ports:
      - "3000:3000"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 5s
      retries: 3
```

## Limits and Sizing

| Metric | Default | Tunable |
|--------|---------|---------|
| Max parallel nodes | 10 | `WorkflowSettings.max_parallel_nodes` |
| Event buffer size | 1000 | `RuntimeConfig.event_buffer_size` |
| Max execution time | no limit | `WorkflowSettings.max_execution_time_ms` |
| Plugin directory | `/etc/flow/plugins` | `--plugin-dir` flag |
| Node retry delay | 1000ms | `RetryPolicy.delay_ms` |

## See Also

- [cli-reference.md](../getting-started/cli-reference.md) — CLI commands and flags
- [events.md](../using/events.md) — event system and JSON output format
- [architecture.md](../developing/architecture.md) — execution model and crate structure
