# Agent Skill And Project Contract

Tree Ring Memory ships two integration aids for agent workflows:

- `skills/tree-ring-memory/SKILL.md`: a portable agent skill that teaches an agent when to recall, remember, redact, forget, and avoid memory capture.
- `templates/dox/AGENTS.md`: a DOX-style project contract template for repos that want Tree Ring Memory guidance alongside source code.

## Skill Usage

Use the skill in agent runtimes that support local skills or instruction packs.
The skill is framework-agnostic and does not assume Agent Zero, Claude, Codex, or another single host.

Recommended activation moments:

- project start or resume
- user says "remember this"
- user asks what was decided
- user corrects the agent
- a repeated mistake appears
- a durable decision is made
- a future idea should be tracked
- work is closing out

## Project Contract Usage

Use `templates/dox/AGENTS.md` when a repo wants local memory rules.
Copy it to the project root as `AGENTS.md`, or merge its sections into an existing project contract.

The contract intentionally says that Tree Ring Memory is not authoritative over source docs.
Agents should still read local project instructions and source evidence directly.

## Minimal CLI Flow

```bash
tree-ring init
tree-ring recall "project startup warnings"
tree-ring remember "Use protocol-first design." --event-type decision --scope project --tag architecture
tree-ring forget mem_example --mode redact --reason "remove sensitive detail"
```

## Minimal Python Flow

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(
    summary="Use project-scoped recall before changing release behavior.",
    event_type="lesson",
    scope="project",
    project="example",
    tags=["release", "workflow"],
)

results = memory.recall("release behavior", project="example")
```

## Safety Rule

When in doubt, do not store the memory.
Prefer a short, redacted, source-linked lesson over detailed sensitive capture.
