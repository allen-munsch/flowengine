# Node Type: `flow.branch`

## Category

flow

## Description

Routes an input payload to one of two output ports based on a boolean condition. This is the primary flow-control primitive — the "if" gate. Only one output port fires per execution.

## Config

No configuration.

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `condition` | Yes | Boolean condition deciding which output port gets the payload |
| `payload` | No | The value to route (any Value type) |

## Output Ports

| Port | Description |
|------|-------------|
| `true_out` | Payload value when condition is truthy |
| `false_out` | Payload value when condition is falsy |

## JSON Example

```json
{
  "id": "66666666-6666-4666-8666-666666666666",
  "node_type": "flow.branch",
  "name": "Check Status",
  "config": {}
}
```

## YAML Example

```yaml
id: "66666666-6666-4666-8666-666666666666"
node_type: flow.branch
name: "Check Status"
config: {}
```

## Notes

- Only one output port fires — the non-routed path does not execute downstream nodes.
- A missing `condition` input defaults to `false`.
- The `condition` must be a boolean Value. Non-boolean values are treated as `false`.
- Pair with `flow.merge` to rejoin the two paths.
