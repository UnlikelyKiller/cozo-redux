# Conductor: CozoDB-redux

Master registry for development tracks and architectural upgrades.

## Active Tracks

| Track ID | Status | Objective | Owner |
| :--- | :--- | :--- | :--- |
| **013** | `In Progress` | Dependency Transitivity — swapvec path dependency for downstream lz4_flex fix | Orchestrator |

## Completed Tracks

| Track ID | Objective |
| :--- | :--- |
| **001** | Infrastructure & Security Patches (`lz4_flex`, `tokio`) — *partial: patch not transitive* |
| **012** | Storage Scale — Vector quantization (Product Quantization) |
| **002** | Unmaintained Hygiene (`lazy_static`, `adler`, `fxhash`) |
| **003** | Platform Modernization (`instant` -> `web-time`) |
| **004** | Serialization Overhaul (`bincode` -> `postcard`) |
| **005** | Security Infrastructure (Semgrep, Gitleaks, Pre-commit) |
| **006** | Memory Efficiency — DataValue shrinking + SmallVec Tuple |
| **007** | Query Execution — parallel joins, filter, unification |
| **008** | Storage Layer — TempStore write-buffer, ByteRange alloc elimination, sled range bounds |
| **009** | Search Performance — Parallel FTS sort + HNSW batch distance |
| **010** | HNSW Durability — Graph repair on deletion (re-link neighbors) |
| **011** | HNSW Precision — In-loop predicate filtering with ef expansion |

---
*Updated: 2026-05-11*
