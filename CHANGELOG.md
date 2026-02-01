# CHANGELOG

## Unreleased
- Added table introspection APIs (`list_tables`, `describe_table`) and CLI commands (`list-tables`, `describe-table`).
- Added `embeddb-cli search-text` to embed query text and run kNN search without manual vectors.
- Made kNN sort robust against non-finite distances.
- Added table stats API and CLI command (`table_stats`, `table-stats`).
- Added optional HTTP server (`embeddb-server`) behind the `http` feature flag for CRUD + search.
- Added HTTP endpoints for `flush` and `compact`.
- Added HTTP API reference with example payloads.
- Added JSON Schema contract tests for HTTP request payloads.
- Added response and error JSON schema contract coverage.
- Added list/describe response schema contract tests.
- Added stats/search/process response schema contract tests.
- Added row CRUD response schema contract tests.
- Added flush/compact response schema contract tests.
- Scaffolded workspace, docs, and CLI/server placeholders.
- Added WAL with replay + truncation handling and basic crash-recovery tests.
- Implemented in-memory tables with typed schemas and row CRUD.
- Added embedding job queue with idempotent status updates and local embedder API.
- Added brute-force kNN search (cosine/L2) over stored vectors.
- Added SST flush + L0 compaction with tombstone support.
- Expanded CLI with table CRUD, job processing, search, flush, and compaction commands.
