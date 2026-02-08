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

## Next 3 improvements
1. Add update semantics for rows that have already flushed to SST.
2. Add end-to-end server startup smoke in CI (process-level check with HTTP calls).
3. Implement background embedding retries/backoff with observability counters.
