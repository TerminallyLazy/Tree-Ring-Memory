#!/usr/bin/env python3
"""Build the OpenAI upload artifact without repository lifecycle hooks."""

from __future__ import annotations

import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo


PLUGIN = Path(__file__).resolve().parents[1]
PROFILE = PLUGIN / "packaging" / "codex-skills-only"
PACKAGE_ROOT = Path("tree-ring-memory")
FIXED_TIMESTAMP = (2026, 1, 1, 0, 0, 0)


def write_file(archive: ZipFile, source: Path, destination: Path) -> None:
    info = ZipInfo(str(PACKAGE_ROOT / destination), FIXED_TIMESTAMP)
    info.compress_type = ZIP_DEFLATED
    info.external_attr = 0o100644 << 16
    archive.writestr(info, source.read_bytes())


def build(destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(destination, "w") as archive:
        write_file(
            archive,
            PROFILE / ".codex-plugin" / "plugin.json",
            Path(".codex-plugin/plugin.json"),
        )
        for path in sorted((PLUGIN / "skills").rglob("*")):
            if path.is_file():
                write_file(archive, path, path.relative_to(PLUGIN))
        for path in sorted((PLUGIN / "assets").rglob("*")):
            if path.is_file():
                write_file(archive, path, path.relative_to(PLUGIN))
        for name in ("LICENSE", "PRIVACY.md", "SECURITY.md", "TERMS.md"):
            write_file(archive, PLUGIN / name, Path(name))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path, help="Destination ZIP path")
    args = parser.parse_args()
    build(args.output.resolve())


if __name__ == "__main__":
    main()
