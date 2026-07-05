# Tree Ring Memory CPython Extension

This directory is an optional Rust PyO3 crate. It builds only the CPython
extension module:

```text
tree_ring_memory._tree_ring_memory_native
```

It does not depend on a root Python package or Python wrapper modules. The
canonical Tree Ring Memory runtime is the Rust CLI and Rust crates.

Development build:

```bash
maturin develop
```

Repository checks do not require maturin. The extension target can be checked
from the repo root with:

```bash
cargo build -p tree-ring-memory-python --features extension-module
```
