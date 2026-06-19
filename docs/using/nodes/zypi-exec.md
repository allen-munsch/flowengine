# Node Type: `zypi.exec`

## Category

zypi

## Description

Executes a command in a Zypi Firecracker microVM with sub-second boot time. Supports file injection, input passthrough as env vars, session reuse, and flexible command format.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | String | `"http://localhost:4000"` | Zypi server URL |
| `image` | String | `"ubuntu:24.04"` | Container image for the microVM |
| `command` | String or Array of String | (required) | Command to execute; string form is whitespace-split |
| `env` | Object | `{}` | Environment variables for the command |
| `workdir` | String or null | null | Working directory inside the microVM |
| `timeout` | Number or null | null | Execution timeout in seconds (default 300 at API level) |
| `memory_mb` | Number or null | null | Memory limit in MB |
| `vcpus` | Number or null | null | vCPU count |
| `session_id` | String or null | null | Reuse an existing session (from config or input port) |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `session_id` | No | Reuse a session from `zypi.session_create` |
| `files` | No | Files to inject (Object of path → content) |
| `file:<path>` | No | Individual file injection (e.g. `file:/app/script.py`); content auto base64-encoded for Bytes |
| `*` | No | Any other input port — its value becomes an uppercased env var |

## Output Ports

| Port | Description |
|------|-------------|
| `output` | Command stdout (JSON-parsed if possible, else String) |
| `stdout` | Raw stdout |
| `stderr` | Raw stderr |
| `exit_code` | Exit code (Number, 0 = success) |
| `success` | Boolean success indicator |
| `duration_ms` | Execution duration in milliseconds |
| `session_id` | Session ID for reuse |

## JSON Example

```json
{
  "id": "44444444-4444-4444-8444-444444444444",
  "node_type": "zypi.exec",
  "name": "Run Script",
  "config": {
    "image": "python:3.11",
    "command": ["python", "/app/script.py"],
    "timeout": 10
  }
}
```

## YAML Example

```yaml
id: "44444444-4444-4444-8444-444444444444"
node_type: zypi.exec
name: "Run Script"
config:
  image: "python:3.11"
  command:
    - python
    - /app/script.py
  timeout: 10
```

## Notes

- Boots a Firecracker microVM per execution (sub-second via CoW snapshots).
- Session reuse via `session_id` avoids boot overhead for repeated calls.
- File injection happens before command execution — use `file:<path>` ports or `files` Object input.
- All non-file input ports are converted to uppercased env vars (e.g. `payload` → `PAYLOAD`).
- Config `session_id` takes priority over input port `session_id`.