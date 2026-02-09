# INCIDENTS

## 2026-02-09: Pending embedding jobs could stall after flush/reopen
- Status: fixed
- Impact: Rows with `pending` embedding metadata were skipped by processing if row data had moved from memtable to SST, leaving jobs indefinitely pending.
- Detection: Code review + regression test design for post-flush processing paths.
- Root cause: `process_pending_jobs` only loaded row fields from `table_state.rows` and ignored SST-backed rows.
- Fix:
  - Added shared row lookup helpers that search memtable first, then SST files.
  - Switched `process_pending_jobs` to use shared lookup.
  - Added regression `process_pending_jobs_after_flush_and_reopen`.
- Prevention rules:
  - Any path that reads row state must use shared visibility helpers rather than direct memtable access.
  - New mutation/recovery features require at least one flush/reopen regression test.

## 2026-02-09: `update_row` rejected valid SST-backed rows
- Status: fixed
- Impact: `update_row` returned `row not found` for rows flushed out of memtable, breaking expected update semantics.
- Detection: Backlog task validation from `CLONE_FEATURES.md` plus targeted code sweep.
- Root cause: Existence check used `table_state.rows.contains_key` only.
- Fix:
  - Reused shared row visibility helper for existence checks.
  - Added regression `update_row_after_flush_and_compaction`.
- Prevention rules:
  - Existence checks for row mutations must include SST lookup and tombstone semantics.

## 2026-02-09: CI HTTP process smoke failed on missing `rg`
- Status: fixed
- Impact: CI run `21810844985` failed before `build/audit/gitleaks`, blocking merge confidence for the new smoke coverage.
- Detection: GitHub Actions failure logs (`HTTP server process smoke` step).
- Root cause: Script used `rg` for output assertion, but GitHub Ubuntu runner image did not provide `rg` in PATH.
- Fix:
  - Replaced `rg` usage with POSIX-portable `grep -q`.
  - Validated locally and in follow-up CI run `21810875687`.
- Prevention rules:
  - CI helper scripts should rely on portable shell/coreutils by default.
  - If non-default tools are required, install them explicitly in workflow setup.
