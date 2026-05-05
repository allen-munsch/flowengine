"""FlowClient — HTTP client for FlowEngine server."""

from __future__ import annotations

import json
from typing import Any, Optional
from urllib.parse import urljoin

import requests


class FlowClient:
    """
    HTTP client for interacting with a FlowEngine server.

    Usage:
        client = FlowClient("http://localhost:3000")
        result = client.execute(workflow_dict, inputs={"url": "https://..."})
    """

    def __init__(self, base_url: str = "http://localhost:3000"):
        self.base_url = base_url.rstrip("/")
        self._session = requests.Session()

    def health(self) -> dict:
        """Check server health."""
        r = self._session.get(f"{self.base_url}/health", timeout=5)
        r.raise_for_status()
        return r.json()

    def list_nodes(self) -> list[dict]:
        """List available node types."""
        r = self._session.get(f"{self.base_url}/api/nodes", timeout=5)
        r.raise_for_status()
        return r.json()

    def list_workflows(self) -> list[dict]:
        """List all registered workflows."""
        r = self._session.get(f"{self.base_url}/api/workflows", timeout=5)
        r.raise_for_status()
        return r.json()

    def create_workflow(self, workflow: dict) -> str:
        """Create a workflow and return its ID."""
        r = self._session.post(
            f"{self.base_url}/api/workflows",
            json=workflow,
            timeout=10,
        )
        r.raise_for_status()
        return r.json()["id"]

    def get_workflow(self, workflow_id: str) -> dict:
        """Get workflow details."""
        r = self._session.get(
            f"{self.base_url}/api/workflows/{workflow_id}",
            timeout=5,
        )
        r.raise_for_status()
        return r.json()

    def delete_workflow(self, workflow_id: str) -> dict:
        """Delete a workflow."""
        r = self._session.delete(
            f"{self.base_url}/api/workflows/{workflow_id}",
            timeout=5,
        )
        r.raise_for_status()
        return r.json()

    def execute(
        self,
        workflow: dict,
        inputs: Optional[dict[str, Any]] = None,
    ) -> dict:
        """
        Execute a workflow and return results.

        Args:
            workflow: Workflow definition as dict
            inputs: Input values keyed by name

        Returns:
            Execution result dict
        """
        # Create workflow
        workflow_id = self.create_workflow(workflow)

        # Execute
        payload = {"inputs": inputs or {}}
        r = self._session.post(
            f"{self.base_url}/api/workflows/{workflow_id}/execute",
            json=payload,
            timeout=300,
        )
        r.raise_for_status()
        result = r.json()

        # Clean up
        try:
            self.delete_workflow(workflow_id)
        except Exception:
            pass

        return result

    async def execute_async(
        self,
        workflow: dict,
        inputs: Optional[dict[str, Any]] = None,
    ) -> dict:
        """Async version of execute (uses a thread pool)."""
        import asyncio

        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(
            None, lambda: self.execute(workflow, inputs)
        )

    def execute_file(
        self,
        workflow_path: str,
        inputs: Optional[dict[str, Any]] = None,
    ) -> dict:
        """Execute a workflow from a JSON file."""
        with open(workflow_path) as f:
            workflow = json.load(f)
        return self.execute(workflow, inputs)

    def close(self) -> None:
        """Close the HTTP session."""
        self._session.close()

    def __enter__(self) -> FlowClient:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
