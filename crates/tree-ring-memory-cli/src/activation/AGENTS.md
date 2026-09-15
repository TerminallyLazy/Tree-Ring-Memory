# activation

## Purpose

Project-local lifecycle bridges and proof of actual recall.

## Ownership

Adapter detection, bridge filesystem operations, manifests, lifecycle parsing, preflight and launch wrapper.

## Local Contracts

Use SessionStart/SubagentStart for recall and Stop/SubagentStop for bounded agent capture. Normalize equivalent local paths without following symlinks; retain descriptor-relative no-follow access and create-only publication. Lifecycle hooks may skip only a genuinely absent project-local .tree-ring root after input and project-path validation. Existing invalid roots or missing activation records remain errors; only a fresh matching receipt establishes active status. Generated hook guards do not apply to explicit preflight or capture commands.

## Work Guidance

Keep root AGENTS.md references separate from .tree-ring guidance. Never scrape transcripts, manufacture receipts, or treat an Agent Zero marker as installed plugin capability.

Agent Zero capability contracts pair an exact plugin version with its minimum
runtime and minor series. Coordinate the core allowlist and plugin descriptor
before publishing either release; verify the actual pair with CLI activation,
preflight, and receipt-backed status. A passing version probe alone is insufficient.

Recognize earlier Claude handler bundles only by exact recorded ownership, commands, and generated single-handler entry shape. Preserve custom handlers and settings; bridge reconciliation still follows create-only publication and cannot silently replace existing hooks or manifests.

When creating root AGENTS.md, record ownership of only the marked Tree Ring block so surrounding project instructions remain editable. Preserve legacy complete-file ownership until explicitly reconciled; never migrate an existing activation manifest automatically.

## Verification

`cargo test -p tree-ring-memory-cli --test harness_activation_acceptance --locked`; activation unit tests and native host probes cover changes to events, ownership or receipts.

## Child DOX Index

None. This document owns the full subtree.
