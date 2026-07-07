# SecretiveShell Awesome llms.txt PR

Target: <https://github.com/SecretiveShell/Awesome-llms-txt>
Submission type: GitHub pull request
Status: submitted as draft PR #101
Public URL: <https://github.com/SecretiveShell/Awesome-llms-txt/pull/101>
Evidence comment:
<https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909533037>
Fork branch:
<https://github.com/TerminallyLazy/Awesome-llms-txt/tree/codex/add-tree-ring-llms-txt>

## Why This Fits

The repository is an index of hosted `llms.txt` files. Tree Ring Memory has a
public hosted `llms.txt` at:

<https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt>

This is a direct discovery fit rather than a broad promotional listing.

## Change

Added Tree Ring Memory to:

- `README.md`
- `json/llms-txt.json`
- `json/urls.json`

The PR template says JSON files are generated after merge, but the repository's
pull-request workflow runs `python scripts/normalize_lists.py --check`, which
requires generated JSON updates in the PR.

## Validation

External repo:

```bash
python3 scripts/normalize_lists.py --check
python3 scripts/check_urls.py --file <single-url-json> --workers 1 --timeout 20 --follow-redirects
git diff --cached --check
```

Tree Ring URL check:

```bash
curl -I -L --max-time 20 https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt
```

Result: Tree Ring `llms.txt` returned HTTP `200`; targeted upstream URL check
reported `checked 1 URLs: 1 ok, 0 failed`.

## Follow-Up

- Monitor Socket Security checks on PR #101.
- Mark ready for review if the maintainer prefers non-draft PRs for index
  additions.

Current PR checks: Socket Security Project Report and Pull Request Alerts both
passed after submission.
