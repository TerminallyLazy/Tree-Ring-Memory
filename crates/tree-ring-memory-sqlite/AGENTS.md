# tree-ring-memory-sqlite

## Purpose

Durable local storage and bounded recall.

## Ownership

Schema, write transactions, FTS, lifecycle maintenance, policy and session recall.

## Local Contracts

Preserve transaction durability, idempotent operations, coordinated-write authorization and schema-v3 writer fencing. Explicit RecallOptions remain conjunctive; SessionRecallScope applies the documented cross-session visibility rules. Filter scope, expiry, supersession, redaction and sensitivity before candidate limits.

DOX batch writes check existing project, source and root provenance within the write transaction. A conflict rejects the whole batch. Adoption of matching legacy records without root provenance requires the caller to verify the source project's own local store.

## Work Guidance

Exercise concurrent writes and old-session recall where relevant. Do not weaken privacy or scope checks to fill a result budget.

## Verification

`cargo test -p tree-ring-memory-sqlite --locked`; performance changes also use `cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 1000`.

## Child DOX Index

None. This document owns the full subtree.
