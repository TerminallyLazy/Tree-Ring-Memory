# Tree Ring Memory Native Python Binding

This package builds only the PyO3 extension module:

```text
tree_ring_memory._tree_ring_memory_native
```

It intentionally does not package `../../src` or own the public
`tree_ring_memory` Python package. Install the main package separately, then
install this native extension into the same environment.

When installed, the public `TreeRingMemory.open()` facade uses this Rust-native
backend. Without it, `TreeRingMemory.open()` fails with a clear install/build
hint. The public package does not include a Python-owned runtime.

Development build:

```bash
pip install -e ../..
maturin develop
```

Repository tests do not require maturin. The extension target can be checked
with:

```bash
cargo build -p tree-ring-memory-python --features extension-module
```
