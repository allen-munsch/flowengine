"""
FlowEngine Python SDK

Fast, lightweight workflow orchestration with optional sandboxed execution
via Zypi Firecracker microVMs.

Usage:
    from flowengine import Flow, task

    @task(retry=3)
    def fetch_data(url: str) -> dict:
        return requests.get(url).json()

    @task()
    def process(data: dict) -> dict:
        return {"summary": data["title"]}

    @task()
    def save(results: dict) -> str:
        with open("output.json", "w") as f:
            json.dump(results, f)
        return "saved"

    flow = Flow("data-pipeline")
    flow >> fetch_data >> process >> save
    flow.run(url="https://api.github.com/zen")

Sandbox mode (requires Zypi):
    from flowengine import Sandbox

    sandbox = Sandbox(image="ubuntu:24.04")
    exit_code, stdout, stderr = sandbox.exec(["python", "script.py"])
"""

from .client import FlowClient
from .flow import Flow, FlowBuilder
from .task import Task, task
from .sandbox import Sandbox, SandboxConfig
from .types import WorkflowSpec, NodeSpec, Connection, RetryPolicy

__all__ = [
    "Flow",
    "FlowBuilder",
    "FlowClient",
    "Task",
    "task",
    "Sandbox",
    "SandboxConfig",
    "WorkflowSpec",
    "NodeSpec",
    "Connection",
    "RetryPolicy",
]
