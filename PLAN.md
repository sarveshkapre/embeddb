# EmbedDB

EmbedDB is a single-node embedded database (Rust) with durable WAL-backed writes, LSM storage (memtable → SST + compaction), typed tables, and automatic per-row embeddings with local-first vector search.

## Features (current)
- Durable primary writes (WAL first).
- Typed tables (schema validation) with row CRUD.
- Embedding jobs with idempotent status tracking (`pending`/`ready`/`failed`) and content hashing.
- Brute-force kNN search (cosine/L2) over stored vectors.
- SST flush + basic L0 compaction (tombstones supported).
- CLI for core operations (create/insert/get/delete/jobs/process/search/flush/compact + list/describe tables).

## Shipped (this run)
- Added table introspection APIs (`list_tables`, `describe_table`) and CLI commands (`list-tables`, `describe-table`).
- Added `embeddb-cli search-text` to embed query text and run kNN search without manual vectors.
- Made kNN sort robust (`total_cmp`) to avoid panics on non-finite distances.
- Added table stats API and CLI (`table_stats`, `table-stats`) for quick table health insight.
- Added optional HTTP server (`embeddb-server`) behind the `http` feature flag for CRUD + search.
- Added HTTP endpoints for `flush` and `compact`.
- Added HTTP API reference doc with example payloads.

## Next (tight scope)
- More crash-recovery/compaction correctness tests.
- Add JSON schema/contract tests for HTTP payloads.

## Top risks / unknowns
- Compaction correctness and read visibility across memtable/SST levels.
- WAL durability/fsync strategy tradeoffs (performance vs correctness).
- Embedding job idempotency and failure handling across crashes.

## Commands
See `docs/PROJECT.md` for the full command list; common ones:
- Setup: `make setup`
- Quality gate: `make check`
- CLI help: `cargo run -p embeddb-cli -- --help`
