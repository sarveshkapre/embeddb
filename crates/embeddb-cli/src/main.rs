use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use embeddb::{
    Column, Config, DataType, DistanceMetric, EmbedDb, Embedder, EmbeddingSpec, TableSchema, Value,
};
use serde::Deserialize;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "embeddb")]
#[command(about = "EmbedDB CLI")]
struct Cli {
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    ListTables,
    DescribeTable {
        table: String,
    },
    CreateTable {
        table: String,
        #[arg(long)]
        schema: PathBuf,
        #[arg(long)]
        embed_fields: Option<String>,
    },
    Insert {
        table: String,
        #[arg(long)]
        row: String,
    },
    Get {
        table: String,
        row_id: u64,
    },
    Delete {
        table: String,
        row_id: u64,
    },
    Jobs {
        table: String,
    },
    ProcessJobs {
        table: String,
    },
    Search {
        table: String,
        #[arg(long)]
        query: String,
        #[arg(long, default_value_t = 5)]
        k: usize,
        #[arg(long, value_enum, default_value_t = MetricArg::Cosine)]
        metric: MetricArg,
    },
    Flush {
        table: String,
    },
    Compact {
        table: String,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum MetricArg {
    Cosine,
    L2,
}

impl From<MetricArg> for DistanceMetric {
    fn from(value: MetricArg) -> Self {
        match value {
            MetricArg::Cosine => DistanceMetric::Cosine,
            MetricArg::L2 => DistanceMetric::L2,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SchemaFile {
    columns: Vec<Column>,
}

struct LocalHashEmbedder;

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

    let cli = Cli::parse();
    let db = EmbedDb::open(Config::new(cli.data_dir))?;

    match cli.command {
        Commands::ListTables => {
            let tables = db.list_tables()?;
            for table in tables {
                println!("{}", table);
            }
        }
        Commands::DescribeTable { table } => {
            let desc = db.describe_table(&table)?;
            println!("{}", serde_json::to_string_pretty(&desc)?);
        }
        Commands::CreateTable {
            table,
            schema,
            embed_fields,
        } => {
            let schema = load_schema(schema)?;
            let embed_spec = embed_fields.map(|fields| {
                let parts: Vec<String> = fields
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                EmbeddingSpec::new(parts)
            });
            db.create_table(table, schema, embed_spec)?;
            println!("ok");
        }
        Commands::Insert { table, row } => {
            let fields = parse_row(&row)?;
            let row_id = db.insert_row(&table, fields)?;
            println!("{}", row_id);
        }
        Commands::Get { table, row_id } => {
            let row = db.get_row(&table, row_id)?;
            println!("{}", serde_json::to_string_pretty(&row)?);
        }
        Commands::Delete { table, row_id } => {
            db.delete_row(&table, row_id)?;
            println!("ok");
        }
        Commands::Jobs { table } => {
            let jobs = db.list_embedding_jobs(&table)?;
            println!("{}", serde_json::to_string_pretty(&jobs)?);
        }
        Commands::ProcessJobs { table } => {
            let processed = db.process_pending_jobs(&table, &LocalHashEmbedder)?;
            println!("{}", processed);
        }
        Commands::Search {
            table,
            query,
            k,
            metric,
        } => {
            let query_vec = parse_vector(&query)?;
            let hits = db.search_knn(&table, &query_vec, k, metric.into())?;
            println!("{}", serde_json::to_string_pretty(&hits)?);
        }
        Commands::Flush { table } => {
            db.flush_table(&table)?;
            println!("ok");
        }
        Commands::Compact { table } => {
            db.compact_table(&table)?;
            println!("ok");
        }
    }

    Ok(())
}

fn load_schema(path: PathBuf) -> Result<TableSchema> {
    let data = fs::read_to_string(path)?;
    let schema: SchemaFile = serde_json::from_str(&data)?;
    Ok(TableSchema::new(schema.columns))
}

fn parse_row(input: &str) -> Result<BTreeMap<String, Value>> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("row must be a JSON object"))?;
    let mut fields = BTreeMap::new();
    for (key, val) in object {
        fields.insert(key.clone(), json_to_value(val)?);
    }
    Ok(fields)
}

fn json_to_value(value: &serde_json::Value) -> Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(v) => Value::Bool(*v),
        serde_json::Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                Value::Int(i)
            } else if let Some(f) = v.as_f64() {
                Value::Float(f)
            } else {
                return Err(anyhow!("invalid number"));
            }
        }
        serde_json::Value::String(v) => Value::String(v.clone()),
        serde_json::Value::Array(arr) => {
            let bytes: Result<Vec<u8>> = arr
                .iter()
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

fn parse_vector(input: &str) -> Result<Vec<f32>> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("query must be a JSON array"))?;
    let mut out = Vec::new();
    for item in arr {
        let num = item
            .as_f64()
            .ok_or_else(|| anyhow!("query values must be numbers"))?;
        out.push(num as f32);
    }
    Ok(out)
}

#[allow(dead_code)]
fn example_schema() -> TableSchema {
    TableSchema::new(vec![
        Column::new("title", DataType::String, false),
        Column::new("body", DataType::String, false),
    ])
}
