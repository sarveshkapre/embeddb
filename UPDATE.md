# Update (2026-02-01)

## Summary
- Added `embeddb` core APIs to list/describe tables, plus corresponding CLI commands.
- Added `embeddb-cli search-text` for query text embedding and kNN search.
- Made kNN sort robust against non-finite distances.
- Added table stats API/CLI for quick table health insight.
- Added optional HTTP server (`embeddb-server`) behind the `http` feature flag for CRUD + search.
- Added HTTP endpoints for `flush` and `compact`.

## Verification
- `make check`

## PR instructions
- No PRs requested; work is on `main`.
