# CozoDB-redux Release Notes

## v0.8.0-redux

This is the consolidated release of the CozoDB-redux fork, incorporating 14 major development tracks for performance, security, and HNSW stability.

### New features

* **Product Quantization for HNSW** (`::hnsw train_pq`) — train codebooks on existing indexes to reduce vector storage and speed up approximate search.
* **In-loop predicate filtering for HNSW** — `filter:` clauses in HNSW queries now use biased traversal with `ef` expansion for correct K-results.

### Bug fixes

* **HNSW Engine Hardening** — Two-phase removal logic ensures no stale edges remain in the graph after deletion, preventing "key not found" panics.
* **Safe Error Propagation** — Replaced internal `unwrap()` and `expect()` calls with `miette`-based `Result` propagation across HNSW and storage layers.
* **HNSW graph repair on deletion** — Former neighbors whose degree drops too low are automatically reconnected via heuristic candidate selection.

### Performance & Efficiency

* **Parallel query execution** — Parallel iterators for joins, filters, unification, and FTS scoring.
* **Memory Efficiency** — `DataValue` shrinking and `SmallVec`-backed `Tuple` implementation to reduce heap overhead.
* **Allocation-free storage** — Elimination of `to_vec()` allocations in range scans across `MemStorage`, `TempStorage`, and `sled`.
* **TempStore write-buffer** — Optimized write path for temporary relations.

### Infrastructure & Security

* **Security Guardrails** — Automated `gitleaks`, `semgrep`, and `pre-commit` infrastructure.
* **Modernized Dependencies** — Migrated to `web-time`, `postcard`, and patched `lz4_flex`/`tokio` vulnerabilities.
* **Clean Hygiene** — Removed unmaintained/deprecated crates (`lazy_static`, `adler`, `fxhash`).

### Compatibility

* Preserves all upstream CozoScript syntax.
* Backward-compatible `HnswIndexManifest` loading.
* Full test suite validation (246 tests).
