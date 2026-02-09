use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "http")]
use std::collections::BTreeMap;
#[cfg(feature = "http")]
use std::net::SocketAddr;
#[cfg(feature = "http")]
use std::path::PathBuf;
#[cfg(feature = "http")]
use std::sync::Arc;

#[cfg(feature = "http")]
use anyhow::anyhow;
#[cfg(feature = "http")]
use embeddb::{Config, DistanceMetric, EmbedDb, Embedder, EmbeddingSpec, TableSchema, Value};
#[cfg(feature = "http")]
use serde::Deserialize;

#[cfg(feature = "http")]
use axum::{
    extract::Query,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

#[cfg(feature = "http")]
use tower_http::trace::TraceLayer;

#[cfg(all(test, feature = "contract-tests"))]
mod contract_tests {
    use jsonschema::JSONSchema;
    use serde_json::Value;

    fn compile_schema(schema: Value) -> JSONSchema {
        JSONSchema::compile(&schema).expect("schema should compile")
    }

    #[test]
    fn create_table_request_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["name", "schema"],
            "properties": {
                "name": { "type": "string", "minLength": 1 },
                "schema": {
                    "type": "object",
                    "required": ["columns"],
                    "properties": {
                        "columns": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "required": ["name", "data_type", "nullable"],
                                "properties": {
                                    "name": { "type": "string", "minLength": 1 },
                                    "data_type": {
                                        "type": "string",
                                        "enum": ["Int", "Float", "Bool", "String", "Bytes"]
                                    },
                                    "nullable": { "type": "boolean" }
                                }
                            }
                        }
                    }
                },
                "embedding_fields": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 }
                }
            }
        });

        let validator = compile_schema(schema);

        let valid = serde_json::json!({
            "name": "notes",
            "schema": {
                "columns": [
                    { "name": "title", "data_type": "String", "nullable": false }
                ]
            },
            "embedding_fields": ["title"]
        });
        assert!(validator.is_valid(&valid));

        let invalid = serde_json::json!({
            "schema": {
                "columns": [
                    { "name": "title", "data_type": "String", "nullable": false }
                ]
            }
        });
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn insert_row_request_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["fields"],
            "properties": {
                "fields": {
                    "type": "object",
                    "minProperties": 1,
                    "additionalProperties": {
                        "oneOf": [
                            { "type": "integer" },
                            { "type": "number" },
                            { "type": "boolean" },
                            { "type": "string" },
                            { "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 255 } },
                            { "type": "null" }
                        ]
                    }
                }
            }
        });

        let validator = compile_schema(schema);

        let valid = serde_json::json!({
            "fields": {
                "title": "Hello",
                "score": 4.2,
                "bytes": [1, 2, 3],
                "ok": true,
                "optional": null
            }
        });
        assert!(validator.is_valid(&valid));

        let invalid = serde_json::json!({
            "fields": []
        });
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn search_request_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "number" }
                },
                "k": { "type": "integer", "minimum": 1 },
                "metric": { "type": "string", "enum": ["Cosine", "L2"] }
            }
        });

        let validator = compile_schema(schema);

        let valid = serde_json::json!({
            "query": [1.0, 2.0, 3.0, 4.0],
            "k": 5,
            "metric": "Cosine"
        });
        assert!(validator.is_valid(&valid));

        let invalid = serde_json::json!({
            "k": 5
        });
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn search_text_request_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["query_text"],
            "properties": {
                "query_text": { "type": "string", "minLength": 1 },
                "k": { "type": "integer", "minimum": 1 },
                "metric": { "type": "string", "enum": ["Cosine", "L2"] }
            }
        });

        let validator = compile_schema(schema);

        let valid = serde_json::json!({
            "query_text": "hello world",
            "k": 5,
            "metric": "L2"
        });
        assert!(validator.is_valid(&valid));

        let invalid = serde_json::json!({
            "query_text": ""
        });
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn health_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": { "type": "string" }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "status": "ok" });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn db_stats_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["tables", "wal_bytes"],
            "properties": {
                "tables": { "type": "integer", "minimum": 0 },
                "wal_bytes": { "type": "integer", "minimum": 0 }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "tables": 2, "wal_bytes": 1234 });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn create_table_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["ok"],
            "properties": {
                "ok": { "type": "boolean" }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "ok": true });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn error_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["error"],
            "properties": {
                "error": { "type": "string", "minLength": 1 }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "error": "bad request" });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn list_tables_response_schema() {
        let schema = serde_json::json!({
            "type": "array",
            "items": { "type": "string" }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!(["notes", "users"]);
        assert!(validator.is_valid(&ok));
        let invalid = serde_json::json!([1, 2, 3]);
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn describe_table_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["name", "schema"],
            "properties": {
                "name": { "type": "string", "minLength": 1 },
                "schema": {
                    "type": "object",
                    "required": ["columns"],
                    "properties": {
                        "columns": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "required": ["name", "data_type", "nullable"],
                                "properties": {
                                    "name": { "type": "string", "minLength": 1 },
                                    "data_type": {
                                        "type": "string",
                                        "enum": ["Int", "Float", "Bool", "String", "Bytes"]
                                    },
                                    "nullable": { "type": "boolean" }
                                }
                            }
                        }
                    }
                },
                "embedding_spec": {
                    "anyOf": [
                        { "type": "null" },
                        {
                            "type": "object",
                            "required": ["source_fields"],
                            "properties": {
                                "source_fields": {
                                    "type": "array",
                                    "items": { "type": "string", "minLength": 1 }
                                }
                            }
                        }
                    ]
                }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({
            "name": "notes",
            "schema": {
                "columns": [
                    { "name": "title", "data_type": "String", "nullable": false }
                ]
            },
            "embedding_spec": {
                "source_fields": ["title"]
            }
        });
        assert!(validator.is_valid(&ok));
        let invalid = serde_json::json!({
            "name": "notes",
            "schema": { "columns": [] }
        });
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn table_stats_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": [
                "name",
                "rows_mem",
                "tombstones_mem",
                "embeddings_total",
                "embeddings_pending",
                "embeddings_ready",
                "embeddings_failed",
                "sst_files",
                "next_row_id"
            ],
            "properties": {
                "name": { "type": "string", "minLength": 1 },
                "rows_mem": { "type": "integer", "minimum": 0 },
                "tombstones_mem": { "type": "integer", "minimum": 0 },
                "embeddings_total": { "type": "integer", "minimum": 0 },
                "embeddings_pending": { "type": "integer", "minimum": 0 },
                "embeddings_ready": { "type": "integer", "minimum": 0 },
                "embeddings_failed": { "type": "integer", "minimum": 0 },
                "sst_files": { "type": "integer", "minimum": 0 },
                "next_row_id": { "type": "integer", "minimum": 1 }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({
            "name": "notes",
            "rows_mem": 1,
            "tombstones_mem": 0,
            "embeddings_total": 1,
            "embeddings_pending": 1,
            "embeddings_ready": 0,
            "embeddings_failed": 0,
            "sst_files": 0,
            "next_row_id": 2
        });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn search_response_schema() {
        let schema = serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["row_id", "distance"],
                "properties": {
                    "row_id": { "type": "integer", "minimum": 1 },
                    "distance": { "type": "number" }
                }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!([
            { "row_id": 1, "distance": 0.1 },
            { "row_id": 2, "distance": 0.2 }
        ]);
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn process_jobs_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["processed"],
            "properties": {
                "processed": { "type": "integer", "minimum": 0 }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "processed": 2 });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn insert_row_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["row_id"],
            "properties": {
                "row_id": { "type": "integer", "minimum": 1 }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "row_id": 1 });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn delete_row_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["ok"],
            "properties": {
                "ok": { "type": "boolean" }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "ok": true });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn get_row_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["id", "fields"],
            "properties": {
                "id": { "type": "integer", "minimum": 1 },
                "fields": {
                    "type": "object",
                    "additionalProperties": {
                        "oneOf": [
                            { "type": "integer" },
                            { "type": "number" },
                            { "type": "boolean" },
                            { "type": "string" },
                            { "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 255 } },
                            { "type": "null" }
                        ]
                    }
                }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({
            "id": 1,
            "fields": {
                "title": "Hello",
                "score": 4.2,
                "bytes": [1, 2, 3],
                "ok": true,
                "optional": null
            }
        });
        assert!(validator.is_valid(&ok));
    }

    #[test]
    fn flush_compact_response_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["ok"],
            "properties": {
                "ok": { "type": "boolean" }
            }
        });
        let validator = compile_schema(schema);
        let ok = serde_json::json!({ "ok": true });
        assert!(validator.is_valid(&ok));
    }
}

#[cfg(feature = "http")]
struct LocalHashEmbedder;

#[cfg(feature = "http")]
impl Embedder for LocalHashEmbedder {
    fn embed(&self, input: &str) -> Result<Vec<f32>> {
        let mut hash = 0u64;
        for byte in input.as_bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);
        }
        let a = (hash & 0xFFFF) as f32;
        let b = ((hash >> 16) & 0xFFFF) as f32;
        let c = ((hash >> 32) & 0xFFFF) as f32;
        let d = ((hash >> 48) & 0xFFFF) as f32;
        Ok(vec![a, b, c, d])
    }
}

#[cfg(feature = "http")]
const INDEX_HTML: &str = include_str!("ui/index.html");
#[cfg(feature = "http")]
const APP_JS: &str = include_str!("ui/app.js");
#[cfg(feature = "http")]
const STYLES_CSS: &str = include_str!("ui/styles.css");
#[cfg(feature = "http")]
const FAVICON_SVG: &str = include_str!("ui/favicon.svg");

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    #[cfg(feature = "http")]
    return run_http();

    #[cfg(not(feature = "http"))]
    {
        println!("embeddb-server scaffold (enable HTTP with: cargo run -p embeddb-server --features http)");
        Ok(())
    }
}

#[cfg(feature = "http")]
fn run_http() -> Result<()> {
    let addr: SocketAddr = std::env::var("EMBEDDB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse()
        .map_err(|_| anyhow!("invalid EMBEDDB_ADDR"))?;
    let data_dir =
        PathBuf::from(std::env::var("EMBEDDB_DATA_DIR").unwrap_or_else(|_| "./data".to_string()));

    let db = EmbedDb::open(Config::new(data_dir))?;
    let state = Arc::new(AppState { db });
    let app = build_router(state);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        tracing::info!(%addr, "embeddb-server listening");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

#[cfg(feature = "http")]
struct AppState {
    db: EmbedDb,
}

#[cfg(feature = "http")]
fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(ui_index))
        .route("/assets/app.js", get(ui_app_js))
        .route("/assets/styles.css", get(ui_styles))
        .route("/favicon.svg", get(ui_favicon))
        .route("/health", get(health))
        .route("/stats", get(db_stats))
        .route("/tables", get(list_tables).post(create_table))
        .route("/tables/:table", get(describe_table))
        .route("/tables/:table/stats", get(table_stats))
        .route("/tables/:table/rows", post(insert_row))
        .route(
            "/tables/:table/rows/:row_id",
            get(get_row).delete(delete_row),
        )
        .route("/tables/:table/search", post(search))
        .route("/tables/:table/search-text", post(search_text))
        .route("/tables/:table/jobs/process", post(process_jobs))
        .route("/tables/:table/jobs/retry-failed", post(retry_failed_jobs))
        .route("/tables/:table/flush", post(flush_table))
        .route("/tables/:table/compact", post(compact_table))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(feature = "http")]
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

#[cfg(feature = "http")]
impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

#[cfg(feature = "http")]
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}

#[cfg(feature = "http")]
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

#[cfg(feature = "http")]
async fn db_stats(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .db_stats()
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
async fn ui_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[cfg(feature = "http")]
async fn ui_styles() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        STYLES_CSS,
    )
}

#[cfg(feature = "http")]
async fn ui_app_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        APP_JS,
    )
}

#[cfg(feature = "http")]
async fn ui_favicon() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        FAVICON_SVG,
    )
}

#[cfg(feature = "http")]
async fn list_tables(State(state): State<Arc<AppState>>) -> Result<Json<Vec<String>>, ApiError> {
    state
        .db
        .list_tables()
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct CreateTableRequest {
    name: String,
    schema: TableSchema,
    embedding_fields: Option<Vec<String>>,
}

#[cfg(feature = "http")]
async fn create_table(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTableRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let embed_spec = req
        .embedding_fields
        .map(|fields| EmbeddingSpec::new(fields));
    state
        .db
        .create_table(req.name, req.schema, embed_spec)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))))
}

#[cfg(feature = "http")]
async fn describe_table(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .describe_table(&table)
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
async fn table_stats(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .table_stats(&table)
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct InsertRowRequest {
    fields: BTreeMap<String, serde_json::Value>,
}

#[cfg(feature = "http")]
async fn insert_row(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Json(req): Json<InsertRowRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let fields: BTreeMap<String, Value> = req
        .fields
        .into_iter()
        .map(|(key, value)| {
            json_value_to_embeddb(value)
                .map(|parsed| (key, parsed))
                .map_err(|err| ApiError::bad_request(err.to_string()))
        })
        .collect::<Result<_, _>>()?;

    let row_id = state
        .db
        .insert_row(&table, fields)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "row_id": row_id })),
    ))
}

#[cfg(feature = "http")]
async fn get_row(
    State(state): State<Arc<AppState>>,
    Path((table, row_id)): Path<(String, u64)>,
) -> Result<impl IntoResponse, ApiError> {
    match state
        .db
        .get_row(&table, row_id)
        .map_err(|err| ApiError::bad_request(err.to_string()))?
    {
        Some(row) => {
            let mut fields = serde_json::Map::new();
            for (key, value) in row.fields {
                fields.insert(key, embeddb_value_to_json(value));
            }
            Ok(Json(serde_json::json!({
                "id": row.id,
                "fields": fields
            })))
        }
        None => Err(ApiError::not_found("row not found")),
    }
}

#[cfg(feature = "http")]
async fn delete_row(
    State(state): State<Arc<AppState>>,
    Path((table, row_id)): Path<(String, u64)>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .delete_row(&table, row_id)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct SearchRequest {
    query: Vec<f32>,
    k: Option<usize>,
    metric: Option<DistanceMetric>,
}

#[cfg(feature = "http")]
async fn search(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let k = req.k.unwrap_or(5);
    let metric = req.metric.unwrap_or(DistanceMetric::Cosine);
    state
        .db
        .search_knn(&table, &req.query, k, metric)
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct SearchTextRequest {
    query_text: String,
    k: Option<usize>,
    metric: Option<DistanceMetric>,
}

#[cfg(feature = "http")]
async fn search_text(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Json(req): Json<SearchTextRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let k = req.k.unwrap_or(5);
    let metric = req.metric.unwrap_or(DistanceMetric::Cosine);
    let embedder = LocalHashEmbedder;
    let query = embedder
        .embed(&req.query_text)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    state
        .db
        .search_knn(&table, &query, k, metric)
        .map(Json)
        .map_err(|err| ApiError::bad_request(err.to_string()))
}

#[cfg(feature = "http")]
async fn process_jobs(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Query(query): Query<ProcessJobsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let embedder = LocalHashEmbedder;
    let processed = match query.limit {
        Some(limit) => state
            .db
            .process_pending_jobs_with_limit(&table, &embedder, limit)
            .map_err(|err| ApiError::bad_request(err.to_string()))?,
        None => state
            .db
            .process_pending_jobs(&table, &embedder)
            .map_err(|err| ApiError::bad_request(err.to_string()))?,
    };
    Ok(Json(serde_json::json!({ "processed": processed })))
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct ProcessJobsQuery {
    limit: Option<usize>,
}

#[cfg(feature = "http")]
#[derive(Debug, Deserialize)]
struct RetryFailedQuery {
    row_id: Option<u64>,
}

#[cfg(feature = "http")]
async fn retry_failed_jobs(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Query(query): Query<RetryFailedQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let retried = state
        .db
        .retry_failed_jobs(&table, query.row_id)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok(Json(serde_json::json!({ "retried": retried })))
}

#[cfg(feature = "http")]
async fn flush_table(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .flush_table(&table)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(feature = "http")]
async fn compact_table(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .compact_table(&table)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(feature = "http")]
fn json_value_to_embeddb(value: serde_json::Value) -> Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(v) => Value::Bool(v),
        serde_json::Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                Value::Int(i)
            } else if let Some(f) = v.as_f64() {
                Value::Float(f)
            } else {
                return Err(anyhow!("invalid number"));
            }
        }
        serde_json::Value::String(v) => Value::String(v),
        serde_json::Value::Array(values) => {
            let bytes: Result<Vec<u8>> = values
                .into_iter()
                .map(|item| {
                    item.as_u64()
                        .ok_or_else(|| anyhow!("bytes must be u8"))
                        .and_then(|b| u8::try_from(b).map_err(|_| anyhow!("byte out of range")))
                })
                .collect();
            Value::Bytes(bytes?)
        }
        serde_json::Value::Object(_) => return Err(anyhow!("nested objects not supported")),
    })
}

#[cfg(feature = "http")]
fn embeddb_value_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::Int(v) => serde_json::json!(v),
        Value::Float(v) => serde_json::json!(v),
        Value::Bool(v) => serde_json::json!(v),
        Value::String(v) => serde_json::json!(v),
        Value::Bytes(v) => serde_json::json!(v),
        Value::Null => serde_json::Value::Null,
    }
}

#[cfg(all(test, feature = "http"))]
mod http_smoke_tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tempfile::tempdir;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn http_smoke_flow() {
        let dir = tempdir().expect("tempdir");
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).expect("open db");
        let app = build_router(Arc::new(AppState { db }));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/stats")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let create_body = serde_json::json!({
            "name": "notes",
            "schema": {
                "columns": [
                    { "name": "title", "data_type": "String", "nullable": false },
                    { "name": "body", "data_type": "String", "nullable": false }
                ]
            },
            "embedding_fields": ["title", "body"]
        });
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables")
                    .header("content-type", "application/json")
                    .body(Body::from(create_body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::CREATED);

        let insert_body = serde_json::json!({
            "fields": {
                "title": "Hello",
                "body": "World"
            }
        });
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables/notes/rows")
                    .header("content-type", "application/json")
                    .body(Body::from(insert_body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::CREATED);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables/notes/jobs/process")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let search_body = serde_json::json!({
            "query_text": "Hello\nWorld",
            "k": 1
        });
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables/notes/search-text")
                    .header("content-type", "application/json")
                    .body(Body::from(search_body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("body");
        let hits: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let row_id = hits
            .as_array()
            .and_then(|v| v.first())
            .and_then(|v| v.get("row_id"))
            .and_then(|v| v.as_u64())
            .expect("row_id");
        assert_eq!(row_id, 1);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables/notes/flush")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tables/notes/compact")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/tables/notes/rows/1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::OK);
    }
}
