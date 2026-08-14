# Task 3 and Task 4 Activation Integration Report

## Status

Provisional integration complete on `codex/harness-activation-launcher`. Task 3 head
`6517cc51941694d417ff73fa5a0aaa08be3c6bc3` was merged into Task 4 head
`091e6c3`. This includes Task 3's prerequisite root/publication safety commit
`aad19d7` through ancestry. The integration is pending CodeRabbit quota and does
not claim review approval.

Task 5 was not implemented.

## Conflict Resolution

The histories diverged at reviewed Task 3 base `3c2d15c`. The merge produced one
content-conflicted file, `crates/tree-ring-memory-cli/src/activation/bridge.rs`,
with seven localized conflict regions:

1. Imports retain Task 3's root identity and fault-injection support together
   with Task 4's descriptor-based directory enumeration support.
2. `ProjectFs` retains Task 3's final-component no-follow root descriptor,
   device/inode binding, and root revalidation. The same pinned root descriptor
   supplies Task 4's activation `flock`; root identity is checked before and
   after lock acquisition so a replaced root cannot silently split writers.
3. `ResolvedTarget` retains Task 3's snapshot reads and create-only hard-link
   publication. Task 4 receipt creation is exposed only as the narrow
   `ProjectFs::create_receipt_file` API; there is no generic replacement API.
4. Task 4 receipt traversal retains descriptor-relative, component-by-component
   `O_NOFOLLOW` directory enumeration beneath the pinned root.
5. Receipt pruning/invalidation retains a distinct descriptor-relative unlink
   path, limited to validated six-component
   `.tree-ring/activation/receipts/<harness>/<worker>/<receipt>.json` targets.
   This does not enable bridge or manifest removal, and Task 3 deactivation
   remains creation-only/review-only for existing entries.
6. Non-Unix implementations retain fail-closed stubs for the combined pinned
   filesystem surface.
7. Deactivation and Agent Zero tests retain Task 4 adapter-version/bridge-
   fingerprint bookkeeping while preserving Task 3's no-replace review gate.

The small companion change in `activation/manifest.rs` routes receipt creation
and deletion through the narrowed receipt-only `ProjectFs` helpers. Receipt
storage continues to share the activation root lock; persisted manifest,
registry version/fingerprint, store, and root revalidation remain in Task 4's
final commit phase.

## Preserved Contracts

- Bridge and activation-manifest lifecycle is creation-only. Existing final
  entries are never replaced or removed; contested or changed entries return
  `needs-user-review` and are preserved.
- All mutation/traversal remains rooted in retained no-follow descriptors. Root
  identity is rechecked, create publication is no-replace, and publication or
  durability uncertainty leaves disk material intact.
- Receipt creation is create-only. Receipt deletion is separate and applies
  only to validated regular receipt files during pruning/invalidation.
- Preflight retains bounded scoped recall, exact harness/event mapping,
  current persisted-contract and registry revalidation under the shared lock,
  stale-receipt invalidation, non-UTF-8 project fail-closed behavior, and the
  path-free hook-facing storage diagnostic.
- Agent Zero remains binding-only. Missing/external plugin state is not
  installed, rewritten, or removed by core; existing owned binding material is
  preserved when a manifest replacement would be required.

## Focused Verification

- `cargo test -p tree-ring-memory-cli activation::bridge --lib`
  - Passed: 31 tests, 0 failed.
- `cargo test -p tree-ring-memory-cli activation::preflight --lib`
  - Passed: 21 tests, 0 failed.
- `cargo test -p tree-ring-memory-cli activation::manifest --lib`
  - Passed: 14 tests, 0 failed.
- `cargo test -p tree-ring-memory-cli activation::adapters --lib`
  - Passed: 12 tests, 0 failed.
- `cargo test -p tree-ring-memory-sqlite database_path --lib`
  - Passed: 2 tests, 0 failed.
- `cargo clippy -p tree-ring-memory-sqlite -p tree-ring-memory-cli --lib -- -D warnings`
  - Passed with no warnings.
- `cargo fmt --all -- --check`
  - Passed.
- `git diff --check` and `git diff --cached --check`
  - Passed.

Per direction, no full workspace suite was run.

## Review Boundary

This is an isolated integration result and local focused verification only.
CodeRabbit review was not available within quota, so this report does not claim
independent review approval. No lifecycle CLI routing, certification, fixtures,
plugin/documentation work, or Task 5 behavior was added.
