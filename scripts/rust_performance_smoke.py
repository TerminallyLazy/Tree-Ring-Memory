from __future__ import annotations

import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    count = "10000"
    if "--count" in sys.argv:
        index = sys.argv.index("--count")
        count = sys.argv[index + 1]
    subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "--release",
            "-p",
            "tree-ring-memory-sqlite",
            "--example",
            "performance_smoke",
            "--",
            count,
        ],
        cwd=PROJECT_ROOT,
        check=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
