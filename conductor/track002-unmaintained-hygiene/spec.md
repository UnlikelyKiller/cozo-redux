# Track 002: Unmaintained Hygiene

## Objective
Migrate from unmaintained crates (`adler`, `fxhash`) to their modern equivalents (`adler2`, `rustc-hash`) to improve long-term project stability.

## API Contracts & Constraints
- **`adler`** -> **`adler2`**: Drop-in replacement for checksum logic.
- **`fxhash`** -> **`rustc-hash`**: Replace `FxHashMap` and `FxHashSet` exports.
- **Maintenance**: Ensure all new dependencies are actively maintained.
- **TDD Principle**: Verify hash consistency before and after replacement.

## Technical Context
- `fxhash` is transitive via `jieba-rs` and `sled`. We should check if upgrading these crates solves the issue before applying manual patches.
- `adler` is used for internal CRC/checksum validation in storage layers via `miniz_oxide`.
