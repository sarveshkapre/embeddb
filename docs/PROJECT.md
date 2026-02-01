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
- Contract tests: `cargo test -p embeddb-server --features contract-tests`

## Next 3 improvements
1. Add SST flush + compaction (LSM basics)
2. Implement background job worker (batching + retries)
3. Add optional server endpoints for CRUD + search
