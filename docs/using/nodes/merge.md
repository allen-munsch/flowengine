# Node Type: `flow.merge`

## Category

flow

## Description

Collects upstream inputs from multiple paths into a single merged output. Rejoins branches split by `flow.branch` or parallel node paths.

## Config

No configuration.

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `*` | No | Any named port — all inputs are collected into the merged output |

## Output Ports

| Port | Description |
|------|-------------|
| `merged` | Object containing all input port values keyed by port name |

## JSON Example

```json
{
  "id": "77777777-7777-4777-8777-777777777777",
  "node_type": "flow.merge",
  "name": "Combine Results",
  "config": {}
}
```

## YAML Example

```yaml
id: "77777777-7777-4777-8777-777777777777"
node_type: flow.merge
name: "Combine Results"
config: {}
```

## Notes

- All connected input ports are collected into a single Object `{"port_name": value, ...}`.
- Common pattern: `flow.branch` → do work on both paths → `flow.merge` to rejoin.
