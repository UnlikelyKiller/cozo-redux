# Track 007 Plan: Pipeline Parallelism

## Phase 1: Join Parallelization
- [ ] Identify independent branches in `InnerJoin` in `ra.rs`.
- [ ] Use `rayon::join` to evaluate independent sub-expressions in the query plan where possible.
- [ ] Implement parallel tuple extension for hash joins.

## Phase 2: Parallel Iterator Integration
- [ ] Convert key `RelAlgebra` iterators to use `rayon::iter::ParallelIterator` where beneficial.
- [ ] Focus on `UnificationRA` and `FilteredRA` where bytecode evaluation can be expensive.
- [ ] Benchmarking the overhead of `par_iter()` on small data sets to determine a threshold for switching to parallel.

## Phase 3: Scaling Audit
- [ ] Verify thread pool behavior on machines with high core counts.
- [ ] Ensure no deadlocks are introduced by nested Rayon calls.
- [ ] Final performance verification with `wiki_pagerank`.
