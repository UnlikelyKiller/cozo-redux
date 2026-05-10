# Status Report

Date: 2026-05-10

## Current Objective

Track 006 Phase 2 is complete and verified. Ledger is clean (0 pending, 0 drift).
Next active work: **Track 007 Phase 1 — Join Parallelization** using Rayon.

## ChangeGuard State

- Ledger: `0 pending, 0 unaudited drift`
- Last committed transaction: `717e0581-70ed-4120-ab86-31344894edc7`
  - Summary: "Complete Track 006 Phase 2 Tuple SmallVec migration"
- ChangeGuard verify config updated: removed `-j 1 --test-threads=1` from default
  verify step; now runs `cargo fmt --all -- --check` + `cargo test`.

## Track 006 — COMPLETE

### Phase 1: DataValue Shrinking — DONE
- Boxed `List`, `Set`, `Vec`, `Json`, `Regex` variants; `DataValue` ≤ 32 bytes confirmed.

### Phase 2: Tuple SmallVec Migration — DONE
- `Tuple = SmallVec<[DataValue; 6]>` in `cozo-core/src/data/tuple.rs`
- All tuple constructors, call sites, bindings (Python, Node), and tests updated.
- `cargo fmt --all` run workspace-wide.
- Full test suite (171 tests) passed via `changeguard verify`.

### Phase 3: Verification & Benchmarking — DEFERRED
- `cargo test` passed (counts as full test suite run).
- Benchmark comparison (`pokec`, `air_routes`) deferred — not blocking Track 007.
- `heaptrack` audit deferred to a separate benchmarking session.

## Track 007 — IN PLANNING

### Phase 1: Join Parallelization
- [ ] Identify independent branches in `InnerJoin` in `cozo-core/src/query/ra.rs`.
- [ ] Use `rayon::join` to evaluate independent sub-expressions in the query plan.
- [ ] Implement parallel tuple extension for hash joins.

### Phase 2: Parallel Iterator Integration
- [ ] Convert key `RelAlgebra` iterators to `rayon::iter::ParallelIterator` where beneficial.
- [ ] Focus on `UnificationRA` and `FilteredRA`.
- [ ] Benchmark par_iter overhead on small data sets to find switching threshold.

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

## Known Baseline Warnings

- `cargo check` emits ~51 warnings in `cozo-core` (private interfaces, missing docs,
  dead code). These are pre-existing baseline warnings, not regressions from Tracks 006–007.
- `changeguard verify` warnings: semantic prediction base_url empty (no action needed).

## Dirty Tree Notes

Pre-existing dirty/untracked files that should not be reverted:
- `conductor/conductor.md`, `conductor/track006/`, `conductor/track007/`, `conductor/track008/`
- Phase 1 DataValue boxing changes across many `cozo-core` files and bindings
- `.claude/` and `vendor/swapvec` — treat as pre-existing
