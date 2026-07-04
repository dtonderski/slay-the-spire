use crate::{
    bridge::BridgeManager,
    error_payload::error_payload,
    fidelity::FidelityChecker,
    model::{ActionId, BridgeId, LiveError, LiveResult, RunConfig, SessionId},
    session::SessionStore,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct LiveHttpApp<B, F> {
    store: Arc<Mutex<SessionStore<B, F>>>,
}

impl<B, F> Clone for LiveHttpApp<B, F> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

impl<B, F> LiveHttpApp<B, F>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    pub fn new(store: SessionStore<B, F>) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }

    pub fn handle(&self, method: &str, path: &str, body: &str) -> LiveResult<Value> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| LiveError::Blocked("session store lock poisoned".to_owned()))?;
        let parts = path
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();

        match (method, parts.as_slice()) {
            ("GET", ["bridges"]) => Ok(json!({"bridges": store.list_bridges()?})),
            ("POST", ["bridges", "kill-all"]) => Ok(json!({"killed": store.kill_all_bridges()?})),
            ("POST", ["bridges", bridge_id, "kill"]) => {
                store.kill_bridge(&BridgeId((*bridge_id).to_owned()))?;
                Ok(json!({"killed": 1, "bridge_id": bridge_id}))
            }
            ("GET", ["sessions"]) => Ok(json!({"sessions": store.list_sessions()})),
            ("POST", ["sessions", "start"]) => {
                let request: StartRequest = serde_json::from_str(body)?;
                Ok(serde_json::to_value(
                    store.start_run(BridgeId(request.bridge_id), request.config)?,
                )?)
            }
            ("GET", ["sessions", session_id]) => Ok(serde_json::to_value(
                store.refresh_fidelity(&SessionId((*session_id).to_owned()))?,
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
            ("GET", ["sessions", session_id, "fidelity"]) => {
                let snapshot = store.refresh_fidelity(&SessionId((*session_id).to_owned()))?;
                Ok(serde_json::to_value(snapshot.fidelity)?)
            }
            _ => Err(LiveError::NotFound(format!("{method} {path}"))),
        }
    }
}

#[derive(serde::Deserialize)]
struct StartRequest {
    bridge_id: String,
    config: RunConfig,
}

pub const UI_INDEX_HTML: &str = include_str!("../ui/index.html");
pub const UI_MAIN_TS: &str = include_str!("../ui/src/main.ts");
pub const UI_STYLES_CSS: &str = include_str!("../ui/src/styles.css");

pub fn serve_one<B, F>(listener: &TcpListener, app: &LiveHttpApp<B, F>) -> LiveResult<()>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let (stream, _) = listener.accept()?;
    serve_stream(stream, app)
}

pub fn serve_stream<B, F>(mut stream: TcpStream, app: &LiveHttpApp<B, F>) -> LiveResult<()>
where
    B: BridgeManager,
    F: FidelityChecker,
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
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::SystemTime,
    };

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
}
