# Node Type: `docker.run`

## Category

docker

## Description

Executes a command inside a Docker container. Supports env vars, volumes, network mode, stdin modes, output parsing, and auto-pull. This is the v1 implementation.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `image` | String | (required) | Docker image to run |
| `command` | String or Array of String | null | Command to execute (overrides image entrypoint); string form is whitespace-split |
| `entrypoint` | String or Array of String | null | Override container entrypoint |
| `env` | Object | `{}` | Environment variables as `{ "KEY": "value" }` |
| `volumes` | Array of String | `[]` | Volume mounts (`"host:container"` or `"host:container:ro"`) |
| `workdir` | String or null | null | Working directory inside container |
| `user` | String or null | null | User/UID to run as |
| `network` | String or null | null | Network mode (`"bridge"`, `"host"`, `"none"`, or custom) |
| `cpu_limit` | String or null | null | CPU limit (e.g. `"1.5"`, Docker `--cpus` format) |
| `memory_limit` | String or null | null | Memory limit (e.g. `"256m"`) |
| `stdin_mode` | String | `"json"` | How to send inputs: `"none"`, `"raw"`, `"json"`, `"text"` |
| `output_mode` | String | `"auto"` | How to parse stdout: `"auto"` (try JSON), `"json"` (force), `"text"` (raw) |
| `auto_pull` | Bool | `true` | Pull image if not present locally |
| `detached` | Bool | `false` | Run container in detached mode |
| `remove` | Bool | `true` | Auto-remove container after exit (`--rm`) |
| `timeout` | Number or null | null | Execution timeout in seconds |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `data` | Varies | Input data piped to stdin (required if `stdin_mode` is not `"none"`) |

## Output Ports

| Port | Description |
|------|-------------|
| `stdout` | Container stdout |
| `stderr` | Container stderr |
| `exit_code` | Container exit code (Number, 0 = success) |
| `success` | Boolean success indicator |

## JSON Example

```json
{
  "id": "33333333-3333-4333-8333-333333333333",
  "node_type": "docker.run",
  "name": "Build Project",
  "config": {
    "image": "rust:1.75",
    "command": ["cargo", "build", "--release"],
    "workdir": "/app",
    "volumes": [".:/app"],
    "timeout": 600
  }
}
```

## YAML Example

```yaml
id: "33333333-3333-4333-8333-333333333333"
node_type: docker.run
name: "Build Project"
config:
  image: "rust:1.75"
  command:
    - cargo
    - build
    - --release
  workdir: "/app"
  volumes:
    - ".:/app"
  timeout: 600
```

## Notes

- Docker must be installed and the daemon running on the executor host.
- Both v1 and v2 implementations register as `"docker.run"`. The last registered factory wins.
- For IOMode control (flat vs. wrapped JSON serialization), use the v2 node (`docker-v2-exec.md`).