# EmbedDB HTTP API (MVP)

> This server is **optional** and only available when building with the `http` feature flag.

## Run
```bash
cargo run -p embeddb-server --features http
```

Optional env vars:
- `EMBEDDB_ADDR`: bind address (default `127.0.0.1:8080`)
- `EMBEDDB_DATA_DIR`: data directory (default `./data`)
- `EMBEDDB_WAL_AUTOCHECKPOINT_BYTES`: when set, the server will auto-run a WAL `POST /checkpoint` *before* handling a write if `wal.log` is at/above this size (bytes).

Note: The server holds an exclusive lock on `EMBEDDB_DATA_DIR` (via `embeddb.lock`). Don’t point a
second `embeddb-cli` or `embeddb-server` process at the same directory concurrently.

## Web Console
The HTTP server also serves a built-in UI at `http://127.0.0.1:8080`. Use it to create tables,
insert rows, process embedding jobs, and run text search.

## Contract tests
This repo includes JSON Schema-based contract tests for request payloads and an HTTP route smoke test:
```bash
cargo test -p embeddb-server --features http,contract-tests
```
The contract tests also validate core response and error shapes, including list/describe/stats/search and row CRUD/flush/compact responses.

## Common responses
- Success: `200` or `201` with JSON payloads.
- Errors: `{"error":"..."}`

## Endpoints

### Health
`GET /health`
```bash
curl -s http://127.0.0.1:8080/health
```

### DB stats
`GET /stats`
```bash
curl -s http://127.0.0.1:8080/stats
```

### WAL checkpoint
`POST /checkpoint`

Rotates/compacts the WAL to prevent unbounded growth. The checkpoint flushes pending memtable state to SSTs
and rewrites `wal.log` to a minimal snapshot.

```bash
curl -s -X POST http://127.0.0.1:8080/checkpoint
```

### List tables
`GET /tables`
```bash
curl -s http://127.0.0.1:8080/tables
```

### Create table
`POST /tables`
```json
{
  "name": "notes",
  "schema": {
    "columns": [
      { "name": "title", "data_type": "String", "nullable": false },
      { "name": "body", "data_type": "String", "nullable": false }
    ]
  },
  "embedding_fields": ["title", "body"]
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "name": "notes",
  "schema": {
    "columns": [
      { "name": "title", "data_type": "String", "nullable": false },
      { "name": "body", "data_type": "String", "nullable": false }
    ]
  },
  "embedding_fields": ["title", "body"]
}
JSON
```

### Describe table
`GET /tables/:table`
```bash
curl -s http://127.0.0.1:8080/tables/notes
```

### Table stats
`GET /tables/:table/stats`
```bash
curl -s http://127.0.0.1:8080/tables/notes/stats
```

### Insert row
`POST /tables/:table/rows`
```json
{
  "fields": {
    "title": "Hello",
    "body": "World"
  }
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/rows \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "fields": {
    "title": "Hello",
    "body": "World"
  }
}
JSON
```

### Get row
`GET /tables/:table/rows/:row_id`
```bash
curl -s http://127.0.0.1:8080/tables/notes/rows/1
```

### Delete row
`DELETE /tables/:table/rows/:row_id`
```bash
curl -s -X DELETE http://127.0.0.1:8080/tables/notes/rows/1
```

### Search (vector)
`POST /tables/:table/search`
```json
{
  "query": [1.0, 2.0, 3.0, 4.0],
  "k": 5,
  "metric": "Cosine",
  "filter": [
    { "column": "age", "op": "Gte", "value": 21 },
    { "column": "score", "op": "Lt", "value": 0.5 }
  ]
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/search \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "query": [1.0, 2.0, 3.0, 4.0],
  "k": 5,
  "metric": "Cosine",
  "filter": [
    { "column": "age", "op": "Gte", "value": 21 }
  ]
}
JSON
```

### Search (text)
`POST /tables/:table/search-text`
```json
{
  "query_text": "hello world",
  "k": 5,
  "metric": "Cosine",
  "filter": [
    { "column": "title", "op": "Eq", "value": "Hello" }
  ]
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/search-text \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "query_text": "hello world",
  "k": 5,
  "metric": "Cosine",
  "filter": [
    { "column": "title", "op": "Eq", "value": "Hello" }
  ]
}
JSON
```

### Process embedding jobs
`POST /tables/:table/jobs/process`

Optional query params:
- `limit`: max number of pending jobs to process in this request.

```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/jobs/process
```
```bash
curl -s -X POST "http://127.0.0.1:8080/tables/notes/jobs/process?limit=100"
```

### Retry failed embedding jobs
`POST /tables/:table/jobs/retry-failed`

Retry all failed jobs:
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/jobs/retry-failed
```

Retry a single row:
```bash
curl -s -X POST "http://127.0.0.1:8080/tables/notes/jobs/retry-failed?row_id=1"
```

### Flush / Compact
`POST /tables/:table/flush`
`POST /tables/:table/compact`
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/flush
curl -s -X POST http://127.0.0.1:8080/tables/notes/compact
```
