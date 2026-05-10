# Track 004: Serialization Overhaul

## Objective
Migrate from the unmaintained `bincode` v1 to `postcard` to improve security, reduce binary size (via varints), and ensure long-term maintenance.

## API Contracts & Constraints
- **`bincode`** -> **`postcard`**: Migration of `swapvec` and `fast2s` serialization.
- **Binary Compatibility**: **NOT MAINTAINED**. This is a breaking change for persistent storage.
- **TDD Principle**: Existing serialization tests must be updated to assert new formats.

## Technical Context
- `swapvec` relies on `bincode` for disk spilling.
- `fast2s` uses `bincode` for its internal representation.
- **Decision**: For this "redux" fork, we accept the breaking change in favor of modern security and maintenance.
