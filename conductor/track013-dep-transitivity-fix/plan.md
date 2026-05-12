# Track 013 — Implementation Plan

## Tasks

### Phase 1: Dependency Fix
- [x] Change `cozo-core/Cargo.toml` line 144: `swapvec = "0.3.0"` → `swapvec = { path = "../vendor/swapvec" }`
- [x] Remove `swapvec = { path = "vendor/swapvec" }` from workspace `[patch.crates-io]` in root `Cargo.toml`
- [x] Run `cargo tree -i lz4_flex` to confirm only 0.12.2 resolves
- [x] Run `cargo update` to refresh lockfile if needed

### Phase 2: CI Gate
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --lib --tests --features compact,storage-rocksdb,requests,storage-sled -- -D warnings`
- [x] `cargo test --workspace`

### Phase 3: Finalization
- [x] Update `conductor/conductor.md` — mark Track 012 complete, add Track 013
- [ ] Update `docs/status.md` with Track 013 completion
- [ ] Commit with provenance via ChangeGuard ledger
