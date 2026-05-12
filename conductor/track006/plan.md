# Track 006 Plan: Memory Efficiency

## Phase 1: DataValue Shrinking
- [x] Add `size_of` check to unit tests to establish baseline.
- [x] Box the `List` variant in `DataValue`.
- [x] Box the `Set` variant in `DataValue`.
- [x] Box the `Vec` (Vector) variant in `DataValue`.
- [x] Box the `Json` variant in `DataValue`.
- [x] Box the `Regex` variant in `DataValue`.
- [x] Verify `DataValue` size is <= 32 bytes.
- [x] Fix any resulting compilation errors in data accessors.

## Phase 2: Tuple SmallVec Migration
- [x] Add `smallvec` dependency to `cozo-core/Cargo.toml`.
- [x] Redefine `Tuple` as `SmallVec<[DataValue; 6]>` (balanced capacity).
- [x] Update `decode_tuple_from_key` and other constructors to use `SmallVec`.
- [x] Audit `tuple.clone()` calls and ensure they leverage SmallVec's efficient cloning where possible.

## Phase 3: Verification & Benchmarking
- [ ] Run full test suite (`cargo test`).
- [ ] Run `pokec` benchmark and compare with baseline.
- [ ] Run `air_routes` tests and verify no regressions.
- [ ] Perform a manual memory audit using `heaptrack` if possible.
