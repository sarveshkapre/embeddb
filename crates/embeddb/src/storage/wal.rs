use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use anyhow::Result;
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

use crate::schema::{EmbeddingSpec, RowData, TableSchema};
use crate::EmbeddingStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalRecord {
    CreateTable {
        name: String,
        schema: TableSchema,
        embedding_spec: Option<EmbeddingSpec>,
    },
    PutRow {
        table: String,
        row_id: u64,
        row: RowData,
    },
    DeleteRow {
        table: String,
        row_id: u64,
    },
    EnqueueEmbedding {
        table: String,
        row_id: u64,
        content_hash: String,
    },
    UpdateEmbeddingStatus {
        table: String,
        row_id: u64,
        status: EmbeddingStatus,
        last_error: Option<String>,
    },
    StoreEmbedding {
        table: String,
        row_id: u64,
        vector: Vec<f32>,
    },
}

#[derive(Debug)]
pub struct Wal {
    path: PathBuf,
    file: File,
}

impl Wal {
    pub fn open(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;

        Ok(Self { path, file })
    }

    pub fn append(&mut self, record: &WalRecord, sync: bool) -> Result<()> {
        let data = serde_json::to_vec(record)?;
        let mut hasher = Hasher::new();
        hasher.update(&data);
        let checksum = hasher.finalize();
        let len = data.len() as u32;

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&checksum.to_le_bytes())?;
        self.file.write_all(&data)?;
        self.file.flush()?;
        if sync {
            self.file.sync_data()?;
        }
        Ok(())
    }

    pub fn replay(&self) -> Result<Vec<WalRecord>> {
        let file = OpenOptions::new().read(true).open(&self.path)?;
        let mut reader = BufReader::new(file);

        let mut records = Vec::new();

        loop {
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(()) => {}
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    return Err(err.into());
                }
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut checksum_buf = [0u8; 4];
            if reader.read_exact(&mut checksum_buf).is_err() {
                break;
            }
            let expected = u32::from_le_bytes(checksum_buf);
            let mut data = vec![0u8; len];
            if reader.read_exact(&mut data).is_err() {
                break;
            }

            let mut hasher = Hasher::new();
            hasher.update(&data);
            let actual = hasher.finalize();
            if actual != expected {
                break;
            }

            match serde_json::from_slice::<WalRecord>(&data) {
                Ok(record) => {
                    records.push(record);
                }
                Err(_) => {
                    break;
                }
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn wal_replay_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal.log");
        let mut wal = Wal::open(path.clone()).unwrap();

        wal.append(
            &WalRecord::DeleteRow {
                table: "t".to_string(),
                row_id: 1,
            },
            true,
        )
        .unwrap();

        let wal = Wal::open(path).unwrap();
        let records = wal.replay().unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn wal_ignores_partial_record() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal.log");
        let mut wal = Wal::open(path.clone()).unwrap();

        wal.append(
            &WalRecord::DeleteRow {
                table: "t".to_string(),
                row_id: 2,
            },
            true,
        )
        .unwrap();

        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&10u32.to_le_bytes()).unwrap();
        file.flush().unwrap();

        let wal = Wal::open(path).unwrap();
        let records = wal.replay().unwrap();
        assert_eq!(records.len(), 1);
    }
}
