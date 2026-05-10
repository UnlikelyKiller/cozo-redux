# Status Report

Date: 2026-05-10

## Current Objective

Track 008 **COMPLETE**. All three phases done and pushed.

## ChangeGuard State

- Ledger: `0 pending, 0 unaudited drift`
- Last committed git commit: TBD — perf(track008): Phase 3 — eliminate to_vec() in sled.rs range bounds

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

## Track 008 — IN PROGRESS

### Phase 1: TempStore Optimization — DONE (commit `65e9fe29`)
- **Write-buffer pattern**: `pending: Vec<(Tuple, bool)>` accumulates O(1) pushes from `put()` / `put_with_skip()`.
- **`commit_pending()`**: stable-sorts pending by tuple key, extends BTreeMap (`inner`) in one pass. Called at `wrap()` — the natural write→read boundary.
- **`exists()` during write phase**: checks committed BTreeMap first, then linear-scans pending (bounded by LIMIT on the limit-checking path — small in practice).
- **Range scan invariant preserved**: BTreeMap (`inner`) is always sorted; `range_iter()` has `debug_assert!(pending.is_empty())` to guard the read-time invariant.
- **`merge_in()` unchanged**: operates directly on `inner`; debug_asserts confirm both self and new have empty pending at merge time.
- **Hash-set dedup note**: a separate `HashSet` would only speed up `exists()` during writes (limit-checking path only). Not worth the extra memory; the Vec linear scan is sufficient given pending is LIMIT-bounded.

### Phase 2: MemStorage/TempStorage scan allocation reduction — DONE (commit `34d945b9`)
- **Architecture reality**: `StoreTx` is byte-based (`&[u8]` everywhere). A truly Arc<Tuple>-native backend
  would encode/decode at every API boundary, trading one overhead for another.
- **Implemented instead**: `ByteRange<'a>` struct implementing `RangeBounds<[u8]>` exclusively. Passed
  directly to `BTreeMap::range()` instead of `lower.to_vec()..upper.to_vec()`. Eliminates 2 Vec<u8>
  allocations per range scan call across `del_range_from_persisted`, `range_scan_tuple`, `range_scan`,
  and `range_count` in both `MemStorage` (mem.rs) and `TempStorage` (temp.rs).
- **Why not a tuple bound**: `(Bound<&[u8]>, Bound<&[u8]>)` triggers compiler ambiguity between
  `Vec<u8>: Borrow<[u8]>` and `Vec<u8>: Borrow<Vec<u8>>`. A concrete struct with a single
  `impl RangeBounds<[u8]>` forces unique T=[u8] inference.

### Phase 3: Final Hygiene — DONE
- **sled.rs**: Replaced all 9 `lower.to_vec()..upper.to_vec()` range bounds with `lower..upper`.
  Sled's `Tree::range<K: AsRef<[u8]>, R: RangeBounds<K>>` accepts `&[u8]` directly; no intermediate
  `Vec<u8>` needed. Eliminates 18 heap allocations per transaction that uses sled ranges.
- **Persistence backends**: Full 246-test suite passes with `compact,storage-rocksdb,requests`.
  RocksDB and SQLite backends show no regressions. Sled code verified to compile with `storage-sled`.
