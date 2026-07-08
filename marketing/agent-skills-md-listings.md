# Agent-Skills.md Listings

## Target

- Directory: `https://agent-skills.md`
- Main repo listing:
  `https://agent-skills.md/skills/TerminallyLazy/Tree-Ring-Memory/tree-ring-memory`
- Dedicated skill repo listing:
  `https://agent-skills.md/skills/TerminallyLazy/tree-ring-memory-skill/tree-ring-memory`
- Source repos:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory` and
  `https://github.com/TerminallyLazy/tree-ring-memory-skill`
- Latest evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4912355745`

## Submission

- Route: public repository intake at `https://agent-skills.md/submit`
- Direct API path:
  `https://agent-skills.md/api/add/https%3A%2F%2Fgithub.com%2FTerminallyLazy%2Ftree-ring-memory-skill`
- API result:
  `{"ok":true,"repoId":"TerminallyLazy/tree-ring-memory-skill","skillsAdded":1,"alreadyExists":false}`

## Validation

- Verified the dedicated skill listing returns HTTP 200.
- Verified the dedicated listing renders:
  - page title: `tree-ring-memory Skill | Agent Skills`
  - author: `TerminallyLazy`
  - repository: `TerminallyLazy/tree-ring-memory-skill`
  - license: `MIT`
  - install commands for `add-skill`
- Rechecked the existing main repo listing and verified it still returns HTTP
  200 with title `Tree Ring Memory Skill | Agent Skills`.
- Verified the homepage search response includes the TerminallyLazy listing
  data after the new intake.

## Notes

Agent-Skills.md now gives Tree Ring Memory two discovery surfaces: the original
main repository skill path and the dedicated root-level skill repo created for
direct clone/install flows.
