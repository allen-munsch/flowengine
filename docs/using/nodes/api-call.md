# Node Type: `api.call`

## Category

api

## Description

Runs a Python script with optional pip package dependencies in an isolated Zypi sandbox. The "n8n marketplace" pattern — add integrations via workflow templates, not Rust code.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `script` | String | (required) | Python script source code |
| `packages` | Array of String or String | `[]` | pip packages to install before execution; string form is comma-separated |
| `env` | Object | `{}` | Environment variables (e.g. API keys) passed securely to the sandbox |
| `timeout` | Number | `60` | Execution timeout in seconds |
| `memory_mb` | Number | `256` | Memory limit in MB |
| `zypi_url` | String | `"http://localhost:4000"` | Zypi sandbox API URL |
| `image` | String | `"ubuntu:24.04"` | Sandbox container image |
| `name` | String | `"api.call"` | Human-readable name for logging |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `stdin` | No | Data piped to script's stdin; also available as uppercased env vars |

## Output Ports

| Port | Description |
|------|-------------|
| `output` | Script stdout (parsed as JSON if possible, otherwise String) |
| `stdout` | Raw stdout as String |
| `stderr` | Stderr as String |
| `duration_ms` | Execution duration in milliseconds |

## JSON Example

```json
{
  "id": "11111111-1111-4111-8111-111111111111",
  "node_type": "api.call",
  "name": "Analyze Data",
  "config": {
    "script": "import json, sys\ndata = json.loads(sys.stdin.read())\nprint(json.dumps({'count': len(data)}))",
    "packages": ["requests"],
    "timeout": 10
  }
}
```

## YAML Example

```yaml
id: "11111111-1111-4111-8111-111111111111"
node_type: api.call
name: "Analyze Data"
config:
  script: |
    import json, sys
    data = json.loads(sys.stdin.read())
    print(json.dumps({'count': len(data)}))
  packages:
    - requests
  timeout: 10
```

## Notes

- Each execution installs packages fresh via pip in the sandbox (cold start).
- API keys passed via `env` config are set as environment variables — they do not appear in the script source.
- Input values are also available as uppercased env vars (e.g. `stdin` → `STDIN`).
- The script runs inside a `/usr/bin/python3 -c` invocation in the Zypi sandbox.
