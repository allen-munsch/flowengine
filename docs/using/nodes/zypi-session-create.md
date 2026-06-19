# Node Type: `zypi.session_create`

## Category

zypi

## Description

Creates a long-lived Zypi Firecracker microVM session. The session ID can be passed to `zypi.exec`, `api.call`, or `browser.render` nodes to avoid microVM cold-start overhead across multiple executions.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | String | `"http://localhost:4000"` | Zypi server URL |
| `image` | String | `"ubuntu:24.04"` | Container image to boot |
| `memory_mb` | Number or null | null | Memory limit in MB |
| `vcpus` | Number or null | null | vCPU count |
| `timeout_seconds` | Number | `300` | Auto-terminate idle session after this many seconds |
| `env` | Object | `{}` | Session-wide environment variables |

## Input Ports

None.

## Output Ports

| Port | Description |
|------|-------------|
| `session_id` | Session ID string for reuse by other nodes |
| `exit_code` | Session creation exit code (Number, 0 = success) |
| `stdout` | Raw stdout from session creation |
| `stderr` | Raw stderr from session creation |
| `container_id` | Internal container ID (for debugging) |
| `ip` | MicroVM IP address |
| `image` | Image used for the session |
| `duration_ms` | Session creation time in milliseconds |

## JSON Example

```json
{
  "id": "55555555-5555-4555-8555-555555555555",
  "node_type": "zypi.session_create",
  "name": "Create Python Session",
  "config": {
    "image": "python:3.11",
    "memory_mb": 512,
    "timeout_seconds": 600
  }
}
```

## YAML Example

```yaml
id: "55555555-5555-4555-8555-555555555555"
node_type: zypi.session_create
name: "Create Python Session"
config:
  image: "python:3.11"
  memory_mb: 512
  timeout_seconds: 600
```

## Notes

- Sessions auto-terminate after `timeout_seconds` of inactivity.
- Connect to `zypi.exec` or `api.call` via the `session_id` input port (or config).
- Recommended pattern: create session → run multiple commands → let session expire.