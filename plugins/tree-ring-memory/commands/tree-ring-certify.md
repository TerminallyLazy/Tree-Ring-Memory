---
description: Generate Tree Ring harness or recall-quality evidence without confusing it with the full framework release suite
argument-hint: "[harness|quality] [project root]"
allowed-tools: ["Bash", "Read"]
---

# Tree Ring Certify

Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` when present,
then follow the skill's Runtime Bootstrap And Updates procedure and confirm the
selected binary reports 0.15.0 or newer. Bootstrap or upgrade when the user's
request already authorizes it; otherwise obtain permission before downloading
or changing software. Never invent certification results.

For an installed runtime, choose the requested self-contained evidence path:

```bash
tree-ring integrations certify --source-root <project-root>
tree-ring recall-quality --source-root <project-root>
```

Default to harness certification and the current project root. Report the
observed pass/fail/skip counts, evidence directory, and any next steps. These
commands write evidence under `target/tree-ring-certification/`, but they do not
activate a harness or prove that an agent used recalled context; check
receipt-backed integration status separately.

The full `sh scripts/certify-tree-ring.sh` release suite is repository-only.
Run it only when the current directory is a canonical Tree Ring Memory source
checkout containing that script, the Rust workspace, `install.sh`, fixtures,
and build tooling. Never copy or download the script into another project and
never represent the installed CLI checks as full framework release
certification. The TUI's `/evidence refresh` action only displays that external
command; it does not execute the suite.
