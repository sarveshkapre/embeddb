# PROJECT

## Commands
- Setup: `make setup`
- Dev: `make dev`
- Test: `make test`
- Lint: `make lint`
- Typecheck: `make typecheck`
- Build: `make build`
- Check: `make check`
- Release: `make release`

## HTTP server (optional)
- Run: `cargo run -p embeddb-server --features http`
- Env: `EMBEDDB_ADDR=127.0.0.1:8080`, `EMBEDDB_DATA_DIR=./data`
- Contract + smoke tests: `cargo test -p embeddb-server --features http,contract-tests`
- Process-level smoke: `bash scripts/http_process_smoke.sh`

## Next 3 improvements
1. Implement background embedding retries/backoff with observability counters.
2. Add metrics counters for embedding throughput, WAL fsync counts, and compaction durations.
3. Add HNSW v1 index for faster search on larger datasets.
