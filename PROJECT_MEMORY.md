# PROJECT_MEMORY

## Decisions

### 2026-02-09: Unify row visibility across memtable and SST for core mutation/read paths
- Decision: Added shared SST-aware row lookup helpers and reused them in `update_row`, `delete_row`, `get_row`, and `process_pending_jobs`.
- Why: Core behavior diverged by path; updates required memtable residency and pending embeddings could stall after flush/reopen.
- Evidence:
  - `crates/embeddb/src/lib.rs`
  - `tests::update_row_after_flush_and_compaction`
  - `tests::process_pending_jobs_after_flush_and_reopen`
- Commit: `0e0b5d5547660e47d14276f2b5fd3487d2aff914`
- Confidence: high
- Trust label: verified-local-tests
- Follow-ups:
  - Add explicit retry/backoff semantics for failed embedding jobs.
  - Add metrics around pending-job drain rate after reopen.

### 2026-02-09: Add process-level HTTP smoke verification to CI
- Decision: Added `scripts/http_process_smoke.sh` and a dedicated CI step that boots the real server and performs CRUD/search/flush/compact requests.
- Why: In-process router tests did not prove startup/listening/runtime behavior of the full server process.
- Evidence:
  - `.github/workflows/ci.yml`
  - `scripts/http_process_smoke.sh`
- Commit: `4eba79164fe02f44f0b224c3ad61eaef7bda1758`, `055da7202f4c4407669595beac9d5713f60ffe26`
- Confidence: high
- Trust label: verified-local-smoke
- Follow-ups:
  - (done) Capture and upload server log artifacts on CI smoke failure.

### 2026-02-09: Add operator controls for embedding job lifecycle (retry + bounded processing)
- Decision:
  - Added `retry_failed_jobs` to reset `failed` jobs back to `pending`.
  - Added bounded processing via `process_pending_jobs_with_limit` and exposed it via CLI (`process-jobs --limit`) and HTTP (`/tables/:table/jobs/process?limit=`).
  - Made `list_embedding_jobs` deterministic (sorted by `row_id`) for stable CLI/HTTP output.
- Why: Production operators need a safe way to recover from transient embedder failures and to bound job processing latency per request.
- Evidence:
  - `crates/embeddb/src/lib.rs` (new APIs + tests)
  - `crates/embeddb-cli/src/main.rs`
  - `crates/embeddb-server/src/main.rs`
  - `docs/HTTP.md`
- Commit: `de72907019a7c52142738fced4dd479bc5ef5b53`
- Confidence: high
- Trust label: verified-local-tests

### 2026-02-09: Upload HTTP process smoke logs as CI artifacts on failure
- Decision: Preserve smoke logs via `EMBEDDB_SMOKE_DIR` and upload them as a GitHub Actions artifact when the smoke step fails.
- Why: Faster CI diagnosis when the server fails to start or endpoints regress.
- Evidence:
  - `.github/workflows/ci.yml`
  - `scripts/http_process_smoke.sh`
  - `.gitignore`
- Commit: `82012e33e08cf0154b5f8f2ef5de3a4c99f0c3c6`
- Confidence: high
- Trust label: verified-local-smoke

### 2026-02-09: Add DB-level stats for operational visibility (tables + WAL bytes)
- Decision: Added `db_stats` (core), `db-stats` (CLI), and `GET /stats` (HTTP) to expose table count and current WAL size in bytes.
- Why: WAL growth and overall DB shape are key operational signals; exposing them makes it easier to diagnose slowdowns and validate cleanup work.
- Evidence:
  - `crates/embeddb/src/lib.rs`
  - `crates/embeddb-cli/src/main.rs`
  - `crates/embeddb-server/src/main.rs`
  - `docs/HTTP.md`
- Commit: `1436fa851761529fa6fe40de898a471797090947`
- Confidence: high
- Trust label: verified-local-tests

## Verification Evidence
- `cargo fmt --all -- --check` (pass)
- `cargo clippy --workspace --all-targets -- -D warnings` (pass)
- `cargo test --workspace` (pass)
- `cargo test -p embeddb-server --features http,contract-tests` (pass)
- `bash scripts/http_process_smoke.sh` (pass)
- `cargo build --workspace` (pass)
