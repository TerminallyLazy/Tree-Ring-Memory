#!/usr/bin/env python3
"""Validate the repo-scoped Codex and Claude Code plugin packages."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / "plugins" / "tree-ring-memory"
CODEX_SKILLS_ONLY = PLUGIN / "packaging" / "codex-skills-only"
CODEX_SKILLS_BUILDER = PLUGIN / "packaging" / "build-codex-skills-only.py"
LIFECYCLE_EVENTS = {"SessionStart", "SubagentStart", "Stop", "SubagentStop"}
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
    require(manifest.get("version") == "0.3.4", "Codex manifest version is stale")
    require(manifest.get("skills") == "./skills/", "Codex skills path is stale")
    require(manifest.get("hooks") == "./hooks/codex-hooks.json", "Codex lifecycle hook path is stale")
    for unsupported in ("mcpServers", "apps"):
        require(unsupported not in manifest, f"repository Codex plugin must not declare {unsupported}")
    interface = manifest.get("interface")
    require(isinstance(interface, dict), "Codex interface metadata is required")
    require("screenshots" not in interface, "skills-only Codex ZIP must not declare screenshots")
    for asset_key in ("composerIcon", "logo"):
        asset = interface.get(asset_key)
        require(isinstance(asset, str) and asset.startswith("./assets/"), f"Codex {asset_key} path is invalid")
        require((PLUGIN / asset).is_file(), f"Codex {asset_key} asset is missing")

    validate_hook_config(
        PLUGIN / "hooks" / "codex-hooks.json",
        command='"${PLUGIN_ROOT}/hooks/codex-hook.sh"',
        expect_exec_form=False,
    )
    validate_hook_script(PLUGIN / "hooks" / "codex-hook.sh", "codex")


def validate_claude() -> None:
    marketplace = load_json(ROOT / ".claude-plugin" / "marketplace.json")
    require(marketplace.get("name") == "tree-ring-memory", "Claude marketplace name is stale")
    require(marketplace.get("version") == "0.3.3", "Claude marketplace version is stale")
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
    require(manifest.get("hooks") == "./hooks/claude-hooks.json", "Claude lifecycle hook path is stale")

    expected_commands = {
        "tree-ring-audit.md",
        "tree-ring-capture.md",
        "tree-ring-certify.md",
        "tree-ring-dox-sync.md",
        "tree-ring-recall.md",
        "tree-ring-status.md",
        "tree-ring-update.md",
    }
    actual_commands = {path.name for path in (PLUGIN / "commands").glob("*.md")}
    require(actual_commands == expected_commands, "Claude command package is incomplete")

    validate_hook_config(
        PLUGIN / "hooks" / "claude-hooks.json",
        command="${CLAUDE_PLUGIN_ROOT}/hooks/claude-hook.sh",
        expect_exec_form=True,
    )
    validate_hook_script(PLUGIN / "hooks" / "claude-hook.sh", "claude-code")


def validate_hook_config(path: Path, *, command: str, expect_exec_form: bool) -> None:
    config = load_json(path)
    events = config.get("hooks")
    require(isinstance(events, dict), f"{path.relative_to(ROOT)} hooks object is required")
    require(set(events) == LIFECYCLE_EVENTS, f"{path.relative_to(ROOT)} must register the exact lifecycle contract")

    for event in sorted(LIFECYCLE_EVENTS):
        groups = events[event]
        require(isinstance(groups, list) and len(groups) == 1, f"{path.relative_to(ROOT)} {event} group is invalid")
        require("matcher" not in groups[0], f"{path.relative_to(ROOT)} {event} must handle every start source")
        handlers = groups[0].get("hooks")
        require(isinstance(handlers, list) and len(handlers) == 1, f"{path.relative_to(ROOT)} {event} handler is invalid")
        handler = handlers[0]
        require(handler.get("type") == "command", f"{path.relative_to(ROOT)} {event} must use a command hook")
        require(handler.get("command") == command, f"{path.relative_to(ROOT)} {event} command is stale")
        require(handler.get("timeout") == 10, f"{path.relative_to(ROOT)} {event} timeout must remain bounded")
        require(handler.get("async") in (None, False), f"{path.relative_to(ROOT)} {event} must not run in the background")
        if expect_exec_form:
            require(handler.get("args") == [], f"{path.relative_to(ROOT)} {event} must use safe exec form")
        else:
            require("args" not in handler, f"{path.relative_to(ROOT)} {event} uses unsupported Codex args")
            require(
                handler.get("additionalContextLimit") == 6000,
                f"{path.relative_to(ROOT)} {event} context limit is stale",
            )


def validate_hook_script(path: Path, harness: str) -> None:
    text = path.read_text(encoding="utf-8")
    require(os.access(path, os.X_OK), f"{path.relative_to(ROOT)} must be executable")
    require(".tree-ring/bin/tree-ring" in text, f"{path.relative_to(ROOT)} must prefer the project-local CLI")
    require("git rev-parse --show-toplevel" in text, f"{path.relative_to(ROOT)} must resolve the project root")
    managed_hook = ".codex/hooks.json" if harness == "codex" else ".claude/settings.json"
    require(managed_hook in text, f"{path.relative_to(ROOT)} must detect the project-managed hook")
    for version in (2, 3):
        require(
            f'Tree Ring Memory managed lifecycle v{version}"' in text,
            f"{path.relative_to(ROOT)} must recognize managed lifecycle v{version}",
        )
    require(
        text.index(managed_hook) < text.index('exec "$tree_ring"'),
        f"{path.relative_to(ROOT)} must enforce ownership before invoking the CLI",
    )
    require(
        f'--root .tree-ring integrations hook --harness {harness} --input-json-stdin' in text,
        f"{path.relative_to(ROOT)} does not invoke the {harness} lifecycle entry point",
    )
    require("PLUGIN_DATA" not in text, f"{path.relative_to(ROOT)} must not persist lifecycle input")
    require("CLAUDE_PLUGIN_DATA" not in text, f"{path.relative_to(ROOT)} must not persist lifecycle input")
    require(">>" not in text and "tee " not in text, f"{path.relative_to(ROOT)} must not append lifecycle input")

    events = {
        "SessionStart": b'{"hook_event_name":"SessionStart","session_id":"validation-session"}\n',
        "SubagentStart": b'{"hook_event_name":"SubagentStart","session_id":"validation-session","agent_id":"worker-1","agent_type":"worker"}\n',
        "Stop": b'{"hook_event_name":"Stop","session_id":"validation-session","stop_hook_active":false,"transcript_path":"/private/transcript.jsonl","last_assistant_message":"must remain opaque"}\n',
        "SubagentStop": b'{"hook_event_name":"SubagentStop","session_id":"validation-session","agent_id":"worker-1","agent_type":"worker","transcript_path":"/private/worker.jsonl","last_assistant_message":"must remain opaque"}\n',
    }
    with tempfile.TemporaryDirectory() as temporary:
        project = Path(temporary)
        cli = project / ".tree-ring" / "bin" / "tree-ring"
        cli.parent.mkdir(parents=True)
        cli.write_text(
            "#!/bin/sh\n"
            "printf '%s\\n' \"$@\" > \"$TREE_RING_TEST_ARGS\"\n"
            "cat > \"$TREE_RING_TEST_STDIN\"\n"
            "printf '%s\\n' '{\"hookSpecificOutput\":{\"additionalContext\":\"validated\"}}'\n",
            encoding="utf-8",
        )
        cli.chmod(0o755)
        args_capture = project / "args"
        stdin_capture = project / "stdin"
        environment = os.environ.copy()
        environment["TREE_RING_TEST_ARGS"] = str(args_capture)
        environment["TREE_RING_TEST_STDIN"] = str(stdin_capture)
        for event_name in sorted(LIFECYCLE_EVENTS):
            event = events[event_name]
            result = subprocess.run(
                [str(path)],
                cwd=project,
                env=environment,
                input=event,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            require(stdin_capture.read_bytes() == event, f"{path.relative_to(ROOT)} changed {event_name} JSON on stdin")
            require(
                args_capture.read_text(encoding="utf-8").splitlines()
                == ["--root", ".tree-ring", "integrations", "hook", "--harness", harness, "--input-json-stdin"],
                f"{path.relative_to(ROOT)} passed unexpected lifecycle arguments",
            )
            require(b"validated" in result.stdout, f"{path.relative_to(ROOT)} did not forward CLI output")
            args_capture.unlink()
            stdin_capture.unlink()

        managed_path = project / managed_hook
        managed_path.parent.mkdir(parents=True, exist_ok=True)
        for version in (2, 3):
            managed_path.write_text(
                f'{{"description":"Tree Ring Memory managed lifecycle v{version}"}}\n',
                encoding="utf-8",
            )
            duplicate = subprocess.run(
                [str(path)],
                cwd=project,
                env=environment,
                input=events["Stop"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            require(duplicate.stdout == b"", f"{path.relative_to(ROOT)} emitted duplicate v{version} context")
            require(not args_capture.exists(), f"{path.relative_to(ROOT)} invoked the CLI for managed v{version}")
            require(not stdin_capture.exists(), f"{path.relative_to(ROOT)} persisted managed v{version} input")

        managed_path.write_text(
            '{"description":"Tree Ring Memory managed lifecycle v4"}\n',
            encoding="utf-8",
        )
        unsupported = subprocess.run(
            [str(path)],
            cwd=project,
            env=environment,
            input=events["SessionStart"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        require(args_capture.exists(), f"{path.relative_to(ROOT)} incorrectly accepted managed lifecycle v4")
        require(stdin_capture.read_bytes() == events["SessionStart"], f"{path.relative_to(ROOT)} dropped v4 fallback input")
        require(b"validated" in unsupported.stdout, f"{path.relative_to(ROOT)} did not run the v4 fallback")


def validate_codex_skills_only() -> None:
    repository_manifest = load_json(PLUGIN / ".codex-plugin" / "plugin.json")
    skills_manifest = load_json(CODEX_SKILLS_ONLY / ".codex-plugin" / "plugin.json")
    expected_manifest = dict(repository_manifest)
    expected_manifest.pop("hooks", None)
    require(skills_manifest == expected_manifest, "skills-only Codex manifest drifted from repository metadata")
    for unsupported in ("mcpServers", "apps", "hooks"):
        require(unsupported not in skills_manifest, f"skills-only Codex ZIP must not declare {unsupported}")
    require("screenshots" not in skills_manifest.get("interface", {}), "skills-only Codex ZIP must not declare screenshots")

    with tempfile.TemporaryDirectory() as temporary:
        first = Path(temporary) / "first.zip"
        second = Path(temporary) / "second.zip"
        for destination in (first, second):
            subprocess.run([sys.executable, str(CODEX_SKILLS_BUILDER), str(destination)], check=True)
        require(first.read_bytes() == second.read_bytes(), "skills-only Codex ZIP must be deterministic")
        with ZipFile(first) as archive:
            names = set(archive.namelist())
            prefix = "tree-ring-memory/"
            require(names and all(name.startswith(prefix) for name in names), "skills-only Codex ZIP needs one package root")
            manifest_name = prefix + ".codex-plugin/plugin.json"
            require(manifest_name in names, "skills-only Codex ZIP manifest is missing")
            built_manifest = json.loads(archive.read(manifest_name))
            require(built_manifest == skills_manifest, "skills-only Codex ZIP manifest is stale")
            require(
                not any(
                    marker in name
                    for name in names
                    for marker in ("/hooks/", "/commands/", "/.claude-plugin/", "/packaging/")
                ),
                "skills-only Codex ZIP contains repository-only components",
            )
            require(
                prefix + "skills/tree-ring-memory/SKILL.md" in names,
                "skills-only Codex ZIP is missing its skill",
            )
            require(
                prefix + "assets/tree-ring-memory-logo.png" in names,
                "skills-only Codex ZIP is missing its declared assets",
            )


def validate_shared_contract() -> None:
    skill = PLUGIN / "skills" / "tree-ring-memory" / "SKILL.md"
    require_markers(
        skill,
        [
            "Runtime Bootstrap And Updates",
            "0.15.0 or newer",
            "--project --init --release latest --no-animation",
            "tree-ring update --check",
            "which -a tree-ring",
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
    require_markers(
        PLUGIN / "commands" / "tree-ring-update.md",
        [
            "tree-ring update --check",
            "preserve the active",
            "CLIs older than 0.15.0",
            "--root .tree-ring init",
        ],
    )
    require_markers(
        PLUGIN / "README.md",
        [
            "SessionStart`, `SubagentStart`, `Stop`, and `SubagentStop",
            "one agent-mediated memory checkpoint",
            "never inspects or persists `transcript_path`, `last_assistant_message`",
            "strict `tree-ring capture` command template",
            "normal-sensitivity candidate",
            "at most 10 seconds",
            "does not register prompt, tool, compaction, or `SessionEnd` hooks",
        ],
    )
    require_markers(
        PLUGIN / "PRIVACY.md",
        [
            "one agent-mediated memory checkpoint",
            "never inspects or persists `transcript_path`, `last_assistant_message`",
            "normal sensitivity",
            "strict `tree-ring capture`",
        ],
    )
    require_markers(
        PLUGIN / "SECURITY.md",
        [
            "`SessionStart`, `SubagentStart`, `Stop`, and `SubagentStop`",
            "never run as a `SessionEnd` or background recorder",
            "strict `tree-ring capture`",
            "accepts only normal sensitivity",
        ],
    )
    require_markers(
        skill,
        [
            "one synchronous, agent-mediated memory checkpoint",
            "never inspects or persists `transcript_path`, `last_assistant_message`",
            "up to three concise candidates",
            "strict `tree-ring capture` command template",
        ],
    )
    require_markers(
        PLUGIN / "commands" / "tree-ring-capture.md",
        [
            "## Lifecycle Stop Checkpoint",
            "tree-ring --root .tree-ring capture",
            "--operation-id auto-<checkpoint>-<1..3>",
            "--source-ref agent-checkpoint:<checkpoint>",
            "accepts normal sensitivity only",
        ],
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
    validate_codex_skills_only()
    validate_claude()
    validate_shared_contract()
    print("Tree Ring Memory repository plugins and Codex skills-only ZIP validated")


if __name__ == "__main__":
    main()
