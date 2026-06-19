# Node Type: `debug.log`

## Category

debug

## Description

Logs its `message` input for debugging. Also logs all other inputs for visibility. Outputs the message value.

## Config

No configuration.

## Input Ports

| Port | Required | Description |
|------|----------|-------------|
| `message` | No | Message to log (any Value type; defaults to "(no message)") |

## Output Ports

| Port | Description |
|------|-------------|
| `message` | Pass-through of the `message` input as a String |

## JSON Example

```json
{
  "id": "eeeeeeee-eeee-4eee-8eee-555555555555",
  "node_type": "debug.log",
  "name": "Inspect Response",
  "config": {}
}
```

## YAML Example

```yaml
id: "eeeeeeee-eeee-4eee-8eee-555555555555"
node_type: debug.log
name: "Inspect Response"
config: {}
```

## Notes

- All inputs are logged individually for visibility, not just `message`.
- With `--output json`, log output appears as `node_log` events.
