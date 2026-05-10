# Dependency Upgrade & Security Hardening Plan

This document outlines the planned upgrades to address upstream vulnerabilities and unmaintained dependencies identified in the CozoDB-redux fork.

## 1. Security Advisories (RUSTSEC)

| Dependency | Advisory | Severity | Current | Target | Impact Path |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **lz4_flex** | [RUSTSEC-2026-0041](https://rustsec.org/advisories/RUSTSEC-2026-0041) | High | 0.10.0 | **>=0.11.6** | `cozo` -> `swapvec` -> `lz4_flex` |
| **lru** | [RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002) | Low | N/A | **0.16.3** | *No longer present in tree* |

### Notes on `lz4_flex`:
- **Issue**: Memory leak/uninitialized memory leakage during malformed decompression.
- **Fix**: Upgrade to 0.11.6+ or 0.12.1+.
- **Mitigation (if upgrade fails)**: Enable `safe-decode` feature flag.

## 2. Unmaintained Dependencies

These crates are flagged as unmaintained and should be replaced with modern community standards.

| Crate | Current | Recommended Alternative | Rationale |
| :--- | :--- | :--- | :--- |
| **adler** | 1.0.2 | **adler2** | Maintained fork with identical API. |
| **fxhash** | 0.2.1 | **rustc-hash** | Maintained by the Rust compiler team; industry standard. |
| **instant** | 0.1.12 | **web-time** | Correct cross-platform time handling (including WASM). |
| **bincode** | 1.3.3 | **postcard** / **rkyv** | Modern serialization with better safety and performance. |

## 3. Implementation Strategy

### Phase 1: Direct Upgrades
- Bump `lz4_flex` version in `Cargo.toml`.
- Attempt to patch transitive dependencies using the `[patch.crates-io]` section if direct bumping is blocked.

### Phase 2: Structural Replacements
- Replace `fxhash` usages with `rustc-hash`.
- Migrate `adler` to `adler2`.
- Evaluate the impact of replacing `bincode` (requires re-serialization of any persistent data formats).

---
*Generated: 2026-05-10*
