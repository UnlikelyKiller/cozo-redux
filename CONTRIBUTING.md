## Contributing

Thanks for your interest in contributing to CozoDB-redux!

This is a maintained fork of the original [CozoDB](https://github.com/cozodb/cozo) project, which has not seen updates in several years. We keep the same MPL 2.0 license and aim to preserve compatibility while fixing bugs and adding performance improvements.

### How to contribute

* **Bug reports**: [Open an issue](https://github.com/UnlikelyKiller/cozo-redux/issues).
* **Feature requests**: [Open a discussion](https://github.com/UnlikelyKiller/cozo-redux/discussions) first — we want to keep the scope focused.
* **Pull requests**: Fork the repo, create a branch, and open a PR against `main`.

### Requirements for PRs

* All code must pass `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings`.
* All tests must pass: `cargo test --workspace`.
* If you change behavior, add a test.
* If you change the query engine or storage layer, run ChangeGuard impact analysis (`changeguard scan --impact`) and document the risk.

### What we are looking for

* Bug fixes (especially correctness or durability issues).
* Performance improvements with benchmarks.
* Documentation improvements.
* Dependency updates that fix security advisories.

### What we are cautious about

* Breaking changes to CozoScript syntax or public APIs.
* Changes that increase binary size significantly without clear benefit.
* Features that require new system dependencies or external services.

The upstream CLA requirement does **not** apply to this fork.
