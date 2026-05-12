# Plan: Track 004 - Serialization Overhaul

## Phase 1: Dependency Integration
- `[ ]` Add `postcard = { version = "1.0", features = ["use-std"] }` to `cozo-core/Cargo.toml`.
- `[ ]` Remove `bincode` from `cozo-core/Cargo.toml`.

## Phase 2: Refactoring (TDD)
- **swapvec Migration**:
    - `[ ]` Update `swapvec` integration to use `postcard::to_stdvec` and `postcard::from_bytes`.
- **fast2s Migration**:
    - `[ ]` Update `fast2s` to use `postcard`.

## Phase 3: Verification
- `[ ]` `cargo test` (Ensure all serialization/deserialization cycles are valid).
- `[ ]` Benchmark comparison (Expect smaller disk footprint due to varints).

## Phase 4: Finalization
- `[ ]` Commit with ledger category `ARCHITECTURE`.
