# Node Type: `docker.run` (v2)

## Category

docker

## Description

Enhanced Docker container execution with IOMode control for flexible Value serialization. Supports the same config as v1 plus `io_mode`, `output_mode`, and an `output` port with parsed results. Both v1 and v2 use the `"docker.run"` type string — the last one registered wins.

## Config

Same as v1 (`docker-exec.md`), plus:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `io_mode` | String | `"auto"` | Serialization mode: `"auto"` (smart), `"flat"` (extract values), `"wrapped"` (full Value enum) |
| `output_mode` | String | `"auto"` | Output parsing: `"auto"` (try JSON), `"json"` (force), `"text"` (raw) |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `data` | No | Data sent to container stdin (mode depends on `stdin_mode` config) |

## Output Ports

| Port | Description |
|------|-------------|
| `output` | Parsed container output (based on `output_mode`) |
| `stdout` | Raw stdout from container |
| `stderr` | Raw stderr from container |
| `exit_code` | Container exit code (Number, 0 = success) |
| `success` | Boolean success indicator |

## JSON Example

```json
{
  "id": "33333333-3333-4333-8333-333333333333",
  "node_type": "docker.run",
  "name": "Isolated Test",
  "config": {
    "image": "python:3.11-slim",
    "command": ["python", "-c", "print('hello')"],
    "io_mode": "flat",
    "output_mode": "json",
    "remove": true,
    "timeout": 30
  }
}
```

## YAML Example

```yaml
id: "33333333-3333-4333-8333-333333333333"
node_type: docker.run
name: "Isolated Test"
config:
  image: "python:3.11-slim"
  command:
    - python
    - -c
    - "print('hello')"
  io_mode: flat
  output_mode: json
  remove: true
  timeout: 30
```

## Notes

- Same type string as v1 — only one can be active in a registry at a time.
- `io_mode` controls how Value types are serialized to JSON before piping to the container:
  - `auto` / `flat`: Extracts actual values (e.g. `{"value": 21}` not `{"type":"Number","value":21}`).
  - `wrapped`: Full Value enum structure (backward-compatible).
- The `output` port returns stdout parsed via `output_mode` (auto tries JSON, falls back to string).