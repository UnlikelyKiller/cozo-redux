# Track 001: Infrastructure & Security Patches

## Objective
Upgrade vulnerable dependencies identified by RUSTSEC to their patched versions while ensuring build and test integrity. This track follows strict TDD: every fix must be preceded by a verification step.

## API Contracts & Constraints
- **`lz4_flex`**: Upgrade from 0.10.0 to 0.12.1. (Impacts: `swapvec` integration).
- **`lru`**: Ensure all transitive references are updated to 0.16.3+ (Check parents: `tikv-client`, `rocksdb`).
- **Zero Regression**: Full test suite must pass after upgrades.
- **TDD Principle**: Use `cargo audit` and targeted unit tests to verify vulnerability presence/absence.

## Technical Context
- `lz4_flex` is a dependency of `swapvec`. We must verify if `swapvec` 0.3.0 is compatible with `lz4_flex` 0.12 or if we need a patch.
- `lru` is a common transitive dependency in query engines and caches.
