# Node Type: `time.delay`

## Category

time

## Description

Pauses workflow execution for a specified duration. All inputs pass through to outputs after the delay.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `delay_ms` | Number | `1000` | Delay duration in milliseconds |

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `*` | No | Any named port — all inputs pass through after delay |

## Output Ports

| Port | Description |
|------|-------------|
| `*` | All input ports are forwarded to same-named output ports |

## JSON Example

```json
{
  "id": "ffffffff-ffff-4fff-8fff-666666666666",
  "node_type": "time.delay",
  "name": "Rate Limit",
  "config": {
    "delay_ms": 2000
  }
}
```

## YAML Example

```yaml
id: "ffffffff-ffff-4fff-8fff-666666666666"
node_type: time.delay
name: "Rate Limit"
config:
  delay_ms: 2000
```

## Notes

- Useful for rate limiting or waiting for external services to settle.
- The delay runs within the async runtime — other parallel nodes are not blocked.
