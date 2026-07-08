# dmgrok Agent Plugins Source Issue

## Target

- Repository: `https://github.com/dmgrok/agent-plugins`
- Submission type: new skills provider issue
- Source repository:
  `https://github.com/TerminallyLazy/tree-ring-memory-skill`
- Provider name: `Tree Ring Memory`
- Provider ID: `tree-ring-memory`
- Skills location: root directory, single `SKILL.md`
- Branch: `main`

## Submitted

- Issue: `https://github.com/dmgrok/agent-plugins/issues/92`
- Validation run:
  `https://github.com/dmgrok/agent-plugins/actions/runs/28933569343`
- Validation comment:
  `https://github.com/dmgrok/agent-plugins/issues/92#issuecomment-4913543694`
- Auto-created PR: `https://github.com/dmgrok/agent-plugins/pull/93`
- Central evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4913577062`
- PR branch: `add-provider-tree-ring-memory`
- PR head commit: `16545f8356ec3473121f204251e6148af89e59bd`
- Current PR state: open, ready, mergeable, no status checks reported

## Issue Body

````markdown
### Repository URL
https://github.com/TerminallyLazy/tree-ring-memory-skill

### Provider Name
Tree Ring Memory

### Provider ID
tree-ring-memory

### Skills Location
Root directory (single SKILL.md)

### Default Branch
main

### Why should this source be included?
Tree Ring Memory provides a root-level agent skill for lifecycle-aware memory
workflows: recall, deliberate capture, evidence-backed lessons, audit,
consolidation, redaction, deletion, and forgetting.

The skill is useful for Claude, Codex, and other agent hosts that need
project-scoped memory guidance without scraping transcripts or storing secrets.
It points agents to the local-first Rust CLI when a real memory store is
available, but the skill itself remains framework-agnostic guidance.

Maintainer affiliation disclosure: Tree Ring Memory is maintained by the
submitter.

### Category Focus
Development, AI/ML Integration, Automation, Documentation

### Quality Checklist
- [x] Repository is publicly accessible
- [x] Repository has a clear license file
- [x] Skills follow the SKILL.md format with YAML frontmatter
- [x] Skills have clear names and descriptions
- [x] No hardcoded secrets, API keys, or sensitive data
- [x] No malicious or harmful content
- [x] Repository has been active within the last 6 months

### Provider Configuration

```json
{
  "tree-ring-memory": {
    "name": "Tree Ring Memory",
    "repo": "https://github.com/TerminallyLazy/tree-ring-memory-skill",
    "api_tree_url": "https://api.github.com/repos/TerminallyLazy/tree-ring-memory-skill/git/trees/main?recursive=1",
    "raw_base": "https://raw.githubusercontent.com/TerminallyLazy/tree-ring-memory-skill/main",
    "skills_path_prefix": ""
  }
}
```
````

## Validation

- Checked `dmgrok/agent-plugins` catalog and CDN export for `tree-ring`,
  `terminallylazy`, and `Tree Ring Memory`; no existing catalog entry found.
- Checked open PRs and issues for `Tree Ring Memory`; no duplicate found.
- Verified `TerminallyLazy/tree-ring-memory-skill` has MIT license,
  root-level `SKILL.md`, README, SECURITY.md, and validation workflow success.
- The target validation workflow accepted the issue, passed gitleaks, labeled
  the issue `approved`, and created PR `#93` to add the provider configuration
  to `scripts/aggregate.py`.
