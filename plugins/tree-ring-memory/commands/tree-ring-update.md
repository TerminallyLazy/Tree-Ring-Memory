---
description: Check for or install a verified Tree Ring Memory CLI update without changing installation scope
argument-hint: "[--check]"
allowed-tools: ["Bash", "Read"]
---

# Tree Ring Update

Resolve the real project root and read project-local `.tree-ring/SKILL.md` and
`.tree-ring/CLI.md` when present. Prefer `.tree-ring/bin/tree-ring` for that
project when it exists; otherwise resolve the active global binary with
`command -v tree-ring`. Use `which -a tree-ring` to detect older shadowing
copies.

For a read-only release check, run the selected binary:

```bash
tree-ring update --check
```

If the user asked only to check, report the installed version, available
version, executable path, and install method without changing files. If the
user already authorized an update, run `tree-ring update`; otherwise obtain
permission immediately before the update. The updater must preserve the active
project-local, direct-prefix, or Homebrew scope and verify official release
assets. Never install a second copy merely to bypass an older active binary.

CLIs older than 0.15.0 lack the update command. Upgrade with the same manager or
prefix: `brew upgrade tree-ring` for Homebrew, the official installer with
`--project --release latest` for an existing project-local binary, or
`--install-dir <existing-prefix> --release latest` for another direct install.
Do not change global scope or edit shell startup files without separate user
authorization.

After updating, verify the selected binary with `--version`, return to the
actual project root, and run:

```bash
tree-ring --root .tree-ring init
tree-ring --root .tree-ring integrations status --verbose
```

Use `.tree-ring/bin/tree-ring` for both commands when the project has a local
binary. Confirm the files remain under the intended project's `.tree-ring/`.
Initialization refreshes managed guidance but does not prove harness activation.
