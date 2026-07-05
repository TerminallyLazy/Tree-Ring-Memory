from __future__ import annotations

from pathlib import Path

from tree_ring_memory.native_backend import NativeTreeRingMemory


class TreeRingMemory:
    """Rust-native public facade."""

    @classmethod
    def open(cls, root: str | Path) -> NativeTreeRingMemory:
        return NativeTreeRingMemory.open(root)
