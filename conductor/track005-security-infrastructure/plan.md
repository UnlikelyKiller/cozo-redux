# Plan: Track 005 - Security Infrastructure

## Phase 1: Tooling Installation
- `[x]` Install `pre-commit` via `pip`.
- `[x]` Enable `core.longpaths` in Git.
- `[x]` Create `.pre-commit-config.yaml`.
- `[x]` Create `.gitleaksignore` and `.semgrepignore`.

## Phase 2: Hook Setup
- `[x]` `pre-commit install`.
- `[/]` `pre-commit run --all-files` (Initialization & Scan).

## Phase 3: Push & Verification
- `[ ]` Commit configuration files.
- `[ ]` Push to `origin/main`.
- `[ ]` Verify CI gates pass on a sample (non-secret) change.
