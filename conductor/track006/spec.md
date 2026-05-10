# Track 006: Memory Efficiency - Core Data Structures

## Objective
Reduce the memory footprint of CozoDB's core data representation and minimize heap allocations during query execution by optimizing `DataValue` and `Tuple`.

## Requirements
1.  **Shrink `DataValue`**: Reduce the size of the `DataValue` enum from 56 bytes to <= 32 bytes by boxing large or rare variants (`List`, `Set`, `Vec`, `Json`, `Regex`).
2.  **Optimize `Tuple`**: Replace `Vec<DataValue>` with `SmallVec<[DataValue; 4]>` (or 8) to keep common small tuples on the stack.
3.  **Preserve Semantics**: Ensure all existing comparisons, hashing, and serialization behaviors remain unchanged.
4.  **Performance Improvement**: Target a measurable reduction in memory usage and improvement in benchmark execution time.
5.  **Recursion Safety**: Boxing large variants must also protect against stack overflows in recursive operations (e.g., `PartialOrd`, `Hash`) for deeply nested structures.

## API Contracts
- `Tuple` remains a public type alias but now points to a `SmallVec` implementation.
- `DataValue` public interface (getters/setters) remains unchanged.

## Testing Strategy
- **Unit Tests**: Add tests to `value.rs` to assert `size_of::<DataValue>()`. Include tests for deeply nested lists to verify stack safety.
- **Integration Tests**: Run `air_routes.rs` and other full-system tests.
- **Benchmarks**: Use `pokec.rs` and `wiki_pagerank.rs` to measure the impact of reduced allocations and better cache locality.
