from __future__ import annotations

from tree_ring_memory.models import MemoryEvent, MemoryLink, MemoryReview, MemorySource, ValidationError


__all__ = [
    "TreeRingMemory",
    "MemoryEvent",
    "MemoryLink",
    "MemoryReview",
    "MemorySource",
    "ValidationError",
]


def __getattr__(name: str):
    if name == "TreeRingMemory":
        from tree_ring_memory.api import TreeRingMemory

        return TreeRingMemory
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
