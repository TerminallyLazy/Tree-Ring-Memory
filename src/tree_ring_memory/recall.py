from __future__ import annotations

from dataclasses import dataclass
from datetime import UTC, datetime
from math import exp
import re
from typing import Sequence

from tree_ring_memory.models import MemoryEvent
from tree_ring_memory.store import SQLiteMemoryStore


FAILURE_TERMS = {
    "error",
    "failure",
    "regression",
    "bug",
    "rejected",
    "rollback",
    "stale",
    "conflict",
    "security",
    "privacy",
    "mistake",
}
HEARTWOOD_TERMS = {"preference", "rule", "constraint", "decision", "durable"}
SEED_TERMS = {"planning", "roadmap", "future", "alternative", "experiment", "explore"}

_TERM_RE = re.compile(r"\w+")
_RING_INTENT_TERMS = FAILURE_TERMS | HEARTWOOD_TERMS | SEED_TERMS


@dataclass(slots=True)
class RecallResult:
    memory: MemoryEvent
    score: float
    ranking: dict[str, float]


class MemoryRetriever:
    def __init__(self, store: SQLiteMemoryStore) -> None:
        self.store = store

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
        if not query.strip():
            return []

        candidates = self._search_candidates(query, include_superseded=include_superseded)

        filtered = [
            event
            for event in candidates
            if self._matches(event, project, agent_profile, scope, rings, event_types, include_sensitive)
        ]
        results = [self._score(event, query, explain_ranking) for event in filtered]
        results.sort(key=lambda result: result.score, reverse=True)
        return results[:limit]

    def _search_candidates(self, query: str, *, include_superseded: bool) -> list[MemoryEvent]:
        seen_queries: set[str] = set()
        for search_query in _search_queries(query):
            if search_query in seen_queries:
                continue
            seen_queries.add(search_query)

            candidates = self.store.search_text(search_query, include_superseded=include_superseded)
            if candidates:
                return candidates
        return []

    def _matches(
        self,
        event: MemoryEvent,
        project: str | None,
        agent_profile: str | None,
        scope: str | None,
        rings: Sequence[str] | None,
        event_types: Sequence[str] | None,
        include_sensitive: bool,
    ) -> bool:
        if project is not None and event.project != project:
            return False
        if agent_profile is not None and event.agent_profile != agent_profile:
            return False
        if scope is not None and event.scope != scope:
            return False
        if rings is not None and event.ring not in rings:
            return False
        if event_types is not None and event.event_type not in event_types:
            return False
        if not include_sensitive and event.sensitivity != "normal":
            return False
        return True

    def _score(self, event: MemoryEvent, query: str, explain_ranking: bool) -> RecallResult:
        textual = self._textual_match(event, query)
        recency = self._recency_score(event.created_at)
        authority = self._source_authority(event)
        ring_boost = self._ring_boost(event, query)
        ranking = {
            "textual_match": textual,
            "salience": event.salience,
            "confidence": event.confidence,
            "recency": recency,
            "source_authority": authority,
            "ring_boost": ring_boost,
        }
        score = (
            0.25 * textual
            + 0.25 * event.salience
            + 0.25 * event.confidence
            + 0.20 * recency
            + 0.05 * authority
            + ring_boost
        )
        return RecallResult(event, score, ranking if explain_ranking else {})

    def _textual_match(self, event: MemoryEvent, query: str) -> float:
        terms = _terms(query)
        if not terms:
            return 0.1

        text = " ".join([event.summary, event.details, " ".join(event.tags)]).casefold()
        matches = sum(1 for term in terms if term in text)
        return matches / len(terms)

    def _recency_score(self, created_at: datetime) -> float:
        if created_at.tzinfo is None:
            created_at = created_at.replace(tzinfo=UTC)
        age_days = max((datetime.now(UTC) - created_at).total_seconds() / 86400, 0)
        return exp(-age_days / 30)

    def _source_authority(self, event: MemoryEvent) -> float:
        authority_by_type = {
            "user": 1.0,
            "contract": 0.9,
            "eval": 0.8,
            "file": 0.7,
            "tool": 0.6,
            "summary": 0.5,
            "manual": 0.4,
        }
        return authority_by_type.get(event.source.type, 0.3)

    def _ring_boost(self, event: MemoryEvent, query: str) -> float:
        terms = set(_terms(query))
        if event.ring == "scar" and terms & FAILURE_TERMS:
            return 0.2
        if event.ring == "heartwood" and terms & HEARTWOOD_TERMS:
            return 0.15
        if event.ring == "seed" and terms & SEED_TERMS:
            return 0.12
        return 0.0


def _terms(text: str) -> list[str]:
    return [term.casefold() for term in _TERM_RE.findall(text)]


def _search_queries(query: str) -> list[str]:
    terms = _terms(query)
    queries = [query]

    for index, term in enumerate(terms):
        if term in _RING_INTENT_TERMS:
            remaining = terms[:index] + terms[index + 1 :]
            if remaining:
                queries.append(" ".join(remaining))

    return queries
