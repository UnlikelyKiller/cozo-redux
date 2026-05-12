# Track 008: Storage Layer - Architecture & Hygiene

## Objective
Optimize the storage layer for better performance in non-persistent and temporary scenarios, reducing the overhead of byte-encoding and improving deduplication speed.

## Requirements
1.  **Native In-Memory Storage**: Refactor `MemStorage` to optionally store `Tuple` objects directly instead of `Vec<u8>` keys/values, eliminating encoding/decoding overhead for in-memory databases.
2.  **Optimized Deduplication**: Update `RegularTempStore` to use a `HashSet` (specifically `FxHashSet` or `DashSet`) instead of `BTreeMap` for cases where order is not required (e.g., standard deduplication).
3.  **Invariant Preservation**: Ensure that switching to `HashSet` does not break any implicit ordering requirements in the Datalog engine (e.g., for stable Magic Set rewrites).
4.  **Clean Provenance**: Ensure all storage changes are tracked via the ChangeGuard ledger and maintain architectural boundaries.

## API Contracts
- `Storage` trait remains unchanged.
- `MemStorage` internal structures refactored.

## Testing Strategy
- **Micro-benchmarks**: Measure the latency of `put` and `get` operations in `MemStorage`.
- **Deduplication Benchmarks**: Measure the speed of `TempStore` operations for large intermediate results.
