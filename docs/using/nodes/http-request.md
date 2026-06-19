# Node Type: `http.request`

## Category

http

## Description

Makes HTTP requests. Supports GET, POST, PUT, DELETE methods. Request body from input port, response body/status/headers as outputs.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `method` | String | `"GET"` | HTTP method |
| `headers` | Object or null | null | Request headers as `{ "Header-Name": "value" }` |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `url` | Yes | Request URL (string) |
| `body` | No | Request body (serialized to string, or JSON for POST/PUT) |

## Output Ports

| Port | Description |
|------|-------------|
| `body` | Response body as string |
| `status` | HTTP status code (Number) |
| `headers` | Response headers as Object |

## JSON Example

```json
{
  "id": "aaaaaaaa-bbbb-4ccc-8ddd-111111111111",
  "node_type": "http.request",
  "name": "Fetch Data",
  "config": {
    "method": "GET"
  }
}
```

## YAML Example

```yaml
id: "aaaaaaaa-bbbb-4ccc-8ddd-111111111111"
node_type: http.request
name: "Fetch Data"
config:
  method: GET
```

## Notes

- `url` is a required input — set it via an upstream connection, not in config.
- POST/PUT requests use the `body` input for the request payload.
- Response headers are returned as an Object with header names as keys.
