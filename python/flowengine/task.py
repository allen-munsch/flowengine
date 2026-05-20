"""Task decorator — wraps Python functions as FlowEngine nodes."""

from __future__ import annotations

import inspect
import uuid
from typing import Any, Callable, Optional

from .types import NodeSpec, RetryPolicy


class Task:
    """
    A workflow task wrapping a Python function.

    Usage:
        @task(retry=3, timeout=30)
        def my_task(data: dict) -> dict:
            return {"result": data["value"] * 2}
    """

    def __init__(
        self,
        fn: Callable,
        *,
        name: Optional[str] = None,
        retry: Optional[int] = None,
        timeout: Optional[int] = None,
        node_type: str = "shell.exec",
        image: Optional[str] = None,
        **config: Any,
    ):
        self._fn = fn
        self._name = name or fn.__name__
        self._node_type = node_type
        self._image = image
        self._retry_count = retry
        self._timeout = timeout
        self._config = config
        self._node_id: Optional[str] = None
        self._upstream: dict[str, dict[str, str]] = {}  # node_id -> {to_port: from_port}

        # Introspect function signature for ports
        self._sig = inspect.signature(fn)

    def _register(self) -> None:
        """Assign a node ID (called when added to a flow)."""
        if self._node_id is None:
            self._node_id = str(uuid.uuid4())

    def _set_input(self, port: str, from_node_id: str, from_port: str) -> None:
        """Set an upstream dependency."""
        self._upstream.setdefault(from_node_id, {})[port] = from_port

    def __rshift__(self, other: Task) -> Task:
        """Task >> Task — connect output to input."""
        other._set_input("input", self._node_id, "output")
        return other

    def __rrshift__(self, other: Any) -> Task:
        """Flow >> Task compatibility."""
        if hasattr(other, "_last_node_id"):
            self._set_input("input", other._last_node_id, "output")
        return self

    def _to_node_spec(self) -> NodeSpec:
        config = dict(self._config)

        if self._node_type == "shell.exec":
            # Extract source code for shell execution
            source = inspect.getsource(self._fn)
            # Build a self-contained script
            script = self._build_script(source)
            config["command"] = "python3"
            config["args"] = ["-c", script]
            if self._timeout:
                config["timeout"] = float(self._timeout)
        elif self._node_type == "zypi.exec":
            source = inspect.getsource(self._fn)
            script = self._build_script(source)
            config["command"] = "python3 -c " + script
            if self._image:
                config["image"] = self._image
            if self._timeout:
                config["timeout"] = float(self._timeout)
        elif self._node_type == "http.request":
            config.setdefault("method", "GET")

        retry_policy = None
        if self._retry_count:
            retry_policy = RetryPolicy(max_attempts=self._retry_count)

        return NodeSpec(
            id=self._node_id or str(uuid.uuid4()),
            node_type=self._node_type,
            name=self._name,
            config=config,
            retry_policy=retry_policy,
        )

    def _build_script(self, source: str) -> str:
        """Build a self-contained Python script from the decorated function."""
        # Strip decorator
        lines = source.split("\n")
        script_lines = []
        in_fn = False
        indent = 0

        for line in lines:
            if line.strip().startswith("def "):
                in_fn = True
                indent = len(line) - len(line.lstrip())
                # Rewrite function to handle stdin/stdout
                script_lines.append("import sys, json")
                script_lines.append("")
                script_lines.append(line)
            elif in_fn:
                script_lines.append(line)

        # Add invocation code at the end
        script_lines.append("")
        script_lines.append("")
        script_lines.append("if __name__ == '__main__':")
        script_lines.append("    import sys, json")
        script_lines.append(f"    kwargs = json.load(sys.stdin) if not sys.stdin.isatty() else {{}}")
        script_lines.append(f"    result = {self._name}(**kwargs)")
        script_lines.append("    if isinstance(result, (dict, list)):")
        script_lines.append("        json.dump(result, sys.stdout)")
        script_lines.append("    else:")
        script_lines.append("        print(result)")

        return "\n".join(script_lines)

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        """Allow the task to still be called as a plain function."""
        return self._fn(*args, **kwargs)

    def __repr__(self) -> str:
        return f"Task({self._name!r}, type={self._node_type!r})"


def task(
    fn: Optional[Callable] = None,
    *,
    name: Optional[str] = None,
    retry: Optional[int] = None,
    timeout: Optional[int] = None,
    node_type: str = "shell.exec",
    image: Optional[str] = None,
    **config: Any,
) -> Task:
    """
    Decorator to turn a function into a FlowEngine Task.

    Usage:
        @task(retry=3)
        def my_task(data: dict) -> dict:
            return {"result": data["value"] * 2}
    """
    if fn is not None:
        return Task(
            fn,
            name=name,
            retry=retry,
            timeout=timeout,
            node_type=node_type,
            image=image,
            **config,
        )
    else:

        def decorator(f: Callable) -> Task:
            return Task(
                f,
                name=name or f.__name__,
                retry=retry,
                timeout=timeout,
                node_type=node_type,
                image=image,
                **config,
            )

        return decorator
