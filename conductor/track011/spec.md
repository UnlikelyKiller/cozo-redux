# Track 011: HNSW Precision — In-Loop Predicate Filtering

## Objective
Move the HNSW search filter predicate from post-hoc (applied after finding K global neighbors) to in-loop (applied during graph traversal). Nodes that fail the predicate still navigate the graph but are excluded from the result set. Prevents silent result truncation for scoped queries like "only in src/security/".

## Problem
`hnsw_search_level()` collects the K globally-nearest neighbors without regard to the filter. The predicate is applied in `hnsw_knn()` after the search. When the top-K global neighbors don't pass the filter, the user silently gets fewer than K results. For ChangeGuard's "scoped by file path" queries, this is a correctness issue.

## Requirements
1. **In-loop filter parameter**: `hnsw_search_level()` accepts `filter: Option<(&[Bytecode], SourceSpan)>`.
2. **Biased traversal**: Nodes failing the predicate enter the candidates PQ (for graph navigation) but NOT `found_nn` (the result set).
3. **Separate traversal frontier**: When filter is active, a `traversal_nn` PQ (ef-bounded) tracks the exploration bound independently from `found_nn`, so non-passing nodes don't starve traversal or tighten the bound.
4. **ef expansion**: `hnsw_knn()` doubles ef (`config.ef * 2`) when filter is present, compensating for expected rejection rate.
5. **Construction path unaffected**: All calls in `hnsw_put_vector` pass `None`; construction semantics unchanged.
6. **Post-hoc filter preserved**: The existing post-hoc filter in `hnsw_knn` remains, covering filters that reference extra bindings (distance, field, vector) not available during in-loop eval.

## API Contracts
- `hnsw_search_level(q, ef, cur_level, orig_table, idx_table, found_nn, vec_cache, filter)` — filter added last.
- All existing callers updated to pass `None`.
- `hnsw_knn` passes `filter_ref` (from `filter_bytecode`) to level-0 search only.
- Filter evaluates on the base relation tuple (key + value fields); does NOT include distance/field/vector bindings.

## Testing Strategy
- **Correctness**: Insert 100 "a" + 100 "b" tagged vectors; filter to "a"; verify all K returned are tagged "a".
- **Completeness**: With K=5 and 100 valid "a" vectors, verify exactly 5 results returned (not fewer).
- Existing 246 tests pass with `compact,storage-rocksdb,requests`.
- `compact-single-threaded` compiles and produces identical results.
