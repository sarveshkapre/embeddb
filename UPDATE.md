# Update (2026-02-01)

## Summary
- Added `embeddb` core APIs to list/describe tables, plus corresponding CLI commands.
- Added `embeddb-cli search-text` for query text embedding and kNN search.
- Made kNN sort robust against non-finite distances.
- Added table stats API/CLI for quick table health insight.
- Added optional HTTP server (`embeddb-server`) behind the `http` feature flag for CRUD + search.
- Added HTTP endpoints for `flush` and `compact`.
- Added HTTP API reference doc and examples.
- Added JSON Schema contract tests for HTTP request payloads.
- Added response and error JSON schema contract coverage.
- Added list/describe response schema contract tests.
- Added stats/search/process response schema contract tests.
- Added row CRUD response schema contract tests.

## Verification
- `make check`
- `cargo test -p embeddb-server --features contract-tests`

## PR instructions
- No PRs requested; work is on `main`.
