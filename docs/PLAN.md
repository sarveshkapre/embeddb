# PLAN

## Stack
- Language: Rust (2021 edition)
- Storage: file-backed WAL + SST files (LSM), memory memtable
- CLI: `clap`
- Logging: `tracing`

## Architecture (MVP)
- Storage
  - WAL: append-only record of row mutations and job status updates.
  - Memtable: in-memory map keyed by row id with latest values.
  - SST: immutable sorted table files flushed from memtable.
  - Compaction: merge SSTs into larger levels, preserving latest row versions.
- Tables and rows
  - Each table has a typed schema; rows have stable row ids.
  - Writes: validate schema → write WAL → update memtable → schedule embedding job.
- Embeddings
  - Job queue persisted in WAL with statuses: `pending`, `ready`, `failed`.
  - Idempotence keyed by row id + content hash.
  - Embedder interface supports local-first implementation; remote embedder behind feature flag.
- Vector search
  - MVP: brute-force scan over stored vectors with cosine/L2.
  - v1: HNSW index with rebuild/refresh on compaction.
- Observability
  - Structured logs and counters for job throughput, WAL flushes, compaction duration.
- Crash recovery
  - Rebuild memtable and job queue from WAL, then replay SSTs.

## Milestones
1. Scaffold (this phase)
2. WAL + memtable read/write, crash recovery tests
3. SST flush + basic compaction
4. Embedding jobs + status tracking + local embedder
5. Vector search (brute-force MVP)
6. Optional server/CLI endpoints
7. HNSW v1 and performance tuning

## MVP checklist
- [x] WAL append and replay
- [x] Memtable get/put/delete
- [x] Table schemas and typed rows
- [x] Embedding job queue with statuses
- [x] Local embedder interface + test embedder
- [x] Brute-force kNN search (cosine/L2)
- [x] Crash recovery tests (WAL replay/truncation)

## Risks
- Compaction correctness and version visibility
- WAL durability and fsync strategy
- Job idempotency across crashes
- Index rebuild cost and search latency
