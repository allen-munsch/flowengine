# FlowEngine Phase Plan: Prefect-Like Sandbox Runner

## Phase 1: Core Foundation (~2 hrs)
1.1 Add `Value::Blob` for file data support
1.2 Add streaming stdout/stderr events (`NodeEvent::StdoutLine`, `NodeEvent::StderrLine`)
1.3 Implement exponential backoff retry in executor
1.4 Add `max_concurrent_nodes` to runtime config

## Phase 2: New Execution Nodes (~3 hrs)
2.1 `shell.exec` — local process execution (tokio::process::Command)
2.2 `zypi.exec` — Zypi Firecracker microVM via HTTP API
2.3 File injection: `NodeContext::write_files()`, nodes write Blob inputs to tempdir
2.4 `zypi.import_image`, `zypi.list_images` utilities

## Phase 3: Python SDK (~2 hrs)
3.1 `flowengine` Python package (pip installable)
3.2 `@flow` and `@task` decorators
3.3 `Sandbox` class mirroring `bubbleproc.py` API
3.4 Server client and embedded execution modes

## Phase 4: Persistence & Caching (~2 hrs)
4.1 SQLite persistence for workflow state
4.2 Node-level result caching with content-fingerprint
4.3 Cache invalidation policies
4.4 Workflow history and replay

## Phase 5: Polish & Integration (~1 hr)
5.1 Example workflows for zypi patterns
5.2 Integration tests
5.3 CLI improvements
5.4 Documentation
