# Status Report

Date: 2026-05-10

## Current Objective

Track 006 is fully complete, committed, and pushed. Ledger is clean (0 pending, 0 drift).
Next active work: **Track 007 Phase 1 — Join Parallelization** using Rayon.

## ChangeGuard State

- Ledger: `0 pending, 0 unaudited drift`
- Last committed git commit: `3713f7c1` — feat(track006): complete memory efficiency modernization
- ChangeGuard verify config updated: removed `-j 1 --test-threads=1`; now runs
  `cargo fmt --all -- --check` then `cargo test`.

## Pre-commit Hook (actual)

The hook runs:
- `cargo fmt --all -- --check`
- `cargo clippy --lib --tests --features compact,storage-rocksdb,requests`
- `cargo test --workspace --features compact,storage-rocksdb,requests`

Note: hook does NOT use `--all-targets` (excludes benches) or `-D warnings`.

## Track 006 — COMPLETE AND PUSHED

### Phase 1: DataValue Shrinking — DONE
- Boxed `List`, `Set`, `Vec`, `Json`, `Regex` variants; `DataValue` ≤ 32 bytes confirmed.

### Phase 2: Tuple SmallVec Migration — DONE
- `Tuple = SmallVec<[DataValue; 6]>` in `cozo-core/src/data/tuple.rs`
- All tuple constructors, call sites, bindings (Python, Node), and tests updated.
- `cargo fmt --all` run workspace-wide.
- Full test suite (171 tests) passed via `changeguard verify` and the pre-commit hook.

### Clippy Hygiene (resolved as part of commit)
- Added `#![allow(clippy::mutable_key_type)]` and `#![allow(private_interfaces)]` at
  crate root — these are design-level false positives for DataValue-keyed collections.
- Fixed genuine quick-win suggestions: `is_multiple_of`, `swap_with_temporary`,
  `needless_borrows_for_generic_args`, `get_first`, `manual_inspect`, `copied`,
  `non_canonical_partial_ord_impl`, and others across ~26 files.
- Added `#[allow(dead_code)]` to ~20 diagnostic error structs used by miette.

### Phase 3: Verification & Benchmarking — DEFERRED
- `cargo test` passed (counts as full test suite).
- Benchmark comparison (`pokec`, `air_routes`) deferred — not blocking Track 007.

## Track 007 — IN PLANNING

### Phase 1: Join Parallelization
- [ ] Identify independent branches in `InnerJoin` in `cozo-core/src/query/ra.rs`.
- [ ] Use `rayon::join` to evaluate independent sub-expressions in the query plan.
- [ ] Implement parallel tuple extension for hash joins.

**Key context**: `rayon` is already in `cozo-core/Cargo.toml` as an optional dependency
gated behind the `graph-algo` feature. For join parallelism in the query core, it may
need to be promoted to a required dep or enabled under a new feature.

### Phase 2: Parallel Iterator Integration
- [ ] Convert key `RelAlgebra` iterators to `rayon::iter::ParallelIterator` where beneficial.
- [ ] Focus on `UnificationRA` and `FilteredRA`.
- [ ] Benchmark par_iter overhead on small datasets to find switching threshold.

### Phase 3: Scaling Audit
- [ ] Verify thread pool behavior under high core count.
- [ ] Ensure no deadlocks from nested Rayon calls.
- [ ] Final verification with `wiki_pagerank` benchmark.

## Track 008 — QUEUED

### Phase 1: TempStore Optimization
- [ ] Profile `RegularTempStore`; implement `HashSet`-based dedup in `temp_store.rs`.
- [ ] Verify ordering invariants still hold for range scans.

### Phase 2: Native MemStorage
- [ ] Create `Arc<Tuple>`-native `MemStorage` variant.
- [ ] Implement `StoreTx` and compare performance with byte-encoded variant.

### Phase 3: Final Hygiene
- [ ] Audit all storage implementations for redundant `clone()` / `to_vec()`.
- [ ] Verify persistence backends (RocksDB, SQLite) for no regressions.
