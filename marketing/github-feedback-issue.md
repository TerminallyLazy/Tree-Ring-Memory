# Launch feedback: try Tree Ring Memory and tell us where agent memory breaks

Tree Ring Memory is in protocol-preview status. The goal is simple: make agent
memory useful without turning it into a transcript dump.

Launch page:

https://terminallylazy.github.io/Tree-Ring-Memory/

Try the installer:

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
tree-ring init
tree-ring remember "Use project-scoped recall before risky release changes." --event-type lesson --scope project
tree-ring recall "release changes"
```

The model:

- fresh work stays detailed
- older learning compresses into rings
- failures and regressions remain visible as scars
- durable truths become heartwood
- speculative future work stays as seeds
- wrong or sensitive memory can be forgotten, redacted, or superseded

Feedback I am especially interested in:

- Which agent frameworks should get first-class bridge support?
- Where does the ring model feel too simple or too heavy?
- What should explainable recall show by default?
- What privacy and forgetting controls are missing?
- What would make this easy to adopt in your agent workflow?
- What CLI or TUI friction shows up in the first 10 minutes?

If you try it, please include:

- operating system
- install path: global or project-local
- agent workflow or framework, if any
- command that failed or confused you
- what you expected memory to do instead
