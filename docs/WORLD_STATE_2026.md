# World State 2026: Embedded / Vector Search Expectations

Date: 2026-02-17

## Signals from the ecosystem
1. Hybrid retrieval (dense + sparse/keyword) is now a default expectation, not an advanced feature.
   - Qdrant docs: <https://qdrant.tech/documentation/concepts/hybrid-queries/>
   - LanceDB hybrid search docs: <https://docs.lancedb.com/search/hybrid-search>
2. Retrieval quality is increasingly measured explicitly (for example: NDCG/MRR), and teams iterate with evaluation loops.
   - Qdrant hybrid article: <https://qdrant.tech/articles/hybrid-search/>
3. Filter-aware ANN behavior and filtered search correctness are a key differentiator.
   - pgvector 0.8.0 release notes (iterative scans): <https://www.postgresql.org/about/news/pgvector-080-released-2952/>
4. Local-first stacks combine vector search + full-text search + metadata filters in one developer surface.
   - LanceDB search docs: <https://docs.lancedb.com/search>
   - Turso extension overview (FTS + vector-oriented SQLite ecosystem): <https://docs.turso.tech/features/sqlite-extensions>

## Implications for EmbedDB
- Keep core local-first value proposition; avoid network dependency by default.
- Prioritize practical retrieval quality over algorithm novelty.
- Expose filter + keyword + vector retrieval in one workflow.
- Improve operator ergonomics (inspection, retries, snapshots, diagnostics) to reduce production friction.

## Highest-impact additions to pursue now
1. Console-first operator workflows (stats, jobs, checkpoint, snapshots).
2. Better filtered search ergonomics and discoverability in UI.
3. Repeatable smoke/e2e checks for the built-in console + HTTP server.
4. Retrieval quality harness (small benchmark fixtures + NDCG-style regression checks).
5. Hybrid search MVP (keyword + vector fusion), while keeping brute-force baseline deterministic.
