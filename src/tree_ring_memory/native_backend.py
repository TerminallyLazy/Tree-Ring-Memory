from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from tree_ring_memory.models import MemoryEvent
from tree_ring_memory.recall import RecallResult


class NativeTreeRingMemory:
    """Python facade for the optional PyO3 native module.

    This v0.3 preview wrapper is intentionally explicit. The default
    `TreeRingMemory` facade remains Python-backed until native parity is broader.
    """

    def __init__(self, native: Any) -> None:
        self._native = native

    @classmethod
    def open(cls, root: str | Path) -> NativeTreeRingMemory:
        try:
            from tree_ring_memory._tree_ring_memory_native import TreeRingMemoryNative
        except ImportError as exc:
            raise ImportError(
                "Tree Ring Memory native bindings are not installed. "
                "Build them with `cd bindings/python && maturin develop`."
            ) from exc
        return cls(TreeRingMemoryNative.open(str(root)))

    def remember(
        self,
        *,
        summary: str,
        event_type: str,
        scope: str = "global",
        ring: str = "cambium",
        project: str | None = None,
        tags: list[str] | None = None,
    ) -> MemoryEvent:
        payload = self._native.remember_json(summary, event_type, ring, scope, project, tags or [])
        return MemoryEvent.from_dict(json.loads(payload))

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        include_sensitive: bool = False,
        limit: int = 8,
    ) -> list[RecallResult]:
        payload = json.loads(self._native.recall_json(query, project, limit, include_sensitive))
        return [
            RecallResult(
                memory=MemoryEvent.from_dict(item["memory"]),
                score=float(item["score"]),
                ranking={key: float(value) for key, value in item.get("ranking", {}).items()},
            )
            for item in payload
        ]

    def forget(self, memory_id: str, *, mode: str, reason: str) -> None:
        self._native.forget(memory_id, mode, reason)
