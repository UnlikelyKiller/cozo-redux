# Track 008 Plan: Storage Layer Optimization

## Phase 1: TempStore Optimization
- [ ] Profile `RegularTempStore` usage to identify cases where `BTreeMap` is overkill.
- [ ] Implement `HashSet`-based deduplication in `temp_store.rs`.
- [ ] Verify that ordering invariants are still maintained where required (e.g. for range scans).

## Phase 2: Native MemStorage
- [ ] Create a variant of `MemStorage` that stores `Arc<Tuple>` directly.
- [ ] Implement `StoreTx` for the native tuple storage.
- [ ] Compare performance with the existing byte-encoded `MemStorage`.

## Phase 3: Final Hygiene
- [ ] Audit all storage implementations for redundant `clone()` and `to_vec()` calls.
- [ ] Final verification of persistence backends (RocksDB, SQLite) to ensure no regressions from core data structure changes.
