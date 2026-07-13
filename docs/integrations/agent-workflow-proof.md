# Agent Workflow Proof

The workflow-proof example is an explicit, controlled comparison for three
small synthetic, project-safe scenarios. It is not a normal `tree-ring`
subcommand, background process, CI job, or universal benchmark.

Run it deliberately from a source checkout with a locally available Codex CLI:

```bash
cargo run --locked -p tree-ring-memory-cli --example workflow_proof -- \
  fixtures/workflow-proof target/tree-ring-certification/workflow-proof
```

The command runs the same task in three retained workspaces for every fixture:

- `no_memory` receives no memory context.
- `raw_memory` receives the visible normal, non-superseded seed memories in
  fixture order.
- `tree_ring` receives the normal, non-superseded memories returned by local
  Tree Ring retrieval.

The fixture pack exercises constraint recall (`no-background-writer`), current
rules over superseded rules (`stale-cli-contract`), and a failure scar changing
the recovery decision (`scar-recovery`). The agent task only asks it to inspect
the materialized workspace and prepare `decision.md`; deterministic validators
remain outside the request.

## Evidence and Reproducibility

The selected output directory retains all trial workspaces at
`trials/<scenario>/<arm>/workspace/`, plus a machine-readable
`workflow-proof-report.json` and a readable `workflow-proof-summary.md`.
Treat `workflow-proof-report.json` as observed paired evidence for these
specific controlled fixtures: inspect the retained workspaces, memory context,
agent response, and deterministic file checks before drawing a conclusion.

For every run, record alongside the output:

- the Tree Ring commit (`git rev-parse HEAD`);
- the Codex CLI version (`codex --version`) and selected model, if `--model` is
  supplied;
- the complete command, timestamp, and any non-default Codex binary path.

No unit test, normal certification command, or CI job invokes Codex
automatically. A real model run happens only when an operator explicitly runs
the example above, and a failed control arm remains evidence rather than a
runner failure.

This pack does not establish a universal model score, a general claim that
memory improves all workflows, or a replacement for external evaluations. The
next validation step is to run external benchmark adapters and preserve their
native reports beside this controlled evidence.
