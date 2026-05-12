# Track 010: HNSW Durability — Graph Repair on Deletion

## Objective
Restore HNSW graph connectivity after node deletion by reconnecting under-connected former neighbors. Prevents the "phantom isolation" effect from ChangeGuard's watch-cycle delete+reinsert pattern, which silently fragments the graph over time.

## Problem
`hnsw_remove_vec()` deleted node N's self-record and all its edges, then decremented neighbor degrees. It did NOT reconnect neighbors. Over many watch cycles, isolated clusters formed at sparse upper levels, degrading recall silently. The comment "this still has some probability of disconnecting the graph" acknowledged the issue.

## Requirements
1. **Post-deletion repair**: After deleting node N's edges at each layer, for each former neighbor A: if A's degree falls below `max(1, m_max / 2)`, trigger reconnection.
2. **Candidate pool**: Current neighbors + 1-hop expansion (neighbors-of-neighbors), excluding N.
3. **Neighbor selection**: Reuse existing `hnsw_select_neighbours_heuristic` for candidate pruning.
4. **Degree maintenance**: Write new bidirectional edges and update degrees; call `hnsw_shrink_neighbour` if new neighbor exceeds m_max.
5. **Signature threading**: `hnsw_remove` and `hnsw_remove_vec` receive `manifest: &HnswIndexManifest` and `vec_cache: &mut VectorCache` to support repair.
6. **m_max levels**: Level 0 uses `m_max0`, all other levels use `m_max`.

## API Contracts
- `hnsw_remove(&mut self, orig_table, idx_table, manifest, tuple)` — manifest added.
- `hnsw_remove_vec(...)` — manifest and vec_cache added.
- `stored.rs` updated to pass manifest (was previously discarded with `_`).
- `hnsw_repair_node()` is private to `SessionTx`.

## Testing Strategy
- Existing 246-test suite passes with `compact,storage-rocksdb,requests`.
- `compact-single-threaded` compiles clean.
- Watch-pattern: insert → delete → reinsert cycle verifies no recall degradation.
