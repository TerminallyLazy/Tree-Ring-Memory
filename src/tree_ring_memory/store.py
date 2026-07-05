from __future__ import annotations

import json
from pathlib import Path
import re
import sqlite3
from typing import Any

from tree_ring_memory.models import MemoryEvent
from tree_ring_memory.models import now_utc
from tree_ring_memory.sensitivity import SensitivityGuard


EXPORT_RECORD_TYPE = "tree_ring_memory_export"
MEMORY_EVENT_RECORD_TYPE = "memory_event"
EXPORT_SCHEMA_VERSION = 1
PLUGIN_VERSION = "0.4.0"
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
            """
        )
        self.connection.commit()

    def put(self, event: MemoryEvent) -> None:
        payload = event.to_dict()
        source = payload["source"]
        with self.connection:
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

        old.superseded_by = new_id
        with self.connection:
            self.connection.execute(
                "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
                (new_id, json.dumps(old.to_dict(), sort_keys=True), old_id),
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

    def _apply_supersedes(self, event: MemoryEvent) -> None:
        for old_id in event.supersedes:
            self.supersede(old_id, event.id)

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
