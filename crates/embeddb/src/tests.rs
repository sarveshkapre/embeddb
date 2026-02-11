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
        .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, meta.next_retry_at_ms)
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
    assert!(stats.wal_bytes > 0);
    assert!(stats.wal_durable_appends > 0);
    assert!(stats.wal_sync_ops >= stats.wal_durable_appends);
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
fn search_knn_filtered_applies_scalar_filters() {
    let dir = tempdir().unwrap();
    let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

    let schema = TableSchema::new(vec![
        Column::new("title", DataType::String, false),
        Column::new("score", DataType::Float, false),
        Column::new("age", DataType::Int, false),
    ]);
    let embed_spec = EmbeddingSpec::new(vec!["title"]);
    db.create_table("notes", schema, Some(embed_spec)).unwrap();

    let mut a = BTreeMap::new();
    a.insert("title".to_string(), Value::String("Hello".to_string()));
    a.insert("score".to_string(), Value::Float(0.1));
    a.insert("age".to_string(), Value::Int(10));
    let row_a = db.insert_row("notes", a).unwrap();

    let mut b = BTreeMap::new();
    b.insert("title".to_string(), Value::String("Greetings".to_string()));
    b.insert("score".to_string(), Value::Float(0.9));
    b.insert("age".to_string(), Value::Int(99));
    let row_b = db.insert_row("notes", b).unwrap();

    db.process_pending_jobs("notes", &DummyEmbedder).unwrap();

    let filters = vec![FilterCondition {
        column: "score".to_string(),
        op: FilterOp::Lt,
        value: Value::Float(0.5),
    }];
    let hits = db
        .search_knn_filtered("notes", &[5.0], 10, DistanceMetric::L2, &filters)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].row_id, row_a);

    let filters = vec![FilterCondition {
        column: "age".to_string(),
        op: FilterOp::Gte,
        value: Value::Int(50),
    }];
    let hits = db
        .search_knn_filtered("notes", &[5.0], 10, DistanceMetric::L2, &filters)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].row_id, row_b);

    let filters = vec![FilterCondition {
        column: "title".to_string(),
        op: FilterOp::Eq,
        value: Value::String("Hello".to_string()),
    }];
    let hits = db
        .search_knn_filtered("notes", &[5.0], 10, DistanceMetric::L2, &filters)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].row_id, row_a);
}

#[test]
fn wal_autocheckpoint_triggers_before_write() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let db =
        EmbedDb::open(Config::new(data_dir.clone()).with_wal_autocheckpoint_bytes(512)).unwrap();
    db.create_table(
        "notes",
        TableSchema::new(vec![Column::new("title", DataType::String, false)]),
        None,
    )
    .unwrap();

    for i in 0..200u64 {
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String(format!("row-{i}")));
        db.insert_row("notes", fields).unwrap();
    }

    // Auto-checkpoint flushes tables; ensure we produced SST output without explicitly calling
    // flush/compact/checkpoint.
    let stats = db.table_stats("notes").unwrap();
    assert!(stats.sst_files > 0);
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

#[test]
fn snapshot_export_and_restore_roundtrip() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let db = EmbedDb::open(Config::new(data_dir.clone())).unwrap();

    let schema = TableSchema::new(vec![Column::new("title", DataType::String, false)]);
    db.create_table("notes", schema, None).unwrap();

    let mut fields = BTreeMap::new();
    fields.insert("title".to_string(), Value::String("Hello".to_string()));
    let row_id = db.insert_row("notes", fields).unwrap();

    let snap_parent = tempdir().unwrap();
    let snap_dir = snap_parent.path().join("snapshot");
    let _ = db.export_snapshot(&snap_dir).unwrap();
    drop(db);

    let restored_parent = tempdir().unwrap();
    let restored_dir = restored_parent.path().join("restored");
    let _ = EmbedDb::restore_snapshot(&snap_dir, &restored_dir).unwrap();

    let reopened = EmbedDb::open(Config::new(restored_dir)).unwrap();
    let row = reopened.get_row("notes", row_id).unwrap().unwrap();
    assert_eq!(
        row.fields.get("title"),
        Some(&Value::String("Hello".to_string()))
    );
}

#[test]
fn table_and_db_stats_track_runtime_operation_metrics() {
    let dir = tempdir().unwrap();
    let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();

    let schema = TableSchema::new(vec![
        Column::new("title", DataType::String, false),
        Column::new("body", DataType::String, false),
    ]);
    db.create_table(
        "notes",
        schema,
        Some(EmbeddingSpec::new(vec!["title", "body"])),
    )
    .unwrap();

    let mut fields = BTreeMap::new();
    fields.insert("title".to_string(), Value::String("Hello".to_string()));
    fields.insert("body".to_string(), Value::String("World".to_string()));
    db.insert_row("notes", fields).unwrap();
    db.process_pending_jobs("notes", &DummyEmbedder).unwrap();
    db.flush_table("notes").unwrap();
    db.compact_table("notes").unwrap();
    db.checkpoint().unwrap();

    let table_stats = db.table_stats("notes").unwrap();
    assert_eq!(table_stats.embeddings_processed_total, 1);
    assert_eq!(table_stats.embeddings_failed_total, 0);
    assert_eq!(table_stats.embeddings_retried_total, 0);
    assert!(table_stats.wal_durable_appends >= 4);
    assert_eq!(table_stats.flush_count, 1);
    assert_eq!(table_stats.compact_count, 1);

    let db_stats = db.db_stats().unwrap();
    assert!(db_stats.wal_durable_appends >= table_stats.wal_durable_appends);
    assert!(db_stats.wal_sync_ops > db_stats.wal_durable_appends);
    assert_eq!(db_stats.checkpoints, 1);
    assert_eq!(db_stats.auto_checkpoints, 0);
    assert_eq!(db_stats.embeddings_processed_total, 1);
    assert_eq!(db_stats.embeddings_failed_total, 0);
    assert_eq!(db_stats.embeddings_retried_total, 0);
    assert!(db_stats.flush_count_total >= 1);
    assert!(db_stats.compact_count_total >= 1);
}

#[test]
fn table_and_db_stats_track_retry_and_failure_metrics() {
    let dir = tempdir().unwrap();
    let db = EmbedDb::open(Config::new(dir.path().to_path_buf())).unwrap();
    db.create_table(
        "notes",
        TableSchema::new(vec![Column::new("title", DataType::String, false)]),
        Some(EmbeddingSpec::new(vec!["title"])),
    )
    .unwrap();

    let mut fields = BTreeMap::new();
    fields.insert("title".to_string(), Value::String("Hello".to_string()));
    let row_id = db.insert_row("notes", fields).unwrap();

    let now_ms = 1_000_000u64;
    let processed = db
        .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, now_ms)
        .unwrap();
    assert_eq!(processed, 1);
    let retried = db.retry_failed_jobs("notes", Some(row_id)).unwrap();
    assert_eq!(retried, 0);

    // Drive to failed and retry.
    let mut tick = now_ms;
    for _ in 0..EMBEDDING_MAX_ATTEMPTS {
        let _ = db
            .process_pending_jobs_internal_at("notes", &AlwaysFailEmbedder, None, tick)
            .unwrap();
        let jobs = db.list_embedding_jobs("notes").unwrap();
        if jobs[0].status == EmbeddingStatus::Failed {
            break;
        }
        tick = jobs[0].next_retry_at_ms;
    }
    let retried = db.retry_failed_jobs("notes", Some(row_id)).unwrap();
    assert_eq!(retried, 1);

    let table_stats = db.table_stats("notes").unwrap();
    assert!(table_stats.embeddings_failed_total >= EMBEDDING_MAX_ATTEMPTS as u64);
    assert_eq!(table_stats.embeddings_retried_total, 1);
    let db_stats = db.db_stats().unwrap();
    assert!(db_stats.embeddings_failed_total >= EMBEDDING_MAX_ATTEMPTS as u64);
    assert_eq!(db_stats.embeddings_retried_total, 1);
}
