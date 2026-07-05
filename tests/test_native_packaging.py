from __future__ import annotations

import tomllib
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_native_binding_pyproject_is_extension_only():
    pyproject = tomllib.loads((PROJECT_ROOT / "bindings" / "python" / "pyproject.toml").read_text())
    maturin = pyproject["tool"]["maturin"]

    assert pyproject["project"]["name"] == "tree-ring-memory-native"
    assert pyproject["project"]["version"] == "0.7.0"
    assert maturin["module-name"] == "tree_ring_memory._tree_ring_memory_native"
    assert maturin["python-source"] == "python"


def test_main_python_package_does_not_claim_rust_cli_entrypoint():
    pyproject = tomllib.loads((PROJECT_ROOT / "pyproject.toml").read_text())

    assert "scripts" not in pyproject.get("project", {})
