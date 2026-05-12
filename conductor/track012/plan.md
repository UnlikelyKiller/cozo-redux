# Track 012 Plan: Storage Scale — Vector Quantization (Product Quantization)

## Phase 1: Codebook Training & Storage
- [x] Add `PqConfig { num_subspaces: usize, num_centroids: usize }` as optional field in `HnswIndexManifest`.
- [x] Implement K-means on subspaces using manual Lloyd algorithm.
- [x] Store codebook as a special key in the index relation.
- [x] Add runtime op: `::hnsw train_pq rel:index { subspaces: N, centroids: M, samples: K }`.
- [x] Expose training command in `runtime/db.rs`.

## Phase 2: Encoding on Insert
- [x] In `hnsw_put_vector()`: if PQ config present, encode vector to uint8 codes.
- [x] Store codes at a separate key in the index relation.
- [x] Full vectors retained initially; phased out in Phase 4.

## Phase 3: Approximate Distance Search
- [x] In `hnsw_search_level()`: precompute lookup tables before main loop.
  - `dist_table[subspace][centroid] = distance(q_subspace, codebook[subspace][centroid])`
- [x] Use `dist_table` for candidate traversal (fast approximate distance).
- [ ] Use exact distance for final re-ranking of top `ef * overquery_factor` candidates.

## Phase 4: Migration & Compatibility
- [x] Existing non-PQ indexes: unchanged (PQ is opt-in via `train_pq`).
- [ ] Add `hnsw_convert_to_pq(name)` for converting existing indexes.
- [ ] Optionally drop full vectors after codebook is trained and codes stored.

## Testing
- [x] `test_hnsw_pq_training_and_search`: verifies training, storage, and search correctness.
- [x] All 179 existing tests pass without regression.
- [x] `cargo fmt` and `cargo clippy -- -D warnings` clean.
