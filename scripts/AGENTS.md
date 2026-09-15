# scripts

## Purpose

Repository validation and release tooling.

## Ownership

Package validator, release archive builder and certification script; coordinate root install.sh and .github workflows here.

## Local Contracts

Release tag and binary version must agree. Published tarballs and plugin ZIPs require checksums and expected contents. Preserve platform constraints and installation scope.

## Work Guidance

Read a script before running mutations or networked release steps. Keep exact-head evidence and distinguish unavailable external review from passing validation.

## Verification

`python3 scripts/validate-plugin-packages.py`; `sh -n install.sh scripts/package-release.sh scripts/certify-tree-ring.sh`.

## Child DOX Index

None. This document owns the full subtree.
