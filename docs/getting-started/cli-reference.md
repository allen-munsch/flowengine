# CLI Reference

## `flow run`

Execute a workflow file.

```
flow run --file <PATH> [--input <JSON>] [--verbose] [--output <FORMAT>]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--file` | `-f` | Path to workflow file (JSON or YAML). Use `-` for stdin. |
| `--input` | `-i` | Input data as a JSON object string. |
| `--verbose` | `-v` | Enable DEBUG-level tracing output. |
| `--output` | `-F` | Output format: `text` (default) or `json`. |

### `--output text` (default)

Human-readable output with emoji markers:

```
▶️  Workflow started
  ⚡ Starting node: <id> (<node_type>)
     ℹ️  [<id>] <message>
     ⚠️  [<id>] <message>
     📊 [<id>] <percent>% - <message>
     📤 [<id>] <stdout line>
     📥 [<id>] <stderr line>
  ✅ Node <id> completed in <ms>ms
  ❌ Node <id> failed: <error>
✨ Workflow completed successfully in <ms>ms
```

### `--output json`

One JSON object per line, suitable for streaming consumption:

| Event JSON | Description |
|-----------|-------------|
| `{"type":"workflow_started"}` | Execution began |
| `{"type":"node_started","node_id":"...","node_type":"..."}` | Node execution started |
| `{"type":"node_log","node_id":"...","level":"info","message":"..."}` | Info message from node |
| `{"type":"node_log","node_id":"...","level":"warning","message":"..."}` | Warning from node |
| `{"type":"node_progress","node_id":"...","percent":50.0,"message":"..."}` | Progress update |
| `{"type":"node_stdout","node_id":"...","line":"..."}` | Stdout line from process |
| `{"type":"node_stderr","node_id":"...","line":"..."}` | Stderr line from process |
| `{"type":"node_completed","node_id":"...","duration_ms":234}` | Node finished successfully |
| `{"type":"node_failed","node_id":"...","error":"..."}` | Node failed |
| `{"type":"workflow_completed","success":true,"duration_ms":236}` | Workflow finished |

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Workflow completed successfully |
| 1 | Workflow failed or parse error |

## `flow validate`

Check a workflow file for structural validity without executing it.

```
flow validate --file <PATH>
```

Reports parse errors and missing required fields. Exit code 0 on valid, 1 on invalid.

## `flow nodes`

List all registered node types with their categories.

```
flow nodes
```

Output format lists each node type string (e.g. `http.request`, `debug.log`, `flow.branch`).

## `flow init`

Create a new example workflow file.

```
flow init --output <PATH>
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--output` | `-o` | `workflow.json` | Output file path |

The generated workflow has two nodes (HTTP request → debug log) and is ready to run.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `FLOW_BIN` | Path to the `flow` binary (used by Python SDK `build_async()`) |

## Format Detection

The CLI auto-detects workflow format:

- `.yaml` / `.yml` extension → YAML parser
- All other extensions (`.json`, no extension) → JSON parser
- Reading from stdin (`--file -`): first non-whitespace char `{` → JSON; otherwise → YAML

Format is resolved by file extension only when a path is provided. The `parse_workflow_file` function is used. For direct strings, `parse_workflow` tries both parsers with the first-guess heuristic.
