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

## CLI examples
```bash
# List tables
cargo run -p embeddb-cli -- list-tables

# WAL checkpoint (compact wal.log after flush/compaction cycles)
cargo run -p embeddb-cli -- checkpoint

# Table stats
cargo run -p embeddb-cli -- table-stats notes

# Text search (embeds the query via the local hash embedder)
cargo run -p embeddb-cli -- search-text notes --query-text "hello world" --k 5
```

## Server (optional HTTP, behind feature flag)
```bash
# Start HTTP server
cargo run -p embeddb-server --features http

# Override address/data dir
EMBEDDB_ADDR=127.0.0.1:9090 EMBEDDB_DATA_DIR=./data cargo run -p embeddb-server --features http
```

## Web Console
When the HTTP server is running, open `http://127.0.0.1:8080` to use the built-in console for
creating tables, inserting rows, processing embeddings, and running text search.

## HTTP examples
```bash
curl -s http://127.0.0.1:8080/health
curl -s -X POST http://127.0.0.1:8080/tables/notes/flush
curl -s -X POST http://127.0.0.1:8080/tables/notes/compact
```

HTTP contract + route smoke tests:
```bash
cargo test -p embeddb-server --features http,contract-tests
bash scripts/http_process_smoke.sh
```

Full HTTP reference: `docs/HTTP.md`.

## Repository layout
- `crates/embeddb`: core library
- `crates/embeddb-cli`: CLI (scaffold)
- `crates/embeddb-server`: optional server (scaffold)
- `docs/`: project docs and plans

## License
MIT.
