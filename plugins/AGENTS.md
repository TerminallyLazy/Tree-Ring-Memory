# plugins

## Purpose

Canonical native plugin distribution material.

## Ownership

tree-ring-memory contains Codex/Claude manifests, commands, skills, hooks, legal text and public ZIP builder. Root marketplaces point here.

## Local Contracts

The public OpenAI upload renders skill front matter without legacy metadata, preserves the skill body, and includes supported interface fields in skills/<skill>/agents/openai.yaml. Keep the public listing shortDescription within 30 characters. Native/shared skill source remains separate. Ship all four native lifecycle hooks. Codex skills-only means no MCP dependency; retain hooks and executable ZIP permissions. Effective host-owned project hooks cause plugin hooks to stand down; Codex linked-worktree root layers use the proven primary hook source, while Claude keeps local ownership checks. A genuinely absent project-local .tree-ring is a quiet skip; an existing entry, including a dangling symlink, must retain runtime diagnostics. Never redirect worktree memory to the primary checkout's store.

## Work Guidance

Keep shared skill instructions aligned with the CLI and preserve canonical hook bytes when syncing standalone marketplaces. Do not put stores or user paths in archives.

## Verification

`python3 scripts/validate-plugin-packages.py`; inspect deterministic archives and run unpacked hook probes.

## Child DOX Index

None. This document owns the full subtree.
