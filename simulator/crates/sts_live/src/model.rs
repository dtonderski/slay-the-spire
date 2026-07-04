use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, io};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BridgeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    Ironclad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunSeed {
    External(String),
    Numeric(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunConfig {
    pub character: Character,
    pub ascension: u8,
    pub seed: RunSeed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub id: BridgeId,
    pub process_id: Option<u32>,
    pub client_id: Option<String>,
    pub connected: bool,
    pub last_heartbeat_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LivePhase {
    Unknown,
    Menu,
    Neow,
    Map,
    Combat,
    Reward,
    Event,
    Shop,
    Rest,
    GameOver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalActionKind {
    StartRun,
    ChooseNeow,
    ChooseMapNode,
    PlayCard,
    UsePotion,
    DiscardPotion,
    EndTurn,
    ChooseReward,
    SkipReward,
    OpenChest,
    ShopBuy,
    ShopRemove,
    RestSite,
    EventChoice,
    Confirm,
    RequestState,
    AbandonRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegalAction {
    pub id: ActionId,
    pub kind: LegalActionKind,
    pub label: String,
    pub enabled: bool,
    pub command: Value,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveState {
    pub sequence: u64,
    pub phase: LivePhase,
    pub legal_actions: Vec<LegalAction>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityKind {
    Unknown,
    Ok,
    Lost,
    UnverifiedStart,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityStatus {
    pub kind: FidelityKind,
    pub first_divergent_step: Option<u64>,
    pub compact_diff: Vec<String>,
    pub message: Option<String>,
}

impl FidelityStatus {
    pub fn unknown() -> Self {
        Self {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycle {
    NotAttached,
    Attached,
    Recording,
    FidelityOk,
    FidelityLost,
    Blocked,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedState {
    pub reason_code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: SessionId,
    pub bridge_id: BridgeId,
    pub lifecycle: SessionLifecycle,
    pub trace_path: String,
    pub run_config: Option<RunConfig>,
    pub latest_state: Option<LiveState>,
    pub fidelity: FidelityStatus,
    pub blocked: Option<BlockedState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceRecord {
    Metadata {
        schema: u32,
        source: String,
        session_id: SessionId,
        bridge_id: BridgeId,
        run_config: Option<RunConfig>,
    },
    State {
        sequence: u64,
        state: LiveState,
    },
    Action {
        sequence: u64,
        action: LegalAction,
    },
    Response {
        sequence: u64,
        response: Value,
    },
    Error {
        sequence: u64,
        reason_code: String,
        message: String,
    },
    RunAbandoned {
        sequence: u64,
        reason: String,
    },
}

#[derive(Debug)]
pub enum LiveError {
    Bridge(String),
    InvalidAction(String),
    Io(io::Error),
    Json(serde_json::Error),
    NotFound(String),
    TraceExists(String),
    Blocked(String),
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(message) => write!(f, "bridge error: {message}"),
            Self::InvalidAction(message) => write!(f, "invalid action: {message}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Json(err) => write!(f, "json error: {err}"),
            Self::NotFound(message) => write!(f, "not found: {message}"),
            Self::TraceExists(path) => write!(f, "trace already exists: {path}"),
            Self::Blocked(message) => write!(f, "blocked: {message}"),
        }
    }
}

impl std::error::Error for LiveError {}

impl From<io::Error> for LiveError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for LiveError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type LiveResult<T> = Result<T, LiveError>;
