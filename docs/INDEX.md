# Flow Engine Documentation Index

## Getting Started

- [getting-started.md](getting-started/getting-started.md) — Installation, first workflow, Python SDK quickstart
- [workflow-format.md](getting-started/workflow-format.md) — Full format reference (JSON, YAML, Value types, Arc variants)
- [cli-reference.md](getting-started/cli-reference.md) — All CLI commands, flags, exit codes, environment variables

## Using

- [nodes/index.md](using/nodes/index.md) — Quick-lookup table for all 14 built-in node types
- [python-sdk.md](using/python-sdk.md) — `@task`, `Flow`, `FlowBuilder`, `FlowClient`, `Sandbox`
- [events.md](using/events.md) — `ExecutionEvent`, `NodeEvent`, event bus, `--output json`, Iggy integration

## Developing

- [node-development.md](developing/node-development.md) — Building custom nodes, `#[derive(NodeConfig)]`, Arc values, testing
- [plugin-development.md](developing/plugin-development.md) — Dynamic `.so` plugins via `NodePlugin` trait and `libloading`
- [architecture.md](developing/architecture.md) — Crate structure, DAG execution, DependencyTracker, ExecutorCache, format detection

## Operating

- [deployment.md](operating/deployment.md) — Flowserver HTTP API, Docker, systemd, TLS, health checks

## Archive

- [archive/README.md](archive/README.md) — Historical planning artifacts (not current documentation)
