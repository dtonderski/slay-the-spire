use crate::{
    bridge::BridgeManager,
    error_payload::error_payload,
    fidelity::FidelityChecker,
    model::{
        ActionId, AutomationConfig, BridgeId, LiveError, LiveResult, RunConfig, SessionId,
        SlayTheDataSearchFilters,
    },
    session::SessionStore,
    session_recovery,
    slaythedata::SlayTheDataIndex,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const SLAYTHEDATA_AUTO_TICK_LIMIT: usize = 10_000;
const SLAYTHEDATA_AUTO_NO_PROGRESS_LIMIT: usize = 200;

pub struct LiveHttpApp<B, F> {
    store: Arc<Mutex<SessionStore<B, F>>>,
    slaythedata_index: SlayTheDataIndex,
}

impl<B, F> Clone for LiveHttpApp<B, F> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            slaythedata_index: self.slaythedata_index.clone(),
        }
    }
}

impl<B, F> LiveHttpApp<B, F>
where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    pub fn new(store: SessionStore<B, F>) -> Self {
        let slaythedata_index = store.slaythedata_index().clone();
        Self {
            store: Arc::new(Mutex::new(store)),
            slaythedata_index,
        }
    }

    pub fn recover_existing_sessions_background(&self) {
        let store = Arc::clone(&self.store);
        thread::spawn(move || {
            recover_existing_sessions_into_store(store);
        });
    }

    pub fn handle(&self, method: &str, path: &str, body: &str) -> LiveResult<Value> {
        let parts = path
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();

        if let ("POST", ["sessions", session_id, "automation", "auto-play"]) =
            (method, parts.as_slice())
        {
            return self.start_auto_play_job(SessionId((*session_id).to_owned()));
        }

        if let ("POST", ["sessions", session_id, "slaythedata", "auto-play"]) =
            (method, parts.as_slice())
        {
            return self.start_slaythedata_auto_play_job(SessionId((*session_id).to_owned()));
        }

        if let ("POST", ["sessions", session_id, "slaythedata", "skip-shop"]) =
            (method, parts.as_slice())
        {
            return self.skip_shop_and_resume_slaythedata(SessionId((*session_id).to_owned()));
        }

        if let ("GET", ["health"]) = (method, parts.as_slice()) {
            return Ok(json!({
                "ok": true,
                "service": "sts_live",
                "backend": "connected"
            }));
        }

        if let ("POST", ["slaythedata", "search"]) = (method, parts.as_slice()) {
            let payload: Value = serde_json::from_str(body)?;
            let include_corpus = payload
                .get("include_corpus")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let request: SlayTheDataSearchFilters = serde_json::from_value(payload)?;
            return Ok(json!({
                "runs": self.slaythedata_index.search_with_corpus(&request, include_corpus)?
            }));
        }

        let mut store = self
            .store
            .lock()
            .map_err(|_| LiveError::Blocked("session store lock poisoned".to_owned()))?;

        match (method, parts.as_slice()) {
            ("GET", ["bridges"]) => Ok(json!({"bridges": store.list_bridges()?})),
            ("GET", ["slaythedata", "runs", run_id, "json"]) => {
                let run_id = run_id
                    .parse::<i64>()
                    .map_err(|_| LiveError::InvalidAction(format!("invalid run id {run_id}")))?;
                store.slaythedata_run_json(run_id)
            }
            ("POST", ["slaythedata", "runs", run_id, "mark-broken"]) => {
                let run_id = run_id
                    .parse::<i64>()
                    .map_err(|_| LiveError::InvalidAction(format!("invalid run id {run_id}")))?;
                let request: MarkBrokenSlayTheDataRunRequest =
                    serde_json::from_str(body).unwrap_or_default();
                Ok(serde_json::to_value(store.mark_slaythedata_run_broken(
                    run_id,
                    request.reason.as_deref(),
                )?)?)
            }
            ("POST", ["slaythedata", "runs", run_id, "unmark-broken"]) => {
                let run_id = run_id
                    .parse::<i64>()
                    .map_err(|_| LiveError::InvalidAction(format!("invalid run id {run_id}")))?;
                Ok(json!({
                    "run_id": run_id,
                    "unmarked": store.unmark_slaythedata_run_broken(run_id)?
                }))
            }
            ("POST", ["bridges", "kill-all"]) => Ok(json!({"killed": store.kill_all_bridges()?})),
            ("POST", ["bridges", bridge_id, "kill"]) => {
                store.kill_bridge(&BridgeId((*bridge_id).to_owned()))?;
                Ok(json!({"killed": 1, "bridge_id": bridge_id}))
            }
            ("GET", ["sessions"]) => Ok(json!({
                "sessions": store.list_session_items()
            })),
            ("POST", ["sessions", session_id, "clear-other-traces"]) => Ok(json!({
                "deleted": store.clear_other_traces(&SessionId((*session_id).to_owned()))?,
                "sessions": store.list_session_items()
            })),
            ("POST", ["sessions", session_id, "add-to-permanent-corpus"]) => {
                let session_id = SessionId((*session_id).to_owned());
                let permanent_root = std::env::var_os("STS_PERMANENT_CORPUS_DIR")
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        LiveError::InvalidAction(
                            "STS_PERMANENT_CORPUS_DIR is required for trace promotion".to_owned(),
                        )
                    })?;
                let path =
                    store.copy_verified_trace_to_permanent_corpus(&session_id, &permanent_root)?;
                let run_id = store.attached_slaythedata_run_id(&session_id)?;
                if let Some(run_id) = run_id {
                    store.mark_slaythedata_run_in_corpus(run_id, &path)?;
                }
                Ok(json!({"path": path.display().to_string(), "run_id": run_id}))
            }
            ("POST", ["sessions", "start"]) => {
                let request: StartRequest = serde_json::from_str(body)?;
                Ok(serde_json::to_value(
                    store.start_run(BridgeId(request.bridge_id), request.config)?,
                )?)
            }
            ("GET", ["sessions", session_id]) => Ok(serde_json::to_value(
                store.session_snapshot(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "request-state"]) => Ok(serde_json::to_value(
                store.request_state(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "abandon"]) => Ok(serde_json::to_value(
                store.abandon_run(&SessionId((*session_id).to_owned()), "operator_http")?,
            )?),
            ("GET", ["sessions", session_id, "actions"]) => Ok(json!({
                "session_id": session_id,
                "legal_actions": store.actions(&SessionId((*session_id).to_owned()))?,
            })),
            ("POST", ["sessions", session_id, "actions", action_id]) => {
                Ok(serde_json::to_value(store.send_action(
                    &SessionId((*session_id).to_owned()),
                    &ActionId((*action_id).to_owned()),
                )?)?)
            }
            ("POST", ["sessions", session_id, "slaythedata", "attach"]) => {
                let request: SlayTheDataAttachRequest = serde_json::from_str(body)?;
                Ok(serde_json::to_value(store.attach_slaythedata_run(
                    &SessionId((*session_id).to_owned()),
                    request.run_id,
                )?)?)
            }
            ("POST", ["sessions", session_id, "slaythedata", "send-next"]) => {
                Ok(serde_json::to_value(store.slaythedata_send_next(
                    &SessionId((*session_id).to_owned()),
                )?)?)
            }
            ("POST", ["sessions", session_id, "slaythedata", "pause"]) => Ok(serde_json::to_value(
                store.slaythedata_pause(&SessionId((*session_id).to_owned()))?,
            )?),
            ("GET", ["sessions", session_id, "automation"]) => Ok(serde_json::to_value(
                store.automation_status(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "automation", "configure"]) => {
                let request: AutomationConfig = serde_json::from_str(body)?;
                Ok(serde_json::to_value(store.configure_automation(
                    &SessionId((*session_id).to_owned()),
                    request,
                )?)?)
            }
            ("POST", ["sessions", session_id, "automation", "plan"]) => Ok(serde_json::to_value(
                store.automation_plan(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "automation", "send-ready"]) => {
                Ok(serde_json::to_value(store.automation_send_ready(
                    &SessionId((*session_id).to_owned()),
                )?)?)
            }
            ("POST", ["sessions", session_id, "automation", "step"]) => Ok(serde_json::to_value(
                store.automation_step(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "automation", "run-one"]) => Ok(
                serde_json::to_value(store.automation_step(&SessionId((*session_id).to_owned()))?)?,
            ),
            ("POST", ["sessions", session_id, "automation", "pause"]) => Ok(serde_json::to_value(
                store.automation_pause(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "automation", "resume"]) => Ok(serde_json::to_value(
                store.automation_resume(&SessionId((*session_id).to_owned()))?,
            )?),
            ("POST", ["sessions", session_id, "automation", "cancel"]) => Ok(serde_json::to_value(
                store.automation_cancel(&SessionId((*session_id).to_owned()))?,
            )?),
            ("GET", ["sessions", session_id, "fidelity"]) => {
                let snapshot = store.refresh_fidelity(&SessionId((*session_id).to_owned()))?;
                Ok(serde_json::to_value(snapshot.fidelity)?)
            }
            _ => Err(LiveError::NotFound(format!("{method} {path}"))),
        }
    }

    fn start_auto_play_job(&self, session_id: SessionId) -> LiveResult<Value> {
        let (snapshot, limit, started) = {
            let mut store = self
                .store
                .lock()
                .map_err(|_| LiveError::Blocked("session store lock poisoned".to_owned()))?;
            store.automation_start_auto_play(&session_id)?
        };
        if started {
            let store = Arc::clone(&self.store);
            let worker_session_id = session_id.clone();
            thread::spawn(move || {
                run_auto_play_job(store, worker_session_id, limit);
            });
        }
        Ok(serde_json::to_value(snapshot)?)
    }

    fn start_slaythedata_auto_play_job(&self, session_id: SessionId) -> LiveResult<Value> {
        let snapshot = {
            let mut store = self
                .store
                .lock()
                .map_err(|_| LiveError::Blocked("session store lock poisoned".to_owned()))?;
            store.slaythedata_start_auto_play(&session_id)?
        };
        let store = Arc::clone(&self.store);
        thread::spawn(move || {
            run_slaythedata_auto_play_job(store, session_id, SLAYTHEDATA_AUTO_TICK_LIMIT)
        });
        Ok(serde_json::to_value(snapshot)?)
    }

    fn skip_shop_and_resume_slaythedata(&self, session_id: SessionId) -> LiveResult<Value> {
        let snapshot = {
            let mut store = self
                .store
                .lock()
                .map_err(|_| LiveError::Blocked("session store lock poisoned".to_owned()))?;
            let snapshot = store.slaythedata_skip_shop(&session_id)?;
            store.slaythedata_start_auto_play(&session_id)?;
            snapshot
        };
        let store = Arc::clone(&self.store);
        thread::spawn(move || {
            run_slaythedata_auto_play_job(store, session_id, SLAYTHEDATA_AUTO_TICK_LIMIT)
        });
        Ok(serde_json::to_value(snapshot)?)
    }
}

fn recover_existing_sessions_into_store<B, F>(store: Arc<Mutex<SessionStore<B, F>>>)
where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    let trace_root = {
        let Ok(store) = store.lock() else {
            eprintln!("session recovery failed: session store lock poisoned");
            return;
        };
        store.trace_root().to_path_buf()
    };
    let mut paths = match session_recovery::trace_paths(&trace_root) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("session recovery failed: {err}");
            return;
        }
    };
    paths.sort_by(|left, right| {
        session_path_number(right)
            .cmp(&session_path_number(left))
            .then_with(|| right.cmp(left))
    });

    let mut recovered = 0usize;
    for path in paths {
        let session = match session_recovery::recover_session(&path) {
            Ok(session) => session,
            Err(err) => {
                eprintln!("session recovery skipped {}: {err}", path.display());
                continue;
            }
        };
        let Ok(mut store) = store.lock() else {
            eprintln!("session recovery failed: session store lock poisoned");
            return;
        };
        store.insert_recovered_session(session);
        recovered += 1;
    }
    eprintln!("session recovery complete: {recovered} sessions");
}

fn session_path_number(path: &Path) -> u64 {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("session-"))
        .and_then(|number| number.parse().ok())
        .unwrap_or(0)
}

fn run_auto_play_job<B, F>(
    store: Arc<Mutex<SessionStore<B, F>>>,
    session_id: SessionId,
    limit: usize,
) where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    for actions_sent in 0..limit {
        let tick = {
            let mut store = match store.lock() {
                Ok(store) => store,
                Err(_) => return,
            };
            store.automation_auto_play_tick(&session_id, actions_sent)
        };
        match tick {
            Ok((_, true)) => thread::sleep(Duration::from_millis(50)),
            Ok((_, false)) => return,
            Err(err) => {
                if let Ok(mut store) = store.lock() {
                    let _ = store.automation_fail_auto_play(&session_id, &err.to_string());
                }
                return;
            }
        }
    }
    if let Ok(mut store) = store.lock() {
        let _ = store.automation_fail_auto_play(
            &session_id,
            "automation reached the configured auto-play action limit",
        );
    }
}

fn run_slaythedata_auto_play_job<B, F>(
    store: Arc<Mutex<SessionStore<B, F>>>,
    session_id: SessionId,
    limit: usize,
) where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    let mut last_progress = None;
    let mut no_progress_ticks = 0usize;
    for _ in 0..limit {
        let tick = {
            let mut store = match store.lock() {
                Ok(store) => store,
                Err(_) => return,
            };
            store.slaythedata_auto_play_tick(&session_id)
        };
        match tick {
            Ok((snapshot, true)) => {
                let progress = (
                    snapshot.slaythedata.next_step_index,
                    snapshot.automation.executed_actions.len(),
                    snapshot
                        .latest_state
                        .as_ref()
                        .map(|state| state.phase.clone()),
                );
                if last_progress.as_ref() == Some(&progress) {
                    no_progress_ticks += 1;
                } else {
                    last_progress = Some(progress);
                    no_progress_ticks = 0;
                }
                if no_progress_ticks >= SLAYTHEDATA_AUTO_NO_PROGRESS_LIMIT {
                    if let Ok(mut store) = store.lock() {
                        let _ = store.slaythedata_fail_auto_play(
                            &session_id,
                            "slaythedata_auto_no_progress",
                            "SlayTheData auto-play stopped after 200 consecutive ticks without guided, combat, or phase progress",
                        );
                    }
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Ok((_, false)) => return,
            Err(err) => {
                if let Ok(mut store) = store.lock() {
                    let _ = store.slaythedata_fail_auto_play(
                        &session_id,
                        "slaythedata_auto_play_failed",
                        &format!("SlayTheData auto-play failed: {err}"),
                    );
                }
                return;
            }
        }
    }
    if let Ok(mut store) = store.lock() {
        let _ = store.slaythedata_fail_auto_play(
            &session_id,
            "slaythedata_auto_action_limit",
            &format!(
                "SlayTheData auto-play stopped after reaching its {limit}-tick catastrophic safety limit"
            ),
        );
    }
}

#[derive(serde::Deserialize)]
struct StartRequest {
    bridge_id: String,
    config: RunConfig,
}

#[derive(serde::Deserialize)]
struct SlayTheDataAttachRequest {
    run_id: i64,
}

#[derive(Default, serde::Deserialize)]
struct MarkBrokenSlayTheDataRunRequest {
    reason: Option<String>,
}

pub const UI_INDEX_HTML: &str = include_str!("../ui/index.html");
pub const UI_MAIN_TS: &str = include_str!("../ui/src/main.ts");
pub const UI_STYLES_CSS: &str = include_str!("../ui/src/styles.css");

pub fn serve_one<B, F>(listener: &TcpListener, app: &LiveHttpApp<B, F>) -> LiveResult<()>
where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    let (stream, _) = listener.accept()?;
    serve_stream(stream, app)
}

pub fn serve_stream<B, F>(mut stream: TcpStream, app: &LiveHttpApp<B, F>) -> LiveResult<()>
where
    B: BridgeManager + Send + 'static,
    F: FidelityChecker + Send + 'static,
{
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let request = read_request(&mut stream)?;
    let response = if request.method == "GET" && is_ui_path(&request.path) {
        ui_response(&request.path)
    } else {
        match app.handle(&request.method, &request.path, &request.body) {
            Ok(value) => http_response(200, "application/json", &value.to_string()),
            Err(err) => http_response(400, "application/json", &error_payload(&err).to_string()),
        }
    };
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn is_ui_path(path: &str) -> bool {
    path == "/" || path == "/index.html" || path.starts_with("/assets/")
}

fn ui_response(path: &str) -> String {
    match read_ui_file(path) {
        Ok((content_type, body)) => http_response(200, content_type, &body),
        Err(message) => http_response(
            503,
            "text/html; charset=utf-8",
            &format!(
                "<!doctype html><title>Live Trace Collector</title><pre>{}</pre>",
                escape_html(&message)
            ),
        ),
    }
}

fn read_ui_file(path: &str) -> Result<(&'static str, String), String> {
    let relative = ui_relative_path(path)?;
    let file = ui_dist_root().join(relative);
    let bytes = fs::read(&file).map_err(|err| {
        format!(
            "UI build output is missing: {}\nRun `npm install` and `npm run build` in simulator/crates/sts_live/ui, or use `npm run dev` for hot reload.",
            err
        )
    })?;
    let body = String::from_utf8(bytes).map_err(|_| {
        format!(
            "UI file is not valid UTF-8 and cannot be served by the development HTTP server: {}",
            file.display()
        )
    })?;
    Ok((content_type(&file), body))
}

fn ui_relative_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim_start_matches('/');
    let relative = if trimmed.is_empty() {
        PathBuf::from("index.html")
    } else {
        let path = Path::new(trimmed);
        if path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err("invalid UI asset path".to_owned());
        }
        path.to_path_buf()
    };
    Ok(relative)
}

fn ui_dist_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ui")
        .join("dist")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        _ => "text/plain; charset=utf-8",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_request(stream: &mut TcpStream) -> LiveResult<HttpRequest> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or(bytes.len());
    let head = String::from_utf8_lossy(&bytes[..header_end]).to_string();
    let content_length = content_length(&head);
    while bytes.len() < header_end + content_length {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = String::from_utf8_lossy(&bytes[header_end..]).to_string();
    let first = head.lines().next().unwrap_or_default();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or("/").to_owned();
    Ok(HttpRequest { method, path, body })
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn http_response(status: u16, content_type: &str, body: &str) -> String {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bridge::FakeBridgeManager, fidelity::TraceFidelityChecker};
    use rusqlite::Connection;
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::SystemTime,
    };

    #[test]
    fn http_health_reports_backend_available() {
        let root = temp_dir("http-health");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));

        let response = app.handle("GET", "/health", "").unwrap();

        assert_eq!(response["ok"], true);
        assert_eq!(response["service"], "sts_live");
        assert_eq!(response["backend"], "connected");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_health_does_not_wait_for_session_store_lock() {
        let root = temp_dir("http-health-no-lock");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let _guard = app.store.lock().unwrap();

        let response = app.handle("GET", "/health", "").unwrap();

        assert_eq!(response["backend"], "connected");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_slaythedata_search_does_not_wait_for_session_store_lock() {
        let root = temp_dir("http-slaythedata-search-no-lock");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let app = LiveHttpApp::new(
            SessionStore::new(
                FakeBridgeManager::with_default_bridge(),
                TraceFidelityChecker,
                &root,
            )
            .with_slaythedata_index(crate::SlayTheDataIndex::new(&db)),
        );
        let _guard = app.store.lock().unwrap();

        let response = app
            .handle(
                "POST",
                "/slaythedata/search",
                r#"{"character":"IRONCLAD","ascension":0,"min_floor_reached":20,"victory":false,"limit":10,"require_supported":true}"#,
            )
            .unwrap();

        assert_eq!(response["runs"].as_array().unwrap().len(), 1);
        assert_eq!(response["runs"][0]["id"], 21);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_starts_session_against_fake_bridge() {
        let root = temp_dir("http-start");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let response = app
            .handle(
                "POST",
                "/sessions/start",
                r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX04"}}}"#,
            )
            .unwrap();
        assert_eq!(response["session_id"], "session-1");

        let sessions = app.handle("GET", "/sessions", "").unwrap();
        assert_eq!(sessions["sessions"][0]["session_id"], "session-1");
        assert_eq!(sessions["sessions"][0]["lifecycle"], "recording");
        assert!(sessions["sessions"][0].get("latest_state").is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_abandon_then_start_creates_next_trace() {
        let root = temp_dir("http-abandon-start");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let first = app
            .handle(
                "POST",
                "/sessions/start",
                r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX04"}}}"#,
            )
            .unwrap();
        assert_eq!(first["session_id"], "session-1");

        let second = app
            .handle("POST", "/sessions/session-1/abandon", "{}")
            .unwrap();

        assert_eq!(second["session_id"], "session-1");
        assert_eq!(second["lifecycle"], "ended");
        let first_trace_path = second["trace_path"].as_str().unwrap().to_owned();

        let started = app
            .handle(
                "POST",
                "/sessions/start",
                r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX05"}}}"#,
            )
            .unwrap();

        assert_eq!(started["session_id"], "session-2");
        assert!(started["run_config"].is_object());
        let second_trace_path = started["trace_path"].as_str().unwrap().to_owned();
        assert_ne!(first_trace_path, second_trace_path);
        let first_trace = fs::read_to_string(first_trace_path).unwrap();
        assert_eq!(first_trace.matches("START IRONCLAD").count(), 1);
        assert!(first_trace.contains("\"command\":\"abandon_run\""));
        let second_trace = fs::read_to_string(second_trace_path).unwrap();
        assert_eq!(second_trace.matches("START IRONCLAD").count(), 1);
        assert!(second_trace.contains("START IRONCLAD 0 CODEX05"));
        let first_after = app.handle("GET", "/sessions/session-1", "").unwrap();
        assert_eq!(first_after["lifecycle"], "ended");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_clears_traces_except_current_session() {
        let root = temp_dir("http-clear-traces");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let first = app
            .handle(
                "POST",
                "/sessions/start",
                r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX04"}}}"#,
            )
            .unwrap();
        let second = app
            .handle(
                "POST",
                "/sessions/start",
                r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX05"}}}"#,
            )
            .unwrap();
        let first_trace = PathBuf::from(first["trace_path"].as_str().unwrap());
        let second_trace = PathBuf::from(second["trace_path"].as_str().unwrap());
        assert!(first_trace.exists());
        assert!(second_trace.exists());

        let response = app
            .handle("POST", "/sessions/session-2/clear-other-traces", "{}")
            .unwrap();

        assert_eq!(response["deleted"], 1);
        assert!(!first_trace.exists());
        assert!(second_trace.exists());
        let sessions = response["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["session_id"], "session-2");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_session_get_returns_cached_snapshot_without_fidelity_replay() {
        let root = temp_dir("http-session-get-snapshot");
        let checks = Arc::new(AtomicUsize::new(0));
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            CountingFidelity {
                checks: Arc::clone(&checks),
            },
            &root,
        ));
        app.handle(
            "POST",
            "/sessions/start",
            r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX04"}}}"#,
        )
        .unwrap();
        assert_eq!(checks.load(Ordering::SeqCst), 1);

        app.handle("GET", "/sessions/session-1", "").unwrap();
        assert_eq!(checks.load(Ordering::SeqCst), 1);

        app.handle("GET", "/sessions/session-1/fidelity", "")
            .unwrap();
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_manual_action_returns_live_state_after_fidelity_refresh() {
        let root = temp_dir("http-action-cached-fidelity");
        let checks = Arc::new(AtomicUsize::new(0));
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            CountingFidelity {
                checks: Arc::clone(&checks),
            },
            &root,
        ));
        app.handle(
            "POST",
            "/sessions/start",
            r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"external":"CODEX04"}}}"#,
        )
        .unwrap();
        assert_eq!(checks.load(Ordering::SeqCst), 1);

        let response = app
            .handle("POST", "/sessions/session-1/actions/talk", "{}")
            .unwrap();
        assert_eq!(response["latest_state"]["phase"], "combat");
        assert_eq!(checks.load(Ordering::SeqCst), 2);

        app.handle("GET", "/sessions/session-1/fidelity", "")
            .unwrap();
        assert_eq!(checks.load(Ordering::SeqCst), 2);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_searches_slaythedata_index() {
        let root = temp_dir("http-slaythedata-search");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let app = LiveHttpApp::new(
            SessionStore::new(
                FakeBridgeManager::with_default_bridge(),
                TraceFidelityChecker,
                &root,
            )
            .with_slaythedata_index(crate::SlayTheDataIndex::new(&db)),
        );

        let response = app
            .handle(
                "POST",
                "/slaythedata/search",
                r#"{"character":"IRONCLAD","ascension":0,"min_floor_reached":20,"victory":false,"limit":10,"require_supported":true}"#,
            )
            .unwrap();

        assert_eq!(response["runs"].as_array().unwrap().len(), 1);
        assert_eq!(response["runs"][0]["id"], 21);
        assert_eq!(response["runs"][0]["victory"], false);
        assert_eq!(response["runs"][0]["materialized"], true);
        assert_eq!(response["runs"][0]["build_version"], "2022-12-18");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_marks_slaythedata_run_broken() {
        let root = temp_dir("http-slaythedata-mark-broken");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let app = LiveHttpApp::new(
            SessionStore::new(
                FakeBridgeManager::with_default_bridge(),
                TraceFidelityChecker,
                &root,
            )
            .with_slaythedata_index(crate::SlayTheDataIndex::new(&db)),
        );

        let response = app
            .handle(
                "POST",
                "/slaythedata/runs/21/mark-broken",
                r#"{"reason":"bad replay"}"#,
            )
            .unwrap();

        assert_eq!(response["run_id"], 21);
        assert_eq!(response["seed_played"], "LOSS");
        assert_eq!(response["reason"], "bad replay");
        let search = app
            .handle(
                "POST",
                "/slaythedata/search",
                r#"{"character":"IRONCLAD","ascension":0,"min_floor_reached":1,"victory":false,"limit":10,"require_supported":true}"#,
            )
            .unwrap();
        assert_eq!(search["runs"].as_array().unwrap().len(), 0);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_api_downloads_slaythedata_json() {
        let root = temp_dir("http-slaythedata-json");
        fs::create_dir_all(&root).unwrap();
        let db = root.join("slaythedata.sqlite3");
        write_slaythedata_locator_db(&db);
        let app = LiveHttpApp::new(
            SessionStore::new(
                FakeBridgeManager::with_default_bridge(),
                TraceFidelityChecker,
                &root,
            )
            .with_slaythedata_index(crate::SlayTheDataIndex::new(&db)),
        );

        let response = app.handle("GET", "/slaythedata/runs/21/json", "").unwrap();

        assert_eq!(response, json!({"build_version": "2022-12-18"}));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_e2e_serves_one_request() {
        let root = temp_dir("http-e2e");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || serve_one(&listener, &app).unwrap());

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"GET /bridges HTTP/1.1\r\nHost: local\r\n\r\n")
            .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();

        assert!(response.contains("200 OK"));
        assert!(response.contains("fake-bridge-1"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn http_e2e_returns_structured_errors() {
        let root = temp_dir("http-error");
        let app = LiveHttpApp::new(SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        ));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || serve_one(&listener, &app).unwrap());

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"GET /sessions/missing HTTP/1.1\r\nHost: local\r\n\r\n")
            .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();

        assert!(response.contains("400 Bad Request"));
        assert!(response.contains(r#""kind":"not_found""#));
        assert!(response.contains(r#""message":"not found: session missing""#));
        fs::remove_dir_all(root).ok();
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-http-{name}-{nonce}"))
    }

    fn write_slaythedata_locator_db(path: &std::path::Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                character_chosen TEXT,
                ascension_level INTEGER,
                floor_reached INTEGER,
                is_daily INTEGER,
                is_endless INTEGER,
                is_trial INTEGER,
                unsupported_any INTEGER,
                seed_played TEXT,
                build_version TEXT,
                victory INTEGER,
                path_length INTEGER,
                card_choice_count INTEGER,
                event_choice_count INTEGER,
                shop_purchase_count INTEGER,
                potion_usage_count INTEGER,
                neow_bonus TEXT,
                neow_cost TEXT
            );
            CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
            CREATE TABLE run_materialized_json (
                run_id INTEGER PRIMARY KEY,
                raw_event_json TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (21, 'IRONCLAD', 0, 35, 0, 0, 0, 0, 'LOSS', '2020-07-30', 0, 35, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs VALUES (22, 'IRONCLAD', 0, 35, 0, 0, 0, 0, 'WIN', '2020-07-30', 1, 35, 5, 1, 1, 0, 'THREE_CARDS', 'NONE')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (21)", [])
            .unwrap();
        conn.execute("INSERT INTO chunk_runs VALUES (22)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO run_materialized_json VALUES (21, '{\"build_version\":\"2022-12-18\"}')",
            [],
        )
        .unwrap();
    }

    struct CountingFidelity {
        checks: Arc<AtomicUsize>,
    }

    impl FidelityChecker for CountingFidelity {
        fn check_trace(&self, _path: &Path) -> LiveResult<crate::model::FidelityStatus> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(crate::model::FidelityStatus::unknown())
        }
    }
}
