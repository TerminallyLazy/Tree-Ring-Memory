from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import venv
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    subprocess.run(command, cwd=cwd, env=env, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Build and import the optional native binding in a temporary venv.")
    parser.add_argument(
        "--install-maturin",
        action="store_true",
        help="install maturin into the temporary venv before running the smoke",
    )
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="tree-ring-native-smoke-") as tmp:
        venv_dir = Path(tmp) / "venv"
        venv.EnvBuilder(with_pip=True).create(venv_dir)
        bin_dir = venv_dir / ("Scripts" if sys.platform == "win32" else "bin")
        python = bin_dir / ("python.exe" if sys.platform == "win32" else "python")
        env = os.environ.copy()
        env["VIRTUAL_ENV"] = str(venv_dir)
        env["PATH"] = f"{bin_dir}{os.pathsep}{env.get('PATH', '')}"

        if args.install_maturin:
            run([str(python), "-m", "pip", "install", "maturin>=1.7,<2"])
        else:
            probe = subprocess.run([str(python), "-m", "maturin", "--version"], check=False)
            if probe.returncode != 0:
                print("maturin is not installed in the temporary venv; rerun with --install-maturin", file=sys.stderr)
                return 2

        run([str(python), "-m", "pip", "install", "-e", str(PROJECT_ROOT)])
        run([str(python), "-m", "maturin", "develop"], cwd=PROJECT_ROOT / "bindings" / "python", env=env)
        run(
            [
                str(python),
                "-c",
                (
                    "import tempfile; "
                    "from pathlib import Path; "
                    "from tree_ring_memory import TreeRingMemory, NativeTreeRingMemory; "
                    "import tree_ring_memory._tree_ring_memory_native as native; "
                    "assert TreeRingMemory.__module__ == 'tree_ring_memory.api'; "
                    "assert NativeTreeRingMemory.__module__ == 'tree_ring_memory.native_backend'; "
                    "memory = TreeRingMemory.open(Path(tempfile.mkdtemp()) / '.tree-ring'); "
                    "assert memory.backend_name == 'rust-native'; "
                    "event = memory.remember(summary='Native default facade works.', event_type='lesson'); "
                    "results = memory.recall('native facade'); "
                    "assert results and results[0].memory.id == event.id; "
                    "print(native.native_version())"
                ),
            ]
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
