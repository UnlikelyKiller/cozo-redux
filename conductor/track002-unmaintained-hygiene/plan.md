# Plan: Track 002 - Unmaintained Hygiene

## Phase 1: adler to adler2
- `[ ]` Replace `adler` with `adler2` in `Cargo.toml`.
- `[ ]` Update imports in source code (if any direct usages exist).
- `[ ]` Verify build.

## Phase 2: fxhash to rustc-hash
- `[ ]` Replace `fxhash` with `rustc-hash` in `Cargo.toml`.
- `[ ]` Globally search and replace `fxhash::` with `rustc_hash::`.
- `[ ]` Fix any associated type mismatches.

## Phase 3: Verification
- `[ ]` `cargo check`
- `[ ]` `cargo test`
