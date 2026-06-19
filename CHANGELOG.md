# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- YAML workflow format support via `WorkflowParser` trait (`JsonParser`, `YamlParser`)
- `parse_workflow()` and `parse_workflow_file()` helpers with format auto-detection
- `BranchNode` (`flow.branch`) — routes payload to `true_out` or `false_out` based on condition
- `MergeNode` (`flow.merge`) — collects all upstream inputs into a merged object
- `--output json` CLI flag for machine-readable event streaming
- `FlowBuilder.build_async()` in Python SDK for local execution via subprocess (no server needed)
- `#[derive(NodeConfig)]` proc macro crate (`flowcore_macros`)

### Fixed
- Removed cyclic `flowpersist` → `flowruntime` dependency
- Added missing `StringArc`, `BytesArc`, `JsonArc` match arms in `docker_v2.rs`

## [0.1.0] — Initial

### Added
- DAG execution engine with typed `Value` enum
- 11 built-in node types: shell, http, time, delay, debug, transform, api-call, browser, zypi, docker
- Event-driven executor with `ExecutionEvent` streaming
- `NodeRegistry` with factory-based node instantiation
- `NodePlugin` trait for dynamic plugin loading (skeleton)
- `flowpersist` SQLite result cache
- CLI binary (`flow`) with 4 commands: run, validate, list, serve
- Python SDK with `FlowBuilder`, `@task` decorator, and HTTP client
- Workflow JSON format with type-tagged values
