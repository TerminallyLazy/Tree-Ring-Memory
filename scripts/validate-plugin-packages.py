#!/usr/bin/env python3
"""Validate the repo-scoped Codex and Claude Code plugin packages."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / "plugins" / "tree-ring-memory"
UNSAFE_TOKEN_EXPORT = "export TREE_RING_COORDINATOR_TOKEN='<"
CANONICAL_ISSUES = "https://github.com/TerminallyLazy/Tree-Ring-Memory/issues"
CANONICAL_ADVISORY = "https://github.com/TerminallyLazy/Tree-Ring-Memory/security/advisories/new"


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise SystemExit(f"{path.relative_to(ROOT)} must contain a JSON object")
    return value


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def require_markers(path: Path, markers: list[str]) -> None:
    text = " ".join(path.read_text(encoding="utf-8").split())
    missing = [marker for marker in markers if " ".join(marker.split()) not in text]
    require(not missing, f"{path.relative_to(ROOT)} is missing: {', '.join(missing)}")


def validate_marketplace_source(source: str, expected: str) -> Path:
    require(source == expected, f"marketplace source must be {expected}")
    require(source.startswith("./") and ".." not in Path(source).parts, "unsafe marketplace source")
    resolved = (ROOT / source).resolve()
    require(resolved == PLUGIN.resolve(), "marketplace source does not resolve to the plugin")
    return resolved


def validate_codex() -> None:
    marketplace = load_json(ROOT / ".agents" / "plugins" / "marketplace.json")
    require(marketplace.get("name") == "tree-ring-memory", "Codex marketplace name is stale")
    require(
        marketplace.get("interface", {}).get("displayName") == "Tree Ring Memory",
        "Codex marketplace display name is stale",
    )
    entries = marketplace.get("plugins")
    require(isinstance(entries, list) and len(entries) == 1, "Codex marketplace must contain one plugin")
    entry = entries[0]
    require(entry.get("name") == "tree-ring-memory", "Codex marketplace plugin name is stale")
    source = entry.get("source")
    require(isinstance(source, dict) and source.get("source") == "local", "Codex source must be local")
    validate_marketplace_source(source.get("path", ""), "./plugins/tree-ring-memory")
    require(entry.get("policy") == {"installation": "AVAILABLE", "authentication": "ON_INSTALL"}, "Codex policy is incomplete")
    require(entry.get("category") == "Developer Tools", "Codex category is stale")

    manifest = load_json(PLUGIN / ".codex-plugin" / "plugin.json")
    require(manifest.get("name") == "tree-ring-memory", "Codex manifest name is stale")
    require(manifest.get("version") == "0.3.2", "Codex manifest version is stale")
    require(manifest.get("skills") == "./skills/", "Codex skills path is stale")
    for unsupported in ("mcpServers", "apps", "hooks"):
        require(unsupported not in manifest, f"skills-only Codex plugin must not declare {unsupported}")
    interface = manifest.get("interface")
    require(isinstance(interface, dict), "Codex interface metadata is required")
    require("screenshots" not in interface, "skills-only Codex ZIP must not declare screenshots")
    for asset_key in ("composerIcon", "logo"):
        asset = interface.get(asset_key)
        require(isinstance(asset, str) and asset.startswith("./assets/"), f"Codex {asset_key} path is invalid")
        require((PLUGIN / asset).is_file(), f"Codex {asset_key} asset is missing")


def validate_claude() -> None:
    marketplace = load_json(ROOT / ".claude-plugin" / "marketplace.json")
    require(marketplace.get("name") == "tree-ring-memory", "Claude marketplace name is stale")
    require(marketplace.get("version") == "0.3.1", "Claude marketplace version is stale")
    require(isinstance(marketplace.get("owner"), dict), "Claude marketplace owner is required")
    entries = marketplace.get("plugins")
    require(isinstance(entries, list) and len(entries) == 1, "Claude marketplace must contain one plugin")
    entry = entries[0]
    require(entry.get("name") == "tree-ring-memory", "Claude marketplace plugin name is stale")
    validate_marketplace_source(entry.get("source", ""), "./plugins/tree-ring-memory")
    require("version" not in entry, "Claude marketplace entry must defer to plugin.json version")

    manifest = load_json(PLUGIN / ".claude-plugin" / "plugin.json")
    require(manifest.get("name") == "tree-ring-memory", "Claude manifest name is stale")
    require(manifest.get("version") == marketplace.get("version"), "Claude versions are not synchronized")
    require(manifest.get("skills") == "./skills/", "Claude skills path is stale")

    expected_commands = {
        "tree-ring-audit.md",
        "tree-ring-capture.md",
        "tree-ring-certify.md",
        "tree-ring-dox-sync.md",
        "tree-ring-recall.md",
        "tree-ring-status.md",
    }
    actual_commands = {path.name for path in (PLUGIN / "commands").glob("*.md")}
    require(actual_commands == expected_commands, "Claude command package is incomplete")


def validate_shared_contract() -> None:
    skill = PLUGIN / "skills" / "tree-ring-memory" / "SKILL.md"
    require_markers(
        skill,
        [
            "Runtime Preflight",
            "0.14.0 or newer",
            "DOX Contract Flow",
            "tree-ring dox sync --source-root <path> --dry-run",
            "Certification Boundary",
            "tree-ring integrations certify --source-root .",
            "tree-ring recall-quality --source-root .",
            "scripts/certify-tree-ring.sh",
            "does not run certification",
            "TREE_RING_COORDINATOR_TOKEN",
            "history-safe, no-echo",
            "configured-awaiting-proof",
            "needs-plugin",
            "same-host local-filesystem processes",
            "schema v3",
        ],
    )
    require_markers(
        PLUGIN / "commands" / "tree-ring-dox-sync.md",
        ["--dry-run", "Current source contracts are authoritative", "must not rewrite root or child `AGENTS.md` files"],
    )
    require_markers(
        PLUGIN / "commands" / "tree-ring-certify.md",
        ["tree-ring integrations certify", "tree-ring recall-quality", "repository-only", "does not execute the suite"],
    )
    require((ROOT / "scripts" / "certify-tree-ring.sh").is_file(), "source certification script is missing")
    require(not (PLUGIN / "scripts" / "certify-tree-ring.sh").exists(), "source certification suite must not be bundled")
    for filename in ("LICENSE", "PRIVACY.md", "SECURITY.md", "TERMS.md", "README.md"):
        require((PLUGIN / filename).is_file(), f"plugin {filename} is missing")
    require_markers(PLUGIN / "SECURITY.md", [CANONICAL_ADVISORY, CANONICAL_ISSUES])
    require_markers(PLUGIN / "PRIVACY.md", [CANONICAL_ISSUES])
    require_markers(PLUGIN / "TERMS.md", [CANONICAL_ISSUES])
    guidance_paths = [
        ROOT / "README.md",
        ROOT / "skills" / "tree-ring-memory" / "SKILL.md",
        ROOT / "templates" / "dox" / "AGENTS.md",
        ROOT / "docs" / "integrations" / "agent-skill.md",
        ROOT / "docs" / "protocol" / "memory-event.md",
        ROOT / "crates" / "tree-ring-memory-cli" / "src" / "agent_awareness.rs",
        skill,
    ]
    for path in guidance_paths:
        text = path.read_text(encoding="utf-8")
        require(UNSAFE_TOKEN_EXPORT not in text, f"unsafe token export remains in {path.relative_to(ROOT)}")
        require("history-safe, no-echo" in text, f"safe token guidance is missing from {path.relative_to(ROOT)}")
    for path in PLUGIN.rglob("*"):
        if path.is_file() and path.suffix in {".md", ".json"}:
            text = path.read_text(encoding="utf-8")
            require("[TODO:" not in text and "Local developer" not in text, f"placeholder remains in {path.relative_to(ROOT)}")
            require(
                "tree-ring-memory-codex-plugin/issues" not in text,
                f"stale support URL remains in {path.relative_to(ROOT)}",
            )


def main() -> None:
    validate_codex()
    validate_claude()
    validate_shared_contract()
    print("Tree Ring Memory Codex and Claude plugin packages validated")


if __name__ == "__main__":
    main()
