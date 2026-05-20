# Flow Engine API Documentation

## Overview

The Flow Engine HTTP API provides REST endpoints for managing and executing workflows, plus WebSocket support for real-time event streaming.

**Base URL**: `http://localhost:3000`

---

## Endpoints

### Health Check

Check if the server is running.

```http
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "service": "flowengine"
}
```

---

### List Workflows

Get all registered workflows.

```http
GET /api/workflows
```

**Response:**
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "My Workflow",
    "description": "Example workflow",
    "nodes": 3,
    "connections": 2
  }
]
```

---

### Create Workflow

Register a new workflow.

```http
POST /api/workflows
Content-Type: application/json
```

**Request Body:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Workflow",
  "description": "Example workflow",
  "nodes": [
    {
      "id": "node-1",
      "node_type": "http.request",
      "config": {
        "method": {
          "type": "String",
          "value": "GET"
        }
      },
      "position": { "x": 100, "y": 100 }
    }
  ],
  "connections": [],
  "triggers": [],
  "settings": {
    "max_parallel_nodes": 10,
    "on_error": "StopWorkflow"
  }
}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Workflow created successfully"
}
```

---

### Get Workflow

Retrieve a specific workflow by ID.

```http
GET /api/workflows/{id}
```

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Workflow",
  "nodes": [...],
  "connections": [...],
  "triggers": [],
  "settings": {...}
}
```

**Error Response (404):**
```json
{
  "error": "Workflow 550e8400-e29b-41d4-a716-446655440000 not found"
}
```

---

### Delete Workflow

Delete a workflow by ID.

```http
DELETE /api/workflows/{id}
```

**Response:**
```json
{
  "message": "Workflow deleted successfully"
}
```

---

### Execute Workflow

Execute a workflow with input data.

```http
POST /api/workflows/{id}/execute
Content-Type: application/json
```

**Request Body:**
```json
{
  "inputs": {
    "url": {
      "type": "String",
      "value": "https://api.github.com/zen"
    }
  }
}
```

**Response:**
```json
{
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "completed_nodes": 2,
  "total_nodes": 2
}
```

**Error Response:**
```json
{
  "error": "Node execution failed: ..."
}
```

---

### List Node Types

Get all available node types.

```http
GET /api/nodes
```

**Response:**
```json
[
  {
    "type": "http.request",
    "description": "Make HTTP requests",
    "category": "http"
  },
  {
    "type": "debug.log",
    "description": "Log values for debugging",
    "category": "debug"
  }
]
```

---

### WebSocket Events

Subscribe to real-time workflow execution events.

```http
GET /api/events
Upgrade: websocket
```

**Event Stream (JSON messages):**

```json
{
  "type": "WorkflowStarted",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "workflow_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

```json
{
  "type": "NodeStarted",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "node_id": "node-1",
  "node_type": "http.request",
  "timestamp": "2024-01-15T10:30:00.005Z"
}
```

```json
{
  "type": "NodeCompleted",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "node_id": "node-1",
  "outputs": {
    "status": { "type": "Number", "value": 200 },
    "body": { "type": "String", "value": "..." }
  },
  "duration_ms": 234,
  "timestamp": "2024-01-15T10:30:00.239Z"
}
```

```json
{
  "type": "NodeEvent",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "node_id": "node-1",
  "event": {
    "event_type": "Info",
    "message": "GET https://api.github.com/zen"
  },
  "timestamp": "2024-01-15T10:30:00.010Z"
}
```

```json
{
  "type": "WorkflowCompleted",
  "execution_id": "123e4567-e89b-12d3-a456-426614174000",
  "success": true,
  "duration_ms": 243,
  "timestamp": "2024-01-15T10:30:00.243Z"
}
```

---

## Examples

### Using cURL

**Create a workflow:**
```bash
curl -X POST http://localhost:3000/api/workflows \
  -H "Content-Type: application/json" \
  -d @examples/github_zen.json
```

**Execute a workflow:**
```bash
curl -X POST http://localhost:3000/api/workflows/{id}/execute \
  -H "Content-Type: application/json" \
  -d '{
    "inputs": {
      "url": {
        "type": "String",
        "value": "https://api.github.com/zen"
      }
    }
  }'
```

**List workflows:**
```bash
curl http://localhost:3000/api/workflows
```

---

### Using JavaScript/Fetch

```javascript
// Create workflow
const workflow = await fetch('http://localhost:3000/api/workflows', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(workflowDefinition)
});

// Execute workflow
const execution = await fetch(`http://localhost:3000/api/workflows/${workflowId}/execute`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    inputs: {
      url: { type: 'String', value: 'https://api.example.com' }
    }
  })
});

const result = await execution.json();
console.log(`Execution ${result.execution_id} completed`);
```

---

### Using WebSocket (JavaScript)

```javascript
const ws = new WebSocket('ws://localhost:3000/api/events');

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  
  switch(data.type) {
    case 'WorkflowStarted':
      console.log('Workflow started:', data.workflow_id);
      break;
      
    case 'NodeStarted':
      console.log('Node started:', data.node_type);
      break;
      
    case 'NodeCompleted':
      console.log('Node completed in', data.duration_ms, 'ms');
      break;
      
    case 'NodeEvent':
      if (data.event.event_type === 'Info') {
        console.log('Info:', data.event.message);
      }
      break;
      
    case 'WorkflowCompleted':
      console.log('Workflow completed!', data.success ? '✓' : '✗');
      break;
  }
};

ws.onopen = () => console.log('Connected to event stream');
ws.onerror = (error) => console.error('WebSocket error:', error);
```

---

### Using Python

```python
import requests
import json

# Create workflow
with open('examples/github_zen.json') as f:
    workflow = json.load(f)

response = requests.post(
    'http://localhost:3000/api/workflows',
    json=workflow
)
workflow_id = response.json()['id']

# Execute workflow
execution = requests.post(
    f'http://localhost:3000/api/workflows/{workflow_id}/execute',
    json={
        'inputs': {
            'url': {
                'type': 'String',
                'value': 'https://api.github.com/zen'
            }
        }
    }
)

result = execution.json()
print(f"Execution {result['execution_id']} completed")
print(f"Nodes: {result['completed_nodes']}/{result['total_nodes']}")
```

---

## Configuration

### Environment Variables

- **`BIND_ADDRESS`** - Server bind address (default: `0.0.0.0:3000`)
  ```bash
  BIND_ADDRESS=127.0.0.1:8080 flowserver
  ```

- **`RUST_LOG`** - Logging level (default: `info`)
  ```bash
  RUST_LOG=debug flowserver
  ```

---

## Error Handling

All errors return appropriate HTTP status codes:

- **400 Bad Request** - Invalid input data
- **404 Not Found** - Workflow not found
- **500 Internal Server Error** - Execution failure

Error response format:
```json
{
  "error": "Detailed error message"
}
```

---

## CORS

The API supports CORS and accepts requests from any origin.

---

## Running the Server

```bash
# Development
cargo run --bin flowserver

# Production
cargo build --release
./target/release/flowserver
```

Server will start on `http://0.0.0.0:3000` by default.

---

## Testing

### Health Check
```bash
curl http://localhost:3000/health
```

### Full Workflow Test
```bash
# 1. Start server
cargo run --bin flowserver

# 2. In another terminal, create workflow
curl -X POST http://localhost:3000/api/workflows \
  -H "Content-Type: application/json" \
  -d @examples/github_zen.json

# 3. Execute it
WORKFLOW_ID="<id-from-step-2>"
curl -X POST http://localhost:3000/api/workflows/$WORKFLOW_ID/execute \
  -H "Content-Type: application/json" \
  -d '{
    "inputs": {
      "url": {
        "type": "String", 
        "value": "https://api.github.com/zen"
      }
    }
  }'
```

---

## Next Steps

- Add authentication (JWT tokens)
- Add rate limiting
- Add workflow versioning
- Add execution history persistence
- Add metrics endpoint (Prometheus)
