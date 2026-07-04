from __future__ import annotations

from pathlib import Path
from typing import Sequence

from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource, now_utc
from tree_ring_memory.recall import MemoryRetriever, RecallResult
from tree_ring_memory.sensitivity import SensitivityGuard, SensitivityResult
from tree_ring_memory.store import SQLiteMemoryStore


class TreeRingMemory:
    def __init__(self, root: Path, store: SQLiteMemoryStore) -> None:
        self.root = root
        self.store = store
        self._retriever = MemoryRetriever(store)
        self._sensitivity_guard = SensitivityGuard()

    @classmethod
    def open(cls, root: str | Path) -> TreeRingMemory:
        root = Path(root)
        root.mkdir(parents=True, exist_ok=True)
        return cls(root, SQLiteMemoryStore.open(root / "memory.sqlite"))

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
        source = source or MemorySource()
        tags = tags or []
        supersedes = supersedes or []
        links = links or []
        review = review or MemoryReview()
        sensitivity_results = self._check_public_text_fields(
            summary,
            details,
            event_type,
            scope,
            ring,
            project,
            agent_profile,
            source.type,
            source.ref,
            source.quote,
            sensitivity,
            retention,
            *tags,
            *supersedes,
            *[value for link in links for value in (link.type, link.target)],
            review.review_reason,
            review.reviewed_at,
            review.reviewed_by,
        )
        if sensitivity == "normal":
            sensitivity = _detected_sensitivity(*sensitivity_results)

        event = MemoryEvent.new(
            summary=summary,
            event_type=event_type,
            scope=scope,
            ring=ring,
            project=project,
            agent_profile=agent_profile,
            details=details,
            source=source,
            tags=tags,
            salience=salience,
            confidence=confidence,
            sensitivity=sensitivity,
            retention=retention,
            expires_at=expires_at,
            supersedes=supersedes,
            links=links,
            review=review,
        )
        self.store.put(event)
        for superseded_id in event.supersedes:
            self.store.supersede(superseded_id, event.id)
        return event

    def recall(
        self,
        query: str,
        *,
        project: str | None = None,
        agent_profile: str | None = None,
        scope: str | None = None,
        rings: Sequence[str] | None = None,
        event_types: Sequence[str] | None = None,
        include_sensitive: bool = False,
        include_superseded: bool = False,
        limit: int = 8,
        explain_ranking: bool = False,
    ) -> list[RecallResult]:
        return self._retriever.recall(
            query,
            project=project,
            agent_profile=agent_profile,
            scope=scope,
            rings=rings,
            event_types=event_types,
            include_sensitive=include_sensitive,
            include_superseded=include_superseded,
            limit=limit,
            explain_ranking=explain_ranking,
        )

    def forget(self, memory_id: str, *, mode: str, reason: str) -> None:
        if not reason.strip():
            raise ValueError("forget reason is required")

        if mode == "delete":
            self.store.delete(memory_id)
            return

        if mode == "redact":
            event = self.store.get(memory_id)
            if event is None:
                return
            event.summary = "[REDACTED]"
            event.details = ""
            event.project = None
            event.agent_profile = None
            event.event_type = "redacted"
            event.tags = []
            event.source = MemorySource()
            event.supersedes = []
            event.links = []
            event.review = MemoryReview()
            event.sensitivity = "private"
            event.updated_at = now_utc()
            self.store.put(event)
            return

        raise ValueError(f"unsupported forget mode: {mode}")

    def _check_public_text_fields(self, *values: str | None) -> list[SensitivityResult]:
        return [self._sensitivity_guard.check_or_raise(value or "") for value in values]


def _detected_sensitivity(*results: SensitivityResult) -> str:
    for result in results:
        if result.sensitivity != "normal":
            return result.sensitivity
    return "normal"
