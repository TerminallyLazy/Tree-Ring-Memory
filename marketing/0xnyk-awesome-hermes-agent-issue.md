# Awesome Hermes Agent Submission Issue

## Target

- Repository: `https://github.com/0xNyk/awesome-hermes-agent`
- Submission type: resource submission issue
- Required workflow: issue form, not a direct PR
- Source repository:
  `https://github.com/TerminallyLazy/tree-ring-memory-skill`
- Suggested category: `agentskills.io Ecosystem`
- Suggested maturity: `beta`
- Issue: `https://github.com/0xNyk/awesome-hermes-agent/issues/216`
- Central evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4913635443`
- Current state: open, no maintainer comments yet

## Issue Body

```markdown
### Resource name

Tree Ring Memory Skill

### URL

https://github.com/TerminallyLazy/tree-ring-memory-skill

### Author

TerminallyLazy

### Category

agentskills.io Ecosystem

### Brief description

Portable `SKILL.md` package for Tree Ring Memory: local-first recall, deliberate
capture, audit, consolidation, privacy-safe redaction, deletion, and
lifecycle-aware forgetting workflows for AI agents.

The skill is framework-agnostic and points agents to the Rust `tree-ring` CLI
when a real local memory store is available, while still giving useful memory
hygiene guidance in any compatible agent host.

### Why it's awesome

- Gives agents an explicit memory lifecycle instead of treating memory as a
  transcript dump.
- Separates fresh context, durable truths, warnings, future seeds, evidence, and
  forgetting into clear operating rules.
- Keeps the default posture local-first and privacy-aware: source-linked recall,
  dry-run imports, redaction, delete/forget paths, and no secret capture.
- Ships as a simple root-level `SKILL.md` repo with MIT license, README,
  SECURITY.md, and validation workflow, so it is easy to inspect or adapt.
- Complements Hermes-style learned skills and curator workflows by making memory
  capture, audit, and pruning more deliberate.

### Why now

As Hermes and agentskills.io-style agents lean harder into self-improving skill
libraries and long-running memory, current users need portable memory hygiene
rules for what to recall, what to preserve, what to redact, and what to forget.

### License

MIT

### Suggested maturity label

beta

### Disclosures

I am the author/maintainer of Tree Ring Memory.
```

## Validation

- Checked existing issues, PRs, and README for `Tree Ring Memory`,
  `tree-ring-memory`, and `TerminallyLazy`; no existing submission found.
- Confirmed target `CONTRIBUTING.md` asks for resource submissions via issue
  rather than direct PR.
- Confirmed source repo is public, MIT licensed, recently updated, and includes
  root-level `SKILL.md`, README, SECURITY.md, and a validation workflow.
