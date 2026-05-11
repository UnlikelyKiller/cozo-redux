## Soft fork notice: CozoDB-redux

Hi CozoDB maintainers and community,

I wanted to let you know that I've started maintaining a fork at **https://github.com/UnlikelyKiller/cozo-redux**.

### Why the fork?

The upstream repository has not seen updates in several years, but CozoDB remains a valuable dependency for production projects. Rather than let it stagnate, I've begun applying bug fixes, performance improvements, and small feature additions while preserving the MPL 2.0 license and CozoScript compatibility.

### What's in the fork so far

* **Bug fixes:** HNSW graph repair on deletion, corrected in-loop predicate filtering.
* **Performance:** parallel joins/filters/unification via `rayon`, parallel FTS sort, batched HNSW distance computation, TempStore write-buffering, allocation-free range scans.
* **New features:** Product Quantization (`::hnsw train_pq`) for HNSW vector indexes.
* **Maintenance:** dependency updates (`tokio`, `lz4_flex`, `web-time`, `postcard`), security tooling, and clippy hygiene.

### Intent

This is a **continuation fork**, not a hostile one. If the original maintainers ever return, I am happy to upstream changes, discuss merging back, or transfer stewardship. The goal is simply to keep CozoDB usable and improving.

Feel free to close this issue if it's not useful — I mostly wanted to document the fork's existence for anyone searching the original repo.
