use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::schema::RowData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SstEntry {
    pub row_id: u64,
    pub row: Option<RowData>,
}

#[derive(Debug, Clone)]
pub struct SstFile {
    pub level: u32,
    pub seq: u64,
    pub path: PathBuf,
}

impl SstFile {
    pub fn filename(level: u32, seq: u64) -> String {
        format!("sst_L{}_{}.json", level, seq)
    }
}

pub fn table_dir(root: &Path, table: &str) -> PathBuf {
    root.join("tables").join(table)
}

pub fn list_sst_files(dir: &Path) -> Result<Vec<SstFile>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
            if let Some((level, seq)) = parse_filename(file_name) {
                files.push(SstFile { level, seq, path });
            }
        }
    }

    files.sort_by_key(|f| (f.level, f.seq));
    Ok(files)
}

pub fn write_sst(dir: &Path, level: u32, seq: u64, entries: &[SstEntry]) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(SstFile::filename(level, seq));
    let file = File::create(&path)?;
    serde_json::to_writer(file, &entries)?;
    Ok(path)
}

pub fn read_sst(path: &Path) -> Result<Vec<SstEntry>> {
    let file = File::open(path)?;
    let entries: Vec<SstEntry> = serde_json::from_reader(file)?;
    Ok(entries)
}

pub fn parse_filename(name: &str) -> Option<(u32, u64)> {
    if !name.starts_with("sst_L") || !name.ends_with(".json") {
        return None;
    }
    let trimmed = name.trim_start_matches("sst_L").trim_end_matches(".json");
    let mut parts = trimmed.split('_');
    let level = parts.next()?.parse::<u32>().ok()?;
    let seq = parts.next()?.parse::<u64>().ok()?;
    Some((level, seq))
}

pub fn max_seq(files: &[SstFile]) -> u64 {
    files.iter().map(|f| f.seq).max().unwrap_or(0)
}

pub fn compact_level_zero(
    files: &[SstFile],
    output_dir: &Path,
    next_seq: u64,
) -> Result<Option<SstFile>> {
    if files.is_empty() {
        return Ok(None);
    }

    let mut merged = std::collections::BTreeMap::<u64, SstEntry>::new();
    let mut sorted = files.to_vec();
    sorted.sort_by_key(|f| f.seq);

    for file in sorted.iter().rev() {
        let entries = read_sst(&file.path)?;
        for entry in entries {
            merged.entry(entry.row_id).or_insert(entry);
        }
    }

    let mut output_entries: Vec<SstEntry> = merged.into_values().collect();
    output_entries.sort_by_key(|entry| entry.row_id);

    let path = write_sst(output_dir, 1, next_seq, &output_entries)?;
    Ok(Some(SstFile {
        level: 1,
        seq: next_seq,
        path,
    }))
}

pub fn remove_files(files: &[SstFile]) -> Result<()> {
    for file in files {
        if file.path.exists() {
            fs::remove_file(&file.path)?;
        }
    }
    Ok(())
}

pub fn find_entry(path: &Path, row_id: u64) -> Result<Option<SstEntry>> {
    let entries = read_sst(path)?;
    if let Ok(idx) = entries.binary_search_by_key(&row_id, |entry| entry.row_id) {
        return Ok(Some(entries[idx].clone()));
    }
    Ok(None)
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    if !path.exists() {
        return Err(anyhow!("failed to create table dir"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{RowData, Value};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn find_entry_binary_search_roundtrip() {
        let dir = tempdir().unwrap();
        let table_dir = dir.path().join("table");
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("hello".to_string()));
        let row = RowData { id: 3, fields };
        let entries = vec![
            SstEntry {
                row_id: 1,
                row: Some(RowData {
                    id: 1,
                    fields: BTreeMap::new(),
                }),
            },
            SstEntry {
                row_id: 2,
                row: None,
            },
            SstEntry {
                row_id: 3,
                row: Some(row.clone()),
            },
        ];
        let path = write_sst(&table_dir, 0, 1, &entries).unwrap();

        let found = find_entry(&path, 3).unwrap().unwrap();
        let found_row = found.row.unwrap();
        assert_eq!(found_row.id, row.id);
        assert_eq!(
            found_row.fields.get("title"),
            Some(&Value::String("hello".to_string()))
        );
        assert!(find_entry(&path, 4).unwrap().is_none());
    }
}
