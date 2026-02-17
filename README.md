# EmbedDB

Rust embedded database with WAL + LSM and automatic per-row embeddings for local-first vector search.

Primary writes commit durably first; embedding jobs then run asynchronously with idempotent status tracking. Typed tables, SST flush, and compaction are built in.

## Status
- MVP in progress. WAL + in-memory tables + embedding jobs + brute-force search + SST flush/L0 compaction implemented.
- Row updates and embedding job processing now work for rows that have already flushed to SSTs.

## Key goals
- Durable primary writes before embedding jobs
- Per-row embedding jobs with status (`pending`, `ready`, `failed`) and content hash (with bounded retries/backoff before terminal `failed`)
- Vector kNN search (cosine/L2) — brute-force MVP → HNSW v1
- Pluggable local-first embedder, optional remote embedder feature flag
- Observability + crash-recovery tests
- Embedded library with optional server + CLI

## Quickstart (scaffold)
```bash
make setup
make check
cargo run -p embeddb-cli -- --help
```

Note: EmbedDB holds an exclusive lock on the configured `data_dir` (via `embeddb.lock`). Only one
process can open a given `data_dir` at a time.

## CLI examples
```bash
# List tables
cargo run -p embeddb-cli -- list-tables

# WAL checkpoint (compact wal.log after flush/compaction cycles)
cargo run -p embeddb-cli -- checkpoint

# Snapshot export/restore (copy-only backup)
cargo run -p embeddb-cli -- snapshot-export ./snapshots/embeddb-1
cargo run -p embeddb-cli -- --data-dir ./data-restored snapshot-restore ./snapshots/embeddb-1

# Table stats
cargo run -p embeddb-cli -- table-stats notes

# List embedding jobs (includes retry metadata: attempts/next_retry_at_ms)
cargo run -p embeddb-cli -- jobs notes

# Text search (embeds the query via the local hash embedder)
cargo run -p embeddb-cli -- search-text notes --query-text "hello world" --k 5
```

## Server (optional HTTP, behind feature flag)
```bash
# Start HTTP server
cargo run -p embeddb-server --features http

# Override address/data dir
EMBEDDB_ADDR=127.0.0.1:9090 EMBEDDB_DATA_DIR=./data cargo run -p embeddb-server --features http

# Optional: auto-run WAL checkpoint before writes when WAL grows above a threshold (bytes)
EMBEDDB_WAL_AUTOCHECKPOINT_BYTES=50000000 cargo run -p embeddb-server --features http
```

## Web Console
When the HTTP server is running, open `http://127.0.0.1:8080` to use the built-in console for
creating tables, inserting rows, processing embeddings, and running text search.

Current console workflows include:
- table + DB stats at a glance
- embedding job queue inspection (`pending` / `ready` / `failed` with retry metadata)
- retry-failed jobs and bounded processing (`limit`)
- one-click checkpoint and snapshot export/restore actions
- filter-aware text search and row-level inspect/delete actions

## HTTP examples
```bash
curl -s http://127.0.0.1:8080/health
curl -s http://127.0.0.1:8080/stats
curl -s http://127.0.0.1:8080/tables/notes/jobs
curl -s -X POST http://127.0.0.1:8080/tables/notes/flush
curl -s -X POST http://127.0.0.1:8080/tables/notes/compact
```

`GET /stats` and `GET /tables/:table/stats` include runtime operation counters (durable WAL appends,
job throughput/failures/retries, and flush/compact/checkpoint counters + durations).

Filtered search (MVP `AND` filters: equality + numeric ranges):
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/search-text \
  -H "Content-Type: application/json" \
  -d '{"query_text":"hello world","k":5,"filter":[{"column":"age","op":"Gte","value":21}]}'
```

HTTP contract + route smoke tests:
```bash
cargo test -p embeddb-server --features http,contract-tests
bash scripts/http_process_smoke.sh
bash scripts/http_console_smoke.sh
```

Self-hosted CI setup and runner registration:
```bash
bash scripts/setup_self_hosted_runner.sh
make ci-local-self-hosted
```
Guide: `docs/SELF_HOSTED_RUNNER.md`.

Full HTTP reference: `docs/HTTP.md`.

## Repository layout
- `crates/embeddb`: core library
- `crates/embeddb-cli`: CLI (scaffold)
- `crates/embeddb-server`: optional server (scaffold)
- `docs/`: project docs and plans

## License
MIT.
