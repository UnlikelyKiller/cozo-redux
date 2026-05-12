# Dependency Upgrade Notes

## Context

ChangeGuard consumes this repository as a git dependency:

```toml
cozo = { git = "https://github.com/UnlikelyKiller/cozo-redux", default-features = false, features = ["storage-sled", "graph-algo", "rayon"] }
```

GitHub Dependabot reported a high-severity alert in ChangeGuard for
`lz4_flex`:

```text
changeguard -> cozo-redux -> swapvec 0.3.0 -> lz4_flex 0.10.0
```

The fixed `lz4_flex` line starts at `0.11.6`; the current latest observed
version is `0.13.1`.

## What We Found

The CozoDB-redux workspace already has a local patch:

```toml
[patch.crates-io]
swapvec = { path = "vendor/swapvec" }
```

and `vendor/swapvec/Cargo.toml` depends on:

```toml
lz4_flex = "0.12.1"
```

In this repository's own workspace, that resolves to `lz4_flex 0.12.2`, which
is above the fixed `0.11.6` floor.

However, Cargo patches are not transitive. When ChangeGuard depends on
CozoDB-redux as a git dependency, the `[patch.crates-io]` section from this
workspace is not applied by the downstream crate. Downstream users therefore
still resolve:

```text
cozo-core -> swapvec 0.3.0 from crates.io -> lz4_flex 0.10.0
```

This means the local workspace is protected, but downstream consumers are not.

## Needed Upstream Fix

CozoDB-redux should make the `swapvec` fix part of the dependency surface that
downstream crates actually consume. Viable options:

1. Replace the `cozo-core` dependency on crates.io `swapvec = "0.3.0"` with a
   maintained fork/git dependency that uses patched `lz4_flex`.
2. Vendor or internalize the required `swapvec` code under CozoDB-redux so
   `cozo-core` no longer depends on crates.io `swapvec`.
3. Remove or replace `swapvec` if it is no longer required for the enabled
   storage/features.
4. Coordinate an upstream `swapvec` release that bumps `lz4_flex`, then update
   `cozo-core` to that release.

Simply updating the workspace-level `[patch.crates-io]` is not sufficient for
ChangeGuard or any other downstream git dependency consumer.

## Compatibility Notes

- `swapvec 0.4.2` was checked and still depends on `lz4_flex 0.10.0`, so a
  plain `swapvec` version bump does not clear the alert.
- ChangeGuard updating its CozoDB-redux git revision to current `main` also
  does not clear the alert unless the downstream crate adds its own patch.
- The fix should be validated by running a downstream dependency tree check:

```powershell
cargo tree -i lz4_flex@0.10.0
```

For a successful fix, that command should no longer find a path through
`cozo -> swapvec`.

## Suggested Verification

After changing the dependency path, verify both CozoDB-redux and ChangeGuard:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For ChangeGuard specifically, also run:

```powershell
cargo tree -i lz4_flex@0.10.0
cargo test --test cozo_vector_ops --test semantic_search
changeguard verify
```
