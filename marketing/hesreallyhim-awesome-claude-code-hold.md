# HesReallyHim Awesome Claude Code Hold

## Target

- Repository: `https://github.com/hesreallyhim/awesome-claude-code`
- Intended form:
  `https://github.com/hesreallyhim/awesome-claude-code/issues/new?template=recommend-resource.yml`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4912530476`

## Result

No public issue was created. GitHub rejected `createIssue` with:

```text
GraphQL: could not be created. Interactions on this repository have been restricted to collaborators only. (createIssue)
```

## Validation

- Checked upstream PRs and issues for existing `Tree Ring`,
  `Tree-Ring-Memory`, `tree-ring`, and `TerminallyLazy` references before
  attempting the submission.
- Found no duplicate Tree Ring recommendation.
- Read `.github/ISSUE_TEMPLATE/recommend-resource.yml`.
- Read the target validator path in `.github/workflows/validate-new-issue.yml`
  and `resources/parse_issue_form.py`.
- Validated the issue body locally with:
  `PYTHONPATH=/tmp/tree-ring-outreach-hesreallyhim python3 -m resources.parse_issue_form --validate`.
- Local validation result: valid with no warnings.

## Prepared Issue Body

```markdown
### Display Name
Tree Ring Memory

### Category
Memory & Context Persistence

### Link
https://github.com/TerminallyLazy/tree-ring-memory-claude-plugin

### Author Name
TerminallyLazy

### Author Link
https://github.com/TerminallyLazy

### Description
Claude Code plugin and skill package for Tree Ring Memory, a local-first memory lifecycle layer for AI agents. It guides recall, capture, audit, redaction, forgetting, and evidence records around the Rust CLI without recording transcripts or running a background service.

### Checklist
- [x] I checked that this resource isn't already on the list
- [x] All links are working and publicly accessible
- [x] This resource is specific to Claude Code
```

## Follow-Up

Hold unless a maintainer invites the submission or an account with collaborator
access can submit through the restricted issue flow.
