# Flow Engine Server Quick Start

## Starting the Server

```bash
# Build and run
cargo run --bin flowserver

# Or use the release build
cargo build --release
./target/release/flowserver
```

Server starts on `http://0.0.0.0:3000`

## Quick Test

### 1. Check Health
```bash
curl http://localhost:3000/health
```

Expected response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "service": "flowengine"
}
```

### 2. List Available Node Types
```bash
curl http://localhost:3000/api/nodes
```

### 3. Create a Workflow

```bash
curl -X POST http://localhost:3000/api/workflows \
  -H "Content-Type: application/json" \
  -d @examples/github_zen.json
```

Save the returned workflow ID.

### 4. Execute the Workflow

```bash
# Replace {workflow_id} with the ID from step 3
curl -X POST http://localhost:3000/api/workflows/{workflow_id}/execute \
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

## Watch Real-Time Events

### Using websocat (WebSocket CLI tool)

```bash
# Install websocat
cargo install websocat

# Connect and watch events
websocat ws://localhost:3000/api/events
```

### Using JavaScript in Browser Console

```javascript
const ws = new WebSocket('ws://localhost:3000/api/events');
ws.onmessage = (e) => console.log(JSON.parse(e.data));
```

## Full Example Script

Save this as `test-server.sh`:

```bash
#!/bin/bash

BASE_URL="http://localhost:3000"

echo "🏥 Health check..."
curl -s $BASE_URL/health | jq

echo -e "\n📦 Available nodes..."
curl -s $BASE_URL/api/nodes | jq

echo -e "\n📝 Creating workflow..."
WORKFLOW_ID=$(curl -s -X POST $BASE_URL/api/workflows \
  -H "Content-Type: application/json" \
  -d @examples/github_zen.json | jq -r '.id')

echo "Created workflow: $WORKFLOW_ID"

echo -e "\n▶️  Executing workflow..."
curl -s -X POST $BASE_URL/api/workflows/$WORKFLOW_ID/execute \
  -H "Content-Type: application/json" \
  -d '{
    "inputs": {
      "url": {
        "type": "String",
        "value": "https://api.github.com/zen"
      }
    }
  }' | jq

echo -e "\n📋 Listing workflows..."
curl -s $BASE_URL/api/workflows | jq
```

Make it executable:
```bash
chmod +x test-server.sh
./test-server.sh
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/api/workflows` | List all workflows |
| POST | `/api/workflows` | Create workflow |
| GET | `/api/workflows/{id}` | Get workflow |
| DELETE | `/api/workflows/{id}` | Delete workflow |
| POST | `/api/workflows/{id}/execute` | Execute workflow |
| GET | `/api/nodes` | List node types |
| WS | `/api/events` | Real-time events |

See [docs/api.md](docs/api.md) for complete API documentation.

## Configuration

Set environment variables:

```bash
# Bind to different address
BIND_ADDRESS=127.0.0.1:8080 flowserver

# Enable debug logging
RUST_LOG=debug flowserver
```

## Docker

```bash
# Build image
docker build -t flowengine .

# Run container
docker run -p 3000:3000 flowengine
```

## Development

Watch and rebuild on changes:

```bash
cargo watch -x 'run --bin flowserver'
```

## Troubleshooting

### Port already in use
```bash
# Find process using port 3000
lsof -i :3000

# Kill it
kill -9 <PID>
```

### CORS errors
CORS is enabled by default for all origins. Check browser console for details.

### WebSocket connection fails
Ensure you're using `ws://` (not `wss://`) for local development.

## Production Deployment

```bash
# Build optimized binary
cargo build --release --bin flowserver

# Run with systemd
sudo cp target/release/flowserver /usr/local/bin/
sudo systemctl start flowengine
```

See [NEXT_STEPS.md](NEXT_STEPS.md) for deployment examples.
