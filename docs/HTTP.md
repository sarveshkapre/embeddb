# EmbedDB HTTP API (MVP)

> This server is **optional** and only available when building with the `http` feature flag.

## Run
```bash
cargo run -p embeddb-server --features http
```

## Contract tests
This repo includes JSON Schema-based contract tests for request payloads:
```bash
cargo test -p embeddb-server --features contract-tests
```
The contract tests also validate core response and error shapes, including list/describe responses.

## Common responses
- Success: `200` or `201` with JSON payloads.
- Errors: `{"error":"..."}`

## Endpoints

### Health
`GET /health`
```bash
curl -s http://127.0.0.1:8080/health
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
  "metric": "Cosine"
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/search \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "query": [1.0, 2.0, 3.0, 4.0],
  "k": 5,
  "metric": "Cosine"
}
JSON
```

### Search (text)
`POST /tables/:table/search-text`
```json
{
  "query_text": "hello world",
  "k": 5,
  "metric": "Cosine"
}
```
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/search-text \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "query_text": "hello world",
  "k": 5,
  "metric": "Cosine"
}
JSON
```

### Process embedding jobs
`POST /tables/:table/jobs/process`
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/jobs/process
```

### Flush / Compact
`POST /tables/:table/flush`
`POST /tables/:table/compact`
```bash
curl -s -X POST http://127.0.0.1:8080/tables/notes/flush
curl -s -X POST http://127.0.0.1:8080/tables/notes/compact
```
