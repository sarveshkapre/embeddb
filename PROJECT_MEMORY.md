# PROJECT_MEMORY

## Decisions

### 2026-02-09: Add WAL checkpoint/rotation to bound WAL growth
- Decision: Implement a DB-level `checkpoint` that flushes memtables to SSTs, then rewrites `wal.log` to a compact snapshot (tables + `next_row_id` + embedding state) and safely rotates via `wal.prev` to tolerate interrupted checkpoints.
- Why: WAL growth is unbounded in the current design; production usage needs a supported way to compact/rotate WAL without losing ID allocation or embedding job state.
- Evidence:
  - `crates/embeddb/src/lib.rs` (`checkpoint`, `CheckpointStats`, rotation + recovery fallback, `flush_table_state`)
  - `crates/embeddb/src/storage/wal.rs` (`WalRecord::SetNextRowId`, `Wal::create_new`, `Wal::sync`)
  - `crates/embeddb-cli/src/main.rs` (`checkpoint`)
  - `crates/embeddb-server/src/main.rs` (`POST /checkpoint` + contract/smoke coverage)
  - Tests: `tests::checkpoint_truncates_wal_and_preserves_next_row_id`, `tests::checkpoint_preserves_embedding_meta_and_vectors`, `tests::open_recovers_from_interrupted_checkpoint_wal_rotation`
  - Smoke: `bash scripts/http_process_smoke.sh` (now includes `/checkpoint`)
- Commit: `125b5a5b8f87b52f59863d278f5edf9528b7c022`
- Confidence: high
- Trust label: verified-local-smoke
- Follow-ups:
  - (done) Add opt-in automatic checkpointing when `wal_bytes` crosses a threshold (config + CLI/HTTP override).
  - Consider persisting embedding vectors/meta outside WAL to further reduce checkpoint size.

### 2026-02-09: Add bounded background embedding retries/backoff (persisted in WAL)
- Decision: Track per-row embedding retry metadata (`attempts`, `next_retry_at_ms`) and apply exponential backoff; only mark jobs `failed` after exceeding max attempts.
- Why: Make background embedding processing more production-safe by avoiding tight failure loops while still converging without manual operator intervention for transient failures.
- Evidence:
  - `crates/embeddb/src/schema.rs` (`EmbeddingMeta`)
  - `crates/embeddb/src/storage/wal.rs` (`WalRecord::UpdateEmbeddingStatus` backward-compatible fields)
  - `crates/embeddb/src/lib.rs` (`process_pending_jobs_internal_at`, retry scheduling)
  - Tests: `tests::embedding_retry_backoff_defers_until_next_retry_time`, `tests::retry_failed_embedding_job_resets_status_and_error`
- Commit: `5f674804fd17041435b66242ebc8042961883984`
- Confidence: high
- Trust label: trusted
- Follow-ups:
  - Expose retry metadata in a user-facing surface (CLI/HTTP) or add counters for retry rate.

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

### 2026-02-09: Add metadata filtering for brute-force kNN search (MVP)
- Decision: Added scalar metadata filtering to brute-force kNN search (MVP `AND` filters: equality + numeric ranges) and exposed it via HTTP and CLI.
- Why: Vector search without scalar filtering is hard to use in real apps; most practical retrieval needs constraints like `tenant_id`, `is_active`, and numeric ranges.
- Evidence:
  - `crates/embeddb/src/lib.rs` (`FilterCondition`, `FilterOp`, `search_knn_filtered`, test `tests::search_knn_filtered_applies_scalar_filters`)
  - `crates/embeddb-server/src/main.rs` (`filter` parsing + request schema updates)
  - `crates/embeddb-cli/src/main.rs` (`search --filter`, `search-text --filter`)
  - Smoke: `bash scripts/http_process_smoke.sh` (filtered search assertion)
  - Docs: `docs/HTTP.md`, `README.md`
- Commit: `58f058a760655cfc91cf80267f8a41f52814dbc4`
- Confidence: med-high
- Trust label: verified-local-smoke
- Follow-ups:
  - Add filter pushdown (avoid per-hit SST point lookups) once storage gains a columnar/secondary index surface.

### 2026-02-09: Add opt-in WAL auto-checkpointing (byte threshold)
- Decision: Added `Config.wal_autocheckpoint_bytes` which triggers a preflight WAL `checkpoint()` before any operation that appends to WAL when `wal.log` is at/above the threshold.
- Why: Operators need a low-touch way to keep WAL growth bounded without external orchestration, while avoiding error-after-success semantics for writes.
- Evidence:
  - `crates/embeddb/src/lib.rs` (`Config::with_wal_autocheckpoint_bytes`, `preflight_wal_autocheckpoint`, test `tests::wal_autocheckpoint_triggers_before_write`)
  - `crates/embeddb-server/src/main.rs` (`EMBEDDB_WAL_AUTOCHECKPOINT_BYTES`)
  - `crates/embeddb-cli/src/main.rs` (`--wal-autocheckpoint-bytes`)
  - Docs: `docs/HTTP.md`, `README.md`
- Commit: `58f058a760655cfc91cf80267f8a41f52814dbc4`
- Confidence: high
- Trust label: verified-local-tests

## Verification Evidence
- `cargo fmt --all` (pass)
- `cargo clippy --workspace --all-targets -- -D warnings` (pass)
- `cargo test --workspace` (pass)
- `cargo test -p embeddb-server --features http,contract-tests` (pass)
- `bash scripts/http_process_smoke.sh` (pass)
- `make check` (pass)
