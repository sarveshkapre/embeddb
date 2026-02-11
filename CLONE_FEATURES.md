# Clone Feature Tracker

## Context Sources
- README and docs
- TODO/FIXME markers in code
- Test and build failures
- Gaps found during codebase exploration

## Candidate Features To Do
- [ ] P2: Bulk ingest CLI (`ingest-jsonl`/`ingest-csv`) with progress and resumable embedding processing.
  Score: impact med-high | effort med-high | strategic fit high | differentiation low-med | risk low-med | confidence med
- [ ] P2: Add optional `--fail-on-embedding-errors` mode for `process-jobs` and HTTP process endpoint to support strict pipelines.
  Score: impact med | effort low-med | strategic fit med-high | differentiation low | risk low | confidence med
- [ ] P2: Add `jobs --status` filter (`pending|ready|failed`) in CLI + HTTP for faster ops triage on large tables.
  Score: impact med | effort low | strategic fit med | differentiation low | risk low | confidence med-high
- [ ] P2: Re-enable blocking `dependency-review` once dependency graph support is confirmed in repo settings.
  Score: impact low-med | effort low | strategic fit med | differentiation none | risk low | confidence med
- [ ] P2: Add a randomized crash/recovery test harness for WAL + flush/compact + reopen invariants.
  Score: impact med-high | effort med-high | strategic fit high | differentiation low | risk med | confidence low-med
- [ ] P2: Add startup integrity check command (`verify`) to scan SST/WAL consistency and report repair hints.
  Score: impact med-high | effort med | strategic fit high | differentiation med | risk med | confidence low-med
- [ ] P3: Add filter pushdown acceleration (avoid per-hit SST point lookups for scalar-filtered vector search).
  Score: impact med-high | effort high | strategic fit med-high | differentiation med | risk med | confidence low-med
- [ ] P3: HNSW v1 index for large-table search latency reduction (persisted index + rebuild strategy).
  Score: impact high | effort high | strategic fit high | differentiation med | risk med-high | confidence low
- [ ] P3: Hybrid keyword + vector search (term match/BM25 candidate stage + vector rerank).
  Score: impact med-high | effort high | strategic fit med-high | differentiation med | risk med | confidence low
- [ ] P3: Add read-only open mode to allow safe concurrent readers with lock semantics documented.
  Score: impact med | effort med | strategic fit med | differentiation low | risk med | confidence low-med
- [ ] P3: Add snapshot manifest checksums + restore verification to harden backup integrity.
  Score: impact med | effort med | strategic fit med-high | differentiation low | risk low-med | confidence med
- [ ] P3: Add metrics export endpoint in Prometheus text format for long-running server observability.
  Score: impact med | effort med | strategic fit med | differentiation low | risk low-med | confidence med

## Implemented
- [x] 2026-02-11: Added runtime operational counters to `db_stats` + `table_stats` (durable WAL appends/sync ops, embedding processed/failed/retried totals, flush/compact/checkpoint counts and cumulative durations).
  Evidence: `crates/embeddb/src/lib.rs` (`DbStats`, `TableStats`, runtime metrics wiring), `crates/embeddb/src/tests.rs` (`table_and_db_stats_track_runtime_operation_metrics`, `table_and_db_stats_track_retry_and_failure_metrics`), `crates/embeddb-server/src/main.rs` (expanded response contracts), `scripts/http_process_smoke.sh` (stats assertions).
- [x] 2026-02-11: Exposed embedding retry metadata in job listings (`attempts`, `next_retry_at_ms`) and added HTTP `GET /tables/:table/jobs`.
  Evidence: `crates/embeddb/src/lib.rs` (`EmbeddingJob`, `list_embedding_jobs`), `crates/embeddb-server/src/main.rs` (route + handler + `list_jobs_response_schema`), `docs/HTTP.md`, `README.md`, `scripts/http_process_smoke.sh`.
- [x] 2026-02-11: Extended HTTP contract/smoke coverage for observability surfaces and jobs visibility.
  Evidence: `crates/embeddb-server/src/main.rs` (db/table stats contract updates + jobs schema + smoke flow job assertion), `scripts/http_process_smoke.sh`, `CHANGELOG.md`.
- [x] 2026-02-10: Refactor: split `embeddb` unit tests into `crates/embeddb/src/tests.rs`, add `EmbedDb::lock_inner()` helper, and clarify `EmbedDb` config field naming (no behavior change).
  Evidence: `crates/embeddb/src/lib.rs`, `crates/embeddb/src/tests.rs`
- [x] 2026-02-10: Added an exclusive `data_dir` lockfile (`embeddb.lock`) held for process lifetime to prevent concurrent opens of the same DB directory.
  Evidence: `crates/embeddb/src/lib.rs` (`EmbedDb::open` lock acquisition), `README.md`, `docs/HTTP.md`, `CHANGELOG.md`
- [x] 2026-02-10: Added portable snapshot export/restore (copy-only backup) APIs + CLI commands, implemented as checkpoint + safe directory copy.
  Evidence: `crates/embeddb/src/lib.rs` (`export_snapshot`, `restore_snapshot`, test `snapshot_export_and_restore_roundtrip`), `crates/embeddb-cli/src/main.rs` (`snapshot-export`, `snapshot-restore`), `README.md`, `CHANGELOG.md`
- [x] 2026-02-09: Added metadata filtering to brute-force kNN search (MVP `AND` filters: equality + numeric ranges) exposed via CLI/HTTP, plus process-smoke coverage.
  Evidence: `crates/embeddb/src/lib.rs` (`FilterCondition`, `FilterOp`, `search_knn_filtered`, tests `search_knn_filtered_applies_scalar_filters`), `crates/embeddb-cli/src/main.rs` (`search --filter`, `search-text --filter`), `crates/embeddb-server/src/main.rs` (`filter` request support), `docs/HTTP.md` and `README.md` examples, `scripts/http_process_smoke.sh` (filtered search assertion).
- [x] 2026-02-09: Added opt-in WAL auto-checkpointing when `wal.log` crosses a configured byte threshold (preflight checkpoint before WAL appends).
  Evidence: `crates/embeddb/src/lib.rs` (`Config::with_wal_autocheckpoint_bytes`, `preflight_wal_autocheckpoint`, test `wal_autocheckpoint_triggers_before_write`), `crates/embeddb-cli/src/main.rs` (`--wal-autocheckpoint-bytes`), `crates/embeddb-server/src/main.rs` (`EMBEDDB_WAL_AUTOCHECKPOINT_BYTES`), `docs/HTTP.md` and `README.md`.
- [x] 2026-02-09: Added DB-level WAL checkpoint that flushes tables and rewrites `wal.log` to a compact snapshot (preserving `next_row_id` and embedding state), exposed via CLI and HTTP.
  Evidence: `crates/embeddb/src/lib.rs` (`checkpoint`, `CheckpointStats`, WAL rotation + recovery), `crates/embeddb/src/storage/wal.rs` (`WalRecord::SetNextRowId`, `create_new`), `crates/embeddb-cli/src/main.rs` (`checkpoint`), `crates/embeddb-server/src/main.rs` (`POST /checkpoint` + contract/smoke coverage), `scripts/http_process_smoke.sh` (checkpoint call), tests `checkpoint_truncates_wal_and_preserves_next_row_id`, `checkpoint_preserves_embedding_meta_and_vectors`, `open_recovers_from_interrupted_checkpoint_wal_rotation`.
- [x] 2026-02-09: Added background embedding retry/backoff with bounded metadata (`attempts`, `next_retry_at_ms`) persisted in WAL.
  Evidence: `crates/embeddb/src/schema.rs` (`EmbeddingMeta`), `crates/embeddb/src/storage/wal.rs` (`WalRecord::UpdateEmbeddingStatus`), `crates/embeddb/src/lib.rs` (retry scheduler), tests `tests::embedding_retry_backoff_defers_until_next_retry_time`, `tests::retry_failed_embedding_job_resets_status_and_error`.
- [x] 2026-02-09: Added DB stats API (`db_stats`, CLI `db-stats`, HTTP `GET /stats`) including WAL size visibility (`wal_bytes`).
  Evidence: `crates/embeddb/src/lib.rs` (`DbStats`, `db_stats`), `crates/embeddb-cli/src/main.rs` (`db-stats`), `crates/embeddb-server/src/main.rs` (`GET /stats`), `docs/HTTP.md`, contract test `db_stats_response_schema`.
- [x] 2026-02-09: Added `retry-failed` embedding jobs (core + CLI + HTTP) to unblock operators after transient embedder failures.
  Evidence: `crates/embeddb/src/lib.rs` (`retry_failed_jobs`), `crates/embeddb-cli/src/main.rs` (`retry-failed`), `crates/embeddb-server/src/main.rs` (`/tables/:table/jobs/retry-failed`), `docs/HTTP.md`.
- [x] 2026-02-09: Preserve and upload HTTP process-smoke server logs as CI artifacts on failure.
  Evidence: `.github/workflows/ci.yml`, `scripts/http_process_smoke.sh`, `.gitignore`.
- [x] 2026-02-09: Added bounded embedding processing via an optional limit (core + CLI + HTTP).
  Evidence: `crates/embeddb/src/lib.rs` (`process_pending_jobs_with_limit`), `crates/embeddb-cli/src/main.rs` (`process-jobs --limit`), `crates/embeddb-server/src/main.rs` (`/tables/:table/jobs/process?limit=`), `docs/HTTP.md`, test `process_pending_jobs_limit_processes_subset`.
- [x] 2026-02-09: Made embedding job listing deterministic (sorted by `row_id`).
  Evidence: `crates/embeddb/src/lib.rs` (`list_embedding_jobs`).
- [x] 2026-02-09: Added SST-aware row visibility for `update_row`, so updates now work after flush/compaction.
  Evidence: `crates/embeddb/src/lib.rs` (`update_row`, `load_row`, `row_exists`), test `update_row_after_flush_and_compaction`.
- [x] 2026-02-09: Fixed pending embedding job processing to read rows from memtable or SST and survive reopen.
  Evidence: `crates/embeddb/src/lib.rs` (`process_pending_jobs`, `load_row`), test `process_pending_jobs_after_flush_and_reopen`.
- [x] 2026-02-09: Added process-level HTTP smoke script and CI step that starts the real server and drives HTTP endpoints.
  Evidence: `scripts/http_process_smoke.sh`, `.github/workflows/ci.yml`.
- [x] 2026-02-09: Improved SST point lookup with binary search and added storage unit coverage.
  Evidence: `crates/embeddb/src/storage/sst.rs` (`find_entry_binary_search_roundtrip`).
- [x] 2026-02-09: Added persistent automation memory docs and incident records for reliability regressions.
  Evidence: `PROJECT_MEMORY.md`, `INCIDENTS.md`.
- [x] 2026-02-08: Stabilized CI secret scanning by setting `actions/checkout` to `fetch-depth: 0`, fixing `gitleaks` git-range scan failures.
  Evidence: `.github/workflows/ci.yml`
- [x] 2026-02-08: Made `dependency-review` non-blocking so unsupported repository settings no longer fail the entire CI run.
  Evidence: `.github/workflows/ci.yml`
- [x] 2026-02-08: Added explicit CI coverage for optional HTTP server features.
  Evidence: `.github/workflows/ci.yml` (`cargo test -p embeddb-server --features http,contract-tests`)
- [x] 2026-02-08: Added HTTP end-to-end route smoke test for health/create/insert/process/search/flush/compact/get.
  Evidence: `crates/embeddb-server/src/main.rs` (`http_smoke_tests::http_smoke_flow`)
- [x] 2026-02-08: Fixed HTTP row payload/response semantics to use natural JSON values instead of internal enum-tagged representation.
  Evidence: `crates/embeddb-server/src/main.rs` (`InsertRowRequest`, `json_value_to_embeddb`, `embeddb_value_to_json`, `get_row`)
- [x] 2026-02-08: Added compaction/reopen regression test validating row visibility and tombstone behavior across restarts.
  Evidence: `crates/embeddb/src/lib.rs` (`compacted_rows_survive_reopen_and_tombstones_hide_deleted_rows`)
- [x] 2026-02-08: Removed dead CLI sample schema helper.
  Evidence: `crates/embeddb-cli/src/main.rs`
- [x] 2026-02-08: Updated docs/changelog to reflect new HTTP test command and CI behavior.
  Evidence: `README.md`, `docs/HTTP.md`, `docs/PROJECT.md`, `CHANGELOG.md`

## Insights
- `gitleaks` failures were caused by shallow checkout history during commit-range scans, not leaked secrets.
- JSON schema contract tests alone missed runtime serde behavior; an in-process HTTP smoke test caught a real API mismatch.
- JSON Schema contract tests should prefer `anyOf` over `oneOf` for overlapping numeric domains (`integer` vs `number`) to avoid false negatives.
- `process_pending_jobs` previously assumed memtable residency; recovery-safe background work needs shared row visibility semantics.
- Process-level server smoke catches startup/runtime integration risks that router-only tests cannot surface.
- CI smoke scripts must avoid assuming optional tools (`rg`) exist on GitHub runners; prefer portable shell utilities.
- Market scan (untrusted web): vector DB baselines expect fast ANN indexes (HNSW/IVF), metadata filtering, and hybrid retrieval hooks.
- Market scan sources (untrusted): DuckDB VSS docs https://duckdb.org/docs/extensions/vss.html
- Market scan sources (untrusted): pgvector https://github.com/pgvector/pgvector
- Market scan sources (untrusted): sqlite-vector https://github.com/asg017/sqlite-vector
- Market scan sources (untrusted): Qdrant filtering model https://qdrant.tech/documentation/concepts/filtering/
- Market scan sources (untrusted): Chroma metadata filtering docs https://docs.trychroma.com/docs/querying-collections/metadata-filtering
- Market scan sources (untrusted): Weaviate filters docs https://docs.weaviate.io/weaviate/search/filters
- Market scan sources (untrusted): Weaviate conditional filters (where) docs https://docs.weaviate.io/weaviate/api/graphql/filters
- Market scan (untrusted web): WAL-backed systems generally need checkpoint/truncation hooks; SQLite exposes manual + auto-checkpointing, including truncate mode, to bound log growth.
- Market scan sources (untrusted): SQLite `wal_checkpoint(TRUNCATE)` + `wal_autocheckpoint` docs https://www.sqlite.org/pragma.html
- Market scan sources (untrusted): pgvector notes on combining ANN with `WHERE` filtering https://github.com/pgvector/pgvector
- Market scan (untrusted web): embedded/vector DB baselines also expose some form of snapshot/backup/export (checkpointed directory snapshots or logical export/import) for portability and DR.
- Market scan sources (untrusted): SQLite Online Backup API https://sqlite.org/backup.html
- Market scan sources (untrusted): RocksDB Checkpoints https://github.com/facebook/rocksdb/wiki/Checkpoints
- Market scan sources (untrusted): DuckDB `EXPORT DATABASE`/`IMPORT DATABASE` https://duckdb.org/docs/stable/sql/statements/export
- Market scan sources (untrusted): Qdrant snapshots https://qdrant.tech/documentation/concepts/snapshots/
- Market scan refresh 2026-02-11 (untrusted web): operator baselines emphasize explicit payload filtering, batch upserts, and visible collection/runtime telemetry.
- Market scan sources (untrusted): Qdrant filtering https://qdrant.tech/documentation/concepts/filtering/
- Market scan sources (untrusted): Qdrant points/batch APIs https://qdrant.tech/documentation/concepts/points/
- Market scan sources (untrusted): pgvector ANN + filtered query patterns https://github.com/pgvector/pgvector
- Market scan sources (untrusted): DuckDB VSS extension baseline https://duckdb.org/docs/stable/core_extensions/vss.html
- Market scan sources (untrusted): Chroma metadata filtering https://docs.trychroma.com/docs/querying-collections/metadata-filtering
- Gap map: missing: persisted ANN index (HNSW) for larger tables.
- Gap map: weak: high-volume ingest ergonomics (batch/resume ingestion).
- Gap map: weak: ANN latency path (still brute-force only).
- Gap map: parity: durable WAL-first writes + background embeddings + brute-force kNN MVP.
- Gap map: parity: scalar metadata filters and operator-visible runtime counters are now available.
- Gap map: differentiator: per-row embedding jobs with idempotent status tracking integrated into a local-first embedded DB.

## Notes
- This file is maintained by the autonomous clone loop.
