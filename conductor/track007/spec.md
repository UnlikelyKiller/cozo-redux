# Track 007: Query Execution - Pipeline Parallelism

## Objective
Increase query throughput by parallelizing independent operations within a Datalog rule, specifically focusing on parallelizing joins and filters that are currently sequential.

## Requirements
1.  **Parallel Join Evaluation**: Modify `InnerJoin` and `NegJoin` implementation in `ra.rs` to allow parallel evaluation of left and right branches when they are independent.
2.  **Parallel Clause Mapping**: parallelize the `map_ok` and `filter_map_ok` chains in `RelAlgebra::iter` using Rayon's parallel iterators where the work per tuple is significant.
3.  **Adaptive Parallelism**: Ensure that parallelization doesn't introduce overhead for very small result sets.
4.  **Resource Governance**: Implement mechanisms to prevent parallel queries from exhausting the global Rayon thread pool, ensuring fair resource allocation.
5.  **Strict Error Propagation**: Ensure `Result` errors are preserved and prioritized across parallel boundaries.

## API Contracts
- No changes to public query APIs.
- Internal `RelAlgebra` structures may be updated to hold Rayon-friendly state.

## Testing Strategy
- **Benchmarks**: Use `wiki_pagerank.rs` and `pokec.rs` as these involve complex joins that will benefit from parallelism.
- **Stress Tests**: Run concurrent queries to ensure Rayon thread pool utilization is efficient and doesn't lead to starvation.
