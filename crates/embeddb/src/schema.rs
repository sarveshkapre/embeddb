use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::EmbeddingStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Int,
    Float,
    Bool,
    String,
    Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

impl Column {
    pub fn new(name: impl Into<String>, data_type: DataType, nullable: bool) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub columns: Vec<Column>,
}

impl TableSchema {
    pub fn new(columns: Vec<Column>) -> Self {
        Self { columns }
    }

    pub fn validate_schema(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for col in &self.columns {
            if !seen.insert(col.name.clone()) {
                return Err(anyhow!("duplicate column: {}", col.name));
            }
        }
        Ok(())
    }

    pub fn validate_row(&self, fields: &BTreeMap<String, Value>) -> Result<()> {
        for col in &self.columns {
            match fields.get(&col.name) {
                Some(value) => {
                    if !value.matches(&col.data_type) {
                        return Err(anyhow!("column '{}' type mismatch", col.name));
                    }
                }
                None => {
                    if !col.nullable {
                        return Err(anyhow!("missing required column '{}'", col.name));
                    }
                }
            }
        }
        for key in fields.keys() {
            if !self.columns.iter().any(|col| &col.name == key) {
                return Err(anyhow!("unknown column '{}'", key));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    Null,
}

impl Value {
    pub fn matches(&self, data_type: &DataType) -> bool {
        matches!(
            (self, data_type),
            (Value::Int(_), DataType::Int)
                | (Value::Float(_), DataType::Float)
                | (Value::Bool(_), DataType::Bool)
                | (Value::String(_), DataType::String)
                | (Value::Bytes(_), DataType::Bytes)
                | (Value::Null, _)
        )
    }

    pub fn as_string(&self) -> Result<String> {
        match self {
            Value::Int(v) => Ok(v.to_string()),
            Value::Float(v) => Ok(v.to_string()),
            Value::Bool(v) => Ok(v.to_string()),
            Value::String(v) => Ok(v.clone()),
            Value::Bytes(v) => Ok(general_purpose::STANDARD.encode(v)),
            Value::Null => Ok("".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowData {
    pub id: u64,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSpec {
    pub source_fields: Vec<String>,
}

impl EmbeddingSpec {
    pub fn new<S: Into<String>>(fields: Vec<S>) -> Self {
        Self {
            source_fields: fields.into_iter().map(Into::into).collect(),
        }
    }

    pub fn input_string(&self, fields: &BTreeMap<String, Value>) -> Result<String> {
        let mut parts = Vec::new();
        for field in &self.source_fields {
            let value = fields
                .get(field)
                .ok_or_else(|| anyhow!("missing embedding field '{}'", field))?;
            parts.push(value.as_string()?);
        }
        Ok(parts.join("\n"))
    }

    pub fn content_hash(&self, fields: &BTreeMap<String, Value>) -> Result<String> {
        let input = self.input_string(fields)?;
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMeta {
    pub status: EmbeddingStatus,
    pub content_hash: String,
    pub last_error: Option<String>,
}
