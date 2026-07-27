use crate::model::LiveResult;
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, SystemTime},
};

const JSON_READ_RETRIES: usize = 250;
const JSON_READ_RETRY_DELAY: Duration = Duration::from_millis(2);

#[derive(Debug, Clone)]
pub(crate) struct BridgeFiles {
    pub status: Value,
    pub summary: Value,
    pub current_state: Value,
    pub status_age: Option<Duration>,
    pub summary_age: Option<Duration>,
}

pub(crate) fn read_bridge_files(session_dir: &Path) -> LiveResult<BridgeFiles> {
    let mut files = BridgeFiles {
        status: read_json(&session_dir.join("status.json"))?,
        summary: read_json(&session_dir.join("summary.json"))?,
        current_state: read_json(&session_dir.join("current_state.json"))?,
        status_age: file_age(&session_dir.join("status.json")),
        summary_age: file_age(&session_dir.join("summary.json")),
    };
    discard_state_from_previous_bridge_process(&mut files);
    Ok(files)
}

pub(crate) fn file_age_ms(path: &Path) -> Option<u64> {
    file_age(path).map(|age| age.as_millis() as u64)
}

fn read_json(path: &Path) -> LiveResult<Value> {
    for attempt in 0..JSON_READ_RETRIES {
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(value) => return Ok(value),
                Err(_) if attempt + 1 < JSON_READ_RETRIES => {
                    thread::sleep(JSON_READ_RETRY_DELAY);
                }
                Err(err) => return Err(err.into()),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(json!({"missing": true}));
            }
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!("JSON read retry loop always returns")
}

fn discard_state_from_previous_bridge_process(files: &mut BridgeFiles) {
    let Some(status_pid) = files.status.get("client_pid").and_then(Value::as_u64) else {
        return;
    };
    if files
        .summary
        .get("client_pid")
        .and_then(Value::as_u64)
        .is_some_and(|pid| pid != status_pid)
    {
        files.summary = json!({"missing": true, "stale_for_client_pid": status_pid});
        files.summary_age = None;
    }
    if files
        .current_state
        .get("client_pid")
        .and_then(Value::as_u64)
        .is_some_and(|pid| pid != status_pid)
    {
        files.current_state = json!({"missing": true, "stale_for_client_pid": status_pid});
    }
}

fn file_age(path: &Path) -> Option<Duration> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}
