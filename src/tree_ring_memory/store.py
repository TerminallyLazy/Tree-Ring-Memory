from __future__ import annotations

from collections import Counter
import json
from pathlib import Path
import re
import sqlite3
from typing import Any
from uuid import uuid4

from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource
from tree_ring_memory.models import now_utc
from tree_ring_memory.sensitivity import SensitivityGuard


EXPORT_RECORD_TYPE = "tree_ring_memory_export"
MEMORY_EVENT_RECORD_TYPE = "memory_event"
EXPORT_SCHEMA_VERSION = 1
PLUGIN_VERSION = "0.7.0"
_PLAIN_TEXT_TERM_RE = re.compile(r"\w+")
_SEARCH_FILLER_TERMS = {
    "a",
    "an",
    "and",
    "about",
    "are",
    "for",
    "in",
    "is",
    "not",
    "of",
    "on",
    "or",
    "the",
    "to",
    "what",
}
AUDIT_TYPES = {"all", "stale", "sensitive", "low_confidence", "supersession", "contradictions"}
_DURABLE_RETENTIONS = {"durable", "user_pinned"}
_CONSOLIDATION_PERIODS = {"daily", "weekly", "monthly", "yearly", "manual"}
_CONSOLIDATION_TAG = "consolidation"


class SQLiteMemoryStore:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection
        self.connection.row_factory = sqlite3.Row
        self.migrate()

    @classmethod
    def open(cls, path: str | Path) -> SQLiteMemoryStore:
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(path, timeout=30.0)
        connection.execute("PRAGMA journal_mode=WAL;")
        connection.execute("PRAGMA busy_timeout=30000;")
        return cls(connection)

    def migrate(self) -> None:
        self.connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS memories (
              id TEXT PRIMARY KEY,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              project TEXT,
              agent_profile TEXT,
              scope TEXT NOT NULL,
              ring TEXT NOT NULL,
              event_type TEXT NOT NULL,
              summary TEXT NOT NULL,
              details TEXT NOT NULL,
              source_json TEXT NOT NULL,
              tags_json TEXT NOT NULL,
              salience REAL NOT NULL,
              confidence REAL NOT NULL,
              sensitivity TEXT NOT NULL,
              retention TEXT NOT NULL,
              expires_at TEXT,
              supersedes_json TEXT NOT NULL,
              superseded_by TEXT,
              links_json TEXT NOT NULL,
              review_json TEXT NOT NULL,
              raw_json TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
              id UNINDEXED,
              summary,
              details,
              tags,
              source_ref
            );
            CREATE TABLE IF NOT EXISTS consolidations (
              id TEXT PRIMARY KEY,
              created_at TEXT NOT NULL,
              period_type TEXT NOT NULL,
              period_key TEXT NOT NULL,
              source_memory_ids_json TEXT NOT NULL,
              output_memory_ids_json TEXT NOT NULL,
              status TEXT NOT NULL,
              notes TEXT NOT NULL
            );
            """
        )
        self.connection.commit()

    def put(self, event: MemoryEvent) -> None:
        with self.connection:
            self._put_event(event)

    def _put_event(self, event: MemoryEvent) -> None:
        payload = event.to_dict()
        source = payload["source"]
        self.connection.execute(
            """
            INSERT OR REPLACE INTO memories (
              id, created_at, updated_at, project, agent_profile, scope, ring,
              event_type, summary, details, source_json, tags_json, salience,
              confidence, sensitivity, retention, expires_at, supersedes_json,
              superseded_by, links_json, review_json, raw_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                event.id,
                payload["created_at"],
                payload["updated_at"],
                event.project,
                event.agent_profile,
                event.scope,
                event.ring,
                event.event_type,
                event.summary,
                event.details,
                json.dumps(source, sort_keys=True),
                json.dumps(event.tags, sort_keys=True),
                event.salience,
                event.confidence,
                event.sensitivity,
                event.retention,
                payload["expires_at"],
                json.dumps(event.supersedes, sort_keys=True),
                event.superseded_by,
                json.dumps([link.to_dict() for link in event.links], sort_keys=True),
                json.dumps(event.review.to_dict(), sort_keys=True),
                json.dumps(payload, sort_keys=True),
            ),
        )
        self.connection.execute("DELETE FROM memory_fts WHERE id = ?", (event.id,))
        self.connection.execute(
            "INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)",
            (event.id, event.summary, event.details, " ".join(event.tags), source.get("ref", "")),
        )

    def get(self, memory_id: str) -> MemoryEvent | None:
        row = self.connection.execute("SELECT raw_json FROM memories WHERE id = ?", (memory_id,)).fetchone()
        if row is None:
            return None
        return self._event_from_row(row)

    def list_all(self, *, include_superseded: bool = False) -> list[MemoryEvent]:
        sql = "SELECT raw_json FROM memories"
        if not include_superseded:
            sql += " WHERE superseded_by IS NULL"
        sql += " ORDER BY created_at DESC"
        rows = self.connection.execute(sql).fetchall()
        return [self._event_from_row(row) for row in rows]

    def search_text(self, query: str, *, include_superseded: bool = False) -> list[MemoryEvent]:
        if not query.strip():
            return self.list_all(include_superseded=include_superseded)

        fts_query = _format_plain_text_fts_query(query)
        if not fts_query:
            return []

        sql = """
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
            """
        if not include_superseded:
            sql += " AND memories.superseded_by IS NULL"
        sql += " ORDER BY rank"

        rows = self.connection.execute(sql, (fts_query,)).fetchall()
        return [self._event_from_row(row) for row in rows]

    def supersede(self, old_id: str, new_id: str) -> None:
        old = self.get(old_id)
        if old is None:
            return

        with self.connection:
            self._supersede_event(old, new_id)

    def _supersede_event(self, old: MemoryEvent, new_id: str) -> None:
        old.superseded_by = new_id
        self.connection.execute(
            "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
            (new_id, json.dumps(old.to_dict(), sort_keys=True), old.id),
        )

    def delete(self, memory_id: str) -> None:
        with self.connection:
            self.connection.execute("DELETE FROM memories WHERE id = ?", (memory_id,))
            self.connection.execute("DELETE FROM memory_fts WHERE id = ?", (memory_id,))

    def export_jsonl(self, *, include_sensitive: bool = False, include_superseded: bool = False) -> str:
        events = [
            event
            for event in self.list_all(include_superseded=include_superseded)
            if include_sensitive or event.sensitivity == "normal"
        ]
        header = {
            "type": EXPORT_RECORD_TYPE,
            "schema_version": EXPORT_SCHEMA_VERSION,
            "plugin_version": PLUGIN_VERSION,
            "created_at": now_utc().isoformat(),
            "memory_count": len(events),
            "sensitive_included": include_sensitive,
        }
        records = [header]
        records.extend({"type": MEMORY_EVENT_RECORD_TYPE, "memory": event.to_dict()} for event in events)
        return "".join(f"{json.dumps(record, sort_keys=True)}\n" for record in records)

    def import_jsonl(
        self,
        data: str,
        *,
        dry_run: bool = False,
        replace_existing: bool = False,
    ) -> dict[str, Any]:
        events = _normalize_import_events(_decode_jsonl(data))
        report = {
            "valid_count": len(events),
            "inserted_count": 0,
            "replaced_count": 0,
            "skipped_duplicate_count": 0,
            "dry_run": dry_run,
        }
        if dry_run:
            return report

        imported_events = []
        for event in events:
            if self.get(event.id) is None:
                self.put(event)
                imported_events.append(event)
                report["inserted_count"] += 1
            elif replace_existing:
                self.put(event)
                imported_events.append(event)
                report["replaced_count"] += 1
            else:
                report["skipped_duplicate_count"] += 1
        for event in imported_events:
            self._apply_supersedes(event)
        return report

    def audit(self, audit_type: str = "all") -> dict[str, Any]:
        audit_type = _normalize_audit_type(audit_type)
        events = self.list_all(include_superseded=True)
        findings: list[dict[str, Any]] = []

        if audit_type in {"all", "stale"}:
            findings.extend(_audit_stale(events))
        if audit_type in {"all", "sensitive"}:
            findings.extend(_audit_sensitive(events))
        if audit_type in {"all", "low_confidence"}:
            findings.extend(_audit_low_confidence(events))
        if audit_type in {"all", "supersession"}:
            findings.extend(_audit_supersession(events))
        if audit_type in {"all", "contradictions"}:
            findings.extend(_audit_contradictions(events))

        return {
            "generated_at": now_utc().isoformat(),
            "audit_type": audit_type,
            "memory_count": len(events),
            "finding_count": len(findings),
            "findings": findings,
        }

    def consolidate(
        self,
        *,
        period_type: str = "daily",
        period_key: str | None = None,
        project: str | None = None,
        dry_run: bool = False,
        force: bool = False,
    ) -> dict[str, Any]:
        period_type = _normalize_consolidation_period(period_type)
        resolved_period_key = period_key or _default_period_key(period_type)
        request = {
            "period_type": period_type,
            "period_key": resolved_period_key,
            "project": project,
            "dry_run": bool(dry_run),
            "force": bool(force),
        }
        report = _build_consolidation_report(self.list_all(), request)
        if dry_run or report["candidate_count"] == 0:
            return report

        source_ids_json = json.dumps(report["source_memory_ids"], sort_keys=True)
        existing = self._find_consolidation(period_type, resolved_period_key, source_ids_json)
        if existing is not None and not force:
            report["id"] = existing["id"]
            report["created_at"] = existing["created_at"]
            report["output_memory_ids"] = existing["output_memory_ids"]
            report["status"] = "unchanged"
            report["notes"] = "Matching consolidation already exists."
            report["outputs"] = []
            return report

        previous_outputs = self._previous_consolidation_outputs(
            period_type,
            resolved_period_key,
        ) if force else []
        output_events = [MemoryEvent.from_dict(output["memory"]) for output in report["outputs"]]
        supersession_pairs = (
            _consolidation_supersession_pairs(previous_outputs, output_events)
            if force
            else []
        )
        report["status"] = "created"
        report["notes"] = "Consolidation summaries stored."
        with self.connection:
            for event in output_events:
                self._put_event(event)
            for old, new_id in supersession_pairs:
                self._supersede_event(old, new_id)
            self._insert_consolidation_record(report, "created")
        return report

    def _apply_supersedes(self, event: MemoryEvent) -> None:
        for old_id in event.supersedes:
            self.supersede(old_id, event.id)

    def _find_consolidation(
        self,
        period_type: str,
        period_key: str,
        source_ids_json: str,
    ) -> dict[str, Any] | None:
        row = self.connection.execute(
            """
            SELECT id, created_at, output_memory_ids_json
            FROM consolidations
            WHERE period_type = ?
              AND period_key = ?
              AND source_memory_ids_json = ?
              AND status = 'created'
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (period_type, period_key, source_ids_json),
        ).fetchone()
        if row is None:
            return None
        return {
            "id": row["id"],
            "created_at": row["created_at"],
            "output_memory_ids": [str(item) for item in json.loads(row["output_memory_ids_json"])],
        }

    def _previous_consolidation_outputs(self, period_type: str, period_key: str) -> list[MemoryEvent]:
        rows = self.connection.execute(
            """
            SELECT output_memory_ids_json
            FROM consolidations
            WHERE period_type = ?
              AND period_key = ?
              AND status = 'created'
            ORDER BY created_at ASC
            """,
            (period_type, period_key),
        ).fetchall()
        output_ids: list[str] = []
        for row in rows:
            output_ids.extend(str(item) for item in json.loads(row["output_memory_ids_json"]))
        return [event for output_id in output_ids if (event := self.get(output_id)) is not None]

    def _insert_consolidation_record(self, report: dict[str, Any], status: str) -> None:
        self.connection.execute(
            """
            INSERT INTO consolidations (
              id, created_at, period_type, period_key, source_memory_ids_json,
              output_memory_ids_json, status, notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                report["id"],
                report["created_at"],
                report["period_type"],
                report["period_key"],
                json.dumps(report["source_memory_ids"], sort_keys=True),
                json.dumps(report["output_memory_ids"], sort_keys=True),
                status,
                report["notes"],
            ),
        )

    def close(self) -> None:
        self.connection.close()

    @staticmethod
    def _event_from_row(row: sqlite3.Row) -> MemoryEvent:
        return MemoryEvent.from_dict(json.loads(row["raw_json"]))


def _format_plain_text_fts_query(query: str) -> str:
    terms = [
        term
        for term in _PLAIN_TEXT_TERM_RE.findall(query)
        if term.casefold() not in _SEARCH_FILLER_TERMS
    ]
    return " AND ".join(f'"{term}"' for term in terms)


def _normalize_consolidation_period(period_type: str) -> str:
    normalized = str(period_type or "daily").strip().casefold()
    if normalized not in _CONSOLIDATION_PERIODS:
        raise ValueError(f"unsupported period_type: {period_type}")
    return normalized


def _default_period_key(period_type: str) -> str:
    now = now_utc()
    if period_type == "daily":
        return now.strftime("%Y-%m-%d")
    if period_type == "weekly":
        year, week, _weekday = now.isocalendar()
        return f"{year}-W{week:02d}"
    if period_type == "monthly":
        return now.strftime("%Y-%m")
    if period_type == "yearly":
        return now.strftime("%Y")
    return now.strftime("manual-%Y%m%dT%H%M%SZ")


def _build_consolidation_report(events: list[MemoryEvent], request: dict[str, Any]) -> dict[str, Any]:
    period_type = str(request["period_type"])
    period_key = str(request["period_key"])
    candidates = [
        event
        for event in events
        if _is_consolidation_candidate(event, period_type, period_key, request.get("project"))
    ]
    candidates.sort(key=lambda event: event.id)
    source_memory_ids = [event.id for event in candidates]
    created_at = now_utc().isoformat()

    if not candidates:
        return {
            "id": _generated_consolidation_id(),
            "created_at": created_at,
            "period_type": period_type,
            "period_key": period_key,
            "candidate_count": 0,
            "source_memory_ids": [],
            "output_memory_ids": [],
            "dry_run": bool(request["dry_run"]),
            "force": bool(request["force"]),
            "status": "dry_run" if request["dry_run"] else "empty",
            "notes": "No memories matched consolidation criteria.",
            "outputs": [],
        }

    groups: dict[tuple[str, str, str, str, str], list[MemoryEvent]] = {}
    for event in candidates:
        groups.setdefault(_consolidation_group_key(event), []).append(event)

    outputs = [
        _build_consolidation_output(key, group, period_type, period_key)
        for key, group in sorted(groups.items(), key=lambda item: item[0])
    ]
    output_memory_ids = [output["memory"]["id"] for output in outputs]
    return {
        "id": _generated_consolidation_id(),
        "created_at": created_at,
        "period_type": period_type,
        "period_key": period_key,
        "candidate_count": len(source_memory_ids),
        "source_memory_ids": source_memory_ids,
        "output_memory_ids": output_memory_ids,
        "dry_run": bool(request["dry_run"]),
        "force": bool(request["force"]),
        "status": "dry_run" if request["dry_run"] else "planned",
        "notes": "Consolidation plan generated.",
        "outputs": outputs,
    }


def _is_consolidation_candidate(
    event: MemoryEvent,
    period_type: str,
    period_key: str,
    project: str | None,
) -> bool:
    if event.superseded_by is not None:
        return False
    if event.event_type == "summary" and event.source.type == "consolidation":
        return False
    if project is not None and event.project != project:
        return False
    if _effective_sensitivity(event) == "secret":
        return False
    if period_type != "manual" and _event_period_key(event, period_type) != period_key:
        return False
    return (
        event.salience >= 0.45
        or event.ring in {"heartwood", "scar", "seed"}
        or event.retention in _DURABLE_RETENTIONS
    )


def _event_period_key(event: MemoryEvent, period_type: str) -> str:
    if period_type == "daily":
        return event.created_at.strftime("%Y-%m-%d")
    if period_type == "weekly":
        year, week, _weekday = event.created_at.isocalendar()
        return f"{year}-W{week:02d}"
    if period_type == "monthly":
        return event.created_at.strftime("%Y-%m")
    if period_type == "yearly":
        return event.created_at.strftime("%Y")
    return "manual"


def _consolidation_group_key(event: MemoryEvent) -> tuple[str, str, str, str, str]:
    sensitivity = _effective_sensitivity(event)
    return (
        event.project or "",
        event.scope,
        event.ring,
        event.event_type,
        "normal" if sensitivity == "normal" else "sensitive",
    )


def _build_consolidation_output(
    key: tuple[str, str, str, str, str],
    events: list[MemoryEvent],
    period_type: str,
    period_key: str,
) -> dict[str, Any]:
    project, scope, ring, event_type, sensitivity_bucket = key
    source_memory_ids = [event.id for event in sorted(events, key=lambda event: event.id)]
    project_value = project or None
    project_label = project or "global"
    sensitive = sensitivity_bucket == "sensitive"
    if sensitive:
        summary = f"Consolidated {len(events)} sensitive memory group requiring review."
    else:
        summary = (
            f"Consolidated {len(events)} {event_type} memory group "
            f"for project {project_label} in {ring} ring."
        )
    memory = MemoryEvent.new(
        summary=summary,
        event_type="summary",
        scope=scope,
        ring=_consolidation_output_ring(period_type, ring, events),
        project=project_value,
        details=(
            f"Period: {period_type}:{period_key}; "
            f"source_count={len(events)}; source_ids={','.join(source_memory_ids)}"
        ),
        source=MemorySource(type="consolidation", ref=f"{period_type}:{period_key}"),
        tags=_top_safe_tags(events),
        salience=_average(event.salience for event in events),
        confidence=_average(event.confidence for event in events),
        sensitivity="private" if sensitive else "normal",
        retention="normal",
        links=[MemoryLink(type="memory", target=memory_id) for memory_id in source_memory_ids],
        review=MemoryReview(
            needs_review=sensitive,
            review_reason="Sensitive memories contributed to this consolidation." if sensitive else None,
        ),
    )
    return {
        "memory": memory.to_dict(),
        "source_memory_ids": source_memory_ids,
    }


def _consolidation_output_ring(period_type: str, ring: str, events: list[MemoryEvent]) -> str:
    if ring in {"scar", "seed"}:
        return ring
    if ring == "heartwood" and _average(event.confidence for event in events) >= 0.75:
        return "heartwood"
    if period_type in {"daily", "manual"}:
        return "outer"
    return "inner"


def _top_safe_tags(events: list[MemoryEvent]) -> list[str]:
    guard = SensitivityGuard(block_secret_storage=False)
    counts: Counter[str] = Counter()
    for event in events:
        for tag in event.tags:
            if guard.inspect(tag).sensitivity == "normal":
                counts[str(tag)] += 1
    ranked = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
    tags = {tag for tag, _count in ranked[:5]}
    tags.add(_CONSOLIDATION_TAG)
    return sorted(tags)


def _effective_sensitivity(event: MemoryEvent) -> str:
    guard = SensitivityGuard(block_secret_storage=False)
    detected = "normal"
    for value in _event_public_text_values(event):
        result = guard.inspect(value)
        if result.sensitivity == "secret":
            return "secret"
        if detected == "normal" and result.sensitivity != "normal":
            detected = result.sensitivity
    if event.sensitivity == "secret":
        return "secret"
    if event.sensitivity != "normal":
        return event.sensitivity
    return detected


def _event_public_text_values(event: MemoryEvent) -> list[str]:
    values = [
        event.id,
        event.created_at.isoformat(),
        event.updated_at.isoformat(),
        event.project or "",
        event.agent_profile or "",
        event.scope,
        event.ring,
        event.event_type,
        event.summary,
        event.details,
        event.source.type,
        event.source.ref,
        event.source.quote,
        event.sensitivity,
        event.retention,
        event.expires_at.isoformat() if event.expires_at else "",
        event.superseded_by or "",
        event.review.review_reason or "",
        event.review.reviewed_at or "",
        event.review.reviewed_by or "",
    ]
    values.extend(event.tags)
    values.extend(event.supersedes)
    for link in event.links:
        values.append(link.type)
        values.append(link.target)
    return values


def _consolidation_supersession_pairs(
    previous_outputs: list[MemoryEvent],
    output_events: list[MemoryEvent],
) -> list[tuple[MemoryEvent, str]]:
    if not output_events:
        return []
    pairs: list[tuple[MemoryEvent, str]] = []
    for index, old in enumerate(previous_outputs):
        target = _best_consolidation_replacement(old, output_events)
        if target is None:
            target = output_events[index % len(output_events)]
        pairs.append((old, target.id))
    return pairs


def _best_consolidation_replacement(
    old: MemoryEvent,
    output_events: list[MemoryEvent],
) -> MemoryEvent | None:
    old_targets = _memory_link_targets(old)
    if not old_targets:
        return None
    best_overlap = 0
    best: MemoryEvent | None = None
    for candidate in output_events:
        overlap = len(old_targets & _memory_link_targets(candidate))
        if overlap == 0:
            continue
        if (
            best is None
            or overlap > best_overlap
            or (overlap == best_overlap and candidate.id < best.id)
        ):
            best_overlap = overlap
            best = candidate
    return best


def _memory_link_targets(event: MemoryEvent) -> set[str]:
    return {link.target for link in event.links if link.type == "memory"}


def _average(values: Any) -> float:
    total = 0.0
    count = 0
    for value in values:
        total += float(value)
        count += 1
    if count == 0:
        return 0.5
    return min(1.0, max(0.0, total / count))


def _generated_consolidation_id() -> str:
    return f"con_{now_utc().strftime('%Y%m%d_%H%M%S')}_{uuid4().hex[:12]}"


def _normalize_audit_type(audit_type: str) -> str:
    normalized = str(audit_type or "all").strip().casefold()
    if normalized not in AUDIT_TYPES:
        raise ValueError(f"unsupported audit_type: {audit_type}")
    return normalized


def _audit_stale(events: list[MemoryEvent]) -> list[dict[str, Any]]:
    now = now_utc()
    findings: list[dict[str, Any]] = []
    for event in events:
        if event.expires_at is None:
            continue
        if event.expires_at.tzinfo is None or event.expires_at.utcoffset() is None:
            findings.append(
                _finding(
                    audit_type="stale",
                    severity="medium",
                    memory_id=event.id,
                    finding="Memory has an invalid expires_at timestamp.",
                    recommended_action=(
                        "Review the memory and set a valid ISO-8601 expires_at value or redact it."
                    ),
                    tags=["retention", "expiry"],
                )
            )
            continue
        if event.expires_at <= now:
            findings.append(
                _finding(
                    audit_type="stale",
                    severity="medium",
                    memory_id=event.id,
                    finding="Memory expires_at is in the past.",
                    recommended_action="Review, delete, redact, or refresh this memory.",
                    tags=["retention", "expiry"],
                )
            )
    return findings


def _audit_sensitive(events: list[MemoryEvent]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for event in events:
        if event.sensitivity == "normal":
            continue
        if event.sensitivity == "secret":
            findings.append(
                _finding(
                    audit_type="sensitive",
                    severity="critical",
                    memory_id=event.id,
                    finding="Secret-like memory is retained.",
                    recommended_action="Redact or delete this memory.",
                    tags=["privacy", "secret"],
                )
            )
        if event.retention in _DURABLE_RETENTIONS:
            findings.append(
                _finding(
                    audit_type="sensitive",
                    severity="high",
                    memory_id=event.id,
                    finding="Sensitive memory has durable retention.",
                    recommended_action="Review whether this memory should be redacted or assigned an expiry.",
                    tags=["privacy", "retention"],
                )
            )
        if event.expires_at is None:
            findings.append(
                _finding(
                    audit_type="sensitive",
                    severity="medium",
                    memory_id=event.id,
                    finding="Sensitive memory is retained without an expiry.",
                    recommended_action="Set expires_at, redact, or delete this memory.",
                    tags=["privacy", "expiry"],
                )
            )
    return findings


def _audit_low_confidence(events: list[MemoryEvent]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for event in events:
        if event.ring == "heartwood" and event.confidence < 0.75:
            findings.append(
                _finding(
                    audit_type="low_confidence",
                    severity="high",
                    memory_id=event.id,
                    finding="Heartwood memory has low confidence.",
                    recommended_action="Review evidence before treating this as durable truth.",
                    tags=["confidence", "heartwood"],
                )
            )
        if event.retention in _DURABLE_RETENTIONS and event.confidence < 0.5:
            findings.append(
                _finding(
                    audit_type="low_confidence",
                    severity="medium",
                    memory_id=event.id,
                    finding="Durable memory has very low confidence.",
                    recommended_action="Review, demote, or supersede this memory.",
                    tags=["confidence", "retention"],
                )
            )
    return findings


def _audit_supersession(events: list[MemoryEvent]) -> list[dict[str, Any]]:
    event_ids = {event.id for event in events}
    by_id = {event.id: event for event in events}
    findings: list[dict[str, Any]] = []
    for event in events:
        if event.id in event.supersedes:
            findings.append(
                _finding(
                    audit_type="supersession",
                    severity="high",
                    memory_id=event.id,
                    finding="Memory supersedes itself.",
                    recommended_action="Remove the self-supersession link.",
                    tags=["supersession", "integrity"],
                )
            )
        if event.superseded_by is not None and event.superseded_by not in event_ids:
            findings.append(
                _finding(
                    audit_type="supersession",
                    severity="high",
                    memory_id=event.id,
                    related_memory_id=event.superseded_by,
                    finding="Memory points to a missing superseded_by target.",
                    recommended_action="Import, restore, or clear the missing supersession target.",
                    tags=["supersession", "integrity"],
                )
            )
        for superseded_id in event.supersedes:
            if superseded_id not in event_ids:
                findings.append(
                    _finding(
                        audit_type="supersession",
                        severity="medium",
                        memory_id=event.id,
                        related_memory_id=superseded_id,
                        finding="Memory supersedes a missing memory.",
                        recommended_action="Import, restore, or remove the missing supersedes reference.",
                        tags=["supersession", "integrity"],
                    )
                )
                continue
            superseded = by_id[superseded_id]
            if superseded.superseded_by != event.id:
                findings.append(
                    _finding(
                        audit_type="supersession",
                        severity="medium",
                        memory_id=event.id,
                        related_memory_id=superseded.id,
                        finding="Supersedes link is missing a reciprocal superseded_by pointer.",
                        recommended_action="Repair the supersession chain.",
                        tags=["supersession", "integrity"],
                    )
                )
    return findings


def _audit_contradictions(events: list[MemoryEvent]) -> list[dict[str, Any]]:
    buckets: dict[tuple[str | None, str, str, str, str], dict[str, list[MemoryEvent]]] = {}
    for event in events:
        directive = _directive_phrase(event.summary)
        if directive is None:
            continue
        action, phrase = directive
        for tag in event.tags:
            key = (event.project, event.scope, event.event_type, tag, phrase)
            bucket = buckets.setdefault(key, {"use": [], "avoid": []})
            bucket[action].append(event)

    findings: list[dict[str, Any]] = []
    emitted_pairs: set[tuple[str, str]] = set()
    for bucket in buckets.values():
        for use_memory in bucket["use"]:
            for avoid_memory in bucket["avoid"]:
                pair_key = tuple(sorted((use_memory.id, avoid_memory.id)))
                if pair_key in emitted_pairs:
                    continue
                emitted_pairs.add(pair_key)
                findings.append(
                    _finding(
                        audit_type="contradictions",
                        severity="medium",
                        memory_id=use_memory.id,
                        related_memory_id=avoid_memory.id,
                        finding="Memories contain contradictory use/avoid guidance.",
                        recommended_action=(
                            "Review the pair and supersede the stale or incorrect memory."
                        ),
                        tags=["contradiction", "review"],
                    )
                )
    return findings


def _directive_phrase(summary: str) -> tuple[str, str] | None:
    normalized = _normalize_contradiction_phrase(summary)
    if normalized.startswith("use "):
        return "use", normalized.removeprefix("use ")
    if normalized.startswith("avoid "):
        return "avoid", normalized.removeprefix("avoid ")
    return None


def _normalize_contradiction_phrase(value: str) -> str:
    return " ".join(_PLAIN_TEXT_TERM_RE.findall(value.casefold()))


def _finding(
    *,
    audit_type: str,
    severity: str,
    memory_id: str,
    finding: str,
    recommended_action: str,
    tags: list[str],
    related_memory_id: str | None = None,
) -> dict[str, Any]:
    return {
        "audit_type": audit_type,
        "severity": severity,
        "memory_id": memory_id,
        "related_memory_id": related_memory_id,
        "finding": finding,
        "recommended_action": recommended_action,
        "tags": tags,
    }


def _decode_jsonl(data: str) -> list[MemoryEvent]:
    events: list[MemoryEvent] = []
    for index, line in enumerate(data.splitlines(), start=1):
        trimmed = line.strip()
        if not trimmed:
            continue
        try:
            value = json.loads(trimmed)
        except json.JSONDecodeError as exc:
            raise ValueError(f"line {index}: invalid json: {exc}") from exc

        record_type = value.get("type") if isinstance(value, dict) else None
        try:
            if record_type == EXPORT_RECORD_TYPE:
                _validate_export_header(value)
                continue
            if record_type == MEMORY_EVENT_RECORD_TYPE:
                events.append(MemoryEvent.from_dict(value.get("memory") or {}))
                continue
            events.append(MemoryEvent.from_dict(value))
        except Exception as exc:
            raise ValueError(f"line {index}: {exc}") from exc
    return events


def _validate_export_header(value: dict[str, Any]) -> None:
    required = {
        "type",
        "schema_version",
        "plugin_version",
        "created_at",
        "memory_count",
        "sensitive_included",
    }
    missing = sorted(required.difference(value))
    if missing:
        raise ValueError(f"missing export header fields: {', '.join(missing)}")
    schema_version = value.get("schema_version")
    if schema_version != EXPORT_SCHEMA_VERSION:
        raise ValueError(f"unsupported export schema version {schema_version}")
    if not isinstance(value.get("plugin_version"), str) or not value["plugin_version"]:
        raise ValueError("plugin_version must be a non-empty string")
    if not isinstance(value.get("created_at"), str) or not value["created_at"]:
        raise ValueError("created_at must be a non-empty string")
    if not isinstance(value.get("memory_count"), int) or value["memory_count"] < 0:
        raise ValueError("memory_count must be a non-negative integer")
    if not isinstance(value.get("sensitive_included"), bool):
        raise ValueError("sensitive_included must be a boolean")


def _normalize_import_events(events: list[MemoryEvent]) -> list[MemoryEvent]:
    return [_normalize_import_event(event) for event in events]


def _normalize_import_event(event: MemoryEvent) -> MemoryEvent:
    guard = SensitivityGuard()
    detected = "normal"
    for value in _event_text_values(event):
        result = guard.check_or_raise(value or "")
        if detected == "normal" and result.sensitivity != "normal":
            detected = result.sensitivity
    if event.sensitivity == "normal" and detected != "normal":
        event.sensitivity = detected
    event.validate()
    return event


def _event_text_values(event: MemoryEvent) -> list[str | None]:
    values: list[str | None] = [
        event.id,
        event.created_at.isoformat(),
        event.updated_at.isoformat(),
        event.project,
        event.agent_profile,
        event.scope,
        event.ring,
        event.event_type,
        event.summary,
        event.details,
        event.source.type,
        event.source.ref,
        event.source.quote,
        event.sensitivity,
        event.retention,
        event.expires_at.isoformat() if event.expires_at else None,
        event.superseded_by,
        event.review.review_reason,
        event.review.reviewed_at,
        event.review.reviewed_by,
    ]
    values.extend(event.tags)
    values.extend(event.supersedes)
    for link in event.links:
        values.extend([link.type, link.target])
    return values
