# Plan: Track 001 - Security Patches

## Phase 1: Baseline & TDD Setup
- `[ ]` Run `cargo audit` to confirm existing vulnerabilities.
- `[ ]` Audit `lru` parents: `cargo tree -i lru`.
- `[ ]` Audit `lz4_flex` parents: `cargo tree -i lz4_flex`.

## Phase 2: Implementation (TDD)
- **lz4_flex Update**:
    - `[ ]` Add `[patch.crates-io]` for `lz4_flex = "0.12.1"` in workspace root `Cargo.toml`.
    - `[ ]` Verify update with `cargo audit`.
- **lru Update**:
    - `[ ]` Identify if `lru` is pinned by `tikv-client` or `rocksdb`.
    - `[ ]` Apply patch or update direct dependency if possible.
    - `[ ]` Verify update with `cargo audit`.

## Phase 3: Verification & CI Gate
- `[ ]` `cargo check --workspace`
- `[ ]` `cargo test --workspace` (Ensure `swapvec` functionality is intact).
- `[ ]` `changeguard verify`

## Phase 4: Finalization
- `[ ]` Commit changes to ChangeGuard ledger.
- `[ ]` Push to `origin/main`.
