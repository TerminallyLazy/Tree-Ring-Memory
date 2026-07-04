from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Sequence

from tree_ring_memory import TreeRingMemory


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="tree-ring",
        description="Local tree-ring memory for AI agents.",
    )
    parser.add_argument("--root", default=".tree-ring", help="memory store root")

    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("init", help="initialize a local memory store")

    remember = subparsers.add_parser("remember", help="store a memory")
    remember.add_argument("summary")
    remember.add_argument("--event-type", required=True)
    remember.add_argument("--ring", default="cambium")
    remember.add_argument("--scope", default="global")
    remember.add_argument("--project")
    remember.add_argument("--tag", action="append", default=[])

    recall = subparsers.add_parser("recall", help="recall memories")
    recall.add_argument("query")
    recall.add_argument("--project")
    recall.add_argument("--limit", type=int, default=8)
    recall.add_argument("--include-sensitive", action="store_true")

    forget = subparsers.add_parser("forget", help="delete or redact a memory")
    forget.add_argument("memory_id")
    forget.add_argument("--mode", choices=["delete", "redact"], default="delete")
    forget.add_argument("--reason", required=True)

    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    memory = TreeRingMemory.open(Path(args.root))

    if args.command == "init":
        print(f"Tree Ring Memory initialized at {memory.root}")
        print("No cloud sync; secret-like memory is blocked by default.")
        return 0

    if args.command == "remember":
        try:
            event = memory.remember(
                summary=args.summary,
                event_type=args.event_type,
                ring=args.ring,
                scope=args.scope,
                project=args.project,
                tags=args.tag,
            )
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        print(event.id)
        return 0

    if args.command == "recall":
        results = memory.recall(
            args.query,
            project=args.project,
            limit=args.limit,
            include_sensitive=args.include_sensitive,
        )
        for result in results:
            event = result.memory
            print(f"{event.id} [{event.ring}] {event.summary} score={result.score:.3f}")
        return 0

    if args.command == "forget":
        try:
            memory.forget(args.memory_id, mode=args.mode, reason=args.reason)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        print(f"Tree Ring Memory forget complete: {args.memory_id}")
        return 0

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
