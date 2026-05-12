# Track 011 Plan: HNSW Precision — In-Loop Predicate Filtering

## Phase 1: In-Loop Filter + ef Expansion — DONE (commit `2919397a`)
- [x] Add `filter: Option<(&[Bytecode], SourceSpan)>` parameter to `hnsw_search_level()`.
- [x] Initialize `traversal_nn: Option<PriorityQueue<...>>` when filter is Some; seed it with the entry-point set from `found_nn`.
- [x] Use `traversal_nn` for traversal bound (outer break and inner push condition) when active; fall back to `found_nn` when filter is None (no overhead).
- [x] In the heap-update loop: pass to traversal frontier unconditionally; check predicate via `orig_table.get(self, &key.0)` + `eval_bytecode_pred` before adding to `found_nn`.
- [x] Skip `found_nn.pop()` cap when filter is active (caller truncates via `ret.truncate`).
- [x] Add `pred_stack: Vec<DataValue>` local inside `hnsw_search_level` for bytecode evaluation.
- [x] Update all callers in `hnsw_put_vector` to pass `None`.
- [x] In `hnsw_knn`: compute `ef_actual = if filter_bytecode.is_some() { config.ef * 2 } else { config.ef }`.
- [x] In `hnsw_knn`: build `filter_ref = filter_bytecode.as_ref().map(|(code, span)| (code.as_slice(), *span))`.
- [x] Pass `None` to upper-level searches; pass `filter_ref` and `ef_actual` to level-0 search.
- [x] Add correctness and completeness tests.

## Phase 2: Dynamic ef Expansion — FUTURE
- [ ] Continue searching until `valid_found >= k` OR candidate queue exhausted (instead of hard 2× ef).
- [ ] Track `valid_found` count inside `hnsw_search_level` alongside `traversal_nn`.

## Phase 3: Query-Time ef Expansion Parameter — FUTURE
- [ ] Expose `:ef_expansion N` as a search-time option in the Cozo query language.
- [ ] Wire through `HnswSearch` config and down to `hnsw_knn`.
