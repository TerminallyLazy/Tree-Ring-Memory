#!/usr/bin/env python3
"""Build the OpenAI skills-only upload, including native Codex lifecycle hooks."""

from __future__ import annotations

import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo


PLUGIN = Path(__file__).resolve().parents[1]
PROFILE = PLUGIN / "packaging" / "codex-skills-only"
PACKAGE_ROOT = Path("tree-ring-memory")
FIXED_TIMESTAMP = (2026, 1, 1, 0, 0, 0)


def public_skill(content: str) -> str:
    """Remove our canonical block-style metadata without changing instructions.

    This handles the checked-in front matter, not arbitrary YAML. Interface
    settings come from the public profile's agents/openai.yaml instead.
    """
    if not content.startswith("---\n"):
        raise ValueError("skill must start with YAML front matter")
    frontmatter, separator, body = content[4:].partition("\n---\n")
    if not separator:
        raise ValueError("skill front matter must end before the body")
    lines = []
    in_metadata = False
    for line in frontmatter.splitlines():
        if line.startswith("metadata:"):
            if line != "metadata:":
                raise ValueError("expected canonical block-style metadata")
            in_metadata = True
        elif in_metadata and (not line.strip() or line.startswith((" ", "\t"))):
            continue
        else:
            in_metadata = False
            lines.append(line)
    return "---\n" + "\n".join(lines) + "\n---\n" + body


def write_file(
    archive: ZipFile, source: Path, destination: Path, *, data: bytes | None = None,
) -> None:
    info = ZipInfo(str(PACKAGE_ROOT / destination), FIXED_TIMESTAMP)
    info.compress_type = ZIP_DEFLATED
    info.create_system = 3
    mode = 0o100755 if source.stat().st_mode & 0o111 else 0o100644
    info.external_attr = mode << 16
    archive.writestr(info, source.read_bytes() if data is None else data)


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
                relative = path.relative_to(PLUGIN)
                if (PROFILE / relative).exists():
                    raise ValueError(f"public skill profile collides with source: {relative}")
                data = public_skill(path.read_text(encoding="utf-8")).encode("utf-8") if path.name == "SKILL.md" else None
                write_file(archive, path, relative, data=data)
        for path in sorted((PROFILE / "skills").rglob("*")):
            if path.is_file():
                write_file(archive, path, path.relative_to(PROFILE))
        for path in sorted((PLUGIN / "assets").rglob("*")):
            if path.is_file():
                write_file(archive, path, path.relative_to(PLUGIN))
        for name in ("codex-hooks.json", "codex-hook.sh"):
            path = PLUGIN / "hooks" / name
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
