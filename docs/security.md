# CozoDB Security Assessment (Redux)

## Accepted Upstream Risks

The following vulnerabilities have been identified and accepted as low risk due to the operational context of **ChangeGuard** and **CozoDB-redux** as local CLI tools.

### 1. lz4_flex 0.10.0 (RUSTSEC-2026-0041) — HIGH
- **Impact**: Memory leak when processing malformed decompression input.
- **Attack Surface**: Only triggered when reading internal sled-backed data files (`.changeguard/state/ledger.cozo`).
- **Rationale**: ChangeGuard has no network exposure and does not ingest untrusted graph store data from external sources.

### 2. lru 0.12.5 (RUSTSEC-2026-0042) — LOW
- **Impact**: Stacked Borrows violation (UB under Miri).
- **Rationale**: No known exploitable security impact. Upstream lacks a compatible patched version.

### 3. Unmaintained Dependencies — LOW
- **Packages**: `adler`, `bincode`, `fxhash`, `instant`.
- **Rationale**: Transitive dependencies with no known CVEs.

## Migration Path
If risk elimination becomes mandatory, the planned migration path is:
- **Storage**: SQLite
- **Graph Logic**: Petgraph

---
*Note: This mirrors the documentation in `c:/dev/changeguard/.agents/skills/changeguard/references/internals.md`.*
