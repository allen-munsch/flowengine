#!/usr/bin/env python3
"""
FlowEngine Python SDK — Integration Example

Demonstrates the full workflow: define, run, sandbox, cache.

Usage:
    # Start the server first:
    cargo run --bin flowserver &
    
    # Then run this:
    python3 python/examples/integration_demo.py
"""

import json
import os
import sys

# Add package to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from flowengine import Flow, FlowBuilder, FlowClient, Sandbox, task


# ── Define tasks ──────────────────────────────────────────────

@task(retry=3, timeout=30)
def fetch_github_zen(url: str) -> str:
    """Fetch GitHub's Zen message."""
    import urllib.request
    with urllib.request.urlopen(url) as r:
        return r.read().decode().strip()


@task()
def process_zen(zen_text: str) -> dict:
    """Process the Zen text."""
    return {
        "zen": zen_text,
        "words": len(zen_text.split()),
        "chars": len(zen_text),
        "uppercase": zen_text.upper(),
    }


@task()
def format_output(data: dict) -> str:
    """Format as JSON string."""
    return json.dumps(data, indent=2)


# ── Method 1: Fluent API ──────────────────────────────────────

def demo_fluent_api():
    print("=" * 60)
    print("Demo 1: Fluent Flow API")
    print("=" * 60)

    flow = Flow(
        "github-zen-pipeline",
        description="Fetches and processes GitHub Zen",
    )
    flow >> fetch_github_zen >> process_zen >> format_output
    flow.save("/tmp/flow_demo.json")
    print(f"  ✅ Workflow saved to /tmp/flow_demo.json")
    print(f"  Workflow JSON:\n{flow.to_json()[:500]}...\n")


# ── Method 2: Builder API ─────────────────────────────────────

def demo_builder_api():
    print("=" * 60)
    print("Demo 2: Builder API with explicit connections")
    print("=" * 60)

    builder = FlowBuilder("explicit-pipeline")
    fetch = builder.add(fetch_github_zen)
    proc = builder.add(process_zen)
    fmt = builder.add(format_output)

    builder.connect(fetch, "output", proc, "input")
    builder.connect(proc, "output", fmt, "input")

    flow = builder.build()
    flow.save("/tmp/flow_builder_demo.json")
    print(f"  ✅ Builder workflow saved to /tmp/flow_builder_demo.json\n")


# ── Method 3: Server execution ────────────────────────────────

def demo_server_execution():
    print("=" * 60)
    print("Demo 3: Execute via FlowEngine Server")
    print("=" * 60)

    client = FlowClient("http://localhost:3000")

    # Check server health
    try:
        health = client.health()
        print(f"  ✅ Server healthy: {health}")
    except Exception as e:
        print(f"  ⚠️  Server not running: {e}")
        print("  Start with: cargo run --bin flowserver")
        return

    # List available nodes
    nodes = client.list_nodes()
    print(f"  📦 Available nodes: {len(nodes)}")
    for n in nodes:
        print(f"     • {n['type']} ({n.get('category', 'general')})")

    print()


# ── Method 4: Sandbox (Zypi) ──────────────────────────────────

def demo_sandbox():
    print("=" * 60)
    print("Demo 4: Zypi Sandbox (Firecracker microVM)")
    print("=" * 60)

    sandbox = Sandbox(zypi_url="http://localhost:4000")

    if not sandbox.health_check():
        print("  ⚠️  Zypi not running at localhost:4000")
        print("  Start Zypi with: docker compose up (in zypi dir)")
        print("  Skipping sandbox demo.\n")
        return

    print("  ✅ Zypi is healthy!")

    exit_code, stdout, stderr = sandbox.exec(
        ["python3", "-c", "print('hello from firecracker!')"]
    )
    print(f"  Result: exit={exit_code}, stdout={stdout.strip()!r}")


# ── Method 5: Check output (raises on failure) ─────────────────

def demo_check_output():
    print("=" * 60)
    print("Demo 5: check_output (raises on failure)")
    print("=" * 60)

    sandbox = Sandbox(zypi_url="http://localhost:4000")
    if not sandbox.health_check():
        print("  ⚠️  Zypi not running, skipping.\n")
        return

    try:
        result = sandbox.check_output(["echo", "success"])
        print(f"  ✅ check_output: {result.strip()!r}")
    except Exception as e:
        print(f"  ❌ {e}")


# ── Run all demos ─────────────────────────────────────────────

if __name__ == "__main__":
    print()
    print("╔══════════════════════════════════════════════════╗")
    print("║   FlowEngine Python SDK — Integration Demo       ║")
    print("╚══════════════════════════════════════════════════╝")
    print()

    demo_fluent_api()
    demo_builder_api()
    demo_server_execution()
    demo_sandbox()
    demo_check_output()

    print("=" * 60)
    print("✅ All demos complete!")
    print("=" * 60)
