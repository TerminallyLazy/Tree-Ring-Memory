from __future__ import annotations

import json
from pathlib import Path
import re
import sqlite3

from tree_ring_memory.models import MemoryEvent


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
        connection = sqlite3.connect(path)
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
