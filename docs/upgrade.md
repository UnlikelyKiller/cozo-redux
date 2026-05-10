# Dependency Upgrade & Security Hardening Plan

This document outlines the planned upgrades to address upstream vulnerabilities and unmaintained dependencies identified in the CozoDB-redux fork.

## 1. Security Advisories (RUSTSEC)

| Dependency | Advisory | Severity | Current | Target | Impact Path |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **lz4_flex** | [RUSTSEC-2026-0041](https://rustsec.org/advisories/RUSTSEC-2026-0041) | High | 0.10.0 | **>=0.11.6** | `cozo` -> `swapvec` -> `lz4_flex` |
| **lru** | [RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002) | Low | N/A | **0.16.3** | *Stacked Borrows violation* |

### Detailed Research: Security Upgrades
- **`lz4_flex` (0.10 -> 0.12.1)**:
    - **Breaking**: Feature flag inversion. `checked-decode` is now the default. `unchecked-decode` must be explicitly enabled for performance.
    - **Fix**: Resolves information leak from uninitialized memory.
- **`lru`**:
    - **Fix**: Addresses unsoundness in `IterMut` that violates Stacked Borrows.

## 2. Unmaintained Dependencies

| Crate | Current | Recommended Alternative | Rationale |
| :--- | :--- | :--- | :--- |
| **adler** | 1.0.2 | **adler2** | Maintained fork; drop-in replacement. |
| **fxhash** | 0.2.1 | **rustc-hash** | Maintained by Rust team; industry standard. |
| **instant** | 0.1.12 | **web-time** | WASM-compatible time standard; replaces abandoned crate. |
| **bincode** | 1.3.3 | **postcard** / **wincode** | Modern efficiency vs. legacy compatibility. |

### Detailed Research: Migrations
- **`instant` -> `web-time`**:
    - **Breaking**: Types are not interchangeable with `std::time::Instant`. Requires import replacement.
- **`bincode` -> `postcard`**:
    - **CRITICAL BREAKING**: Binary format is incompatible. Data serialized with `bincode` cannot be read by `postcard`.
    - **Decision**: Migrate to `postcard`. This will be a clean break in binary format, which is acceptable for the `cozo-redux` fork to prioritize safety and embedded-friendliness.

## 3. Conductor Tracks

The upgrade will be managed via the **Conductor** system in the `conductor/` directory.

### Track Roadmap:
1. **Track 001**: Infrastructure & Security Patches (`lz4_flex`, `lru`).
2. **Track 002**: Unmaintained Hygiene (`adler`, `fxhash`).
3. **Track 003**: Platform Modernization (`instant` -> `web-time`).
4. **Track 004**: Serialization Overhaul (`bincode` -> `postcard`).

---
*Generated: 2026-05-10*
