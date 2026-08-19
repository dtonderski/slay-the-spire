use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, io};
use sts_verify::{SlayTheDataReplayStepKind, TraceProfile};

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

impl RunSeed {
    #[must_use]
    pub fn command_text(&self) -> String {
        match self {
            Self::External(seed) => seed.clone(),
            Self::Numeric(seed) => sts_verify::sts_seed_long_to_string(*seed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunConfig {
    pub character: Character,
    pub ascension: u8,
    pub seed: RunSeed,
    /// Persistent profile input captured before START when supported, with a
    /// first-state fallback for legacy bridges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<TraceProfile>,
}

#[cfg(test)]
mod tests {
    use super::RunSeed;

    #[test]
    fn numeric_run_seed_command_text_uses_game_seed_alphabet() {
        assert_eq!(RunSeed::Numeric(123).command_text(), "3I");
        let signed = -1_271_861_678_227_830_524;
        let encoded = RunSeed::Numeric(signed).command_text();
        assert!(!encoded.contains('-'));
        assert_eq!(sts_verify::sts_seed_string_to_long(&encoded), signed);
    }

    #[test]
    fn external_run_seed_command_text_is_preserved() {
        assert_eq!(
            RunSeed::External("CODEX04".to_owned()).command_text(),
            "CODEX04"
        );
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationPolicy {
    FakePlayFirstCard,
    GreedySearch,
    BeamSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationState {
    Idle,
    AutoPlaying,
    Planning,
    WaitingForFidelity,
    ReadyToSend,
    SendingAction,
    WaitingForLiveState,
    VerifyingTransition,
    Paused,
    Blocked,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationConfig {
    pub policy: AutomationPolicy,
    #[serde(default = "default_automation_depth")]
    pub depth: usize,
    #[serde(default = "default_automation_width")]
    pub width: usize,
    #[serde(default)]
    pub allowed_potion_slots: Vec<usize>,
    #[serde(default = "default_automation_auto_action_limit")]
    pub auto_action_limit: usize,
    #[serde(default = "default_automation_search_transition_budget")]
    pub search_transition_budget: usize,
    #[serde(default = "default_automation_search_time_budget_ms")]
    pub search_time_budget_ms: u64,
    #[serde(default)]
    pub deduplicate_search_states: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationPlannedAction {
    pub action_id: ActionId,
    pub kind: LegalActionKind,
    pub label: String,
    pub source_sequence: u64,
    pub command: Option<String>,
    pub planner_action: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AutomationPlanSnapshot {
    pub actions: Vec<AutomationPlannedAction>,
    #[serde(default)]
    pub played_actions: usize,
    pub predicted_final_hp: Option<i32>,
    pub predicted_monster_hp: Option<i32>,
    pub value: Option<f64>,
    pub nodes: usize,
    pub terminal_reason: Option<String>,
    #[serde(default)]
    pub search_elapsed_ms: u64,
    #[serde(default)]
    pub budget_exhausted: bool,
    #[serde(default)]
    pub timed_out: bool,
    #[serde(default)]
    pub duplicate_checks: usize,
    #[serde(default)]
    pub duplicates: usize,
    #[serde(default)]
    pub cache_hits: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationJobSnapshot {
    pub state: AutomationState,
    pub policy: AutomationPolicy,
    pub config: AutomationConfig,
    pub planned_action: Option<AutomationPlannedAction>,
    pub plan: Option<AutomationPlanSnapshot>,
    #[serde(default)]
    pub executed_actions: Vec<AutomationPlannedAction>,
    pub blocked: Option<BlockedState>,
    pub last_message: Option<String>,
}

impl Default for AutomationJobSnapshot {
    fn default() -> Self {
        let config = AutomationConfig::default();
        Self {
            state: AutomationState::Idle,
            policy: config.policy.clone(),
            config,
            planned_action: None,
            plan: None,
            executed_actions: Vec::new(),
            blocked: None,
            last_message: None,
        }
    }
}

fn default_automation_depth() -> usize {
    100
}

fn default_automation_width() -> usize {
    300
}

fn default_automation_auto_action_limit() -> usize {
    80
}

fn default_automation_search_transition_budget() -> usize {
    100_000
}

fn default_automation_search_time_budget_ms() -> u64 {
    30_000
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            policy: AutomationPolicy::BeamSearch,
            depth: default_automation_depth(),
            width: default_automation_width(),
            allowed_potion_slots: (0..5).collect(),
            auto_action_limit: default_automation_auto_action_limit(),
            search_transition_budget: default_automation_search_transition_budget(),
            search_time_budget_ms: default_automation_search_time_budget_ms(),
            deduplicate_search_states: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataRunOutcome {
    Win,
    Loss,
    Abandon,
}

impl SlayTheDataRunOutcome {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Loss => "loss",
            Self::Abandon => "abandon",
        }
    }

    #[must_use]
    pub fn from_victory(victory: bool) -> Self {
        if victory {
            Self::Win
        } else {
            Self::Loss
        }
    }
}

impl TryFrom<String> for SlayTheDataRunOutcome {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "win" => Ok(Self::Win),
            "loss" => Ok(Self::Loss),
            "abandon" => Ok(Self::Abandon),
            other => Err(format!("unknown SlayTheData run outcome {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlayTheDataSearchFilters {
    #[serde(default = "default_slaythedata_character")]
    pub character: String,
    #[serde(default)]
    pub ascension: Option<u8>,
    #[serde(default = "default_slaythedata_min_floor")]
    pub min_floor_reached: u32,
    #[serde(default)]
    pub max_floor_reached: Option<u32>,
    #[serde(default)]
    pub victory: Option<bool>,
    #[serde(default)]
    pub run_outcome: Option<SlayTheDataRunOutcome>,
    #[serde(default)]
    pub neow_bonus: Option<String>,
    #[serde(default)]
    pub seed_played: Option<String>,
    #[serde(default)]
    pub run_id: Option<i64>,
    #[serde(default = "default_slaythedata_limit")]
    pub limit: usize,
    #[serde(default = "default_true")]
    pub require_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlayTheDataRunSummary {
    pub id: i64,
    pub seed_played: Option<String>,
    pub build_version: Option<String>,
    pub ascension_level: Option<u8>,
    pub floor_reached: Option<u32>,
    pub victory: bool,
    pub run_outcome: SlayTheDataRunOutcome,
    pub path_length: Option<u32>,
    pub card_choice_count: Option<u32>,
    pub event_choice_count: Option<u32>,
    pub shop_purchase_count: Option<u32>,
    pub potion_usage_count: Option<u32>,
    pub neow_bonus: Option<String>,
    pub neow_cost: Option<String>,
    pub guided_score: i64,
    pub materialized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrokenSlayTheDataRun {
    pub run_id: i64,
    pub seed_played: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlayTheDataAdvisorStep {
    pub floor: u32,
    pub ordinal: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<SlayTheDataReplayStepKind>,
    pub status: String,
    pub code: String,
    pub message: String,
    pub command: Option<String>,
    pub action_id: Option<ActionId>,
    pub action_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SlayTheDataSessionSnapshot {
    pub attached_run: Option<SlayTheDataRunSummary>,
    pub advisor: Option<SlayTheDataAdvisorStep>,
    pub next_step_index: usize,
    pub blocked: Option<BlockedState>,
    pub last_message: Option<String>,
    #[serde(default)]
    pub auto_play_paused: bool,
}

fn default_slaythedata_character() -> String {
    "IRONCLAD".to_owned()
}

fn default_slaythedata_min_floor() -> u32 {
    1
}

fn default_slaythedata_limit() -> usize {
    50
}

fn default_true() -> bool {
    true
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionListItem {
    pub session_id: SessionId,
    pub lifecycle: SessionLifecycle,
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
    pub automation: AutomationJobSnapshot,
    pub slaythedata: SlayTheDataSessionSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataCollectionBlockerKind {
    SimulatorFidelityBreak,
    SlaythedataMappingGap,
    SlaythedataIllegalLog,
    SlaythedataIncompatibleRun,
    RunEndedBeforeTarget,
    BridgeOrBackendError,
    CompletedTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataGuidedDivergenceKind {
    RecordedCardRewardUnavailable,
    CompletedGuidancePastLiveFloor,
    RecordedShopPurgeTargetUnavailable,
    RecordedShopPurchaseSkipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataGuidedDivergence {
    pub kind: SlayTheDataGuidedDivergenceKind,
    pub step_index: usize,
    pub floor: u32,
    pub intent: SlayTheDataReplayStepKind,
    pub source_build_version: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlayTheDataRepairPacket {
    pub blocker_kind: SlayTheDataCollectionBlockerKind,
    pub run_id: i64,
    pub seed: Option<String>,
    pub session_id: Option<SessionId>,
    pub trace_path: Option<String>,
    pub current_live_state_summary: Option<Value>,
    pub slaythedata_step: Option<SlayTheDataAdvisorStep>,
    pub legal_live_actions: Vec<LegalAction>,
    pub first_simulator_diff_or_mapping_failure: Option<String>,
    pub reproduce_command: Option<String>,
    pub illegal_run_constant_entry: Option<String>,
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
    Automation {
        sequence: u64,
        event: String,
        details: Value,
    },
    SlayTheData {
        sequence: u64,
        event: String,
        details: Value,
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
