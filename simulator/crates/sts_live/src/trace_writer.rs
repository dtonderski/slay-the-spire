use crate::model::{LiveError, LiveResult, TraceRecord};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRecovery {
    pub path: PathBuf,
    pub records: usize,
}

#[derive(Debug)]
pub struct TraceWriter {
    path: PathBuf,
    file: File,
    next_sequence: u64,
}

impl TraceWriter {
    pub fn create_new(path: impl AsRef<Path>) -> LiveResult<Self> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            return Err(LiveError::TraceExists(path.display().to_string()));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            path,
            file,
            next_sequence: 0,
        })
    }

    pub fn recover_existing(path: impl AsRef<Path>) -> LiveResult<(Self, TraceRecovery)> {
        let path = path.as_ref().to_path_buf();
        let records = read_records(&path)?.len();
        let file = OpenOptions::new().append(true).open(&path)?;
        let writer = Self {
            path: path.clone(),
            file,
            next_sequence: records as u64,
        };
        Ok((writer, TraceRecovery { path, records }))
    }

    pub fn append(&mut self, record: &TraceRecord) -> LiveResult<u64> {
        let sequence = self.next_sequence;
        // Serializing directly into File turns every small serde fragment into
        // a separate write syscall. Live state records are large, and that is
        // especially expensive on an NTFS workspace mounted through WSL.
        // Build one complete JSONL record in memory and publish it with one
        // write while retaining the existing per-record visibility guarantee.
        let mut encoded = serde_json::to_vec(record)?;
        encoded.push(b'\n');
        self.file.write_all(&encoded)?;
        self.file.flush()?;
        self.next_sequence += 1;
        Ok(sequence)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn read_records(path: impl AsRef<Path>) -> LiveResult<Vec<TraceRecord>> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str::<TraceRecord>(&line)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BridgeId, SessionId, TraceRecord};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn trace_writer_refuses_overwrite_and_recovers() {
        let path = temp_trace_path("refuses-overwrite");
        let mut writer = TraceWriter::create_new(&path).unwrap();
        writer
            .append(&TraceRecord::Metadata {
                schema: 1,
                source: "test".to_owned(),
                session_id: SessionId("s".to_owned()),
                bridge_id: BridgeId("b".to_owned()),
                run_config: None,
            })
            .unwrap();

        let overwrite = TraceWriter::create_new(&path).unwrap_err();
        assert!(matches!(overwrite, LiveError::TraceExists(_)));

        let (_writer, recovery) = TraceWriter::recover_existing(&path).unwrap();
        assert_eq!(recovery.records, 1);
        fs::remove_file(path).ok();
    }

    fn temp_trace_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-{name}-{nonce}.jsonl"))
    }
}
