# Track 003: Platform Modernization

## Objective
Migrate from the unmaintained `instant` crate to `web-time` to ensure WASM compatibility and alignment with modern Rust standards.

## API Contracts & Constraints
- **`instant`** -> **`web-time`**: Replace all usages of `Instant::now()` and `SystemTime::now()`.
- **Type Safety**: Ensure `web_time` types are correctly handled in benches and tests.
- **TDD Principle**: WASM build must pass without panics (verified via `wasm-pack` if possible, or build check).

## Technical Context
- `Instant::now()` is used extensively in `cozo-core/benches/` and `cozo-core/tests/` for performance measurements.
- Standard `std::time::Instant` panics on WASM targets; `web-time` polyfills this correctly.
