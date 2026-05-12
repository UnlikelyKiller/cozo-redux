# Track 009: Search Performance — Parallel HNSW/FTS

## Objective
Parallelize the hot path of FTS and HNSW search operations using rayon, leveraging the existing Track 007 infrastructure. Target: significant latency reduction for batch semantic-impact scans (50+ files) in ChangeGuard.

## Requirements
1. **FTS Parallel Sort**: Apply `par_sort_by_key` to the final score-sorted result set when the candidate count exceeds a threshold. Pure in-memory computation — no store access, no Sync requirement.
2. **HNSW Batch Distance**: Restructure `hnsw_search_level` inner loop to batch-collect unvisited neighbors, do sequential store access (`ensure_key`), then parallel distance computation over the warm cache. Safe reborrow of `&mut VectorCache` as `&VectorCache` after all mutations complete.
3. **Outer-Loop Parallelism** (deferred): Add `is_concurrent_read_safe()` to `StoreTx` trait; enable parallel KNN across parent tuples for RocksDB/Sled. Involves `unsafe` raw-pointer sharing; deferred to a later commit.

## API Contracts
- All `#[cfg(feature = "rayon")]` guarded; `compact-single-threaded` path unchanged.
- `VectorCache` must be `Sync` for parallel distance reads (it is: `FxHashMap` + enum, no interior mutability).
- `SessionTx` is not `Sync` — blocks outer-loop parallelism.

## Testing Strategy
- Existing 246-test suite passes with `compact,storage-rocksdb,requests`.
- `compact-single-threaded` compiles and produces identical results.
- New correctness test: parallel sort result == serial sort for identical FTS inputs.
