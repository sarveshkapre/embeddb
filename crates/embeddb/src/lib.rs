//! EmbedDB core library.
//!
//! This crate provides the embedded database engine and public APIs.

mod schema;
mod storage;
mod vector;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use schema::EmbeddingMeta;
use serde::{Deserialize, Serialize};
use storage::sst::{self, SstEntry, SstFile};
use storage::wal::{Wal, WalRecord};
use vector::{distance, SearchResult};

pub use schema::{Column, DataType, EmbeddingSpec, RowData, TableSchema, Value};

const EMBEDDING_MAX_ATTEMPTS: u32 = 5;
const EMBEDDING_BACKOFF_BASE_MS: u64 = 250;
const EMBEDDING_BACKOFF_CAP_MS: u64 = 30_000;

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn embedding_backoff_ms(attempts: u32) -> u64 {
    if attempts <= 1 {
        return EMBEDDING_BACKOFF_BASE_MS;
    }
    let exp = attempts.saturating_sub(1).min(20);
    let mult = 1u64.checked_shl(exp).unwrap_or(u64::MAX);
    EMBEDDING_BACKOFF_BASE_MS
        .saturating_mul(mult)
        .min(EMBEDDING_BACKOFF_CAP_MS)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
}

impl Config {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    L2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingJob {
    pub table: String,
    pub row_id: u64,
    pub status: EmbeddingStatus,
    pub content_hash: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub row_id: u64,
    pub distance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDescriptor {
    pub name: String,
    pub schema: TableSchema,
    pub embedding_spec: Option<EmbeddingSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub name: String,
    pub rows_mem: usize,
    pub tombstones_mem: usize,
    pub embeddings_total: usize,
    pub embeddings_pending: usize,
    pub embeddings_ready: usize,
    pub embeddings_failed: usize,
    pub sst_files: usize,
    pub next_row_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStats {
    pub tables: usize,
    pub wal_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub wal_bytes_before: u64,
    pub wal_bytes_after: u64,
}

#[derive(Debug)]
struct TableState {
    schema: TableSchema,
    next_row_id: u64,
    rows: BTreeMap<u64, RowData>,
    tombstones: BTreeSet<u64>,
    embeddings: HashMap<u64, Vec<f32>>,
    embedding_meta: HashMap<u64, EmbeddingMeta>,
    embedding_spec: Option<EmbeddingSpec>,
    sst_files: Vec<SstFile>,
    next_sst_seq: u64,
}

#[derive(Debug)]
struct DbState {
    tables: HashMap<String, TableState>,
}

#[derive(Debug)]
struct Inner {
    wal: Wal,
    state: DbState,
}

#[derive(Debug)]
pub struct EmbedDb {
    _config: Config,
    inner: Mutex<Inner>,
}

impl EmbedDb {
    pub fn open(config: Config) -> Result<Self> {
        fs::create_dir_all(&config.data_dir)?;

        let wal_path = config.data_dir.join("wal.log");
        let wal_prev_path = config.data_dir.join("wal.prev");
        // Recover from an interrupted checkpoint where `wal.log` was moved aside but the new WAL
        // was not promoted yet. In that case, prefer the previous WAL.
        if !wal_path.exists() && wal_prev_path.exists() {
            fs::rename(&wal_prev_path, &wal_path)?;
        }
        let wal = Wal::open(wal_path)?;

        let mut state = DbState {
            tables: HashMap::new(),
        };

        let records = wal.replay()?;
        for record in records {
            apply_record(&mut state, record)?;
        }

        for (name, table_state) in state.tables.iter_mut() {
            let dir = sst::table_dir(&config.data_dir, name);
            let files = sst::list_sst_files(&dir)?;
            table_state.next_sst_seq = sst::max_seq(&files) + 1;
            table_state.sst_files = files;
        }

        Ok(Self {
            _config: config,
            inner: Mutex::new(Inner { wal, state }),
        })
    }

    pub fn db_stats(&self) -> Result<DbStats> {
        let tables = {
            let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
            inner.state.tables.len()
        };

        let wal_path = self._config.data_dir.join("wal.log");
        let wal_bytes = fs::metadata(wal_path).map(|m| m.len()).unwrap_or(0);

        Ok(DbStats { tables, wal_bytes })
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let mut out: Vec<String> = inner.state.tables.keys().cloned().collect();
        out.sort();
        Ok(out)
    }

    pub fn describe_table(&self, table: &str) -> Result<TableDescriptor> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get(table)
            .ok_or_else(|| anyhow!("table not found"))?;
        Ok(TableDescriptor {
            name: table.to_string(),
            schema: table_state.schema.clone(),
            embedding_spec: table_state.embedding_spec.clone(),
        })
    }

    pub fn table_stats(&self, table: &str) -> Result<TableStats> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get(table)
            .ok_or_else(|| anyhow!("table not found"))?;

        let mut pending = 0usize;
        let mut ready = 0usize;
        let mut failed = 0usize;
        for meta in table_state.embedding_meta.values() {
            match meta.status {
                EmbeddingStatus::Pending => pending += 1,
                EmbeddingStatus::Ready => ready += 1,
                EmbeddingStatus::Failed => failed += 1,
            }
        }

        Ok(TableStats {
            name: table.to_string(),
            rows_mem: table_state.rows.len(),
            tombstones_mem: table_state.tombstones.len(),
            embeddings_total: table_state.embedding_meta.len(),
            embeddings_pending: pending,
            embeddings_ready: ready,
            embeddings_failed: failed,
            sst_files: table_state.sst_files.len(),
            next_row_id: table_state.next_row_id,
        })
    }

    pub fn create_table(
        &self,
        name: impl Into<String>,
        schema: TableSchema,
        embedding_spec: Option<EmbeddingSpec>,
    ) -> Result<()> {
        let name = name.into();
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        if inner.state.tables.contains_key(&name) {
            return Err(anyhow!("table already exists"));
        }

        schema.validate_schema()?;
        let dir = sst::table_dir(&self._config.data_dir, &name);
        sst::ensure_dir(&dir)?;

        let record = WalRecord::CreateTable {
            name: name.clone(),
            schema: schema.clone(),
            embedding_spec: embedding_spec.clone(),
        };
        inner.wal.append(&record, true)?;

        inner.state.tables.insert(
            name,
            TableState {
                schema,
                next_row_id: 1,
                rows: BTreeMap::new(),
                tombstones: BTreeSet::new(),
                embeddings: HashMap::new(),
                embedding_meta: HashMap::new(),
                embedding_spec,
                sst_files: Vec::new(),
                next_sst_seq: 1,
            },
        );

        Ok(())
    }

    pub fn insert_row(&self, table: &str, fields: BTreeMap<String, Value>) -> Result<u64> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let (row_id, embedding_spec) = {
            let table_state = inner
                .state
                .tables
                .get(table)
                .ok_or_else(|| anyhow!("table not found"))?;
            table_state.schema.validate_row(&fields)?;
            (table_state.next_row_id, table_state.embedding_spec.clone())
        };

        let row = RowData {
            id: row_id,
            fields: fields.clone(),
        };

        let record = WalRecord::PutRow {
            table: table.to_string(),
            row_id,
            row: row.clone(),
        };
        // Primary write: durable first.
        inner.wal.append(&record, true)?;

        if let Some(table_state) = inner.state.tables.get_mut(table) {
            if table_state.next_row_id <= row_id {
                table_state.next_row_id = row_id + 1;
            }
            table_state.rows.insert(row_id, row);
            table_state.tombstones.remove(&row_id);
        }

        if let Some(spec) = embedding_spec {
            let content_hash = spec.content_hash(&fields)?;
            let job_record = WalRecord::EnqueueEmbedding {
                table: table.to_string(),
                row_id,
                content_hash: content_hash.clone(),
            };
            inner.wal.append(&job_record, true)?;

            if let Some(table_state) = inner.state.tables.get_mut(table) {
                table_state.embedding_meta.insert(
                    row_id,
                    EmbeddingMeta {
                        status: EmbeddingStatus::Pending,
                        content_hash,
                        last_error: None,
                        attempts: 0,
                        next_retry_at_ms: 0,
                    },
                );
            }
        }

        Ok(row_id)
    }

    pub fn update_row(
        &self,
        table: &str,
        row_id: u64,
        fields: BTreeMap<String, Value>,
    ) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let embedding_spec = {
            let table_state = inner
                .state
                .tables
                .get(table)
                .ok_or_else(|| anyhow!("table not found"))?;
            if !row_exists(table_state, row_id)? {
                return Err(anyhow!("row not found"));
            }
            table_state.schema.validate_row(&fields)?;
            table_state.embedding_spec.clone()
        };
        let row = RowData {
            id: row_id,
            fields: fields.clone(),
        };

        let record = WalRecord::PutRow {
            table: table.to_string(),
            row_id,
            row: row.clone(),
        };
        inner.wal.append(&record, true)?;

        if let Some(table_state) = inner.state.tables.get_mut(table) {
            table_state.rows.insert(row_id, row);
            table_state.tombstones.remove(&row_id);
        }

        if let Some(spec) = embedding_spec {
            let content_hash = spec.content_hash(&fields)?;
            let job_record = WalRecord::EnqueueEmbedding {
                table: table.to_string(),
                row_id,
                content_hash: content_hash.clone(),
            };
            inner.wal.append(&job_record, true)?;

            if let Some(table_state) = inner.state.tables.get_mut(table) {
                table_state.embedding_meta.insert(
                    row_id,
                    EmbeddingMeta {
                        status: EmbeddingStatus::Pending,
                        content_hash,
                        last_error: None,
                        attempts: 0,
                        next_retry_at_ms: 0,
                    },
                );
            }
        }

        Ok(())
    }

    pub fn delete_row(&self, table: &str, row_id: u64) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let exists = {
            let table_state = inner
                .state
                .tables
                .get(table)
                .ok_or_else(|| anyhow!("table not found"))?;
            row_exists(table_state, row_id)?
        };
        if !exists {
            return Err(anyhow!("row not found"));
        }

        let record = WalRecord::DeleteRow {
            table: table.to_string(),
            row_id,
        };
        inner.wal.append(&record, true)?;

        if let Some(table_state) = inner.state.tables.get_mut(table) {
            table_state.rows.remove(&row_id);
            table_state.tombstones.insert(row_id);
            table_state.embeddings.remove(&row_id);
            table_state.embedding_meta.remove(&row_id);
        }

        Ok(())
    }

    pub fn get_row(&self, table: &str, row_id: u64) -> Result<Option<RowData>> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get(table)
            .ok_or_else(|| anyhow!("table not found"))?;
        load_row(table_state, row_id)
    }

    pub fn list_embedding_jobs(&self, table: &str) -> Result<Vec<EmbeddingJob>> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get(table)
            .ok_or_else(|| anyhow!("table not found"))?;

        let mut jobs = Vec::new();
        for (row_id, meta) in &table_state.embedding_meta {
            jobs.push(EmbeddingJob {
                table: table.to_string(),
                row_id: *row_id,
                status: meta.status,
                content_hash: meta.content_hash.clone(),
                last_error: meta.last_error.clone(),
            });
        }

        // Deterministic output for CLI/HTTP consumers.
        jobs.sort_by_key(|job| job.row_id);
        Ok(jobs)
    }

    pub fn retry_failed_jobs(&self, table: &str, row_id: Option<u64>) -> Result<usize> {
        let to_retry: Vec<u64> = {
            let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
            let table_state = inner
                .state
                .tables
                .get(table)
                .ok_or_else(|| anyhow!("table not found"))?;

            let mut out = Vec::new();
            for (id, meta) in &table_state.embedding_meta {
                if meta.status != EmbeddingStatus::Failed {
                    continue;
                }
                if let Some(filter) = row_id {
                    if *id != filter {
                        continue;
                    }
                }
                if row_exists(table_state, *id)? {
                    out.push(*id);
                }
            }
            out
        };

        let mut retried = 0usize;
        for id in to_retry {
            let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
            let status_record = WalRecord::UpdateEmbeddingStatus {
                table: table.to_string(),
                row_id: id,
                status: EmbeddingStatus::Pending,
                last_error: None,
                attempts: Some(0),
                next_retry_at_ms: Some(0),
            };
            inner.wal.append(&status_record, true)?;

            if let Some(table_state) = inner.state.tables.get_mut(table) {
                if let Some(meta) = table_state.embedding_meta.get_mut(&id) {
                    meta.status = EmbeddingStatus::Pending;
                    meta.last_error = None;
                    meta.attempts = 0;
                    meta.next_retry_at_ms = 0;
                }
            }

            retried += 1;
        }

        Ok(retried)
    }

    pub fn process_pending_jobs(&self, table: &str, embedder: &dyn Embedder) -> Result<usize> {
        self.process_pending_jobs_internal(table, embedder, None)
    }

    pub fn process_pending_jobs_with_limit(
        &self,
        table: &str,
        embedder: &dyn Embedder,
        limit: usize,
    ) -> Result<usize> {
        self.process_pending_jobs_internal(table, embedder, Some(limit))
    }

    fn process_pending_jobs_internal(
        &self,
        table: &str,
        embedder: &dyn Embedder,
        limit: Option<usize>,
    ) -> Result<usize> {
        self.process_pending_jobs_internal_at(table, embedder, limit, now_epoch_ms())
    }

    fn process_pending_jobs_internal_at(
        &self,
        table: &str,
        embedder: &dyn Embedder,
        limit: Option<usize>,
        now_ms: u64,
    ) -> Result<usize> {
        let pending_jobs: Vec<(u64, String)> = {
            let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
            let table_state = inner
                .state
                .tables
                .get(table)
                .ok_or_else(|| anyhow!("table not found"))?;

            let spec = match &table_state.embedding_spec {
                Some(spec) => spec.clone(),
                None => return Ok(0),
            };

            let mut jobs = Vec::new();

            let mut pending_row_ids: Vec<u64> = table_state
                .embedding_meta
                .iter()
                .filter_map(|(row_id, meta)| {
                    if meta.status == EmbeddingStatus::Pending && meta.next_retry_at_ms <= now_ms {
                        Some(*row_id)
                    } else {
                        None
                    }
                })
                .collect();
            pending_row_ids.sort();
            if let Some(limit) = limit {
                pending_row_ids.truncate(limit);
            }

            for row_id in pending_row_ids {
                if let Some(row) = load_row(table_state, row_id)? {
                    let input = spec.input_string(&row.fields)?;
                    jobs.push((row_id, input));
                }
            }
            jobs
        };

        let mut processed = 0usize;
        for (row_id, input) in pending_jobs {
            match embedder.embed(&input) {
                Ok(vector) => {
                    let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
                    let store_record = WalRecord::StoreEmbedding {
                        table: table.to_string(),
                        row_id,
                        vector: vector.clone(),
                    };
                    inner.wal.append(&store_record, true)?;

                    if let Some(table_state) = inner.state.tables.get_mut(table) {
                        table_state.embeddings.insert(row_id, vector);
                    }

                    let status_record = WalRecord::UpdateEmbeddingStatus {
                        table: table.to_string(),
                        row_id,
                        status: EmbeddingStatus::Ready,
                        last_error: None,
                        attempts: Some(0),
                        next_retry_at_ms: Some(0),
                    };
                    inner.wal.append(&status_record, true)?;

                    if let Some(table_state) = inner.state.tables.get_mut(table) {
                        if let Some(meta) = table_state.embedding_meta.get_mut(&row_id) {
                            meta.status = EmbeddingStatus::Ready;
                            meta.last_error = None;
                            meta.attempts = 0;
                            meta.next_retry_at_ms = 0;
                        }
                    }
                }
                Err(err) => {
                    let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
                    let (new_attempts, next_retry, new_status) =
                        if let Some(table_state) = inner.state.tables.get(table) {
                            if let Some(meta) = table_state.embedding_meta.get(&row_id) {
                                let attempts = meta.attempts.saturating_add(1);
                                if attempts >= EMBEDDING_MAX_ATTEMPTS {
                                    (attempts, 0u64, EmbeddingStatus::Failed)
                                } else {
                                    (
                                        attempts,
                                        now_ms.saturating_add(embedding_backoff_ms(attempts)),
                                        EmbeddingStatus::Pending,
                                    )
                                }
                            } else {
                                (
                                    1u32,
                                    now_ms.saturating_add(embedding_backoff_ms(1)),
                                    EmbeddingStatus::Pending,
                                )
                            }
                        } else {
                            (
                                1u32,
                                now_ms.saturating_add(embedding_backoff_ms(1)),
                                EmbeddingStatus::Pending,
                            )
                        };
                    let status_record = WalRecord::UpdateEmbeddingStatus {
                        table: table.to_string(),
                        row_id,
                        status: new_status,
                        last_error: Some(err.to_string()),
                        attempts: Some(new_attempts),
                        next_retry_at_ms: Some(next_retry),
                    };
                    inner.wal.append(&status_record, true)?;

                    if let Some(table_state) = inner.state.tables.get_mut(table) {
                        if let Some(meta) = table_state.embedding_meta.get_mut(&row_id) {
                            meta.status = new_status;
                            meta.last_error = Some(err.to_string());
                            meta.attempts = new_attempts;
                            meta.next_retry_at_ms = next_retry;
                        }
                    }
                }
            }

            processed += 1;
        }

        Ok(processed)
    }

    pub fn search_knn(
        &self,
        table: &str,
        query: &[f32],
        k: usize,
        metric: DistanceMetric,
    ) -> Result<Vec<SearchHit>> {
        let inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get(table)
            .ok_or_else(|| anyhow!("table not found"))?;

        let mut results: Vec<SearchResult> = Vec::new();
        for (row_id, vector) in &table_state.embeddings {
            if let Some(meta) = table_state.embedding_meta.get(row_id) {
                if meta.status != EmbeddingStatus::Ready {
                    continue;
                }
            }
            let dist = distance(query, vector, metric);
            results.push(SearchResult {
                row_id: *row_id,
                distance: dist,
            });
        }

        results.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        let hits = results
            .into_iter()
            .take(k)
            .map(|res| SearchHit {
                row_id: res.row_id,
                distance: res.distance,
            })
            .collect();

        Ok(hits)
    }

    pub fn flush_table(&self, table: &str) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get_mut(table)
            .ok_or_else(|| anyhow!("table not found"))?;
        flush_table_state(&self._config.data_dir, table, table_state)
    }

    pub fn compact_table(&self, table: &str) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;
        let table_state = inner
            .state
            .tables
            .get_mut(table)
            .ok_or_else(|| anyhow!("table not found"))?;

        let level_zero: Vec<SstFile> = table_state
            .sst_files
            .iter()
            .filter(|file| file.level == 0)
            .cloned()
            .collect();
        if level_zero.is_empty() {
            return Ok(());
        }

        let dir = sst::table_dir(&self._config.data_dir, table);
        sst::ensure_dir(&dir)?;
        let seq = table_state.next_sst_seq;
        table_state.next_sst_seq += 1;

        if let Some(new_file) = sst::compact_level_zero(&level_zero, &dir, seq)? {
            sst::remove_files(&level_zero)?;
            table_state.sst_files.retain(|file| file.level != 0);
            table_state.sst_files.push(new_file);
        }

        Ok(())
    }

    pub fn checkpoint(&self) -> Result<CheckpointStats> {
        let wal_path = self._config.data_dir.join("wal.log");
        let wal_prev_path = self._config.data_dir.join("wal.prev");
        let wal_new_path = self._config.data_dir.join("wal.log.new");
        let wal_dummy_path = self._config.data_dir.join("wal.checkpoint.tmp");

        let wal_bytes_before = fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);

        let mut inner = self.inner.lock().map_err(|_| anyhow!("lock poisoned"))?;

        // Flush all tables so row data is durably in SSTs and the checkpoint WAL can be compact.
        let table_names: Vec<String> = inner.state.tables.keys().cloned().collect();
        for table in table_names {
            let table_state = inner
                .state
                .tables
                .get_mut(&table)
                .ok_or_else(|| anyhow!("table not found"))?;
            flush_table_state(&self._config.data_dir, &table, table_state)?;
        }

        let mut records: Vec<WalRecord> = Vec::new();
        for (name, table_state) in inner.state.tables.iter() {
            records.push(WalRecord::CreateTable {
                name: name.clone(),
                schema: table_state.schema.clone(),
                embedding_spec: table_state.embedding_spec.clone(),
            });
            records.push(WalRecord::SetNextRowId {
                table: name.clone(),
                next_row_id: table_state.next_row_id,
            });

            for (row_id, meta) in &table_state.embedding_meta {
                records.push(WalRecord::EnqueueEmbedding {
                    table: name.clone(),
                    row_id: *row_id,
                    content_hash: meta.content_hash.clone(),
                });
                records.push(WalRecord::UpdateEmbeddingStatus {
                    table: name.clone(),
                    row_id: *row_id,
                    status: meta.status,
                    last_error: meta.last_error.clone(),
                    attempts: Some(meta.attempts),
                    next_retry_at_ms: Some(meta.next_retry_at_ms),
                });
            }

            for (row_id, vector) in &table_state.embeddings {
                records.push(WalRecord::StoreEmbedding {
                    table: name.clone(),
                    row_id: *row_id,
                    vector: vector.clone(),
                });
            }
        }

        // Write the new WAL snapshot.
        {
            let mut new_wal = Wal::create_new(wal_new_path.clone())?;
            for record in &records {
                new_wal.append(record, false)?;
            }
            new_wal.sync()?;
        }

        // Ensure `wal.log` is closed during rotation (important for Windows semantics).
        inner.wal = Wal::create_new(wal_dummy_path.clone())?;

        // Rotate with a `wal.prev` fallback to tolerate crashes between renames.
        if wal_prev_path.exists() {
            let _ = fs::remove_file(&wal_prev_path);
        }
        if wal_path.exists() {
            fs::rename(&wal_path, &wal_prev_path)?;
        }
        fs::rename(&wal_new_path, &wal_path)?;

        let wal_bytes_after = fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);

        inner.wal = Wal::open(wal_path)?;

        let _ = fs::remove_file(&wal_dummy_path);
        let _ = fs::remove_file(&wal_prev_path);

        Ok(CheckpointStats {
            wal_bytes_before,
            wal_bytes_after,
        })
    }
}

pub trait Embedder: Send + Sync {
    fn embed(&self, input: &str) -> Result<Vec<f32>>;
}

fn load_row(table_state: &TableState, row_id: u64) -> Result<Option<RowData>> {
    if let Some(row) = table_state.rows.get(&row_id) {
        return Ok(Some(row.clone()));
    }
    if table_state.tombstones.contains(&row_id) {
        return Ok(None);
    }

    for file in table_state.sst_files.iter().rev() {
        if let Some(entry) = sst::find_entry(&file.path, row_id)? {
            return Ok(entry.row);
        }
    }

    Ok(None)
}

fn row_exists(table_state: &TableState, row_id: u64) -> Result<bool> {
    Ok(load_row(table_state, row_id)?.is_some())
}

fn apply_record(state: &mut DbState, record: WalRecord) -> Result<()> {
    match record {
        WalRecord::CreateTable {
            name,
            schema,
            embedding_spec,
        } => {
            state.tables.insert(
                name,
                TableState {
                    schema,
                    next_row_id: 1,
                    rows: BTreeMap::new(),
                    tombstones: BTreeSet::new(),
                    embeddings: HashMap::new(),
                    embedding_meta: HashMap::new(),
                    embedding_spec,
                    sst_files: Vec::new(),
                    next_sst_seq: 1,
                },
            );
        }
        WalRecord::SetNextRowId { table, next_row_id } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                table_state.next_row_id = next_row_id;
            }
        }
        WalRecord::PutRow { table, row_id, row } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                table_state.rows.insert(row_id, row);
                table_state.tombstones.remove(&row_id);
                if row_id >= table_state.next_row_id {
                    table_state.next_row_id = row_id + 1;
                }
            }
        }
        WalRecord::DeleteRow { table, row_id } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                table_state.rows.remove(&row_id);
                table_state.tombstones.insert(row_id);
                table_state.embeddings.remove(&row_id);
                table_state.embedding_meta.remove(&row_id);
            }
        }
        WalRecord::EnqueueEmbedding {
            table,
            row_id,
            content_hash,
        } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                table_state.embedding_meta.insert(
                    row_id,
                    EmbeddingMeta {
                        status: EmbeddingStatus::Pending,
                        content_hash,
                        last_error: None,
                        attempts: 0,
                        next_retry_at_ms: 0,
                    },
                );
            }
        }
        WalRecord::UpdateEmbeddingStatus {
            table,
            row_id,
            status,
            last_error,
            attempts,
            next_retry_at_ms,
        } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                if let Some(meta) = table_state.embedding_meta.get_mut(&row_id) {
                    meta.status = status;
                    meta.last_error = last_error;
                    if let Some(attempts) = attempts {
                        meta.attempts = attempts;
                    }
                    if let Some(next_retry_at_ms) = next_retry_at_ms {
                        meta.next_retry_at_ms = next_retry_at_ms;
                    }
                }
            }
        }
        WalRecord::StoreEmbedding {
            table,
            row_id,
            vector,
        } => {
            if let Some(table_state) = state.tables.get_mut(&table) {
                table_state.embeddings.insert(row_id, vector);
            }
        }
    }

    Ok(())
}

fn flush_table_state(
    root: &std::path::Path,
    table: &str,
    table_state: &mut TableState,
) -> Result<()> {
    if table_state.rows.is_empty() && table_state.tombstones.is_empty() {
        return Ok(());
    }

    let dir = sst::table_dir(root, table);
    sst::ensure_dir(&dir)?;

    let mut entries: Vec<SstEntry> = Vec::new();
    for row in table_state.rows.values() {
        entries.push(SstEntry {
            row_id: row.id,
            row: Some(row.clone()),
        });
    }
    for row_id in &table_state.tombstones {
        entries.push(SstEntry {
            row_id: *row_id,
            row: None,
        });
    }
    entries.sort_by_key(|entry| entry.row_id);

    let seq = table_state.next_sst_seq;
    table_state.next_sst_seq += 1;
    let path = sst::write_sst(&dir, 0, seq, &entries)?;
    table_state.sst_files.push(SstFile {
        level: 0,
        seq,
        path,
    });
    table_state.rows.clear();
    table_state.tombstones.clear();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct DummyEmbedder;

    impl Embedder for DummyEmbedder {
        fn embed(&self, input: &str) -> Result<Vec<f32>> {
            Ok(vec![input.len() as f32])
        }
    }

    struct AlwaysFailEmbedder;

    impl Embedder for AlwaysFailEmbedder {
        fn embed(&self, _input: &str) -> Result<Vec<f32>> {
            Err(anyhow!("boom"))
        }
    }

    #[test]
    fn insert_and_process_embedding_job() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        let embed_spec = EmbeddingSpec::new(vec!["title", "body"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));

        let row_id = db.insert_row("notes", fields).unwrap();
        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, EmbeddingStatus::Pending);
        assert_eq!(jobs[0].row_id, row_id);

        let processed = db.process_pending_jobs("notes", &DummyEmbedder).unwrap();
        assert_eq!(processed, 1);

        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs[0].status, EmbeddingStatus::Ready);
    }

    #[test]
    fn retry_failed_embedding_job_resets_status_and_error() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        let embed_spec = EmbeddingSpec::new(vec!["title", "body"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));

        let row_id = db.insert_row("notes", fields).unwrap();

        // Drive the job to terminal failure by repeatedly processing it after its backoff expires.
        let mut now_ms = 1_000_000u64;
        for attempt in 1..EMBEDDING_MAX_ATTEMPTS {
            let processed = db
                .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, now_ms)
                .unwrap();
            assert_eq!(processed, 1);

            let jobs = db.list_embedding_jobs("notes").unwrap();
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].row_id, row_id);
            assert_eq!(jobs[0].status, EmbeddingStatus::Pending);
            assert_eq!(jobs[0].last_error.as_deref(), Some("boom"));

            let inner = db.inner.lock().unwrap();
            let meta = inner
                .state
                .tables
                .get("notes")
                .unwrap()
                .embedding_meta
                .get(&row_id)
                .unwrap();
            assert_eq!(meta.attempts, attempt);
            assert!(meta.next_retry_at_ms > now_ms);
            now_ms = meta.next_retry_at_ms;
        }

        let processed = db
            .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, now_ms)
            .unwrap();
        assert_eq!(processed, 1);

        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].row_id, row_id);
        assert_eq!(jobs[0].status, EmbeddingStatus::Failed);
        assert_eq!(jobs[0].last_error.as_deref(), Some("boom"));

        let retried = db.retry_failed_jobs("notes", None).unwrap();
        assert_eq!(retried, 1);

        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs[0].status, EmbeddingStatus::Pending);
        assert!(jobs[0].last_error.is_none());

        let processed = db.process_pending_jobs("notes", &DummyEmbedder).unwrap();
        assert_eq!(processed, 1);

        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs[0].status, EmbeddingStatus::Ready);
        assert!(jobs[0].last_error.is_none());
    }

    #[test]
    fn embedding_retry_backoff_defers_until_next_retry_time() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
        let embed_spec = EmbeddingSpec::new(vec!["title"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        let row_id = db.insert_row("notes", fields).unwrap();

        let now_ms = 1_000_000u64;
        let processed = db
            .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, now_ms)
            .unwrap();
        assert_eq!(processed, 1);

        let inner = db.inner.lock().unwrap();
        let meta = inner
            .state
            .tables
            .get("notes")
            .unwrap()
            .embedding_meta
            .get(&row_id)
            .unwrap()
            .clone();
        drop(inner);
        assert_eq!(meta.attempts, 1);
        assert!(meta.next_retry_at_ms > now_ms);

        // Too early: should skip.
        let processed = db
            .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, now_ms)
            .unwrap();
        assert_eq!(processed, 0);

        // At/after the scheduled time: should attempt again.
        let processed = db
            .process_pending_jobs_internal_at(
                "notes",
                &AlwaysFailEmbedder,
                None,
                meta.next_retry_at_ms,
            )
            .unwrap();
        assert_eq!(processed, 1);

        let inner = db.inner.lock().unwrap();
        let meta2 = inner
            .state
            .tables
            .get("notes")
            .unwrap()
            .embedding_meta
            .get(&row_id)
            .unwrap();
        assert_eq!(meta2.attempts, 2);
    }

    #[test]
    fn process_pending_jobs_limit_processes_subset() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
        let embed_spec = EmbeddingSpec::new(vec!["title"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        for i in 0..3 {
            let mut fields = BTreeMap::new();
            fields.insert("title".to_string(), Value::String(format!("note-{i}")));
            db.insert_row("notes", fields).unwrap();
        }

        let processed = db
            .process_pending_jobs_with_limit("notes", &DummyEmbedder, 2)
            .unwrap();
        assert_eq!(processed, 2);

        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs.len(), 3);
        assert_eq!(
            jobs.iter()
                .filter(|job| job.status == EmbeddingStatus::Ready)
                .count(),
            2
        );
        assert_eq!(
            jobs.iter()
                .filter(|job| job.status == EmbeddingStatus::Pending)
                .count(),
            1
        );

        let processed = db.process_pending_jobs("notes", &DummyEmbedder).unwrap();
        assert_eq!(processed, 1);
    }

    #[test]
    fn db_stats_reports_tables_and_wal_bytes() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();
        db.create_table(
            "notes",
            TableSchema::new(vec![Column::new("title", DataType::String, false)]),
            None,
        )
        .unwrap();

        let stats = db.db_stats().unwrap();
        assert_eq!(stats.tables, 1);
    }

    #[test]
    fn flush_and_read_from_sst() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        db.create_table("notes", schema, None).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));

        let row_id = db.insert_row("notes", fields).unwrap();
        db.flush_table("notes").unwrap();

        let row = db.get_row("notes", row_id).unwrap();
        assert!(row.is_some());
    }

    #[test]
    fn delete_flush_tombstone_hides_row() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
        db.create_table("notes", schema, None).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        let row_id = db.insert_row("notes", fields).unwrap();
        db.flush_table("notes").unwrap();

        db.delete_row("notes", row_id).unwrap();
        db.flush_table("notes").unwrap();

        let row = db.get_row("notes", row_id).unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn list_and_describe_tables() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        db.create_table(
            "notes",
            TableSchema::new(vec![Column::new("title", DataType::String, false)]),
            Some(EmbeddingSpec::new(vec!["title"])),
        )
        .unwrap();
        db.create_table(
            "users",
            TableSchema::new(vec![Column::new("name", DataType::String, false)]),
            None,
        )
        .unwrap();

        let tables = db.list_tables().unwrap();
        assert_eq!(tables, vec!["notes".to_string(), "users".to_string()]);

        let desc = db.describe_table("notes").unwrap();
        assert_eq!(desc.name, "notes");
        assert!(desc.embedding_spec.is_some());
    }

    #[test]
    fn table_stats_counts_embeddings() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        let embed_spec = EmbeddingSpec::new(vec!["title", "body"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));
        db.insert_row("notes", fields).unwrap();

        let stats = db.table_stats("notes").unwrap();
        assert_eq!(stats.embeddings_total, 1);
        assert_eq!(stats.embeddings_pending, 1);

        let processed = db.process_pending_jobs("notes", &DummyEmbedder).unwrap();
        assert_eq!(processed, 1);

        let stats = db.table_stats("notes").unwrap();
        assert_eq!(stats.embeddings_ready, 1);
        assert_eq!(stats.embeddings_pending, 0);
    }

    #[test]
    fn compacted_rows_survive_reopen_and_tombstones_hide_deleted_rows() {
        let dir = tempdir().unwrap();
        let data_dir = dir.path().to_path_buf();
        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);

        let db = EmbedDb::open(Config::new(data_dir.clone())).unwrap();
        db.create_table("notes", schema.clone(), None).unwrap();

        let mut first = BTreeMap::new();
        first.insert("title".to_string(), Value::String("v1".to_string()));
        let row_id = db.insert_row("notes", first).unwrap();
        db.flush_table("notes").unwrap();

        db.compact_table("notes").unwrap();
        drop(db);

        let reopened = EmbedDb::open(Config::new(data_dir.clone())).unwrap();
        let row = reopened.get_row("notes", row_id).unwrap().unwrap();
        assert_eq!(
            row.fields.get("title"),
            Some(&Value::String("v1".to_string()))
        );

        reopened.delete_row("notes", row_id).unwrap();
        reopened.flush_table("notes").unwrap();
        reopened.compact_table("notes").unwrap();
        drop(reopened);

        let reopened_again = EmbedDb::open(Config::new(data_dir)).unwrap();
        let row = reopened_again.get_row("notes", row_id).unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn update_row_after_flush_and_compaction() {
        let dir = tempdir().unwrap();
        let data_dir = dir.path().to_path_buf();
        let db = EmbedDb::open(Config::new(data_dir.clone())).unwrap();
        db.create_table(
            "notes",
            TableSchema::new(vec![Column::new("title", DataType::String, false)]),
            None,
        )
        .unwrap();

        let mut first = BTreeMap::new();
        first.insert("title".to_string(), Value::String("v1".to_string()));
        let row_id = db.insert_row("notes", first).unwrap();
        db.flush_table("notes").unwrap();

        let mut second = BTreeMap::new();
        second.insert("title".to_string(), Value::String("v2".to_string()));
        db.update_row("notes", row_id, second).unwrap();
        db.flush_table("notes").unwrap();
        db.compact_table("notes").unwrap();
        drop(db);

        let reopened = EmbedDb::open(Config::new(data_dir)).unwrap();
        let row = reopened.get_row("notes", row_id).unwrap().unwrap();
        assert_eq!(
            row.fields.get("title"),
            Some(&Value::String("v2".to_string()))
        );
    }

    #[test]
    fn process_pending_jobs_after_flush_and_reopen() {
        let dir = tempdir().unwrap();
        let data_dir = dir.path().to_path_buf();
        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        let embed_spec = EmbeddingSpec::new(vec!["title", "body"]);

        let db = EmbedDb::open(Config::new(data_dir.clone())).unwrap();
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));
        let row_id = db.insert_row("notes", fields).unwrap();
        db.flush_table("notes").unwrap();
        drop(db);

        let reopened = EmbedDb::open(Config::new(data_dir)).unwrap();
        let jobs = reopened.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, EmbeddingStatus::Pending);

        let processed = reopened
            .process_pending_jobs("notes", &DummyEmbedder)
            .unwrap();
        assert_eq!(processed, 1);

        let jobs = reopened.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs[0].status, EmbeddingStatus::Ready);

        let hits = reopened
            .search_knn("notes", &[11.0], 1, DistanceMetric::L2)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].row_id, row_id);
    }

    #[test]
    fn checkpoint_truncates_wal_and_preserves_next_row_id() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
        db.create_table("notes", schema, None).unwrap();

        for i in 0..200u64 {
            let mut fields = BTreeMap::new();
            fields.insert("title".to_string(), Value::String(format!("row-{i}")));
            let row_id = db.insert_row("notes", fields).unwrap();
            assert_eq!(row_id, i + 1);
        }
        db.flush_table("notes").unwrap();
        db.compact_table("notes").unwrap();

        let before = db.db_stats().unwrap().wal_bytes;
        let stats = db.checkpoint().unwrap();
        assert_eq!(stats.wal_bytes_before, before);
        assert!(stats.wal_bytes_after <= stats.wal_bytes_before);

        drop(db);
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        // Ensure ID allocation continues, even though row data now lives in SSTs.
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("next".to_string()));
        let row_id = db.insert_row("notes", fields).unwrap();
        assert_eq!(row_id, 201);
    }

    #[test]
    fn checkpoint_preserves_embedding_meta_and_vectors() {
        let dir = tempdir().unwrap();
        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

        let schema = TableSchema::new(vec![
            Column::new("title", DataType::String, false),
            Column::new("body", DataType::String, false),
        ]);
        let embed_spec = EmbeddingSpec::new(vec!["title", "body"]);
        db.create_table("notes", schema, Some(embed_spec)).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        fields.insert("body".to_string(), Value::String("World".to_string()));
        let row_id = db.insert_row("notes", fields).unwrap();
        db.process_pending_jobs("notes", &DummyEmbedder).unwrap();

        // Force row to live on SST so correctness doesn't depend on memtable replay.
        db.flush_table("notes").unwrap();
        db.compact_table("notes").unwrap();

        db.checkpoint().unwrap();
        drop(db);

        let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();
        let jobs = db.list_embedding_jobs("notes").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].row_id, row_id);
        assert_eq!(jobs[0].status, EmbeddingStatus::Ready);

        let query = DummyEmbedder.embed("Hello\nWorld").unwrap();
        let hits = db
            .search_knn("notes", &query, 1, DistanceMetric::L2)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].row_id, row_id);
    }

    #[test]
    fn open_recovers_from_interrupted_checkpoint_wal_rotation() {
        let dir = tempdir().unwrap();
        let config = Config::new(dir.path().to_path_buf());
        let db = EmbedDb::open(config.clone()).unwrap();

        let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
        db.create_table("notes", schema, None).unwrap();

        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Hello".to_string()));
        db.insert_row("notes", fields).unwrap();
        drop(db);

        // Simulate a crash after moving wal.log to wal.prev but before promoting a new wal.log.
        let wal_path = config.data_dir.join("wal.log");
        let prev_path = config.data_dir.join("wal.prev");
        fs::rename(&wal_path, &prev_path).unwrap();

        let db = EmbedDb::open(config).unwrap();
        let row = db.get_row("notes", 1).unwrap().unwrap();
        assert_eq!(
            row.fields.get("title"),
            Some(&Value::String("Hello".to_string()))
        );
    }
}
