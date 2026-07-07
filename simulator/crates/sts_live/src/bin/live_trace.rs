use std::{env, net::TcpListener, path::PathBuf, process::exit, thread};

use sts_live::{
    bridge::BridgeManager,
    cli::run_cli,
    cli_output::{format_cli_error, format_cli_success},
    communication::{CommunicationBridgeConfig, CommunicationModBridgeManager},
    fidelity::TraceFidelityChecker,
    http::{serve_stream, LiveHttpApp},
    model::{BridgeId, BridgeStatus, LegalAction, LiveError, LiveResult, LiveState, RunConfig},
    FakeBridgeManager, SessionStore, SlayTheDataIndex,
};

fn main() {
    let trace_root = env::var("STS_LIVE_TRACE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("live_traces"));
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let bridge = runtime_bridge(&mut args);
    let slaythedata_index = runtime_slaythedata_index(&mut args);
    let mut store = SessionStore::new(bridge, TraceFidelityChecker, trace_root)
        .with_slaythedata_index(slaythedata_index);
    if let Err(err) = store.recover_existing_sessions() {
        eprintln!("{}", format_cli_error(&err));
        exit(1);
    }
    if args.first().map(String::as_str) == Some("serve") {
        let addr = serve_addr(&args);
        let listener = TcpListener::bind(&addr).unwrap_or_else(|err| {
            eprintln!("{}", format_cli_error(&LiveError::Io(err)));
            exit(1);
        });
        eprintln!("live-trace listening on http://{addr}");
        let app = LiveHttpApp::new(store);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let app = app.clone();
                    thread::spawn(move || {
                        if let Err(err) = serve_stream(stream, &app) {
                            eprintln!("request failed: {err}");
                        }
                    });
                }
                Err(err) => eprintln!("request failed: {err}"),
            }
        }
    }

    match run_cli(&mut store, args) {
        Ok(value) => println!("{}", format_cli_success(&value).expect("json output")),
        Err(err) => {
            eprintln!("{}", format_cli_error(&err));
            exit(1);
        }
    }
}

enum RuntimeBridge {
    Communication(CommunicationModBridgeManager),
    Fake(FakeBridgeManager),
}

impl BridgeManager for RuntimeBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        match self {
            Self::Communication(bridge) => bridge.list_bridges(),
            Self::Fake(bridge) => bridge.list_bridges(),
        }
    }

    fn start_run(&mut self, bridge_id: &BridgeId, config: &RunConfig) -> LiveResult<LiveState> {
        match self {
            Self::Communication(bridge) => bridge.start_run(bridge_id, config),
            Self::Fake(bridge) => bridge.start_run(bridge_id, config),
        }
    }

    fn abandon_run(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        match self {
            Self::Communication(bridge) => bridge.abandon_run(bridge_id),
            Self::Fake(bridge) => bridge.abandon_run(bridge_id),
        }
    }

    fn request_state(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        match self {
            Self::Communication(bridge) => bridge.request_state(bridge_id),
            Self::Fake(bridge) => bridge.request_state(bridge_id),
        }
    }

    fn send_action(&mut self, bridge_id: &BridgeId, action: &LegalAction) -> LiveResult<LiveState> {
        match self {
            Self::Communication(bridge) => bridge.send_action(bridge_id, action),
            Self::Fake(bridge) => bridge.send_action(bridge_id, action),
        }
    }

    fn kill_bridge(&mut self, bridge_id: &BridgeId) -> LiveResult<()> {
        match self {
            Self::Communication(bridge) => bridge.kill_bridge(bridge_id),
            Self::Fake(bridge) => bridge.kill_bridge(bridge_id),
        }
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        match self {
            Self::Communication(bridge) => bridge.kill_all(),
            Self::Fake(bridge) => bridge.kill_all(),
        }
    }
}

fn runtime_bridge(args: &mut Vec<String>) -> RuntimeBridge {
    if let Some(index) = args.iter().position(|arg| arg == "--fake") {
        args.remove(index);
        return RuntimeBridge::Fake(FakeBridgeManager::with_default_bridge());
    }
    let session_dir = env::var("STS_LIVE_BRIDGE_SESSION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| CommunicationModBridgeManager::default_session_dir(repo_root()));
    let mut config = CommunicationBridgeConfig::new(session_dir);
    config.allow_file_commands = env::var("STS_LIVE_ALLOW_FILE_COMMANDS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    RuntimeBridge::Communication(CommunicationModBridgeManager::new(config))
}

fn runtime_slaythedata_index(args: &mut Vec<String>) -> SlayTheDataIndex {
    if let Some(index) = args.iter().position(|arg| arg == "--slaythedata-db") {
        let _flag = args.remove(index);
        if index < args.len() {
            return SlayTheDataIndex::new(args.remove(index));
        }
    }
    SlayTheDataIndex::default_local()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn serve_addr(args: &[String]) -> String {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--addr" {
            return args
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:8799".to_owned());
        }
        index += 1;
    }
    "127.0.0.1:8799".to_owned()
}
