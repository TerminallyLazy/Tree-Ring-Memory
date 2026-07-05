from __future__ import annotations

import importlib
import json
from pathlib import Path
from typing import Any
from datetime import datetime

from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource, RecallResult


class NativeBindingNotInstalled(ImportError):
    """Raised when the optional Rust native module is absent."""


class NativeTreeRingMemory:
    """Python facade for the optional PyO3 native module.

    The public `TreeRingMemory` facade uses this Rust-native backend by default
    when the extension is installed.
    """

    backend_name = "rust-native"

    def __init__(self, native: Any, root: Path) -> None:
        self._native = native
        self.root = root

    @classmethod
    def open(cls, root: str | Path) -> NativeTreeRingMemory:
        root = Path(root)
        module_name = "tree_ring_memory._tree_ring_memory_native"
        try:
            native_module = importlib.import_module(module_name)
        except ModuleNotFoundError as exc:
            if exc.name != module_name:
                raise
            raise NativeBindingNotInstalled(
                "Tree Ring Memory native bindings are not installed. "
                "Build them with `cd bindings/python && maturin develop`."
            ) from exc
        return cls(native_module.TreeRingMemoryNative.open(str(root)), root)

    def remember(
        self,
        *,
        summary: str,
        event_type: str,
        scope: str = "global",
        ring: str = "cambium",
        project: str | None = None,
        agent_profile: str | None = None,
        details: str = "",
        source: MemorySource | None = None,
        tags: list[str] | None = None,
        salience: float = 0.5,
        confidence: float = 0.5,
        sensitivity: str = "normal",
        retention: str = "normal",
        expires_at=None,
        supersedes: list[str] | None = None,
        links: list[MemoryLink] | None = None,
        review: MemoryReview | None = None,
    ) -> MemoryEvent:
        request = {
            "summary": summary,
            "event_type": event_type,
            "scope": scope,
            "ring": ring,
            "project": project,
            "agent_profile": agent_profile,
            "details": details,
            "source": (source or MemorySource()).to_dict(),
            "tags": tags or [],
            "salience": salience,
            "confidence": confidence,
            "sensitivity": sensitivity,
            "retention": retention,
            "expires_at": _iso_or_none(expires_at),
            "supersedes": supersedes or [],
            "links": [link.to_dict() for link in links or []],
            "review": (review or MemoryReview()).to_dict(),
        }
        payload = self._native.remember_event_json(json.dumps(request, sort_keys=True))
        return MemoryEvent.from_dict(json.loads(payload))

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        agent_profile: str | None = None,
        scope: str | None = None,
        rings: list[str] | None = None,
        event_types: list[str] | None = None,
        include_sensitive: bool = False,
        include_superseded: bool = False,
        limit: int = 8,
        explain_ranking: bool = False,
    ) -> list[RecallResult]:
        request = {
            "query": query,
            "project": project,
            "agent_profile": agent_profile,
            "scope": scope,
            "rings": rings,
            "event_types": event_types,
            "include_sensitive": include_sensitive,
            "include_superseded": include_superseded,
            "limit": limit,
            "explain_ranking": explain_ranking,
        }
        payload = json.loads(self._native.recall_query_json(json.dumps(request, sort_keys=True)))
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

    def export_jsonl(
        self,
        *,
        include_sensitive: bool = False,
        include_superseded: bool = False,
    ) -> str:
        return str(
            self._native.export_jsonl(
                include_sensitive=include_sensitive,
                include_superseded=include_superseded,
            )
        )

    def import_jsonl(
        self,
        data: str,
        *,
        dry_run: bool = False,
        replace_existing: bool = False,
    ) -> dict[str, Any]:
        payload = self._native.import_jsonl(
            data,
            dry_run=dry_run,
            replace_existing=replace_existing,
        )
        return dict(json.loads(payload))

    def audit(self, audit_type: str = "all") -> dict[str, Any]:
        payload = self._native.audit_json(audit_type)
        return dict(json.loads(payload))

    def consolidate(
        self,
        *,
        period_type: str = "daily",
        period_key: str | None = None,
        project: str | None = None,
        dry_run: bool = False,
        force: bool = False,
    ) -> dict[str, Any]:
        payload = self._native.consolidate_json(
            period_type,
            period_key,
            project,
            dry_run,
            force,
        )
        return dict(json.loads(payload))

    def maintain(
        self,
        *,
        project: str | None = None,
        include_superseded: bool = False,
        apply_expired: bool = False,
        apply_secret_redactions: bool = False,
        repair_fts: bool = False,
    ) -> dict[str, Any]:
        payload = self._native.maintain_json(
            project,
            include_superseded,
            apply_expired,
            apply_secret_redactions,
            repair_fts,
        )
        return dict(json.loads(payload))


def _iso_or_none(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, datetime):
        return value.isoformat()
    return str(value)
