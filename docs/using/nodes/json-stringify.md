# Node Type: `transform.json_stringify`

## Category

transform

## Description

Converts a `Value` to a pretty-printed JSON string. Accepts any Value type.

## Config

No configuration.

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `value` | Yes | Value to stringify (any Value type) |

## Output Ports

| Port | Description |
|------|-------------|
| `json` | Pretty-printed JSON string representation |

## JSON Example

```json
{
  "id": "cccccccc-cccc-4ccc-8ccc-333333333333",
  "node_type": "transform.json_stringify",
  "name": "To JSON String",
  "config": {}
}
```

## YAML Example

```yaml
id: "cccccccc-cccc-4ccc-8ccc-333333333333"
node_type: transform.json_stringify
name: "To JSON String"
config: {}
```

## Notes

- Output is pretty-printed with 2-space indentation (via `serde_json::to_string_pretty`).
- Any Value type is accepted — numbers, strings, arrays, objects, booleans, null.