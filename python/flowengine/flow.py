"""Flow and FlowBuilder — define and execute workflows."""

from __future__ import annotations

import json
import uuid
from typing import Any, Callable, Optional

from .client import FlowClient
from .task import Task
from .types import Connection, NodeSpec, WorkflowSettings, WorkflowSpec


class Flow:
    """
    A workflow composed of tasks connected in a DAG.

    Usage:
        flow = Flow("my-pipeline")
        flow >> fetch_data >> process >> save
        result = flow.run(url="https://api.example.com")
    """

    def __init__(
        self,
        name: str,
        description: Optional[str] = None,
        settings: Optional[WorkflowSettings] = None,
        client: Optional[FlowClient] = None,
    ):
        self.id = str(uuid.uuid4())
        self.name = name
        self.description = description
        self.settings = settings or WorkflowSettings()
        self._client = client or FlowClient()
        self._tasks: list[Task] = []
        self._connections: list[Connection] = []
        self._last_node_id: Optional[str] = None

    def __rshift__(self, other: Task) -> Flow:
        """Flow >> task — start a pipeline."""
        self._add_task(other)
        self._last_node_id = other._node_id
        return self

    def then(self, task: Task) -> Flow:
        """Explicit chaining."""
        return self.__rshift__(task)

    def _add_task(self, task: Task) -> None:
        task._register()
        self._tasks.append(task)

        # If there's a previous task, connect them
        if self._last_node_id:
            task._set_input("input", self._last_node_id, "output")

    def run(self, **inputs: Any) -> dict:
        """Execute the workflow synchronously via HTTP API."""
        spec = self._to_spec()
        return self._client.execute(spec.to_dict(), inputs)

    async def run_async(self, **inputs: Any) -> dict:
        """Execute the workflow asynchronously."""
        spec = self._to_spec()
        return await self._client.execute_async(spec.to_dict(), inputs)

    def _to_spec(self) -> WorkflowSpec:
        nodes = [t._to_node_spec() for t in self._tasks]
        connections = list(self._connections)

        # Build connections from task dependency graph
        for task in self._tasks:
            for upstream_id, port_map in task._upstream.items():
                for to_port, from_port in port_map.items():
                    connections.append(
                        Connection(
                            from_node=upstream_id,
                            from_port=from_port,
                            to_node=task._node_id,
                            to_port=to_port,
                        )
                    )

        return WorkflowSpec(
            id=self.id,
            name=self.name,
            description=self.description,
            nodes=nodes,
            connections=connections,
            settings=self.settings,
        )

    def to_json(self, indent: int = 2) -> str:
        """Export workflow as JSON string."""
        return json.dumps(self._to_spec().to_dict(), indent=indent)

    def save(self, path: str) -> None:
        """Save workflow to a JSON file."""
        with open(path, "w") as f:
            f.write(self.to_json())

    def __repr__(self) -> str:
        return f"Flow({self.name!r}, tasks={len(self._tasks)})"


class FlowBuilder:
    """
    Programmatic workflow builder with more control over connections.

    Usage:
        builder = FlowBuilder("pipeline")
        fetch = builder.add(fetch_data)
        proc = builder.add(process)
        builder.connect(fetch, "output", proc, "input")
        flow = builder.build()
    """

    def __init__(
        self,
        name: str,
        description: Optional[str] = None,
        client: Optional[FlowClient] = None,
    ):
        self.name = name
        self.description = description
        self._client = client or FlowClient()
        self._tasks: list[Task] = []
        self._connections: list[Connection] = []

    def add(self, task: Task) -> Task:
        """Add a task to the builder."""
        task._register()
        self._tasks.append(task)
        return task

    def connect(
        self,
        from_task: Task,
        from_port: str,
        to_task: Task,
        to_port: str,
    ) -> FlowBuilder:
        """Connect two tasks."""
        self._connections.append(
            Connection(
                from_node=from_task._node_id,
                from_port=from_port,
                to_node=to_task._node_id,
                to_port=to_port,
            )
        )
        return self

    def build(self) -> Flow:
        """Build the Flow."""
        flow = Flow(
            name=self.name,
            description=self.description,
            client=self._client,
        )
        flow._tasks = self._tasks
        flow._connections = self._connections
        flow._last_node_id = (
            self._tasks[-1]._node_id if self._tasks else None
        )
        return flow

    def __repr__(self) -> str:
        return f"FlowBuilder({self.name!r}, tasks={len(self._tasks)})"
