# Node Type: `transform.json_parse`

## Category

transform

## Description

Parses a JSON string into a structured `Value` (Json variant). The parsed result can be accessed by downstream nodes via port references.

## Config

No configuration.

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `json` | Yes | JSON string to parse |

## Output Ports

| Port | Description |
|------|-------------|
| `parsed` | Parsed JSON as Value (Json variant) |

## JSON Example

```json
{
  "id": "bbbbbbbb-bbbb-4bbb-8bbb-222222222222",
  "node_type": "transform.json_parse",
  "name": "Parse Response",
  "config": {}
}
```

## YAML Example

```yaml
id: "bbbbbbbb-bbbb-4bbb-8bbb-222222222222"
node_type: transform.json_parse
name: "Parse Response"
config: {}
```

## Notes

- Invalid JSON causes the node to fail with a `JsonParseError`.
- The parsed output is a `Value::Json(serde_json::Value)` — access nested fields with dot-separated port references.