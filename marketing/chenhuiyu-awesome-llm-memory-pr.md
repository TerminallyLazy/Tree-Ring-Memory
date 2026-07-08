# Chenhuiyu Awesome LLM Memory PR

## Target

- Repository: `https://github.com/chenhuiyu/awesome-llm-memory`
- PR: `https://github.com/chenhuiyu/awesome-llm-memory/pull/2`
- Fork: `https://github.com/TerminallyLazy/chenhuiyu-awesome-llm-memory`
- Branch: `add-tree-ring-memory`
- Commit: `544f263e485396bdd4f417b537915d9f90bd3aa3`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910886712`

## Placement

- Source file: `data/entries.yaml`
- Generated files: `README.md` and `README.zh-CN.md`
- Section: `Systems & Frameworks (stateful agents / memory managers)`
- Entry:
  `Tree Ring Memory` as a 2026 `Repo` with `long` memory scope for local
  agent-memory lifecycle, SQLite/FTS recall, redaction, consolidation, and
  audits.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `Tree-Ring-Memory`, or `tree-ring` references before opening the PR.
- Confirmed the Tree Ring Memory GitHub URL returned HTTP `200`.
- Ran `python3 scripts/build_readme.py`.
- Ran `python3 scripts/lint_links.py` in a temporary virtualenv:
  69 entries, 0 link warnings, 0 hard errors.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: GitGuardian Security Checks passed.

## Notes

The PR body discloses maintainership. It also notes three existing year-field
fixes in `data/entries.yaml` that keep generated README output consistent with
the checked-in tables.
