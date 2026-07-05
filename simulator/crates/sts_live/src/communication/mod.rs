mod actions;
mod control;
mod files;
mod guard;
mod process;

use crate::{
    bridge::BridgeManager,
    model::{
        BridgeId, BridgeStatus, Character, LegalAction, LiveError, LiveResult, LiveState,
        RunConfig, RunSeed,
    },
};
pub(crate) use actions::live_state_from_files;
use control::{
    control_address, control_is_reachable, request_control_files,
    send_abandon_run as send_abandon_control, send_guarded_command as send_control_command,
    ControlAddress,
};
pub(crate) use files::BridgeFiles;
use files::{file_age_ms, read_bridge_files};
use guard::validate_ready_for_command;
use process::{kill_process, process_exists, process_is_nodejs};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DEFAULT_BRIDGE_ID: &str = "communication-mod";
const LOCAL_PROCESS_PREFIX: &str = "local-process-";
const CONTROL_REACHABILITY_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Debug, Clone)]
pub struct CommunicationBridgeConfig {
    pub session_dir: PathBuf,
    pub stale_after: Duration,
    pub allow_file_commands: bool,
    pub command_timeout: Duration,
    pub discover_local_processes: bool,
}

impl CommunicationBridgeConfig {
    pub fn new(session_dir: impl Into<PathBuf>) -> Self {
        Self {
            session_dir: session_dir.into(),
            stale_after: Duration::from_secs(120),
            allow_file_commands: false,
            command_timeout: Duration::from_secs(5),
            discover_local_processes: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommunicationModBridgeManager {
    config: CommunicationBridgeConfig,
}

impl CommunicationModBridgeManager {
    pub fn new(config: CommunicationBridgeConfig) -> Self {
        Self { config }
    }

    pub fn default_session_dir(repo_root: impl AsRef<Path>) -> PathBuf {
        repo_root
            .as_ref()
            .join("tools")
            .join("communication")
            .join("session")
    }

    fn read_files(&self) -> LiveResult<BridgeFiles> {
        read_bridge_files(&self.config.session_dir)
    }

    fn require_bridge(&self, bridge_id: &BridgeId) -> LiveResult<BridgeFiles> {
        if bridge_id.0 != DEFAULT_BRIDGE_ID {
            return Err(LiveError::NotFound(format!("bridge {}", bridge_id.0)));
        }
        let files = self.read_files()?;
        if files.status.get("missing").and_then(Value::as_bool) == Some(true) {
            return Err(LiveError::Bridge(format!(
                "CommunicationMod session files not found in {}",
                self.config.session_dir.display()
            )));
        }
        if files.summary.get("missing").and_then(Value::as_bool) == Some(true)
            && control_address(&files.status).is_none()
        {
            return Err(LiveError::Bridge(format!(
                "CommunicationMod summary file not found in {} and TCP control is unavailable",
                self.config.session_dir.display()
            )));
        }
        Ok(files)
    }

    fn send_command(
        &self,
        bridge_id: &BridgeId,
        command: &str,
        source_state_id: Option<&str>,
    ) -> LiveResult<LiveState> {
        let files = self.require_bridge(bridge_id)?;
        if let Some(control) = control_address(&files.status) {
            let effective_files = self.effective_command_files(&files, command);
            let control_files = match validate_ready_for_command(
                &effective_files,
                command,
                source_state_id,
                self.config.stale_after,
            ) {
                Ok(()) => effective_files,
                Err(_) => {
                    let control_files = request_control_files(
                        &control,
                        &files.status,
                        self.config.command_timeout,
                    )?;
                    validate_ready_for_command(
                        &control_files,
                        command,
                        source_state_id,
                        self.config.stale_after,
                    )?;
                    control_files
                }
            };
            return send_control_command(
                &control,
                command,
                &control_files,
                self.config.stale_after,
                self.config.command_timeout,
            );
        }
        let effective_files = self.effective_command_files(&files, command);
        validate_ready_for_command(
            &effective_files,
            command,
            source_state_id,
            self.config.stale_after,
        )?;
        if !self.config.allow_file_commands {
            return Err(LiveError::Bridge(
                "TCP bridge control is unavailable; set STS_LIVE_ALLOW_FILE_COMMANDS=1 for legacy file commands".to_owned(),
            ));
        }
        self.send_via_file(command, source_state_id)?;
        self.state_from_files()
    }

    fn send_abandon_run(&self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        let files = self.require_bridge(bridge_id)?;
        if let Some(control) = control_address(&files.status) {
            let control_files =
                request_control_files(&control, &files.status, self.config.command_timeout)?;
            guard::validate_ready_for_operator_control(&control_files, self.config.stale_after)?;
            let state = send_abandon_control(
                &control,
                &control_files,
                self.config.stale_after,
                self.config.command_timeout,
            )?;
            return self.wait_for_abandon_completion(&control, &files.status, state);
        }
        guard::validate_ready_for_operator_control(&files, self.config.stale_after)?;
        if !self.config.allow_file_commands {
            return Err(LiveError::Bridge(
                "TCP bridge control is unavailable; set STS_LIVE_ALLOW_FILE_COMMANDS=1 for legacy file commands".to_owned(),
            ));
        }
        self.send_via_file("ABANDON", None)?;
        self.state_from_files()
    }

    fn wait_for_abandon_completion(
        &self,
        control: &ControlAddress,
        fallback_status: &Value,
        initial: LiveState,
    ) -> LiveResult<LiveState> {
        let mut latest = initial;
        if abandon_complete(&latest) {
            return Ok(latest);
        }
        for _ in 0..12 {
            thread::sleep(Duration::from_millis(300));
            if let Ok(files) =
                request_control_files(control, fallback_status, self.config.command_timeout)
            {
                latest = live_state_from_files(&files);
                if abandon_complete(&latest) {
                    return Ok(latest);
                }
            }
        }
        Ok(latest)
    }

    fn send_via_file(&self, command: &str, source_state_id: Option<&str>) -> LiveResult<()> {
        fs::create_dir_all(&self.config.session_dir)?;
        fs::write(
            self.config.session_dir.join("next_command.json"),
            serde_json::to_vec(&json!({
                "command_id": format!("sts-live-file-{}-{}", std::process::id(), now_ms()),
                "command": command,
                "source_state_id": source_state_id,
                "submitted_at_ms": now_ms(),
                "protocol": "legacy-file",
                "metadata": {"source": "sts_live"},
            }))?,
        )?;
        fs::write(
            self.config.session_dir.join("next_command.txt"),
            format!("{command}\n"),
        )?;
        Ok(())
    }

    fn state_from_files(&self) -> LiveResult<LiveState> {
        Ok(live_state_from_files(&self.read_files()?))
    }

    fn menu_state_from_status(&self, files: &BridgeFiles) -> LiveState {
        let mut fallback = files.clone();
        fallback.summary = menu_summary_from_status(files);
        live_state_from_files(&fallback)
    }

    fn effective_command_files(&self, files: &BridgeFiles, command: &str) -> BridgeFiles {
        let verb = command
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if verb == "start"
            && files
                .summary_age
                .is_none_or(|age| age > self.config.stale_after)
            && files
                .status_age
                .is_some_and(|age| age <= self.config.stale_after)
            && files.status.get("status").and_then(Value::as_str) == Some("ready")
        {
            let mut fallback = files.clone();
            fallback.summary = menu_summary_from_status(files);
            fallback.summary_age = files.status_age;
            return fallback;
        }
        files.clone()
    }
}

impl BridgeManager for CommunicationModBridgeManager {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        let files = self.read_files()?;
        if files.status.get("missing").and_then(Value::as_bool) == Some(true)
            && files.summary.get("missing").and_then(Value::as_bool) == Some(true)
        {
            let trace_process_ids = self.trace_metadata_process_ids(&files);
            return Ok(self.observed_process_bridge_statuses(HashSet::new(), trace_process_ids));
        }
        let observed_process_ids = observed_bridge_process_ids(&files);
        let trace_process_ids = self.trace_metadata_process_ids(&files);
        let status_exited = files.status.get("status").and_then(Value::as_str) == Some("exited");
        let process_id = observed_process_ids.first().copied();
        let control = control_address(&files.status);
        let control_reachable = control
            .as_ref()
            .is_some_and(|control| control_is_reachable(control, CONTROL_REACHABILITY_TIMEOUT));

        let mut bridges = vec![BridgeStatus {
            id: BridgeId(DEFAULT_BRIDGE_ID.to_owned()),
            process_id,
            client_id: Some("CommunicationMod".to_owned()),
            connected: !status_exited && control_reachable,
            last_heartbeat_ms: file_age_ms(&self.config.session_dir.join("status.json")),
        }];
        let mut listed_process_ids = HashSet::new();
        if let Some(pid) = process_id {
            listed_process_ids.insert(pid);
        }
        bridges.extend(
            self.observed_process_bridge_statuses(listed_process_ids.clone(), trace_process_ids),
        );
        for bridge in &bridges {
            if let Some(pid) = bridge.process_id {
                listed_process_ids.insert(pid);
            }
        }
        for pid in observed_process_ids {
            if !listed_process_ids.insert(pid) || !process_exists(pid) {
                continue;
            }
            bridges.push(BridgeStatus {
                id: BridgeId(format!("{LOCAL_PROCESS_PREFIX}{pid}")),
                process_id: Some(pid),
                client_id: Some("observed CommunicationMod process".to_owned()),
                connected: false,
                last_heartbeat_ms: None,
            });
        }
        Ok(bridges)
    }

    fn start_run(&mut self, bridge_id: &BridgeId, config: &RunConfig) -> LiveResult<LiveState> {
        let character = match config.character {
            Character::Ironclad => "IRONCLAD",
        };
        let seed = match &config.seed {
            RunSeed::External(seed) => seed.clone(),
            RunSeed::Numeric(seed) => seed.to_string(),
        };
        let command = format!("START {character} {} {seed}", config.ascension);
        let files = self.require_bridge(bridge_id)?;
        if let Some(control) = control_address(&files.status) {
            return send_control_command(
                &control,
                &command,
                &files,
                self.config.stale_after,
                self.config.command_timeout,
            );
        }
        let effective_files = self.effective_command_files(&files, &command);
        validate_ready_for_command(&effective_files, &command, None, self.config.stale_after)?;
        if !self.config.allow_file_commands {
            return Err(LiveError::Bridge(
                "TCP bridge control is unavailable; set STS_LIVE_ALLOW_FILE_COMMANDS=1 for legacy file commands".to_owned(),
            ));
        }
        self.send_via_file(&command, None)?;
        self.state_from_files()
    }

    fn abandon_run(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.send_abandon_run(bridge_id)
    }

    fn request_state(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        let files = self.require_bridge(bridge_id)?;
        if let Some(control) = control_address(&files.status) {
            match request_control_files(&control, &files.status, self.config.command_timeout) {
                Ok(control_files) => Ok(live_state_from_files(&control_files)),
                Err(LiveError::Bridge(message))
                    if message.contains("no observed state is available") =>
                {
                    Ok(self.menu_state_from_status(&files))
                }
                Err(err) => Err(err),
            }
        } else if self.config.allow_file_commands {
            self.send_command(bridge_id, "STATE", None)
        } else {
            Ok(live_state_from_files(&files))
        }
    }

    fn send_action(&mut self, bridge_id: &BridgeId, action: &LegalAction) -> LiveResult<LiveState> {
        let command = action
            .command
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| LiveError::InvalidAction("action has no bridge command".to_owned()))?;
        let source_state_id = action
            .command
            .get("source_state_id")
            .and_then(Value::as_str);
        self.send_command(bridge_id, command, source_state_id)
    }

    fn kill_bridge(&mut self, bridge_id: &BridgeId) -> LiveResult<()> {
        if let Some(pid) = bridge_id
            .0
            .strip_prefix(LOCAL_PROCESS_PREFIX)
            .and_then(|pid| pid.parse::<u32>().ok())
        {
            return kill_process(pid);
        }
        let pid = self
            .list_bridges()?
            .into_iter()
            .find(|bridge| bridge.id == *bridge_id)
            .and_then(|bridge| bridge.process_id)
            .ok_or_else(|| LiveError::NotFound(format!("bridge {}", bridge_id.0)))?;
        kill_process(pid)
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        let bridges = self.list_bridges()?;
        let mut killed = 0;
        let mut last_error = None;
        for bridge in bridges {
            if bridge.process_id.is_some() {
                match self.kill_bridge(&bridge.id) {
                    Ok(()) => killed += 1,
                    Err(err) => last_error = Some(err),
                }
            }
        }
        if killed == 0 {
            if let Some(err) = last_error {
                return Err(err);
            }
        }
        Ok(killed)
    }
}

impl CommunicationModBridgeManager {
    fn observed_process_bridge_statuses(
        &self,
        mut listed_process_ids: HashSet<u32>,
        process_ids: Vec<u32>,
    ) -> Vec<BridgeStatus> {
        if !self.config.discover_local_processes {
            return Vec::new();
        }
        let mut bridges = Vec::new();
        for pid in process_ids {
            if !listed_process_ids.insert(pid) || !process_exists(pid) || !process_is_nodejs(pid) {
                continue;
            }
            bridges.push(BridgeStatus {
                id: BridgeId(format!("{LOCAL_PROCESS_PREFIX}{pid}")),
                process_id: Some(pid),
                client_id: Some("observed trace_client.js process".to_owned()),
                connected: false,
                last_heartbeat_ms: None,
            });
        }
        bridges
    }

    fn trace_metadata_process_ids(&self, files: &BridgeFiles) -> Vec<u32> {
        let mut process_ids = Vec::new();
        if let Some(path) = files
            .status
            .get("trace_path")
            .or_else(|| files.current_state.get("trace_path"))
            .and_then(Value::as_str)
        {
            push_trace_metadata_process_id(&mut process_ids, Path::new(path));
        }
        process_ids
    }
}

fn observed_bridge_process_ids(files: &BridgeFiles) -> Vec<u32> {
    let mut process_ids = Vec::new();
    for value in [&files.status, &files.summary, &files.current_state] {
        if let Some(pid) = bridge_process_id(value) {
            push_unique(&mut process_ids, pid);
        }
    }
    process_ids
}

fn bridge_process_id(value: &Value) -> Option<u32> {
    let pid = value.get("client_pid")?.as_u64()?;
    u32::try_from(pid).ok()
}

fn menu_summary_from_status(files: &BridgeFiles) -> Value {
    json!({
        "step": files.status.get("step").and_then(Value::as_u64).unwrap_or_default(),
        "state_seq": files.status.get("step").and_then(Value::as_u64).unwrap_or_default(),
        "available_commands": ["start", "state"],
        "ready_for_command": files.status.get("status").and_then(Value::as_str) == Some("ready"),
        "in_game": false,
        "screen_type": "MENU",
        "client_pid": files.status.get("client_pid"),
        "trace_path": files.status.get("trace_path"),
    })
}

fn abandon_complete(state: &LiveState) -> bool {
    matches!(state.phase, crate::model::LivePhase::Menu)
        || state
            .raw
            .pointer("/summary/in_game")
            .or_else(|| state.raw.pointer("/current_state/message/in_game"))
            .and_then(Value::as_bool)
            == Some(false)
        || commands_contain_start(state.raw.pointer("/summary/available_commands"))
        || commands_contain_start(
            state
                .raw
                .pointer("/current_state/message/available_commands"),
        )
}

fn commands_contain_start(commands: Option<&Value>) -> bool {
    match commands {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .any(|command| command.eq_ignore_ascii_case("start")),
        Some(Value::String(commands)) => commands
            .split_whitespace()
            .any(|command| command.eq_ignore_ascii_case("start")),
        _ => false,
    }
}

fn push_unique(process_ids: &mut Vec<u32>, pid: u32) {
    if !process_ids.contains(&pid) {
        process_ids.push(pid);
    }
}

fn push_trace_metadata_process_id(process_ids: &mut Vec<u32>, path: &Path) {
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    let mut first_line = String::new();
    if BufReader::new(file).read_line(&mut first_line).is_err() {
        return;
    }
    let Ok(metadata) = serde_json::from_str::<Value>(&first_line) else {
        return;
    };
    if metadata.get("client").and_then(Value::as_str) != Some("tools/communication/trace_client.js")
    {
        return;
    }
    if let Some(pid) = bridge_process_id(&metadata) {
        push_unique(process_ids, pid);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
