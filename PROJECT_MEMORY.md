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
  - Capture and upload server log artifacts on CI smoke failure.
