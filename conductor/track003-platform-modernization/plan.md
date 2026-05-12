# Plan: Track 003 - Platform Modernization

## Phase 1: Dependency Integration
- `[ ]` Add `web-time = "1.1.0"` to `cozo-core/Cargo.toml`.
- `[ ]` Run `cargo audit` to confirm `instant` is flagged (RUSTSEC-2024-0384).

## Phase 2: Refactoring (TDD)
- `[ ]` Replace `std::time::Instant` with `web_time::Instant` in `cozo-core/src/lib.rs`.
- `[ ]` Replace imports in `benches/*.rs`.
- `[ ]` Replace imports in `tests/*.rs` (specifically `air_routes.rs`).

## Phase 3: Verification
- `[ ]` `cargo check --workspace`
- `[ ]` `cargo build --target wasm32-unknown-unknown` (If environment allows).
- `[ ]` `cargo test --workspace`
