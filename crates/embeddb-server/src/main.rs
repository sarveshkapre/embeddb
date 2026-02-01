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
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

#[cfg(feature = "http")]
use tower_http::trace::TraceLayer;

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

    let app = Router::new()
        .route("/health", get(health))
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
        .layer(TraceLayer::new_for_http())
        .with_state(state);

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
    fields: BTreeMap<String, Value>,
}

#[cfg(feature = "http")]
async fn insert_row(
    State(state): State<Arc<AppState>>,
    Path(table): Path<String>,
    Json(req): Json<InsertRowRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let row_id = state
        .db
        .insert_row(&table, req.fields)
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
        Some(row) => Ok(Json(row)),
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
) -> Result<impl IntoResponse, ApiError> {
    let embedder = LocalHashEmbedder;
    let processed = state
        .db
        .process_pending_jobs(&table, &embedder)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    Ok(Json(serde_json::json!({ "processed": processed })))
}
