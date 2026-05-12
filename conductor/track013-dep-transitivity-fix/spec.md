# Track 013 — Dependency Transitivity Fix

## Objective

Fix `swapvec` dependency transitivity so downstream consumers (ChangeGuard, any crate depending on cozo-redux as a git dependency) resolve the patched `lz4_flex` rather than the vulnerable `lz4_flex 0.10.0`.

## Problem

Track 001 applied a workspace-level `[patch.crates-io]` to redirect `swapvec` to a vendored copy with `lz4_flex 0.12.1`. Cargo patches are NOT transitive — they only apply within the local workspace. Downstream consumers depending on `cozo = { git = "..." }` still resolve:

```
cozo-core -> swapvec 0.3.0 (crates.io) -> lz4_flex 0.10.0
```

`lz4_flex 0.10.0` is affected by CVE-2026-32829 (CVSS 8.2 HIGH).

## Fix

Change `cozo-core/Cargo.toml` from a crates.io dependency:

```toml
swapvec = "0.3.0"
```

to a path dependency pointing at the vendored copy:

```toml
swapvec = { path = "../vendor/swapvec" }
```

This makes the fix part of the dependency surface that downstream crates consume. The vendored swapvec uses `lz4_flex = "0.12.1"` (≥ 0.11.6 floor).

Optionally remove the now-redundant `[patch.crates-io]` entry for `swapvec` from the workspace `Cargo.toml`.

## Requirements

1. `cozo-core/Cargo.toml` uses path dependency for swapvec
2. `cargo tree -i lz4_flex` shows only 0.12.2 within this workspace
3. Downstream consumers pulling cozo-redux as a git dep resolve vendor/swapvec (no crates.io swapvec)
4. Full CI gate passes: fmt, clippy, test
5. Ledger provenance recorded

## Non-Requirements

- Updating swapvec to a newer upstream version (swapvec 0.4.2 still uses lz4_flex 0.10.0)
- Changing swapvec API usage within cozo-core
- Modifying vendored swapvec source
