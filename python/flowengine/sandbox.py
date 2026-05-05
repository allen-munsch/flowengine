"""
Sandbox — execute commands in isolated Zypi Firecracker microVMs.

Mirrors the bubbleproc.py API for drop-in compatibility.

Usage:
    from flowengine import Sandbox

    sandbox = Sandbox(
        zypi_url="http://localhost:4000",
        image="ubuntu:24.04",
    )

    exit_code, stdout, stderr = sandbox.exec(
        ["python3", "-c", "print('hello from firecracker!')"]
    )

    # File injection
    result = sandbox.exec(
        ["python3", "/app/script.py"],
        files={"/app/script.py": "print('sandboxed!')"},
    )
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass, field
from typing import Any, Optional

import requests


class SandboxError(Exception):
    """Raised when sandbox execution fails."""
    pass


@dataclass
class SandboxConfig:
    """Configuration for Zypi sandbox."""
    zypi_url: str = "http://localhost:4000"
    image: str = "ubuntu:24.04"
    timeout: int = 300


class Sandbox:
    """
    Execute commands in Zypi Firecracker microVMs.

    Compatible with bubbleproc.py Sandbox API.
    """

    def __init__(
        self,
        zypi_url: str = "http://localhost:4000",
        image: str = "ubuntu:24.04",
        timeout: int = 300,
        **kwargs: Any,
    ):
        self._config = SandboxConfig(
            zypi_url=zypi_url,
            image=image,
            timeout=timeout,
        )
        self._session = requests.Session()

        # Accept legacy kwargs for compatibility
        self._kw = kwargs

    def exec(
        self,
        cmd: list[str],
        *,
        image: Optional[str] = None,
        env: Optional[dict[str, str]] = None,
        workdir: Optional[str] = None,
        files: Optional[dict[str, str]] = None,
        timeout: Optional[int] = None,
    ) -> tuple[int, str, str]:
        """
        Execute a command in the sandbox.

        Returns (exit_code, stdout, stderr).
        """
        payload: dict[str, Any] = {
            "cmd": cmd,
            "image": image or self._config.image,
        }
        if env:
            payload["env"] = env
        if workdir:
            payload["workdir"] = workdir
        if files:
            payload["files"] = files
        if timeout:
            payload["timeout"] = timeout

        try:
            r = self._session.post(
                f"{self._config.zypi_url}/exec",
                json=payload,
                timeout=timeout or self._config.timeout,
            )
            r.raise_for_status()
            data = r.json()
            return (
                data.get("exit_code", -1),
                data.get("stdout", ""),
                data.get("stderr", ""),
            )
        except requests.exceptions.ConnectionError:
            raise SandboxError(
                f"Cannot connect to Zypi at {self._config.zypi_url}. Is it running?"
            )
        except requests.exceptions.Timeout:
            raise SandboxError(
                f"Zypi execution timed out after {timeout or self._config.timeout}s"
            )
        except Exception as e:
            raise SandboxError(f"Zypi execution failed: {e}")

    def health_check(self) -> bool:
        """Check if Zypi server is reachable."""
        try:
            r = self._session.get(
                f"{self._config.zypi_url}/health",
                timeout=5,
            )
            return r.status_code == 200
        except Exception:
            return False

    def check_output(
        self,
        cmd: list[str],
        **kwargs: Any,
    ) -> str:
        """Run command and return stdout (raises on failure)."""
        exit_code, stdout, stderr = self.exec(cmd, **kwargs)
        if exit_code != 0:
            raise SandboxError(
                f"Command failed with exit code {exit_code}: {stderr}"
            )
        return stdout

    def run(
        self,
        cmd: list[str],
        **kwargs: Any,
    ) -> subprocess.CompletedProcess:
        """Run command and return a CompletedProcess-like result."""
        exit_code, stdout, stderr = self.exec(cmd, **kwargs)
        return subprocess.CompletedProcess(
            args=cmd,
            returncode=exit_code,
            stdout=stdout.encode() if stdout else b"",
            stderr=stderr.encode() if stderr else b"",
        )

    def close(self) -> None:
        """Close the HTTP session."""
        self._session.close()

    def __enter__(self) -> Sandbox:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def __repr__(self) -> str:
        return (
            f"Sandbox(zypi_url={self._config.zypi_url!r}, "
            f"image={self._config.image!r})"
        )
