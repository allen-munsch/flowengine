"""Core type definitions for FlowEngine workflows."""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional


class ErrorHandling(str, Enum):
    STOP = "StopWorkflow"
    CONTINUE = "ContinueOnError"
    RETRY = "RetryWorkflow"


@dataclass
class RetryPolicy:
    max_attempts: int = 3
    delay_ms: int = 1000
    backoff_multiplier: float = 2.0
    max_delay_ms: Optional[int] = 60000
    retry_on_timeout: bool = True


@dataclass
class Position:
    x: float = 0.0
    y: float = 0.0


@dataclass
class NodeSpec:
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    node_type: str = ""
    name: Optional[str] = None
    config: dict[str, Any] = field(default_factory=dict)
    position: Optional[Position] = None
    retry_policy: Optional[RetryPolicy] = None

    def to_dict(self) -> dict:
        result: dict[str, Any] = {
            "id": self.id,
            "node_type": self.node_type,
            "name": self.name,
            "config": self._serialize_config(),
            "position": {"x": self.position.x, "y": self.position.y}
            if self.position
            else None,
        }
        if self.retry_policy:
            result["retry_policy"] = {
                "max_attempts": self.retry_policy.max_attempts,
                "delay_ms": self.retry_policy.delay_ms,
                "backoff_multiplier": self.retry_policy.backoff_multiplier,
                "max_delay_ms": self.retry_policy.max_delay_ms,
                "retry_on_timeout": self.retry_policy.retry_on_timeout,
            }
        return result

    def _serialize_config(self) -> dict[str, dict[str, Any]]:
        """Convert plain config to FlowEngine Value format."""
        result = {}
        for key, value in self.config.items():
            result[key] = self._value_to_typed(value)
        return result

    @staticmethod
    def _value_to_typed(value: Any) -> dict[str, Any]:
        if value is None:
            return {"type": "Null", "value": None}
        elif isinstance(value, bool):
            return {"type": "Bool", "value": value}
        elif isinstance(value, (int, float)):
            return {"type": "Number", "value": float(value)}
        elif isinstance(value, str):
            return {"type": "String", "value": value}
        elif isinstance(value, list):
            return {
                "type": "Array",
                "value": [NodeSpec._value_to_typed(v) for v in value],
            }
        elif isinstance(value, dict):
            return {
                "type": "Object",
                "value": {
                    k: NodeSpec._value_to_typed(v) for k, v in value.items()
                },
            }
        else:
            return {"type": "String", "value": str(value)}


@dataclass
class Connection:
    from_node: str
    from_port: str
    to_node: str
    to_port: str

    def to_dict(self) -> dict:
        return {
            "from_node": self.from_node,
            "from_port": self.from_port,
            "to_node": self.to_node,
            "to_port": self.to_port,
        }


@dataclass
class WorkflowSettings:
    max_execution_time_ms: Optional[int] = None
    max_parallel_nodes: int = 10
    on_error: ErrorHandling = ErrorHandling.STOP

    def to_dict(self) -> dict:
        return {
            "max_execution_time_ms": self.max_execution_time_ms,
            "max_parallel_nodes": self.max_parallel_nodes,
            "on_error": self.on_error.value,
        }


@dataclass
class WorkflowSpec:
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    name: str = ""
    description: Optional[str] = None
    nodes: list[NodeSpec] = field(default_factory=list)
    connections: list[Connection] = field(default_factory=list)
    triggers: list[dict] = field(default_factory=list)
    settings: WorkflowSettings = field(default_factory=WorkflowSettings)

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "nodes": [n.to_dict() for n in self.nodes],
            "connections": [c.to_dict() for c in self.connections],
            "triggers": self.triggers,
            "settings": self.settings.to_dict(),
        }
