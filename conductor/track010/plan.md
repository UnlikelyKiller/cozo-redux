# Track 010 Plan: HNSW Durability — Graph Repair on Deletion

## Phase 1: Repair Implementation — DONE
- [x] Add `hnsw_repair_node(target_key, layer, manifest, orig_table, idx_table, vec_cache)` helper to `SessionTx`.
- [x] Inside repair: check degree vs `max(1, m_max/2)` threshold; skip if sufficient.
- [x] Build candidate pool: current neighbors + 1-hop expansion (collect keys first to avoid borrow conflicts).
- [x] Call `hnsw_select_neighbours_heuristic` on candidate pool.
- [x] Write new bidirectional edges; update target degree and neighbor degrees.
- [x] Call `hnsw_shrink_neighbour` if new neighbor's degree exceeds m_max.

## Phase 2: Wire Repair into Deletion Path — DONE
- [x] Update `hnsw_remove_vec` signature: add `manifest: &HnswIndexManifest` and `vec_cache: &mut VectorCache`.
- [x] After deleting each edge in `hnsw_remove_vec`, call `hnsw_repair_node` on the former neighbor.
- [x] Update `hnsw_remove` signature: add `manifest: &HnswIndexManifest`; create `VectorCache` internally.
- [x] Update `hnsw_put_vector` internal call to `hnsw_remove_vec` to pass manifest and vec_cache.
- [x] Update `hnsw_put` call to `hnsw_remove` to pass manifest.
- [x] Update `stored.rs` call site: pass manifest (was `_`).
- [x] Remove the stale comment "this still has some probability of disconnecting the graph".
- [x] Verify 246 tests pass; both feature paths compile.
