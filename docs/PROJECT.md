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
1. Implement WAL checkpoint/truncation strategy to prevent unbounded WAL growth.
2. Add metadata filtering to kNN search (basic equality + numeric range).
3. Add lightweight metrics counters (embedding throughput/retry rate, WAL fsync counts, compaction durations).
