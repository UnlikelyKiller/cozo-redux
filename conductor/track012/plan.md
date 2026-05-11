# Track 012 Plan: Storage Scale — Vector Quantization (Product Quantization)

## Phase 1: Codebook Training & Storage
- [ ] Add `PqConfig { num_subspaces: u8, bits_per_code: u8 }` as optional field in `HnswIndexManifest`.
- [ ] Implement K-means on subspaces using `ndarray` + `rand` (already in deps).
- [ ] Store codebook as a special key prefix (`__pq_codebook`) in the index relation.
- [ ] Add runtime op: `:train_pq { :index idx_name, :samples 10000 }`.
- [ ] Expose training command in `runtime/db.rs`.

## Phase 2: Encoding on Insert
- [ ] In `hnsw_put_vector()`: if PQ config present, encode vector to uint8 codes.
- [ ] Store codes at a separate key alongside the full vector (for re-ranking).
- [ ] Full vectors retained initially; phased out in Phase 4.

## Phase 3: Approximate Distance Search
- [ ] In `hnsw_search_level()`: precompute lookup tables before main loop.
  - `dist_table[subspace][centroid] = distance(q_subspace, codebook[subspace][centroid])`
- [ ] Use `dist_table` for candidate traversal (fast approximate distance).
- [ ] Use exact distance for final re-ranking of top `ef * overquery_factor` candidates.

## Phase 4: Migration & Compatibility
- [ ] Existing non-PQ indexes: unchanged (PQ is opt-in at index creation).
- [ ] Add `hnsw_convert_to_pq(name)` for converting existing indexes.
- [ ] Optionally drop full vectors after codebook is trained and codes stored.
