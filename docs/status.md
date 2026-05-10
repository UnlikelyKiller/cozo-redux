# Status Report

Date: 2026-05-10

## Current Objective

Track 007 — Join/Iterator Parallelization — **COMPLETE** (all 3 phases committed).
Next active work: **Track 008 Phase 1 — TempStore Optimization**.

## ChangeGuard State

- Ledger: `0 pending, 0 unaudited drift`
- Last committed git commit: `c025fea5` — test(track007): Phase 3 — parallel path correctness tests + scaling audit

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

## Track 007 — COMPLETE

### Phase 1: Join Parallelization — DONE (commit `4d6bbca5`)
- Parallel probe path in `materialized_join` in `cozo-core/src/query/ra.rs`.
- `PAR_THRESHOLD = 512`: when right side ≥ 512 tuples, materialize left and probe via `par_iter().flat_map_iter()`.
- rayon promoted to required dep; `rayon = []` feature marker for `#[cfg]` compatibility.

### Phase 2: Parallel Iterator Integration — DONE (commit `0bb2d076`)
- `FilteredRA::iter`: sample-then-decide pattern with `FILTER_PAR_THRESHOLD = 1024`.
  - Large batches: `into_par_iter().map(|t| { stack=vec![]; eval_bytecode_pred(...) })`.
  - `Result<Option<T>>::transpose()` bridges map→filter_map without extra collect.
  - `#[cfg(not(feature="rayon"))]` on serial block eliminates unreachable-code warning.
- `UnificationRA::iter` (non-multi only): same pattern with `UNIF_PAR_THRESHOLD = 1024`.
  - `is_multi` path remains serial (variable output per tuple — harder to parallelize).
  - Fresh stack per rayon task; `eval_bytecode` is pure.

### Phase 3: Scaling Audit — DONE (commit `c025fea5`)
- **Nested rayon**: `prog.par_iter()` in `eval.rs` predates our changes. Rayon's work-stealing
  handles nested `par_iter` safely (no deadlock). Nested parallelism may compete for threads
  but never starves (calling thread participates in inner work).
- **Thread pool**: global default (num_cpus) is appropriate; no `ThreadPoolBuilder` needed.
- **wiki_pagerank benchmark**: unaffected (exercises fixed-rule graph algos, not join/filter).
- **Correctness tests added**: 3 tests cover each parallel path at/above threshold:
  - `test_parallel_filter_correctness` — 2000 rows, filter, verify 1000 even results
  - `test_parallel_unification_correctness` — 2000 rows, bind doubled=n*2, verify all
  - `test_parallel_join_correctness` — 600-row right side, verify 50 join results

## Track 008 — NEXT

### Phase 1: TempStore Optimization
- [ ] Profile `RegularTempStore`; implement `HashSet`-based dedup in `temp_store.rs`.
- [ ] Verify ordering invariants still hold for range scans.

### Phase 2: Native MemStorage
- [ ] Create `Arc<Tuple>`-native `MemStorage` variant.
- [ ] Implement `StoreTx` and compare performance with byte-encoded variant.

### Phase 3: Final Hygiene
- [ ] Audit all storage implementations for redundant `clone()` / `to_vec()`.
- [ ] Verify persistence backends (RocksDB, SQLite) for no regressions.
