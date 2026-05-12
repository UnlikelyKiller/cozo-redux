# Track 012: Storage Scale — Vector Quantization (Product Quantization)

## Objective
Reduce HNSW index storage from 1,536 bytes/vector (384-dim F32) to ~8 bytes via Product Quantization (PQ). Target: 100k+ function-level embeddings fit on a developer laptop (currently ~150GB uncompressed, ~10MB with PQ at M=8 subspaces).

## Problem
ChangeGuard measured 683MB for just 300 files with zero-vectors. Non-zero vectors (real embeddings) will be significantly larger. Laptops can't hold a monorepo's full semantic index in RAM, making the watch-cycle indexing impractical at scale.

## Approach: Product Quantization
Split each 384d vector into M=8 subspaces of 48d each. Train 256 centroids per subspace via K-means on sampled vectors. Each vector encodes to 8 × uint8 codes = 8 bytes. 192× compression at ~95% recall.

## Requirements
1. **Codebook training**: K-means on subspaces; store codebook as a special key in the index relation. Runtime op: `:train_pq { :index idx_name, :samples 10000 }`.
2. **Encoding on insert**: If PQ config present in `HnswIndexManifest`, encode vector to uint8 codes. Store codes alongside full vector initially (for re-ranking in Phase 3).
3. **Approximate distance**: Precompute per-query lookup tables `dist_table[subspace][centroid]` before the main loop. Use for traversal; use full vectors for final re-ranking.
4. **Opt-in**: Existing non-PQ indexes unchanged. PQ activated at index creation via config flag.
5. **Migration**: `hnsw_convert_to_pq(name)` for converting existing indexes.

## API Contracts
- `PqConfig { num_subspaces: u8, bits_per_code: u8 }` added to `HnswIndexManifest` (optional).
- New runtime commands: `:train_pq`, `:convert_to_pq`.
- `Vector::Quantized(Vec<u8>)` variant may be added to `data/value.rs`.
- Existing `Vector::F32` / `Vector::F64` paths unchanged.

## Testing Strategy
- **Recall**: ≥ 90% recall (PQ top-10 ∩ exact top-10 / 10) on a 10k vector benchmark.
- **Memory**: 100k vectors ≤ 10MB encoded (vs. ~150GB raw F32).
- **Non-PQ regression**: All existing HNSW tests pass unmodified.
