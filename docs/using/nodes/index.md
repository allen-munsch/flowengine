# Node Type Reference

Quick-lookup table of all 14 built-in node types.

| Node Type | Category | Description | Doc |
|-----------|----------|-------------|-----|
| `http.request` | http | Make HTTP requests | [http-request.md](http-request.md) |
| `transform.json_parse` | transform | Parse JSON string to Value | [json-parse.md](json-parse.md) |
| `transform.json_stringify` | transform | Convert Value to JSON string | [json-stringify.md](json-stringify.md) |
| `shell.exec` | shell | Execute local shell commands | [shell-exec.md](shell-exec.md) |
| `debug.log` | debug | Log input values for debugging | [debug-log.md](debug-log.md) |
| `time.delay` | time | Delay execution for specified milliseconds | [delay.md](delay.md) |
| `api.call` | api | Run Python scripts with pip packages in Zypi sandbox | [api-call.md](api-call.md) |
| `browser.render` | browser | Render HTML/URL in headless Chromium sandbox | [browser-render.md](browser-render.md) |
| `docker.run` | docker | Execute Docker containers (v1) | [docker-exec.md](docker-exec.md) |
| `docker.run` | docker | Execute Docker containers with IOMode (v2) | [docker-v2-exec.md](docker-v2-exec.md) |
| `zypi.exec` | zypi | Execute command in Firecracker microVM | [zypi-exec.md](zypi-exec.md) |
| `zypi.session_create` | zypi | Create long-lived Firecracker session | [zypi-session-create.md](zypi-session-create.md) |
| `flow.branch` | flow | Route payload based on boolean condition | [branch.md](branch.md) |
| `flow.merge` | flow | Collect upstream inputs into merged object | [merge.md](merge.md) |

> **Note:** Both `DockerNode` and `DockerNodeV2` register as `"docker.run"`. The last factory registered wins. See [docker-v2-exec.md](docker-v2-exec.md) for the difference.
