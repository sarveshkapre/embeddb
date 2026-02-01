# AGENTS

## Purpose
This repo builds EmbedDB: a single-node embedded database with WAL + LSM storage, typed tables, and automatic per-row embeddings.

## Guardrails
- Durable primary writes must complete before embedding jobs are enqueued.
- Embedding jobs must be idempotent (content hash + job status).
- Keep the MVP small: brute-force kNN before HNSW.
- No network calls by default; remote embedder must be behind a feature flag.

## Commands
- Setup: `make setup`
- Dev: `make dev`
- Test: `make test`
- Lint: `make lint`
- Typecheck: `make typecheck`
- Build: `make build`
- Quality gate: `make check`
- Release: `make release`

## Conventions
- Rust 2021 edition, workspace layout under `crates/`.
- Public APIs go in `crates/embeddb`.
- Keep errors typed (`thiserror`) and structured logs (`tracing`).
