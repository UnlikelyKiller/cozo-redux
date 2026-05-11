# Status Report

Date: 2026-05-11

## Current Objective

Track 012 **IN PROGRESS** — Product Quantization (PQ) for HNSW.

## ChangeGuard State

- Ledger: `1 pending` (Track 012)
- Last committed git commit: `209473cb` — chore: convert swapvec submodule to regular files

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

## Track 008 — COMPLETE

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

## Track 011 — COMPLETE (commit `2919397a`)

### Phase 1: In-Loop Predicate Filter + ef Expansion — DONE
- `hnsw_search_level` gains `filter: Option<(&[Bytecode], SourceSpan)>` parameter.
- Separate `traversal_nn` PQ (ef-bounded) controls exploration bound when filter active.
- Non-passing nodes expand neighbors for traversal but excluded from `found_nn` (biased traversal).
- `hnsw_knn` doubles ef when filter present; passes filter to level-0 search only.
- All construction-path callers pass `None` (unchanged behavior).
- `test_hnsw_in_loop_predicate_filter`: 5 "a" (far) + 5 "b" (near) vectors; query near "b" with tag=="a" filter; verifies 5 results and all tagged "a".
- File: `cozo-core/src/runtime/hnsw.rs`

## Track 010 — COMPLETE (commit `897dddb5`)

### HNSW Graph Repair on Deletion — DONE
- Added `hnsw_repair_node()`: after each deleted edge, reconnects former neighbors whose degree drops below `max(1, m_max/2)`.
- Candidate pool: current neighbors + 1-hop expansion; `hnsw_select_neighbours_heuristic` selects best candidates.
- `hnsw_remove` and `hnsw_remove_vec` extended with `manifest` and `vec_cache` parameters.
- `stored.rs` updated to pass manifest (was `_`).
- Resolved stale comment "this still has some probability of disconnecting the graph."
- All 246 tests pass; both feature paths compile.

## Track 009 — COMPLETE (commit `a262357c`)

### Phase 1: FTS Parallel Sort — DONE (commit `a262357c`)
- `par_sort_by_key` in `fts_search()` when candidate count ≥ `FTS_PAR_SORT_THRESHOLD = 256`.
- Pure computation on already-collected `(Tuple, f64)` scores — no store access required.
- `#[cfg(feature = "rayon")]` guard; `compact-single-threaded` path unchanged.
- File: `cozo-core/src/fts/indexing.rs`

### Phase 2: HNSW Batch Distance Computation — DONE (commit `a262357c`)
- Restructured `hnsw_search_level` inner loop to batch-process each candidate's neighbors.
- **Sequential phase**: `ensure_key` for all unvisited neighbors (store access, requires `&mut VectorCache`).
- **Parallel phase**: `par_iter().map(|k| cache_ref.v_dist(q, k))` when batch ≥ `HNSW_PAR_DIST_THRESHOLD = 8`.
- Reborrow `&mut VectorCache` as `&VectorCache` is safe: all mutations complete before the parallel block.
- `VectorCache` is `Sync` for reads (`FxHashMap<CompoundKey, Vector>` + `HnswDistance` enum, all `Sync`).
- File: `cozo-core/src/runtime/hnsw.rs`

### Phase 3: Outer-Loop Parallelism — PENDING
- Requires `StoreTx: is_concurrent_read_safe()` to enable parallel KNN across parent tuples.
- Highest impact (50 independent HNSW searches in parallel) but involves `unsafe` raw-ptr sharing.

## Track 008 — COMPLETE

### Phase 3: Final Hygiene — DONE
- **sled.rs**: Replaced all 9 `lower.to_vec()..upper.to_vec()` range bounds with `lower..upper`.
  Sled's `Tree::range<K: AsRef<[u8]>, R: RangeBounds<K>>` accepts `&[u8]` directly; no intermediate
  `Vec<u8>` needed. Eliminates 18 heap allocations per transaction that uses sled ranges.
- **Persistence backends**: Full 246-test suite passes with `compact,storage-rocksdb,requests`.
  RocksDB and SQLite backends show no regressions. Sled code verified to compile with `storage-sled`.

## Track 012 — IN PROGRESS

### Phase 1: Codebook Training & Storage — DONE
- `PqConfig` and `PqCodebook` structs added to `HnswIndexManifest` with `#[serde(default)]`.
- K-means Lloyd algorithm implemented in `cozo-core/src/runtime/hnsw.rs`.
- `::hnsw train_pq rel:index { subspaces: N, centroids: M, samples: K }` grammar and parser added.
- `SysOp::TrainPq` dispatched in `runtime/db.rs` with proper relation locking.
- Codebook stored as special key in index relation (`hnsw_store_pq_codebook` / `hnsw_get_pq_codebook`).

### Phase 2: Encoding on Insert — DONE
- `encode_vector_pq` function quantizes F32 vectors to uint8 centroid indices per subspace.
- `hnsw_train_pq` iterates all existing vectors and stores their PQ codes after training.
- `hnsw_put_vector` encodes and stores PQ codes for new insertions when PQ config is active.
- `hnsw_remove_vec` deletes PQ codes when a vector is removed.
- PQ codes stored in index relation with sentinel key `i64::MAX - 1`.

### Phase 3: Approximate Distance Search — DONE
- `VectorCache` extended with `pq_codebook` and `pq_codes` fields.
- `ensure_pq_code` loads PQ codes from index relation into cache.
- `pq_dist` computes approximate distance via lookup tables.
- `hnsw_knn` precomputes `dist_table[subspace][centroid]` for L2 when codebook is present.
- `hnsw_search_level` uses PQ approximate distance for graph traversal when available; falls back to exact distance if codes are missing.
- Construction-path `hnsw_search_level` calls pass `None` for PQ distance table (unchanged behavior).

### Phase 4: Tests & Hygiene — DONE
- `test_hnsw_pq_training_and_search`: creates 50 8-dim vectors, trains PQ with 2 subspaces / 4 centroids, verifies search returns 5 results.
- All 179 tests pass; `cargo fmt` and `cargo clippy -- -D warnings` clean.
- Fixed pre-existing clippy warnings in `query/ra.rs`, `cozo-lib-python/src/lib.rs`, and `cozo-core/tests/air_routes.rs`.

### Known Limitations
- PQ distance currently only supports L2 distance metric.
- No explicit exact-distance re-ranking step; approximate distances used for both traversal and final results.
- Full vectors are still loaded during search (codes are stored but full vectors are still accessed for fallback).
- `hnsw_convert_to_pq` migration command not yet implemented.
