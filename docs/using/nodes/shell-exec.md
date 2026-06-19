# Node Type: `shell.exec`

## Category

shell

## Description

Executes a local shell command and captures stdout, stderr, and exit code. Supports streaming output, environment variables, working directory, and timeout.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `command` | String | (required) | Command to execute |
| `args` | Array of String or String | `[]` | Command arguments (appended to command); string form is whitespace-split |
| `env` | Object | `{}` | Environment variables as `{ "KEY": "value" }` |
| `env_passthrough` | Array of String | `[]` | Variable names to inherit from host environment |
| `workdir` | String or null | null | Working directory for the command |
| `timeout_seconds` | Number or null | null | Execution timeout in seconds |
| `shell` | Bool | `false` | Run via shell (`/bin/sh -c`); enables pipes, redirects, variable expansion |
| `capture_stdout` | Bool | `true` | Capture stdout |
| `capture_stderr` | Bool | `true` | Capture stderr |
| `stream_output` | Bool | `false` | Emit stdout/stderr lines as events in real time |
| `strip_trailing_newline` | Bool | `true` | Strip trailing newlines from stdout |
| `fail_on_error` | Bool | `true` | Treat non-zero exit as a node error |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `stdin` | No | Data to pipe to stdin (String, Bytes, or Json) |

## Output Ports

| Port | Description |
|------|-------------|
| `output` | Command output (JSON-parsed if possible, otherwise String) |
| `stdout` | Raw stdout as String |
| `stderr` | Raw stderr as String |
| `exit_code` | Exit code (Number, 0 = success) |
| `success` | Boolean success indicator |

## JSON Example

```json
{
  "id": "dddddddd-dddd-4ddd-8ddd-444444444444",
  "node_type": "shell.exec",
  "name": "Run Lint",
  "config": {
    "command": "cargo",
    "args": ["clippy", "--workspace"],
    "timeout_seconds": 60,
    "shell": true
  }
}
```

## YAML Example

```yaml
id: "dddddddd-dddd-4ddd-8ddd-444444444444"
node_type: shell.exec
name: "Run Lint"
config:
  command: cargo
  args:
    - clippy
    - --workspace
  timeout_seconds: 60
  shell: true
```

## Notes

- When `shell: true`, the command runs through `/bin/sh -c`, enabling pipes, redirects, and variable expansion.
- When `shell: false`, the command is exec'd directly without shell interpretation.
- `stream_output: true` emits each stdout/stderr line as a `NodeEvent::StdoutLine`/`StderrLine` event in real time.
- Timeout kills the process and returns a `NodeError::Timeout`.
- `fail_on_error: false` allows non-zero exits to be treated as success (exit code still captured).
