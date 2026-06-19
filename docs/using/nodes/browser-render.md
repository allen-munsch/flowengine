# Node Type: `browser.render`

## Category

browser

## Description

Renders a URL or inline HTML in headless Chromium inside a Firecracker sandbox (via Zypi). Supports three modes: DOM dump, text extraction, and screenshot.

## Config

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | String or null | null | URL to render |
| `html` | String or null | null | Inline HTML to render (written to `/tmp/page.html` in sandbox) |
| `mode` | String | `"screenshot"` | Render mode: `"dom"`, `"text"`, or `"screenshot"` |
| `wait_ms` | Number | `1000` | Virtual time budget in milliseconds (allows JS to execute) |
| `timeout` | Number | `30` | Execution timeout in seconds |
| `memory_mb` | Number | `128` | Sandbox memory limit in MB |
| `zypi_url` | String | `"http://localhost:4000"` | Zypi sandbox API URL |
| `image` | String | `"ubuntu:24.04"` | Sandbox container image |

## Input Ports

None.

## Output Ports

| Port | Description |
|------|-------------|
| `output` | Rendered content — DOM (HTML), text, or base64-encoded PNG |
| `stdout` | Raw stdout from chromium |
| `duration_ms` | Execution duration in milliseconds |

## JSON Example

```json
{
  "id": "22222222-2222-4222-8222-222222222222",
  "node_type": "browser.render",
  "name": "Capture Screenshot",
  "config": {
    "url": "https://example.com",
    "mode": "screenshot",
    "wait_ms": 2000
  }
}
```

## YAML Example

```yaml
id: "22222222-2222-4222-8222-222222222222"
node_type: browser.render
name: "Capture Screenshot"
config:
  url: "https://example.com"
  mode: screenshot
  wait_ms: 2000
```

## Notes

- **dom** mode: Returns raw HTML from `--dump-dom`. Use with `flow.branch` + `transform.json_parse` for structured extraction.
- **text** mode: Extracts readable text via lynx/elinks (falls back to raw DOM if not available).
- **screenshot** mode: Returns a base64-encoded PNG (1280×720). Pass to a file writer node to save.
- If neither `url` nor `html` is provided, defaults to `file:///tmp/page.html`.
- Execution runs inside a Firecracker microVM via Zypi.
