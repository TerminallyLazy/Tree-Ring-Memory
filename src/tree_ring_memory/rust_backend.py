from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
from tree_ring_memory.models import MemoryEvent
from tree_ring_memory.recall import RecallResult


PROJECT_ROOT = Path(__file__).resolve().parents[2]


class RustCliTreeRingMemory:
    """Compatibility adapter that routes the Python facade shape through the Rust CLI.

    This is an explicit v0.2 bridge while PyO3 bindings are still planned. It is
    intentionally opt-in so the stable Python reference remains unchanged.
    """

    def __init__(self, root: Path, *, project_root: Path | None = None) -> None:
        self.root = root
        self.project_root = project_root or PROJECT_ROOT
        self.root.mkdir(parents=True, exist_ok=True)
        self._run("init")

    @classmethod
    def open(cls, root: str | Path) -> RustCliTreeRingMemory:
        return cls(Path(root))

    def remember(
        self,
        *,
        summary: str,
        event_type: str,
        scope: str = "global",
        ring: str = "cambium",
        project: str | None = None,
        tags: list[str] | None = None,
        **unsupported: object,
    ) -> MemoryEvent:
        _reject_unsupported("remember", unsupported)
        args = [
            "remember",
            summary,
            "--event-type",
            event_type,
            "--ring",
            ring,
            "--scope",
            scope,
        ]
        if project is not None:
            args.extend(["--project", project])
        for tag in tags or []:
            args.extend(["--tag", tag])
        payload = json.loads(self._run(*args, json_output=True).stdout)
        return MemoryEvent.from_dict(payload)

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        include_sensitive: bool = False,
        limit: int = 8,
        **unsupported: object,
    ) -> list[RecallResult]:
        _reject_unsupported("recall", unsupported)
        args = ["recall", query, "--limit", str(limit)]
        if project is not None:
            args.extend(["--project", project])
        if include_sensitive:
            args.append("--include-sensitive")
        payload = json.loads(self._run(*args, json_output=True).stdout)
        return [
            RecallResult(
                memory=MemoryEvent.from_dict(item["memory"]),
                score=float(item["score"]),
                ranking={key: float(value) for key, value in item.get("ranking", {}).items()},
            )
            for item in payload
        ]

    def forget(self, memory_id: str, *, mode: str, reason: str) -> None:
        self._run("forget", memory_id, "--mode", mode, "--reason", reason, json_output=True)

    def _run(self, *args: str, json_output: bool = False) -> subprocess.CompletedProcess[str]:
        command = ["cargo", "run", "-q", "-p", "tree-ring-memory-cli", "--", "--root", str(self.root)]
        if json_output:
            command.append("--json")
        command.extend(args)
        env = os.environ.copy()
        result = subprocess.run(
            command,
            cwd=self.project_root,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            raise ValueError(result.stderr.strip() or result.stdout.strip() or "rust backend command failed")
        return result


def _reject_unsupported(operation: str, values: dict[str, object]) -> None:
    unsupported = {
        key: value
        for key, value in values.items()
        if value not in (None, [], {}, False)
    }
    if unsupported:
        names = ", ".join(sorted(unsupported))
        raise NotImplementedError(
            f"RustCliTreeRingMemory.{operation} does not support these Python facade fields yet: {names}"
        )
