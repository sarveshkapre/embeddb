# Clone Feature Tracker

## Context Sources
- README and docs
- TODO/FIXME markers in code
- Test and build failures
- Gaps found during codebase exploration

## Candidate Features To Do
- [ ] P2: Re-enable blocking dependency-review once repository security/dependency-graph support is confirmed.
- [ ] P2: Add metrics counters for embedding throughput, WAL fsync counts, and compaction durations.
- [ ] P2: Implement background embedding retries/backoff with bounded retry metadata.
- [ ] P2: Add CI artifact upload for HTTP process-smoke server logs on failure.
- [ ] P3: Add HNSW v1 index path for large-table search latency reduction.

## Implemented
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
- `process_pending_jobs` previously assumed memtable residency; recovery-safe background work needs shared row visibility semantics.
- Process-level server smoke catches startup/runtime integration risks that router-only tests cannot surface.
- CI smoke scripts must avoid assuming optional tools (`rg`) exist on GitHub runners; prefer portable shell utilities.

## Notes
- This file is maintained by the autonomous clone loop.
