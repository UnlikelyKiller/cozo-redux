# Status Report

Date: 2026-05-10

## Current Objective

Track 007 Phase 1 — Join Parallelization — complete and committed (`4d6bbca5`).
Next active work: **Track 007 Phase 2 — Parallel Iterator Integration** for `UnificationRA` and `FilteredRA`.

## ChangeGuard State

- Ledger: `0 pending, 0 unaudited drift`
- Last committed git commit: `4d6bbca5` — feat(track007): Phase 1 — parallel InnerJoin via rayon in materialized_join

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
- Full test suite (171 tests) passed.

## Track 007 — IN PROGRESS

### Phase 1: Join Parallelization — DONE (commit `4d6bbca5`)
- Parallel probe path in `materialized_join` in `cozo-core/src/query/ra.rs`.
- `PAR_THRESHOLD = 512`: when right side ≥ 512 tuples, materialize left side and
  probe via `par_iter().flat_map_iter()`.
- rayon promoted from optional to required dep; `rayon = []` feature marker added
  so `#[cfg(feature = "rayon")]` guards work correctly.
- Parallel block gated with `#[cfg(feature = "rayon")]` for `compact-single-threaded` compat.
- Both feature configs (`compact` and `compact-single-threaded`) compile clean.
- All tests pass via `changeguard verify`.

### Phase 2: Parallel Iterator Integration — NEXT
- [ ] Identify hot `UnificationRA::iter` and `FilteredRA::iter` call sites.
- [ ] Evaluate whether `par_bridge()` or full materialization is appropriate.
- [ ] Benchmark par_iter overhead on small datasets to find switching threshold.
- [ ] Gate any parallel paths with `#[cfg(feature = "rayon")]`.

### Phase 3: Scaling Audit — QUEUED
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
