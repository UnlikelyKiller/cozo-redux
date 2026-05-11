# Track 009 Plan: Search Performance — Parallel HNSW/FTS

## Phase 1: FTS Parallel Sort — DONE
- [x] Add `#[cfg(feature = "rayon")] use rayon::prelude::*` to `fts/indexing.rs`.
- [x] Add `FTS_PAR_SORT_THRESHOLD = 256` constant.
- [x] In `fts_search()`: branch on threshold, call `par_sort_by_key` when large.
- [x] Add `#[cfg(not(feature = "rayon"))]` serial fallback.
- [x] Verify existing tests pass.

## Phase 2: HNSW Batch Distance Computation — DONE
- [x] Add `HNSW_PAR_DIST_THRESHOLD = 8` constant to `hnsw.rs`.
- [x] Restructure `hnsw_search_level` inner loop: collect unvisited neighbors, sequential `ensure_key`, then parallel `v_dist` over warm cache.
- [x] Add comment explaining safe reborrow from `&mut VectorCache` to `&VectorCache`.
- [x] Add `#[cfg(feature = "rayon")]` / `#[cfg(not(feature = "rayon"))]` guards.
- [x] Verify all 246 tests pass; both feature paths compile.

## Phase 3: Outer-Loop Parallelism — DEFERRED
- [ ] Add `fn is_concurrent_read_safe(&self) -> bool` to `StoreTx` trait (default `false`).
- [ ] Override to `true` in RocksDB and Sled backends.
- [ ] In `HnswSearchRA::iter()`: if concurrent-safe and parent count ≥ threshold, parallelize KNN searches.
- [ ] Add SAFETY comment for raw-pointer sharing.
