# tree-ring-memory-core

## Purpose

Memory-domain behavior without a host harness.

## Ownership

src models, scoring, sensitivity, import/export, maintenance, DOX/Revolve ingestion, quality and workflow evaluation.

## Local Contracts

Preserve event validation, schema-compatible serialization, source provenance and sensitivity handling. Adapter ingestion summarizes source material; it does not turn instructions into authority.

DOX keeps stable source IDs and adds a canonical source-root fingerprint in a `dox-root` link. A secret-bearing source file is skipped entirely. A moved or copied root has a different fingerprint; do not silently rebind existing provenance.

## Work Guidance

Change the owning module and its existing tests; coordinate persistence or public CLI effects with sibling crates.

## Verification

`cargo test -p tree-ring-memory-core --locked`.

## Child DOX Index

None. This document owns the full subtree.
