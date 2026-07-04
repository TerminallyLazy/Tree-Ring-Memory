from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from itertools import count
from math import isfinite
from typing import Any


RINGS = {"cambium", "outer", "inner", "heartwood", "scar", "seed"}
SCOPES = {"global", "project", "agent", "session", "workflow", "tool", "eval", "manual"}
SENSITIVITY = {"normal", "private", "secret", "health", "financial", "legal", "personal_identifier"}
RETENTION = {"ephemeral", "normal", "durable", "user_pinned", "forget_after_date"}

_ID_COUNTER = count(1)


class ValidationError(ValueError):
    """Raised when a memory protocol object is invalid."""


def now_utc() -> datetime:
    return datetime.now(UTC)


def parse_datetime(value: str | None, field_name: str) -> datetime | None:
    if value is None:
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValidationError(f"{field_name} must be ISO-8601 datetime") from exc


def _validate_score(name: str, value: float) -> float:
    number = float(value)
    if not isfinite(number) or number < 0 or number > 1:
        raise ValidationError(f"{name} must be a finite number between 0 and 1")
    return number


@dataclass(slots=True)
class MemorySource:
    type: str = "manual"
    ref: str = ""
    quote: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {"type": self.type, "ref": self.ref, "quote": self.quote}

    @classmethod
    def from_dict(cls, value: dict[str, Any] | None) -> MemorySource:
        value = value or {}
        return cls(
            type=str(value.get("type") or "manual"),
            ref=str(value.get("ref") or ""),
            quote=str(value.get("quote") or ""),
        )


@dataclass(slots=True)
class MemoryLink:
    type: str
    target: str

    def to_dict(self) -> dict[str, str]:
        return {"type": self.type, "target": self.target}

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> MemoryLink:
        return cls(type=str(value.get("type") or ""), target=str(value.get("target") or ""))


@dataclass(slots=True)
class MemoryReview:
    needs_review: bool = False
    review_reason: str | None = None
    reviewed_at: str | None = None
    reviewed_by: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "needs_review": self.needs_review,
            "review_reason": self.review_reason,
            "reviewed_at": self.reviewed_at,
            "reviewed_by": self.reviewed_by,
        }

    @classmethod
    def from_dict(cls, value: dict[str, Any] | None) -> MemoryReview:
        value = value or {}
        return cls(
            needs_review=bool(value.get("needs_review", False)),
            review_reason=value.get("review_reason"),
            reviewed_at=value.get("reviewed_at"),
            reviewed_by=value.get("reviewed_by"),
        )


@dataclass(slots=True)
class MemoryEvent:
    id: str
    created_at: datetime
    updated_at: datetime
    scope: str
    ring: str
    event_type: str
    summary: str
    project: str | None = None
    agent_profile: str | None = None
    details: str = ""
    source: MemorySource = field(default_factory=MemorySource)
    tags: list[str] = field(default_factory=list)
    salience: float = 0.5
    confidence: float = 0.5
    sensitivity: str = "normal"
    retention: str = "normal"
    expires_at: datetime | None = None
    supersedes: list[str] = field(default_factory=list)
    superseded_by: str | None = None
    links: list[MemoryLink] = field(default_factory=list)
    review: MemoryReview = field(default_factory=MemoryReview)

    @classmethod
    def new(
        cls,
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
        expires_at: datetime | None = None,
        supersedes: list[str] | None = None,
        links: list[MemoryLink] | None = None,
        review: MemoryReview | None = None,
    ) -> MemoryEvent:
        timestamp = now_utc()
        event = cls(
            id=f"mem_{timestamp.strftime('%Y%m%d_%H%M%S')}_{next(_ID_COUNTER):06d}",
            created_at=timestamp,
            updated_at=timestamp,
            project=project,
            agent_profile=agent_profile,
            scope=scope,
            ring=ring,
            event_type=event_type,
            summary=summary,
            details=details,
            source=source or MemorySource(),
            tags=tags or [],
            salience=salience,
            confidence=confidence,
            sensitivity=sensitivity,
            retention=retention,
            expires_at=expires_at,
            supersedes=supersedes or [],
            links=links or [],
            review=review or MemoryReview(),
        )
        event.validate()
        return event

    def validate(self) -> None:
        if not self.summary.strip():
            raise ValidationError("summary is required")
        if not self.event_type.strip():
            raise ValidationError("event_type is required")
        if self.scope not in SCOPES:
            raise ValidationError(f"invalid scope: {self.scope}")
        if self.ring not in RINGS:
            raise ValidationError(f"invalid ring: {self.ring}")
        if self.sensitivity not in SENSITIVITY:
            raise ValidationError(f"invalid sensitivity: {self.sensitivity}")
        if self.retention not in RETENTION:
            raise ValidationError(f"invalid retention: {self.retention}")
        self.salience = _validate_score("salience", self.salience)
        self.confidence = _validate_score("confidence", self.confidence)

    def to_dict(self) -> dict[str, Any]:
        self.validate()
        return {
            "id": self.id,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
            "project": self.project,
            "agent_profile": self.agent_profile,
            "scope": self.scope,
            "ring": self.ring,
            "event_type": self.event_type,
            "summary": self.summary,
            "details": self.details,
            "source": self.source.to_dict(),
            "tags": list(self.tags),
            "salience": self.salience,
            "confidence": self.confidence,
            "sensitivity": self.sensitivity,
            "retention": self.retention,
            "expires_at": self.expires_at.isoformat() if self.expires_at else None,
            "supersedes": list(self.supersedes),
            "superseded_by": self.superseded_by,
            "links": [link.to_dict() for link in self.links],
            "review": self.review.to_dict(),
        }

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> MemoryEvent:
        event = cls(
            id=str(value.get("id") or ""),
            created_at=parse_datetime(value.get("created_at"), "created_at") or now_utc(),
            updated_at=parse_datetime(value.get("updated_at"), "updated_at") or now_utc(),
            project=value.get("project"),
            agent_profile=value.get("agent_profile"),
            scope=str(value.get("scope") or "global"),
            ring=str(value.get("ring") or "cambium"),
            event_type=str(value.get("event_type") or ""),
            summary=str(value.get("summary") or ""),
            details=str(value.get("details") or ""),
            source=MemorySource.from_dict(value.get("source")),
            tags=[str(tag) for tag in value.get("tags") or []],
            salience=float(value.get("salience", 0.5)),
            confidence=float(value.get("confidence", 0.5)),
            sensitivity=str(value.get("sensitivity") or "normal"),
            retention=str(value.get("retention") or "normal"),
            expires_at=parse_datetime(value.get("expires_at"), "expires_at"),
            supersedes=[str(item) for item in value.get("supersedes") or []],
            superseded_by=value.get("superseded_by"),
            links=[MemoryLink.from_dict(link) for link in value.get("links") or []],
            review=MemoryReview.from_dict(value.get("review")),
        )
        event.validate()
        return event
