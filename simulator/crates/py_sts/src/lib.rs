use pyo3::create_exception;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sts_core::combat::ExhaustSelectPurpose;
use sts_core::potion::IRONCLAD_POTION_POOL;
use sts_core::{
    apply_combat_action_with_events, apply_run_decision_action, fair_combat_observation,
    fair_run_observation, legal_combat_actions, legal_run_decision_actions, player_choices,
    potion_key, resolve_player_choice, restore_combat_snapshot_json, restore_run_snapshot_json,
    CardDefinition, CardId, CardKeywords, CardType, CardValues, CombatAction, CombatPhase,
    CombatState, DecisionRevision, EventAction, FairCombatObservation, FairObservationError,
    MapAction, MonsterId, MonsterIntent, PlayerChoice, PlayerChoiceError, PlayerChoiceRequest,
    Potion, Relic, RestAction, RunAction, RunDecisionAction, RunPhase, RunState, Snapshot,
    TargetRequirement, ALL_RELICS, SNAPSHOT_SCHEMA_VERSION,
};

const AGENT_REWARD_GOLD_PER_HP: f64 = 10.0;
const AGENT_REWARD_HP_PER_POTION: f64 = 8.0;

create_exception!(_native, NoActiveCombatError, PyValueError);
create_exception!(_native, InvalidAuthoritativeStateError, PyValueError);
create_exception!(_native, UnknownPublicContentError, PyValueError);
create_exception!(_native, NotInCombatError, PyValueError);
create_exception!(_native, DecisionUnavailableError, PyValueError);
create_exception!(_native, StaleDecisionError, PyValueError);
create_exception!(_native, InvalidChoiceError, PyValueError);

#[derive(serde::Serialize)]
struct CardCatalogueEntry<'a> {
    content_key: &'a str,
    display_name: &'a str,
    printed_cost: i8,
    card_type: &'static str,
    rarity: Option<&'static str>,
    target: &'static str,
    values: CardValues,
    keywords: CardKeywords,
    is_curse: bool,
}

impl<'a> From<&'a CardDefinition> for CardCatalogueEntry<'a> {
    fn from(definition: &'a CardDefinition) -> Self {
        Self {
            content_key: definition.key,
            display_name: definition.name,
            printed_cost: definition.cost,
            card_type: match definition.card_type {
                CardType::Attack => "attack",
                CardType::Skill => "skill",
                CardType::Power => "power",
                CardType::Status => "status",
            },
            rarity: definition.rarity.map(|rarity| match rarity {
                sts_core::CardRarity::Common => "common",
                sts_core::CardRarity::Uncommon => "uncommon",
                sts_core::CardRarity::Rare => "rare",
            }),
            target: match definition.target {
                TargetRequirement::Enemy => "enemy",
                TargetRequirement::AllEnemies => "all_enemies",
                TargetRequirement::None => "none",
            },
            values: definition.values,
            keywords: definition.keywords,
            is_curse: sts_core::content::cards::is_curse_content_id(definition.id),
        }
    }
}

fn card_catalogue_entries() -> Vec<CardCatalogueEntry<'static>> {
    let mut definitions = sts_core::content::cards::ALL_CARDS
        .iter()
        .map(CardCatalogueEntry::from)
        .collect::<Vec<_>>();
    definitions.sort_by_key(|definition| definition.content_key);
    definitions
}

#[derive(serde::Serialize)]
struct FairDecisionWire {
    schema_version: u32,
    decision_revision: u64,
    observation: FairCombatObservation,
    choices: Vec<PlayerChoice>,
}

#[derive(serde::Serialize)]
struct FairStepWire {
    terminal: bool,
    decision: Option<FairDecisionWire>,
}

#[derive(serde::Serialize)]
struct BeamCloneSearchWire {
    nodes: usize,
    value: f64,
    budget_exhausted: bool,
}

#[derive(serde::Serialize)]
struct BeamCloneStepWire {
    observation: FairCombatObservation,
    choices: Vec<PlayerChoice>,
    selected_index: usize,
    teacher_visit_counts: Vec<u64>,
    search: BeamCloneSearchWire,
}

#[derive(serde::Serialize)]
struct CombatEpisodeOutcomeWire {
    status: &'static str,
    terminal_hp: i32,
    terminal_max_hp: i32,
    hp_change: i32,
    max_hp_change: i32,
    gold_change: i32,
    potion_slots: Vec<Option<&'static str>>,
    counter_changes: Vec<serde_json::Value>,
    terminal: bool,
    truncated: bool,
    accepted_decisions: usize,
    player_turns: usize,
    truncation_trigger: Option<&'static str>,
}

#[derive(serde::Serialize)]
struct BeamCloneEpisodeWire {
    schema_version: u32,
    teacher_name: &'static str,
    teacher_version: &'static str,
    steps: Vec<BeamCloneStepWire>,
    outcome: CombatEpisodeOutcomeWire,
}

#[pyclass(name = "FairCombatEnv")]
#[derive(Clone)]
pub struct PyFairCombatEnv {
    state: RunState,
    revision: DecisionRevision,
}

#[pymethods]
impl PyFairCombatEnv {
    #[staticmethod]
    pub fn combat_fixture() -> Self {
        Self {
            state: RunState::combat_fixture(),
            revision: DecisionRevision::new(0),
        }
    }

    #[staticmethod]
    pub fn from_snapshot_for_testing(json: &str) -> PyResult<Self> {
        let snapshot = restore_run_snapshot_json(json).map_err(|error| {
            PyValueError::new_err(format!("invalid fair combat snapshot: {error}"))
        })?;
        if snapshot.state.phase != RunPhase::Combat {
            return Err(PyValueError::new_err(
                "fair combat environment requires combat phase",
            ));
        }
        Ok(Self {
            state: snapshot.state,
            revision: DecisionRevision::new(0),
        })
    }

    #[pyo3(name = "clone")]
    pub fn clone_env(&self) -> Self {
        self.clone()
    }

    pub fn decision_json(&self) -> PyResult<String> {
        to_json(&fair_decision_wire(&self.state, self.revision)?)
    }

    pub fn step_json(&mut self, request_json: &str) -> PyResult<String> {
        let request: PlayerChoiceRequest = serde_json::from_str(request_json)
            .map_err(|_| InvalidChoiceError::new_err("invalid public choice request"))?;
        let action = resolve_player_choice(&self.state, self.revision, request)
            .map_err(fair_choice_error)?;
        let next = apply_run_decision_action(&self.state, action)
            .map_err(|_| DecisionUnavailableError::new_err("public choice could not be applied"))?;
        let revision = self
            .revision
            .checked_next()
            .ok_or_else(|| PyRuntimeError::new_err("public decision revision exhausted"))?;

        let terminal = fair_combat_terminal(&next);
        let decision = if terminal {
            None
        } else {
            Some(fair_decision_wire(&next, revision)?)
        };
        let result = to_json(&FairStepWire { terminal, decision })?;
        self.state = next;
        self.revision = revision;
        Ok(result)
    }

    fn __repr__(&self) -> String {
        format!("FairCombatEnv(revision={})", self.revision.get())
    }
}

#[pyclass(name = "Action")]
#[derive(Clone)]
pub struct PyAction {
    action: RunDecisionAction,
    revision: DecisionRevision,
    public_choice: Option<PlayerChoice>,
    public_action_json: String,
}

#[pymethods]
impl PyAction {
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub fn family(&self) -> &'static str {
        if self.public_choice.is_some() {
            "combat"
        } else {
            run_action_family(&self.action)
        }
    }

    pub fn kind(&self) -> &'static str {
        self.public_choice
            .map(player_choice_kind)
            .unwrap_or_else(|| run_action_kind(&self.action))
    }

    pub fn public_choice_json(&self) -> PyResult<Option<String>> {
        self.public_choice.as_ref().map(to_json).transpose()
    }

    /// Serialized public descriptor for every action family, including the
    /// non-combat actions that do not have a legacy `PlayerChoice` value.
    pub fn public_action_json(&self) -> String {
        self.public_action_json.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Action(revision={}, family='{}', kind='{}')",
            self.revision(),
            self.family(),
            self.kind()
        )
    }
}

#[pyclass(name = "ExactCombatAction")]
#[derive(Clone)]
pub struct PyExactCombatAction {
    action: CombatAction,
}

#[pymethods]
impl PyExactCombatAction {
    #[staticmethod]
    pub fn end_turn() -> Self {
        Self {
            action: CombatAction::EndTurn,
        }
    }

    #[staticmethod]
    pub fn play_card(card_id: u64, target: Option<u64>) -> Self {
        Self {
            action: CombatAction::PlayCard {
                card_id: CardId::new(card_id),
                target: target.map(MonsterId::new),
            },
        }
    }

    pub fn json(&self) -> PyResult<String> {
        to_json(&self.action)
    }

    pub fn kind(&self) -> &'static str {
        match self.action {
            CombatAction::PlayCard { .. } => "play_card",
            CombatAction::EndTurn => "end_turn",
        }
    }

    pub fn card_id(&self) -> Option<u64> {
        match self.action {
            CombatAction::PlayCard { card_id, .. } => Some(card_id.get()),
            CombatAction::EndTurn => None,
        }
    }

    pub fn target(&self) -> Option<u64> {
        match self.action {
            CombatAction::PlayCard { target, .. } => target.map(MonsterId::get),
            CombatAction::EndTurn => None,
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ExactCombatAction({})", self.json()?))
    }
}

#[pyclass(name = "DebugTransition")]
#[derive(Clone)]
pub struct PyDebugTransition {
    #[pyo3(get)]
    pub action_json: String,
    #[pyo3(get)]
    pub previous_hash: String,
    #[pyo3(get)]
    pub resulting_hash: String,
    #[pyo3(get)]
    pub events_json: String,
    #[pyo3(get)]
    pub rng_draws_json: String,
    #[pyo3(get)]
    pub simulator_error: Option<String>,
}

#[pyclass(name = "ExactStepResult")]
#[derive(Clone)]
pub struct PyExactStepResult {
    #[pyo3(get)]
    pub state_json: String,
    #[pyo3(get)]
    pub snapshot_json: String,
    #[pyo3(get)]
    pub snapshot_hash: String,
    #[pyo3(get)]
    pub phase: String,
    #[pyo3(get)]
    pub exact_legal_actions: Vec<PyExactCombatAction>,
    #[pyo3(get)]
    pub transition: PyDebugTransition,
    #[pyo3(get)]
    pub terminal: bool,
    #[pyo3(get)]
    pub terminal_reason: Option<String>,
}

type ExactRunActionKind = RunDecisionAction;

#[pyclass(name = "ExactRunAction")]
#[derive(Clone)]
pub struct PyExactRunAction {
    action: ExactRunActionKind,
}

#[pymethods]
impl PyExactRunAction {
    #[staticmethod]
    pub fn skip_reward() -> Self {
        Self {
            action: ExactRunActionKind::Run(RunAction::SkipReward),
        }
    }

    #[staticmethod]
    pub fn take_gold_reward() -> Self {
        Self {
            action: ExactRunActionKind::Run(RunAction::TakeGoldReward),
        }
    }

    #[staticmethod]
    pub fn open_card_reward() -> Self {
        Self {
            action: ExactRunActionKind::Run(RunAction::OpenCardReward),
        }
    }

    pub fn json(&self) -> PyResult<String> {
        run_action_json(&self.action)
    }

    pub fn family(&self) -> &'static str {
        run_action_family(&self.action)
    }

    pub fn kind(&self) -> String {
        run_action_kind(&self.action).to_owned()
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ExactRunAction({})", self.json()?))
    }
}

#[pyclass(name = "ExactRunStepResult")]
#[derive(Clone)]
pub struct PyExactRunStepResult {
    #[pyo3(get)]
    pub state_json: String,
    #[pyo3(get)]
    pub snapshot_json: String,
    #[pyo3(get)]
    pub snapshot_hash: String,
    #[pyo3(get)]
    pub phase: String,
    #[pyo3(get)]
    pub current_decision: String,
    #[pyo3(get)]
    pub exact_legal_actions: Vec<PyExactRunAction>,
    #[pyo3(get)]
    pub transition: PyDebugTransition,
    #[pyo3(get)]
    pub unsupported_reason: Option<String>,
}

#[pyclass(name = "RustSearchRecommendation")]
#[derive(Clone)]
pub struct PyRustSearchRecommendation {
    #[pyo3(get)]
    pub best_action: Option<PyExactRunAction>,
    #[pyo3(get)]
    pub principal_variation: Vec<PyExactRunAction>,
    #[pyo3(get)]
    pub value: f64,
    #[pyo3(get)]
    pub actions: usize,
    #[pyo3(get)]
    pub nodes: usize,
    #[pyo3(get)]
    pub terminal_reason: Option<String>,
    #[pyo3(get)]
    pub final_hp: f64,
    #[pyo3(get)]
    pub monster_hp: f64,
}

#[pyclass(name = "OmniCombatEnv")]
#[derive(Clone)]
pub struct PyOmniCombatEnv {
    state: CombatState,
}

#[pymethods]
impl PyOmniCombatEnv {
    #[staticmethod]
    pub fn initial_fixture() -> Self {
        Self {
            state: CombatState::initial_fixture(),
        }
    }

    #[staticmethod]
    pub fn from_state_json_for_debugging(json: &str) -> PyResult<Self> {
        let state: CombatState = serde_json::from_str(json).map_err(|error| {
            PyValueError::new_err(format!("invalid combat state JSON: {error}"))
        })?;
        state
            .validate()
            .map_err(|error| PyValueError::new_err(format!("invalid combat state: {error}")))?;
        Ok(Self { state })
    }

    #[staticmethod]
    pub fn from_snapshot_json(json: &str) -> PyResult<Self> {
        let snapshot = restore_combat_snapshot_json(json).map_err(|error| {
            PyValueError::new_err(format!("invalid combat snapshot JSON or state: {error}"))
        })?;
        Ok(Self {
            state: snapshot.state,
        })
    }

    #[pyo3(name = "clone")]
    pub fn clone_env(&self) -> Self {
        self.clone()
    }

    pub fn state_json(&self) -> PyResult<String> {
        to_json(&self.state)
    }

    pub fn snapshot_json(&self) -> PyResult<String> {
        self.state
            .validate()
            .map_err(|error| PyRuntimeError::new_err(format!("invalid combat state: {error}")))?;
        self.state.snapshot().canonical_json().map_err(|error| {
            PyRuntimeError::new_err(format!("snapshot serialization failed: {error:?}"))
        })
    }

    pub fn snapshot_hash(&self) -> PyResult<String> {
        snapshot_hash(&self.state)
    }

    pub fn phase(&self) -> String {
        phase_name(self.state.phase).to_owned()
    }

    pub fn exact_legal_actions(&self) -> PyResult<Vec<PyExactCombatAction>> {
        exact_legal_actions(&self.state)
    }

    pub fn step(&mut self, action: &PyExactCombatAction) -> PyResult<PyExactStepResult> {
        if is_terminal(self.state.phase) {
            return Err(PyValueError::new_err(format!(
                "combat is terminal: {}",
                phase_name(self.state.phase)
            )));
        }

        let previous_hash = snapshot_hash(&self.state)?;
        let action_json = to_json(&action.action)?;
        let transition =
            apply_combat_action_with_events(&self.state, action.action).map_err(|error| {
                PyValueError::new_err(format!("illegal exact combat action: {error:?}"))
            })?;
        let resulting_hash = snapshot_hash(&transition.state)?;
        let events_json = to_json(&transition.event_log)?;

        self.state = transition.state;

        let terminal_reason = terminal_reason(self.state.phase).map(str::to_owned);
        Ok(PyExactStepResult {
            state_json: self.state_json()?,
            snapshot_json: self.snapshot_json()?,
            snapshot_hash: resulting_hash.clone(),
            phase: self.phase(),
            exact_legal_actions: self.exact_legal_actions()?,
            transition: PyDebugTransition {
                action_json,
                previous_hash,
                resulting_hash,
                events_json,
                rng_draws_json: "[]".to_owned(),
                simulator_error: None,
            },
            terminal: terminal_reason.is_some(),
            terminal_reason,
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "OmniCombatEnv(phase={}, snapshot_hash={})",
            self.phase(),
            self.snapshot_hash()?
        ))
    }
}

#[pyclass(name = "OmniRunEnv")]
#[derive(Clone)]
pub struct PyOmniRunEnv {
    state: RunState,
    revision: DecisionRevision,
}

#[pymethods]
impl PyOmniRunEnv {
    #[staticmethod]
    pub fn combat_fixture() -> Self {
        Self {
            state: RunState::combat_fixture(),
            revision: DecisionRevision::new(0),
        }
    }

    #[staticmethod]
    pub fn map_fixture() -> Self {
        Self {
            state: RunState::map_fixture(),
            revision: DecisionRevision::new(0),
        }
    }

    #[staticmethod]
    #[pyo3(signature = (seed, ascension=None))]
    pub fn new_ironclad(seed: &str, ascension: Option<u8>) -> PyResult<Self> {
        if seed.trim().is_empty() {
            return Err(PyValueError::new_err(
                "new_ironclad requires a nonempty seed",
            ));
        }
        let ascension = ascension.unwrap_or(0);
        let numeric_seed = stable_seed(seed)
            .map_err(|error| PyValueError::new_err(format!("invalid seeded run: {error}")))?;
        let state = RunState::try_seeded_ironclad(numeric_seed, ascension)
            .map_err(|error| PyValueError::new_err(format!("invalid seeded run: {error}")))?;
        state
            .validate()
            .map_err(|error| PyValueError::new_err(format!("invalid seeded run: {error}")))?;
        Ok(Self {
            state,
            revision: DecisionRevision::new(0),
        })
    }

    #[staticmethod]
    pub fn from_state_json_for_debugging(json: &str) -> PyResult<Self> {
        let state: RunState = serde_json::from_str(json)
            .map_err(|error| PyValueError::new_err(format!("invalid run state JSON: {error}")))?;
        state
            .validate()
            .map_err(|error| PyValueError::new_err(format!("invalid run state: {error}")))?;
        Ok(Self {
            state,
            revision: DecisionRevision::new(0),
        })
    }

    #[staticmethod]
    pub fn from_snapshot_json(json: &str) -> PyResult<Self> {
        let snapshot = restore_run_snapshot_json(json).map_err(|error| {
            PyValueError::new_err(format!("invalid run snapshot JSON or state: {error}"))
        })?;
        Ok(Self {
            state: snapshot.state,
            revision: DecisionRevision::new(0),
        })
    }

    #[pyo3(name = "clone")]
    pub fn clone_env(&self) -> Self {
        self.clone()
    }

    pub fn state_json(&self) -> PyResult<String> {
        to_json(&self.state)
    }

    pub fn snapshot_json(&self) -> PyResult<String> {
        self.state
            .validate()
            .map_err(|error| PyRuntimeError::new_err(format!("invalid run state: {error}")))?;
        run_snapshot(&self.state).canonical_json().map_err(|error| {
            PyRuntimeError::new_err(format!("snapshot serialization failed: {error:?}"))
        })
    }

    pub fn snapshot_hash(&self) -> PyResult<String> {
        run_snapshot_hash(&self.state)
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub fn phase(&self) -> String {
        run_phase_name(self.state.phase).to_owned()
    }

    pub fn current_decision(&self) -> String {
        run_current_decision(&self.state).to_owned()
    }

    pub fn unsupported_reason(&self) -> Option<String> {
        run_unsupported_reason(&self.state).map(str::to_owned)
    }

    pub fn exact_legal_actions(&self) -> PyResult<Vec<PyExactRunAction>> {
        exact_run_legal_actions(&self.state)
    }

    pub fn legal_actions(&self) -> PyResult<Vec<PyAction>> {
        public_run_actions(&self.state, self.revision)
    }

    pub fn observation_json(&self) -> PyResult<String> {
        to_json(
            &fair_run_observation(&self.state)
                .map_err(|error| DecisionUnavailableError::new_err(error.to_string()))?,
        )
    }

    /// Debug-only state mutation seam for Python experimentation.
    pub fn debug_add_card(&mut self, key: &str) -> PyResult<()> {
        let content_id = sts_core::content::cards::ALL_CARDS
            .iter()
            .find(|definition| definition.key == key)
            .map(|definition| definition.id)
            .ok_or_else(|| UnknownPublicContentError::new_err("unknown card key"))?;
        let mut next = self.state.clone();
        next.gain_deck_card(content_id)
            .map_err(|error| PyValueError::new_err(format!("could not add card: {error}")))?;
        next.validate().map_err(|error| {
            PyValueError::new_err(format!("added card invalidates run: {error}"))
        })?;
        self.state = next;
        self.bump_debug_revision()?;
        Ok(())
    }

    /// Debug-only state mutation seam for Python experimentation.
    pub fn debug_add_relic(&mut self, name: &str) -> PyResult<()> {
        let relic = Relic::from_trace_name(name)
            .ok_or_else(|| UnknownPublicContentError::new_err("unknown relic name"))?;
        let mut next = self.state.clone();
        next.gain_relic_key(relic)
            .map_err(|error| PyValueError::new_err(format!("could not add relic: {error}")))?;
        next.validate().map_err(|error| {
            PyValueError::new_err(format!("added relic invalidates run: {error}"))
        })?;
        self.state = next;
        self.bump_debug_revision()?;
        Ok(())
    }

    /// Debug-only state mutation seam for Python experimentation.
    pub fn debug_add_potion(&mut self, name: &str) -> PyResult<()> {
        let potion = potion_from_name(name)
            .ok_or_else(|| UnknownPublicContentError::new_err("unknown potion name"))?;
        let mut next = self.state.clone();
        next.gain_potion(potion)
            .map_err(|error| PyValueError::new_err(format!("could not add potion: {error}")))?;
        next.validate().map_err(|error| {
            PyValueError::new_err(format!("added potion invalidates run: {error}"))
        })?;
        self.state = next;
        self.bump_debug_revision()?;
        Ok(())
    }

    fn bump_debug_revision(&mut self) -> PyResult<()> {
        self.revision = self
            .revision
            .checked_next()
            .ok_or_else(|| PyRuntimeError::new_err("public decision revision exhausted"))?;
        Ok(())
    }

    pub fn step_action(&mut self, action: &PyAction) -> PyResult<Option<String>> {
        if action.revision != self.revision {
            return Err(StaleDecisionError::new_err("public run decision is stale"));
        }
        let is_currently_legal = public_run_actions(&self.state, self.revision)?
            .iter()
            .any(|candidate| candidate.action == action.action);
        if !is_currently_legal {
            return Err(InvalidChoiceError::new_err("public run action is invalid"));
        }
        let next = apply_exact_run_action(&self.state, &action.action)
            .map_err(|_| InvalidChoiceError::new_err("public run action is invalid"))?;
        let combat_outcome = if self.state.phase == RunPhase::Combat {
            classify_combat_episode_transition(&self.state, &action.action, &next)
                .map(str::to_owned)
        } else {
            None
        };
        let revision = self
            .revision
            .checked_next()
            .ok_or_else(|| PyRuntimeError::new_err("public decision revision exhausted"))?;
        self.state = next;
        self.revision = revision;
        Ok(combat_outcome)
    }

    pub fn step(&mut self, action: &PyExactRunAction) -> PyResult<PyExactRunStepResult> {
        let previous_hash = run_snapshot_hash(&self.state)?;
        let action_json = run_action_json(&action.action)?;
        let next = apply_exact_run_action(&self.state, &action.action).map_err(|error| {
            PyValueError::new_err(format!("illegal exact run action: {error:?}"))
        })?;
        let resulting_hash = run_snapshot_hash(&next)?;

        let revision = self
            .revision
            .checked_next()
            .ok_or_else(|| PyRuntimeError::new_err("public decision revision exhausted"))?;
        self.state = next;
        self.revision = revision;

        Ok(PyExactRunStepResult {
            state_json: self.state_json()?,
            snapshot_json: self.snapshot_json()?,
            snapshot_hash: resulting_hash.clone(),
            phase: self.phase(),
            current_decision: self.current_decision(),
            exact_legal_actions: self.exact_legal_actions()?,
            transition: PyDebugTransition {
                action_json,
                previous_hash,
                resulting_hash,
                events_json: "[]".to_owned(),
                rng_draws_json: "[]".to_owned(),
                simulator_error: None,
            },
            unsupported_reason: self.unsupported_reason(),
        })
    }

    #[pyo3(signature = (depth=12, width=48, transition_budget=20000, max_decisions=512, max_player_turns=100, deduplicate_search_states=true))]
    pub fn beam_clone_episode_json(
        &self,
        depth: usize,
        width: usize,
        transition_budget: usize,
        max_decisions: usize,
        max_player_turns: usize,
        deduplicate_search_states: bool,
    ) -> PyResult<String> {
        beam_clone_episode_json(
            &self.state,
            depth,
            width,
            transition_budget,
            max_decisions,
            max_player_turns,
            deduplicate_search_states,
        )
    }

    pub fn rust_greedy_combat_search(
        &self,
        max_actions: usize,
        objective: Option<&str>,
        allowed_potions: Option<Vec<String>>,
    ) -> PyResult<PyRustSearchRecommendation> {
        rust_greedy_combat_search(&self.state, max_actions, objective, allowed_potions)
    }

    pub fn rust_beam_combat_search(
        &self,
        max_actions: usize,
        objective: Option<&str>,
        allowed_potions: Option<Vec<String>>,
        beam_width: usize,
    ) -> PyResult<PyRustSearchRecommendation> {
        rust_beam_combat_search(
            &self.state,
            max_actions,
            objective,
            allowed_potions,
            beam_width,
        )
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "RunEnv(phase={}, revision={}, snapshot_hash={})",
            self.phase(),
            self.revision(),
            self.snapshot_hash()?
        ))
    }
}

fn fair_decision_wire(state: &RunState, revision: DecisionRevision) -> PyResult<FairDecisionWire> {
    let observation = fair_combat_observation(state).map_err(fair_observation_error)?;
    let choice_set = player_choices(state, revision).map_err(fair_choice_error)?;
    Ok(FairDecisionWire {
        schema_version: choice_set.schema_version,
        decision_revision: choice_set.decision_revision.get(),
        observation,
        choices: choice_set.choices,
    })
}

fn fair_observation_error(error: FairObservationError) -> PyErr {
    match error {
        FairObservationError::NoActiveCombat => NoActiveCombatError::new_err("no active combat"),
        FairObservationError::InvalidAuthoritativeState => {
            InvalidAuthoritativeStateError::new_err("authoritative combat state is invalid")
        }
        FairObservationError::UnknownPublicContent => {
            UnknownPublicContentError::new_err("public combat content is unknown")
        }
    }
}

fn fair_choice_error(error: PlayerChoiceError) -> PyErr {
    match error {
        PlayerChoiceError::NotInCombat => {
            NotInCombatError::new_err("public choice requires combat")
        }
        PlayerChoiceError::DecisionUnavailable => {
            DecisionUnavailableError::new_err("public combat decision is unavailable")
        }
        PlayerChoiceError::StaleDecision => {
            StaleDecisionError::new_err("public combat decision is stale")
        }
        PlayerChoiceError::InvalidChoice => {
            InvalidChoiceError::new_err("public combat choice is invalid")
        }
    }
}

fn fair_combat_terminal(state: &RunState) -> bool {
    state.phase != RunPhase::Combat
        || state
            .combat
            .as_ref()
            .is_none_or(|combat| matches!(combat.phase, CombatPhase::Won | CombatPhase::Lost))
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "NoActiveCombatError",
        module.py().get_type::<NoActiveCombatError>(),
    )?;
    module.add(
        "InvalidAuthoritativeStateError",
        module.py().get_type::<InvalidAuthoritativeStateError>(),
    )?;
    module.add(
        "UnknownPublicContentError",
        module.py().get_type::<UnknownPublicContentError>(),
    )?;
    module.add(
        "NotInCombatError",
        module.py().get_type::<NotInCombatError>(),
    )?;
    module.add(
        "DecisionUnavailableError",
        module.py().get_type::<DecisionUnavailableError>(),
    )?;
    module.add(
        "StaleDecisionError",
        module.py().get_type::<StaleDecisionError>(),
    )?;
    module.add(
        "InvalidChoiceError",
        module.py().get_type::<InvalidChoiceError>(),
    )?;
    module.add_class::<PyExactCombatAction>()?;
    module.add_class::<PyExactRunAction>()?;
    module.add_class::<PyAction>()?;
    module.add_class::<PyDebugTransition>()?;
    module.add_class::<PyExactStepResult>()?;
    module.add_class::<PyExactRunStepResult>()?;
    module.add_class::<PyRustSearchRecommendation>()?;
    module.add_class::<PyFairCombatEnv>()?;
    module.add_class::<PyOmniCombatEnv>()?;
    module.add_class::<PyOmniRunEnv>()?;
    module.add_function(wrap_pyfunction!(slaythedata_preflight_json, module)?)?;
    module.add_function(wrap_pyfunction!(sts_seed_long_to_string, module)?)?;
    module.add_function(wrap_pyfunction!(card_keys, module)?)?;
    module.add_function(wrap_pyfunction!(card_catalogue_json, module)?)?;
    module.add_function(wrap_pyfunction!(combat_player_choice_kinds, module)?)?;
    module.add_function(wrap_pyfunction!(relic_names, module)?)?;
    module.add_function(wrap_pyfunction!(potion_names, module)?)?;
    Ok(())
}

#[pyfunction]
fn sts_seed_long_to_string(seed: i64) -> String {
    sts_verify::sts_seed_long_to_string(seed)
}

#[pyfunction]
fn card_keys() -> Vec<&'static str> {
    sts_core::content::cards::ALL_CARDS
        .iter()
        .map(|definition| definition.key)
        .collect()
}

#[pyfunction]
fn card_catalogue_json() -> PyResult<String> {
    to_json(&card_catalogue_entries())
}

#[pyfunction]
fn combat_player_choice_kinds() -> Vec<&'static str> {
    PlayerChoice::KIND_NAMES.to_vec()
}

#[pyfunction]
fn relic_names() -> Vec<&'static str> {
    ALL_RELICS.iter().map(|relic| relic.trace_name()).collect()
}

#[pyfunction]
fn potion_names() -> Vec<&'static str> {
    IRONCLAD_POTION_POOL
        .iter()
        .map(|potion| potion_name(*potion))
        .collect()
}

fn potion_name(potion: Potion) -> &'static str {
    match potion {
        Potion::Fire => "Fire",
        Potion::Block => "Block",
        Potion::Fear => "Fear",
        Potion::GamblersBrew => "GamblersBrew",
        Potion::Blood => "Blood",
        Potion::Elixir => "Elixir",
        Potion::HeartOfIron => "HeartOfIron",
        Potion::Dexterity => "Dexterity",
        Potion::Energy => "Energy",
        Potion::Explosive => "Explosive",
        Potion::Strength => "Strength",
        Potion::Swift => "Swift",
        Potion::Weak => "Weak",
        Potion::Attack => "Attack",
        Potion::Skill => "Skill",
        Potion::Power => "Power",
        Potion::Colorless => "Colorless",
        Potion::Flex => "Flex",
        Potion::Speed => "Speed",
        Potion::BlessingOfTheForge => "BlessingOfTheForge",
        Potion::Regen => "Regen",
        Potion::Ancient => "Ancient",
        Potion::LiquidBronze => "LiquidBronze",
        Potion::EssenceOfSteel => "EssenceOfSteel",
        Potion::Duplication => "Duplication",
        Potion::DistilledChaos => "DistilledChaos",
        Potion::LiquidMemories => "LiquidMemories",
        Potion::Cultist => "Cultist",
        Potion::FruitJuice => "FruitJuice",
        Potion::SneckoOil => "SneckoOil",
        Potion::Fairy => "Fairy",
        Potion::SmokeBomb => "SmokeBomb",
        Potion::EntropicBrew => "EntropicBrew",
    }
}

fn potion_from_name(name: &str) -> Option<Potion> {
    IRONCLAD_POTION_POOL
        .iter()
        .copied()
        .find(|potion| potion_name(*potion) == name)
}

#[pyfunction]
fn slaythedata_preflight_json(content: &str, line_index: Option<usize>) -> PyResult<String> {
    let imported = if let Some(line_index) = line_index {
        sts_verify::import_slaythedata_jsonl_line(content, line_index)
    } else {
        sts_verify::import_slaythedata_run_json(content)
    }
    .map_err(|error| PyValueError::new_err(format!("SlayTheData import failed: {error}")))?;
    let plan = sts_verify::slaythedata_replay_plan(&imported);
    let report = sts_verify::slaythedata_replay_preflight(&plan);
    to_json(&report)
}

fn exact_legal_actions(state: &CombatState) -> PyResult<Vec<PyExactCombatAction>> {
    Ok(legal_combat_actions(state)
        .map_err(|error| PyValueError::new_err(format!("invalid combat state: {error}")))?
        .into_iter()
        .map(|action| PyExactCombatAction { action })
        .collect())
}

fn snapshot_hash(state: &CombatState) -> PyResult<String> {
    state
        .snapshot()
        .hash()
        .map(|hash| hash.to_string())
        .map_err(|error| PyRuntimeError::new_err(format!("snapshot hashing failed: {error:?}")))
}

fn run_snapshot(state: &RunState) -> Snapshot<RunState> {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        state: state.clone(),
    }
}

fn stable_seed(seed: &str) -> Result<u64, String> {
    if let Ok(value) = seed.parse::<u64>() {
        return Ok(value);
    }
    sts_verify::try_sts_seed_string_to_long(seed)
        .map(|value| value as u64)
        .map_err(|error| error.to_string())
}

fn run_snapshot_hash(state: &RunState) -> PyResult<String> {
    run_snapshot(state)
        .hash()
        .map(|hash| hash.to_string())
        .map_err(|error| PyRuntimeError::new_err(format!("snapshot hashing failed: {error:?}")))
}

fn to_json<T: serde::Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value)
        .map_err(|error| PyRuntimeError::new_err(format!("JSON serialization failed: {error}")))
}

fn exact_run_legal_actions(state: &RunState) -> PyResult<Vec<PyExactRunAction>> {
    Ok(exact_run_legal_action_kinds(state)?
        .into_iter()
        .map(|action| PyExactRunAction { action })
        .collect())
}

fn public_run_actions(state: &RunState, revision: DecisionRevision) -> PyResult<Vec<PyAction>> {
    if state.phase == RunPhase::Combat {
        let choice_set = player_choices(state, revision).map_err(fair_choice_error)?;
        return choice_set
            .choices
            .into_iter()
            .map(|choice| {
                let action = resolve_player_choice(
                    state,
                    revision,
                    PlayerChoiceRequest {
                        decision_revision: revision,
                        choice,
                    },
                )
                .map_err(fair_choice_error)?;
                Ok(PyAction {
                    action,
                    revision,
                    public_choice: Some(choice),
                    public_action_json: to_json(&choice)?,
                })
            })
            .collect();
    }

    exact_run_legal_action_kinds(state)?
        .into_iter()
        .map(|action| {
            Ok(PyAction {
                public_action_json: public_run_action_json(state, &action)?,
                action,
                revision,
                public_choice: None,
            })
        })
        .collect::<PyResult<Vec<_>>>()
}

fn public_run_action_json(state: &RunState, action: &RunDecisionAction) -> PyResult<String> {
    let mut descriptor = serde_json::Map::new();
    descriptor.insert(
        "kind".to_owned(),
        serde_json::Value::String(run_action_kind(action).to_owned()),
    );
    match action {
        RunDecisionAction::Event(EventAction::Choose { choice_index }) => {
            descriptor.insert("option_slot".to_owned(), (*choice_index).into());
        }
        RunDecisionAction::GridSelect { index }
        | RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index })
        | RunDecisionAction::Run(RunAction::ChooseHandSelect { index })
        | RunDecisionAction::Run(RunAction::ChooseDrawSelect { index })
        | RunDecisionAction::Run(RunAction::ChooseDiscardSelect { index })
        | RunDecisionAction::Run(RunAction::ChooseExhaustSelect { index }) => {
            descriptor.insert("option_slot".to_owned(), (*index).into());
        }
        RunDecisionAction::Map(MapAction::ChooseNode { node_id }) => {
            let map = state
                .map
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("map action has no map state"))?;
            let node_slot = map
                .map
                .nodes
                .iter()
                .position(|node| node.id == *node_id)
                .ok_or_else(|| PyValueError::new_err("map action references an unknown node"))?;
            descriptor.insert("node_slot".to_owned(), node_slot.into());
        }
        RunDecisionAction::Rest(RestAction::Smith { card_id })
        | RunDecisionAction::Rest(RestAction::RemoveCard { card_id }) => {
            let card_slot = state
                .deck
                .iter()
                .position(|card| card.id == *card_id)
                .ok_or_else(|| PyValueError::new_err("rest action references an unknown card"))?;
            descriptor.insert("card_slot".to_owned(), card_slot.into());
        }
        RunDecisionAction::Run(RunAction::TakeCardReward { card_id }) => {
            let reward = state
                .reward
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("reward action has no reward state"))?;
            let reward_slot = reward
                .choices
                .iter()
                .position(|card| card.id == *card_id)
                .ok_or_else(|| PyValueError::new_err("reward action references an unknown card"))?;
            descriptor.insert("reward_slot".to_owned(), reward_slot.into());
        }
        RunDecisionAction::Run(action) => match action {
            RunAction::TakePotionReward { index }
            | RunAction::TakeRelicRewardAt { index }
            | RunAction::ChooseBossRelicReward { index }
            | RunAction::OpenQueuedCardReward { index } => {
                descriptor.insert("reward_slot".to_owned(), (*index).into());
            }
            RunAction::BuyShopCard { slot }
            | RunAction::BuyShopRelic { slot }
            | RunAction::BuyShopPotion { slot } => {
                descriptor.insert("shop_slot".to_owned(), (*slot).into());
            }
            RunAction::UsePotion { slot, .. } | RunAction::DiscardPotion { slot } => {
                descriptor.insert("potion_slot".to_owned(), (*slot).into());
            }
            _ => {}
        },
        RunDecisionAction::Combat(_)
        | RunDecisionAction::GridConfirm
        | RunDecisionAction::GridCancel
        | RunDecisionAction::Rest(_) => {}
    }
    to_json(&serde_json::Value::Object(descriptor))
}

fn exact_run_legal_action_kinds(state: &RunState) -> PyResult<Vec<ExactRunActionKind>> {
    legal_run_decision_actions(state).map_err(|error| {
        PyValueError::new_err(format!("invalid exact run decision state: {error:?}"))
    })
}

fn run_player_hp(state: &RunState) -> (i32, i32) {
    state
        .combat
        .as_ref()
        .map(|combat| (combat.player.hp, combat.player.max_hp))
        .unwrap_or((state.player_hp, state.player_max_hp))
}

fn classify_combat_episode_transition(
    before: &RunState,
    action: &RunDecisionAction,
    after: &RunState,
) -> Option<&'static str> {
    if let Some(combat) = after.combat.as_ref() {
        if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
            return Some("lost");
        }
        if combat.phase == CombatPhase::Won {
            return Some("won");
        }
    }
    if after.phase == RunPhase::Combat {
        return None;
    }
    let escaped = matches!(
        action,
        RunDecisionAction::Run(RunAction::UsePotion { slot, .. })
            if before.potion_at_slot(*slot) == Some(Potion::SmokeBomb)
    );
    Some(if escaped { "escaped" } else { "won" })
}

fn beam_clone_episode_json(
    root: &RunState,
    depth: usize,
    width: usize,
    transition_budget: usize,
    max_decisions: usize,
    max_player_turns: usize,
    deduplicate_search_states: bool,
) -> PyResult<String> {
    if root.phase != RunPhase::Combat || root.combat.is_none() {
        return Err(PyValueError::new_err(
            "beam cloning requires an active combat root",
        ));
    }
    if depth == 0
        || width == 0
        || transition_budget == 0
        || max_decisions == 0
        || max_player_turns == 0
    {
        return Err(PyValueError::new_err(
            "beam clone bounds and search parameters must be positive",
        ));
    }
    root.validate()
        .map_err(|error| PyValueError::new_err(format!("invalid combat root: {error}")))?;
    let (root_hp, root_max_hp) = run_player_hp(root);
    let root_gold = root.gold;
    let mut state = root.clone();
    let mut steps = Vec::new();
    let mut accepted_decisions = 0usize;
    let mut player_turns = 1usize;
    let mut terminal_status = None;
    let mut truncation_trigger = None;

    loop {
        let observation = fair_combat_observation(&state).map_err(fair_observation_error)?;
        let revision = DecisionRevision::new(accepted_decisions as u64);
        let choice_set = player_choices(&state, revision).map_err(fair_choice_error)?;
        if choice_set.choices.is_empty() {
            return Err(DecisionUnavailableError::new_err(
                "beam clone reached an empty ongoing public decision",
            ));
        }
        let teacher = sts_live::automation::beam_teacher_decision(
            &state,
            depth,
            width,
            transition_budget,
            deduplicate_search_states,
        )
        .map_err(|error| PyRuntimeError::new_err(format!("beam teacher failed: {error}")))?;
        let authoritative = choice_set
            .choices
            .iter()
            .copied()
            .map(|choice| {
                resolve_player_choice(
                    &state,
                    revision,
                    PlayerChoiceRequest {
                        decision_revision: revision,
                        choice,
                    },
                )
                .map_err(fair_choice_error)
            })
            .collect::<PyResult<Vec<_>>>()?;
        let selected_index = authoritative
            .iter()
            .position(|action| *action == teacher.action)
            .ok_or_else(|| {
                DecisionUnavailableError::new_err(
                    "beam teacher action is absent or ambiguous at public boundary",
                )
            })?;
        if authoritative
            .iter()
            .enumerate()
            .any(|(index, action)| index != selected_index && *action == teacher.action)
        {
            return Err(DecisionUnavailableError::new_err(
                "beam teacher action maps to multiple public rows",
            ));
        }
        let mut visits = vec![0; authoritative.len()];
        visits[selected_index] = 1;
        steps.push(BeamCloneStepWire {
            observation,
            choices: choice_set.choices,
            selected_index,
            teacher_visit_counts: visits,
            search: BeamCloneSearchWire {
                nodes: teacher.nodes,
                value: teacher.value,
                budget_exhausted: teacher.budget_exhausted,
            },
        });

        let action = teacher.action;
        let next = apply_run_decision_action(&state, action)
            .map_err(|error| PyRuntimeError::new_err(format!("beam action failed: {error}")))?;
        accepted_decisions += 1;
        if let Some(status) = classify_combat_episode_transition(&state, &action, &next) {
            terminal_status = Some(status);
            state = next;
            break;
        }
        if accepted_decisions >= max_decisions {
            truncation_trigger = Some("accepted_decisions");
            state = next;
            break;
        }
        let starts_next_turn = matches!(action, RunDecisionAction::Combat(CombatAction::EndTurn))
            && next
                .combat
                .as_ref()
                .is_some_and(|combat| combat.phase == CombatPhase::WaitingForPlayer);
        if starts_next_turn {
            if player_turns >= max_player_turns {
                truncation_trigger = Some("player_turns");
                state = next;
                break;
            }
            player_turns += 1;
        }
        state = next;
    }

    let status = terminal_status.unwrap_or("truncated");
    let (terminal_hp, terminal_max_hp) = run_player_hp(&state);
    let potion_slots = (0..state.potion_capacity())
        .map(|slot| state.potion_at_slot(slot).map(potion_key))
        .collect();
    let terminal = terminal_status.is_some();
    to_json(&BeamCloneEpisodeWire {
        schema_version: 1,
        teacher_name: "sts_live_incumbent_beam",
        teacher_version: "beam_clone_v1",
        steps,
        outcome: CombatEpisodeOutcomeWire {
            status,
            terminal_hp,
            terminal_max_hp,
            hp_change: terminal_hp - root_hp,
            max_hp_change: terminal_max_hp - root_max_hp,
            gold_change: state.gold - root_gold,
            potion_slots,
            counter_changes: Vec::new(),
            terminal,
            truncated: !terminal,
            accepted_decisions,
            player_turns,
            truncation_trigger,
        },
    })
}

fn rust_greedy_combat_search(
    state: &RunState,
    max_actions: usize,
    objective: Option<&str>,
    allowed_potions: Option<Vec<String>>,
) -> PyResult<PyRustSearchRecommendation> {
    let objective = objective.unwrap_or("tactical_survival");
    let allowed_potions = allowed_potions.map(|names| {
        names
            .into_iter()
            .map(|name| normalize_potion_name(&name))
            .collect::<Vec<_>>()
    });
    let mut current = state.clone();
    let mut best_first_action: Option<ExactRunActionKind> = None;
    let mut principal_variation: Vec<ExactRunActionKind> = Vec::new();
    let mut actions_taken = 0usize;
    let mut nodes = 1usize;
    let mut terminal_reason = run_terminal_reason(&current);

    while terminal_reason.is_none() && actions_taken < max_actions {
        let actions = filtered_run_actions(&current, allowed_potions.as_deref())?;
        if actions.is_empty() {
            break;
        }
        let mut best_action: Option<ExactRunActionKind> = None;
        let mut best_score = f64::NEG_INFINITY;
        for action in actions {
            let Ok(next) = apply_exact_run_action(&current, &action) else {
                continue;
            };
            nodes += 1;
            let reason = run_terminal_reason(&next);
            let score = rust_run_score(&next, reason.as_deref(), objective)?;
            if best_action.is_none() || score > best_score {
                best_score = score;
                best_action = Some(action);
            }
        }
        let Some(action) = best_action else {
            break;
        };
        if best_first_action.is_none() {
            best_first_action = Some(action);
        }
        principal_variation.push(action);
        current = apply_exact_run_action(&current, &action).map_err(|error| {
            PyValueError::new_err(format!("rust greedy selected illegal action: {error:?}"))
        })?;
        actions_taken += 1;
        terminal_reason = run_terminal_reason(&current);
    }

    let value = rust_run_score(&current, terminal_reason.as_deref(), objective)?;
    let (final_hp, monster_hp) = run_combat_hp(&current);
    Ok(PyRustSearchRecommendation {
        best_action: best_first_action.map(|action| PyExactRunAction { action }),
        principal_variation: principal_variation
            .into_iter()
            .map(|action| PyExactRunAction { action })
            .collect(),
        value,
        actions: actions_taken,
        nodes,
        terminal_reason,
        final_hp,
        monster_hp,
    })
}

#[derive(Clone)]
struct RustBeamNode {
    state: RunState,
    first_action: Option<ExactRunActionKind>,
    principal_variation: Vec<ExactRunActionKind>,
    actions: usize,
    score: f64,
    terminal_reason: Option<String>,
}

fn rust_beam_combat_search(
    state: &RunState,
    max_actions: usize,
    objective: Option<&str>,
    allowed_potions: Option<Vec<String>>,
    beam_width: usize,
) -> PyResult<PyRustSearchRecommendation> {
    if beam_width == 0 {
        return Err(PyValueError::new_err("beam_width must be at least 1"));
    }
    let objective = objective.unwrap_or("tactical_survival");
    let allowed_potions = allowed_potions.map(|names| {
        names
            .into_iter()
            .map(|name| normalize_potion_name(&name))
            .collect::<Vec<_>>()
    });
    let terminal_reason = run_terminal_reason(state);
    let initial_score = rust_run_score(state, terminal_reason.as_deref(), objective)?;
    let mut best = RustBeamNode {
        state: state.clone(),
        first_action: None,
        principal_variation: Vec::new(),
        actions: 0,
        score: initial_score,
        terminal_reason,
    };
    let mut frontier = vec![best.clone()];
    let mut nodes = 1usize;

    for _ in 0..max_actions {
        let mut next_frontier = Vec::new();
        for node in std::mem::take(&mut frontier) {
            if node.terminal_reason.is_some() {
                if rust_node_better(&node, &best) {
                    best = node.clone();
                }
                next_frontier.push(node);
                continue;
            }
            let actions = filtered_run_actions(&node.state, allowed_potions.as_deref())?;
            if actions.is_empty() {
                if rust_node_better(&node, &best) {
                    best = node.clone();
                }
                next_frontier.push(node);
                continue;
            }
            for action in actions {
                let Ok(next_state) = apply_exact_run_action(&node.state, &action) else {
                    continue;
                };
                nodes += 1;
                let terminal_reason = run_terminal_reason(&next_state);
                let score = rust_run_score(&next_state, terminal_reason.as_deref(), objective)?
                    - rust_action_penalty(&action);
                let mut principal_variation = node.principal_variation.clone();
                principal_variation.push(action);
                let child = RustBeamNode {
                    state: next_state,
                    first_action: node.first_action.or(Some(action)),
                    principal_variation,
                    actions: node.actions + 1,
                    score,
                    terminal_reason,
                };
                if rust_node_better(&child, &best) {
                    best = child.clone();
                }
                next_frontier.push(child);
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        next_frontier.sort_by(rust_node_order);
        next_frontier.truncate(beam_width);
        frontier = next_frontier;
    }

    for node in frontier {
        if rust_node_better(&node, &best) {
            best = node;
        }
    }

    let (final_hp, monster_hp) = run_combat_hp(&best.state);
    Ok(PyRustSearchRecommendation {
        best_action: best.first_action.map(|action| PyExactRunAction { action }),
        principal_variation: best
            .principal_variation
            .into_iter()
            .map(|action| PyExactRunAction { action })
            .collect(),
        value: best.score,
        actions: best.actions,
        nodes,
        terminal_reason: best.terminal_reason,
        final_hp,
        monster_hp,
    })
}

fn rust_node_better(candidate: &RustBeamNode, best: &RustBeamNode) -> bool {
    if candidate.first_action.is_some() && best.first_action.is_none() {
        return true;
    }
    if candidate.first_action.is_none() && best.first_action.is_some() {
        return false;
    }
    if candidate.terminal_reason.as_deref() == Some("won")
        && best.terminal_reason.as_deref() != Some("won")
    {
        return true;
    }
    if candidate.terminal_reason.as_deref() != Some("lost")
        && best.terminal_reason.as_deref() == Some("lost")
    {
        return true;
    }
    candidate.score > best.score
}

fn rust_node_order(left: &RustBeamNode, right: &RustBeamNode) -> std::cmp::Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left.actions.cmp(&right.actions))
}

fn filtered_run_actions(
    state: &RunState,
    allowed_potions: Option<&[String]>,
) -> PyResult<Vec<ExactRunActionKind>> {
    let actions: Vec<_> = exact_run_legal_action_kinds(state)?
        .into_iter()
        .filter(|action| rust_action_allowed(state, action, allowed_potions))
        .collect();
    Ok(preferred_select_actions(state, &actions).unwrap_or(actions))
}

fn preferred_select_actions(
    state: &RunState,
    actions: &[ExactRunActionKind],
) -> Option<Vec<ExactRunActionKind>> {
    if actions.is_empty() || !actions.iter().all(is_run_select_action) {
        return None;
    }
    let confirm = actions
        .iter()
        .find(|action| is_run_select_confirm(action))?;
    if should_confirm_selected_single_exhaust(state) {
        return Some(vec![*confirm]);
    }
    if let Some(action) = preferred_bad_exhaust_action(state, actions) {
        return Some(vec![action]);
    }
    Some(vec![*confirm])
}

fn is_run_select_action(action: &ExactRunActionKind) -> bool {
    matches!(
        action,
        ExactRunActionKind::Run(RunAction::ChooseHandSelect { .. })
            | ExactRunActionKind::Run(RunAction::ConfirmHandSelect)
            | ExactRunActionKind::Run(RunAction::ChooseDrawSelect { .. })
            | ExactRunActionKind::Run(RunAction::ConfirmDrawSelect)
            | ExactRunActionKind::Run(RunAction::ChooseDiscardSelect { .. })
            | ExactRunActionKind::Run(RunAction::ConfirmDiscardSelect)
            | ExactRunActionKind::Run(RunAction::ChooseExhaustSelect { .. })
            | ExactRunActionKind::Run(RunAction::ConfirmExhaustSelect)
    )
}

fn is_run_select_confirm(action: &ExactRunActionKind) -> bool {
    matches!(
        action,
        ExactRunActionKind::Run(RunAction::ConfirmHandSelect)
            | ExactRunActionKind::Run(RunAction::ConfirmDrawSelect)
            | ExactRunActionKind::Run(RunAction::ConfirmDiscardSelect)
            | ExactRunActionKind::Run(RunAction::ConfirmExhaustSelect)
    )
}

fn should_confirm_selected_single_exhaust(state: &RunState) -> bool {
    let Some(combat) = state.combat.as_ref() else {
        return false;
    };
    let Some(select) = combat.exhaust_select() else {
        return false;
    };
    !select.selected_hand_indices.is_empty()
        && !matches!(
            select.purpose,
            ExhaustSelectPurpose::Exhaust
                | ExhaustSelectPurpose::PurityExhaustUpTo3
                | ExhaustSelectPurpose::GamblingChip
        )
}

fn preferred_bad_exhaust_action(
    state: &RunState,
    actions: &[ExactRunActionKind],
) -> Option<ExactRunActionKind> {
    let combat = state.combat.as_ref()?;
    let before = combat.exhaust_select()?;
    for action in actions {
        if !matches!(
            action,
            ExactRunActionKind::Run(RunAction::ChooseExhaustSelect { .. })
        ) {
            continue;
        }
        let Ok(next) = apply_exact_run_action(state, action) else {
            continue;
        };
        let Some(next_combat) = next.combat.as_ref() else {
            continue;
        };
        let Some(after) = next_combat.exhaust_select() else {
            continue;
        };
        if after.selected_hand_indices.len() <= before.selected_hand_indices.len() {
            continue;
        }
        if after.selected_hand_indices.iter().any(|index| {
            !before.selected_hand_indices.contains(index)
                && combat
                    .piles
                    .hand
                    .get(*index)
                    .map(|card| is_bad_exhaust_content_id(card.content_id.get()))
                    .unwrap_or(false)
        }) {
            return Some(*action);
        }
    }
    None
}

fn is_bad_exhaust_content_id(content_id: u64) -> bool {
    matches!(
        content_id,
        4 | 5 | 6 | 7 | 61 | 62 | 63 | 64 | 65 | 66 | 67 | 68 | 69 | 70 | 71 | 72
    )
}

fn rust_action_allowed(
    state: &RunState,
    action: &ExactRunActionKind,
    allowed_potions: Option<&[String]>,
) -> bool {
    let Some(allowed_potions) = allowed_potions else {
        return true;
    };
    let ExactRunActionKind::Run(RunAction::UsePotion { slot, .. }) = action else {
        return true;
    };
    state
        .potions
        .get(*slot)
        .map(|potion| {
            allowed_potions
                .iter()
                .any(|allowed| *allowed == normalize_potion_name(&format!("{potion:?}")))
        })
        .unwrap_or(false)
}

fn normalize_potion_name(name: &str) -> String {
    let normalized: String = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    normalized
        .strip_suffix("potion")
        .unwrap_or(&normalized)
        .to_owned()
}

fn rust_action_penalty(action: &ExactRunActionKind) -> f64 {
    match action {
        // `agent_reward_hp_equivalent` values every retained potion at eight
        // HP, so potion cost persists in all later beam nodes without an
        // additional one-ply action penalty.
        ExactRunActionKind::Run(RunAction::UsePotion { .. }) => 0.0,
        ExactRunActionKind::Run(RunAction::ChooseHandSelect { .. })
        | ExactRunActionKind::Run(RunAction::ChooseDrawSelect { .. })
        | ExactRunActionKind::Run(RunAction::ChooseDiscardSelect { .. })
        | ExactRunActionKind::Run(RunAction::ChooseExhaustSelect { .. }) => 2.0,
        _ => 0.0,
    }
}

fn run_terminal_reason(state: &RunState) -> Option<String> {
    if let Some(combat) = state.combat.as_ref() {
        if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
            return Some("lost".to_owned());
        }
        if combat.phase == CombatPhase::Won {
            return Some("won".to_owned());
        }
    }
    if state.phase != RunPhase::Combat {
        return Some("won".to_owned());
    }
    None
}

fn rust_run_score(
    state: &RunState,
    terminal_reason: Option<&str>,
    objective: &str,
) -> PyResult<f64> {
    let Some(combat) = state.combat.as_ref() else {
        return Ok(match terminal_reason {
            Some("won") => 1_000_000.0,
            Some("lost") => -1_000_000.0,
            _ => 0.0,
        });
    };
    let player_hp = agent_reward_hp_equivalent(state, combat);
    let player_block = f64::from(combat.player.block);
    let player_energy = f64::from(combat.player.energy);
    let alive_monsters: Vec<_> = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .collect();
    let incoming: f64 = alive_monsters
        .iter()
        .map(|monster| f64::from(intent_damage(monster.intent)))
        .sum();
    let unblocked = (incoming - player_block).max(0.0);
    let useful_block = player_block.min(incoming);
    let monster_hp: f64 = alive_monsters
        .iter()
        .map(|monster| f64::from(monster.hp))
        .sum();
    let monster_block: f64 = alive_monsters
        .iter()
        .map(|monster| f64::from(monster.block))
        .sum();
    let alive_count = alive_monsters.len() as f64;
    let state_score = match objective {
        "survive_then_damage" => {
            player_hp * 10.0 + player_block * 1.5 + player_energy * 0.25
                - monster_hp * 3.0
                - monster_block * 0.5
                - alive_count * 25.0
        }
        "tactical_survival" => {
            player_hp * 25.0 - unblocked * 45.0 + useful_block * 7.5 + player_energy * 0.5
                - monster_hp * 4.0
                - monster_block * 0.75
                - alive_count * 60.0
        }
        "terminal_tactical" => {
            player_hp * 22.0 - unblocked * 42.0 + useful_block * 6.0 + player_energy * 0.5
                - monster_hp * 12.0
                - monster_block
                - alive_count * 250.0
        }
        "aggressive_lethal" => {
            player_hp * 8.0 + useful_block * 2.0
                - unblocked * 10.0
                - monster_hp * 9.0
                - alive_count * 100.0
        }
        "hp_preserving_lethal" => {
            player_hp * 120.0 + useful_block * 20.0 - unblocked * 160.0 + player_energy
                - monster_hp * 6.0
                - monster_block * 0.5
                - alive_count * 300.0
        }
        _ => {
            return Err(PyValueError::new_err(format!(
                "unsupported rust greedy objective: {objective}"
            )))
        }
    };
    let terminal_adjustment = if objective == "terminal_tactical" && terminal_reason.is_none() {
        -10_000.0
    } else {
        0.0
    };
    Ok(match terminal_reason {
        Some("won") => 1_000_000.0 + state_score,
        Some("lost") => -1_000_000.0 + state_score,
        _ => state_score + terminal_adjustment,
    })
}

fn agent_reward_hp_equivalent(state: &RunState, combat: &CombatState) -> f64 {
    f64::from(combat.player.hp)
        + f64::from(state.gold) / AGENT_REWARD_GOLD_PER_HP
        + state.potions.len() as f64 * AGENT_REWARD_HP_PER_POTION
}

fn run_combat_hp(state: &RunState) -> (f64, f64) {
    let Some(combat) = state.combat.as_ref() else {
        return (f64::from(state.player_hp), 0.0);
    };
    let monster_hp = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| f64::from(monster.hp))
        .sum();
    (f64::from(combat.player.hp), monster_hp)
}

fn intent_damage(intent: MonsterIntent) -> i32 {
    match intent {
        MonsterIntent::Attack { damage }
        | MonsterIntent::AttackAndBlock { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrailAndVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrail { damage, .. }
        | MonsterIntent::AttackHealSelf { damage }
        | MonsterIntent::AttackAddWoundsToDiscard { damage, .. }
        | MonsterIntent::AttackAddSlimedToDiscard { damage, .. }
        | MonsterIntent::AttackAddVoidToDraw { damage, .. }
        | MonsterIntent::AttackStealGold { damage, .. } => damage,
        MonsterIntent::AttackMultiple { damage, hits } => damage * hits,
        MonsterIntent::AddBurnToDiscard { damage, .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => damage,
        _ => 0,
    }
}

fn apply_exact_run_action(
    state: &RunState,
    action: &ExactRunActionKind,
) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(state, *action)
}

fn run_action_json(action: &ExactRunActionKind) -> PyResult<String> {
    match action {
        ExactRunActionKind::Combat(action) => to_json(action),
        ExactRunActionKind::Event(action) => to_json(action),
        ExactRunActionKind::GridSelect { index } => {
            to_json(&serde_json::json!({ "SelectGridCard": { "index": index } }))
        }
        ExactRunActionKind::GridConfirm => to_json(&serde_json::json!("ConfirmGrid")),
        ExactRunActionKind::GridCancel => to_json(&serde_json::json!("CancelGrid")),
        ExactRunActionKind::Map(action) => to_json(action),
        ExactRunActionKind::Rest(action) => to_json(action),
        ExactRunActionKind::Run(action) => to_json(action),
    }
}

fn run_action_family(action: &ExactRunActionKind) -> &'static str {
    match action {
        ExactRunActionKind::Combat(_) => "combat",
        ExactRunActionKind::Event(_) => "event",
        ExactRunActionKind::GridSelect { .. }
        | ExactRunActionKind::GridConfirm
        | ExactRunActionKind::GridCancel => "grid",
        ExactRunActionKind::Map(_) => "map",
        ExactRunActionKind::Rest(_) => "rest",
        ExactRunActionKind::Run(_) => "run",
    }
}

fn player_choice_kind(choice: PlayerChoice) -> &'static str {
    match choice {
        PlayerChoice::PlayHandSlot { .. } => "play_hand_slot",
        PlayerChoice::EndTurn => "end_turn",
        PlayerChoice::UsePotionSlot { .. } => "use_potion_slot",
        PlayerChoice::DiscardPotionSlot { .. } => "discard_potion_slot",
        PlayerChoice::ToggleVisibleCard { .. } => "toggle_visible_card",
        PlayerChoice::ChooseVisibleOption { .. } => "choose_visible_option",
        PlayerChoice::ConfirmSelection => "confirm_selection",
        PlayerChoice::SkipSelection => "skip_selection",
        PlayerChoice::Proceed => "proceed",
    }
}

fn run_action_kind(action: &ExactRunActionKind) -> &'static str {
    match action {
        ExactRunActionKind::Combat(CombatAction::PlayCard { .. }) => "play_card",
        ExactRunActionKind::Combat(CombatAction::EndTurn) => "end_turn",
        ExactRunActionKind::Event(EventAction::Choose { .. }) => "event_choose",
        ExactRunActionKind::GridSelect { .. } => "select_grid_card",
        ExactRunActionKind::GridConfirm => "confirm_grid",
        ExactRunActionKind::GridCancel => "cancel_grid",
        ExactRunActionKind::Map(MapAction::ChooseNode { .. }) => "choose_map_node",
        ExactRunActionKind::Rest(RestAction::Heal) => "rest_heal",
        ExactRunActionKind::Rest(RestAction::OpenSmith) => "rest_open_smith",
        ExactRunActionKind::Rest(RestAction::OpenRemove) => "rest_open_remove",
        ExactRunActionKind::Rest(RestAction::Smith { .. }) => "rest_smith",
        ExactRunActionKind::Rest(RestAction::RemoveCard { .. }) => "rest_remove_card",
        ExactRunActionKind::Rest(RestAction::Lift) => "rest_lift",
        ExactRunActionKind::Rest(RestAction::Dig) => "rest_dig",
        ExactRunActionKind::Rest(RestAction::Recall) => "rest_recall",
        ExactRunActionKind::Rest(RestAction::Proceed) => "rest_proceed",
        ExactRunActionKind::Run(RunAction::SkipReward) => "skip_reward",
        ExactRunActionKind::Run(RunAction::CloseCardReward) => "close_card_reward",
        ExactRunActionKind::Run(RunAction::TakeCardReward { .. }) => "take_card_reward",
        ExactRunActionKind::Run(RunAction::TakeSingingBowlReward) => "take_singing_bowl_reward",
        ExactRunActionKind::Run(RunAction::TakeGoldReward) => "take_gold_reward",
        ExactRunActionKind::Run(RunAction::TakeStolenGoldReward) => "take_stolen_gold_reward",
        ExactRunActionKind::Run(RunAction::TakePotionReward { .. }) => "take_potion_reward",
        ExactRunActionKind::Run(RunAction::TakeRelicReward) => "take_relic_reward",
        ExactRunActionKind::Run(RunAction::TakeRelicRewardAt { .. }) => "take_relic_reward_at",
        ExactRunActionKind::Run(RunAction::TakeSapphireKey) => "take_sapphire_key",
        ExactRunActionKind::Run(RunAction::TakeEmeraldKey) => "take_emerald_key",
        ExactRunActionKind::Run(RunAction::ChooseBossRelicReward { .. }) => {
            "choose_boss_relic_reward"
        }
        ExactRunActionKind::Run(RunAction::Proceed) => "proceed",
        ExactRunActionKind::Run(RunAction::OpenChest) => "open_chest",
        ExactRunActionKind::Run(RunAction::OpenCardReward) => "open_card_reward",
        ExactRunActionKind::Run(RunAction::OpenQueuedCardReward { .. }) => {
            "open_queued_card_reward"
        }
        ExactRunActionKind::Run(RunAction::SkipPotionReward) => "skip_potion_reward",
        ExactRunActionKind::Run(RunAction::BuyShopCard { .. }) => "buy_shop_card",
        ExactRunActionKind::Run(RunAction::BuyShopRelic { .. }) => "buy_shop_relic",
        ExactRunActionKind::Run(RunAction::BuyShopPotion { .. }) => "buy_shop_potion",
        ExactRunActionKind::Run(RunAction::UsePotion { .. }) => "use_potion",
        ExactRunActionKind::Run(RunAction::DiscardPotion { .. }) => "discard_potion",
        ExactRunActionKind::Run(RunAction::ChooseCombatCardReward { .. }) => {
            "choose_combat_card_reward"
        }
        ExactRunActionKind::Run(RunAction::SkipCombatCardReward) => "skip_combat_card_reward",
        ExactRunActionKind::Run(RunAction::ChooseHandSelect { .. }) => "choose_hand_select",
        ExactRunActionKind::Run(RunAction::ConfirmHandSelect) => "confirm_hand_select",
        ExactRunActionKind::Run(RunAction::ConfirmHandSelectWithoutRetrieval) => {
            "confirm_hand_select_without_retrieval"
        }
        ExactRunActionKind::Run(RunAction::ChooseDrawSelect { .. }) => "choose_draw_select",
        ExactRunActionKind::Run(RunAction::ConfirmDrawSelect) => "confirm_draw_select",
        ExactRunActionKind::Run(RunAction::ChooseDiscardSelect { .. }) => "choose_discard_select",
        ExactRunActionKind::Run(RunAction::ConfirmDiscardSelect) => "confirm_discard_select",
        ExactRunActionKind::Run(RunAction::ChooseExhaustSelect { .. }) => "choose_exhaust_select",
        ExactRunActionKind::Run(RunAction::ConfirmExhaustSelect) => "confirm_exhaust_select",
        ExactRunActionKind::Run(RunAction::EnterShop) => "enter_shop",
        ExactRunActionKind::Run(RunAction::LeaveShop) => "leave_shop",
        ExactRunActionKind::Run(RunAction::OpenShopRemove) => "open_shop_remove",
    }
}

fn run_current_decision(state: &RunState) -> &'static str {
    if state.card_grid.is_some() {
        return "grid";
    }
    match state.phase {
        RunPhase::Combat => "combat",
        RunPhase::Reward => "reward",
        RunPhase::Treasure => "treasure",
        RunPhase::Rest => "rest",
        RunPhase::Event => "event",
        RunPhase::Shop => "shop",
        RunPhase::Idle if state.map.is_some() => "map",
        RunPhase::Idle => "idle",
        RunPhase::Victory | RunPhase::Complete => "complete",
    }
}

fn run_unsupported_reason(state: &RunState) -> Option<&'static str> {
    if state.phase == RunPhase::Complete {
        return None;
    }
    match legal_run_decision_actions(state) {
        Ok(actions) if actions.is_empty() => {
            Some("no exact run legal-action adapter for current decision")
        }
        Err(_) => Some("invalid run state at exact action boundary"),
        Ok(_) => None,
    }
}

fn run_phase_name(phase: RunPhase) -> &'static str {
    match phase {
        RunPhase::Combat => "combat",
        RunPhase::Reward => "reward",
        RunPhase::Treasure => "treasure",
        RunPhase::Rest => "rest",
        RunPhase::Event => "event",
        RunPhase::Shop => "shop",
        RunPhase::Idle => "idle",
        RunPhase::Victory | RunPhase::Complete => "complete",
    }
}

fn phase_name(phase: CombatPhase) -> &'static str {
    match phase {
        CombatPhase::WaitingForPlayer => "waiting_for_player",
        CombatPhase::MonsterTurn => "monster_turn",
        CombatPhase::Won => "won",
        CombatPhase::Lost => "lost",
    }
}

fn is_terminal(phase: CombatPhase) -> bool {
    matches!(phase, CombatPhase::Won | CombatPhase::Lost)
}

fn terminal_reason(phase: CombatPhase) -> Option<&'static str> {
    match phase {
        CombatPhase::Won => Some("won"),
        CombatPhase::Lost => Some("lost"),
        CombatPhase::WaitingForPlayer | CombatPhase::MonsterTurn => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::Potion;

    #[test]
    fn card_catalogue_is_complete_sorted_unique_and_public() {
        let catalogue = card_catalogue_entries();
        assert_eq!(catalogue.len(), sts_core::content::cards::ALL_CARDS.len());
        assert_eq!(catalogue.len(), 251);
        assert!(catalogue
            .windows(2)
            .all(|pair| pair[0].content_key < pair[1].content_key));

        let payload = serde_json::to_value(&catalogue).expect("catalogue serializes");
        let records = payload.as_array().expect("catalogue is an array");
        assert!(records.iter().all(|record| {
            let object = record.as_object().expect("catalogue entry is an object");
            !object.contains_key("id")
                && !object.contains_key("content_id")
                && !object.contains_key("upgrade")
        }));
    }

    #[test]
    fn card_catalogue_marks_curses_without_changing_card_type() {
        let catalogue = card_catalogue_entries();
        let parasite = catalogue
            .iter()
            .find(|definition| definition.content_key == "Parasite")
            .expect("Parasite is catalogued");
        assert!(parasite.is_curse);
        assert_eq!(parasite.card_type, "status");

        let wound = catalogue
            .iter()
            .find(|definition| definition.content_key == "Wound")
            .expect("Wound is catalogued");
        assert!(!wound.is_curse);
        assert_eq!(wound.card_type, "status");
    }

    #[test]
    fn schema_one_combat_snapshot_migrates_when_state_is_valid() {
        let env = PyOmniCombatEnv::initial_fixture();
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&env.snapshot_json().expect("snapshot JSON")).expect("valid JSON");
        snapshot["schema_version"] = serde_json::Value::from(1);

        PyOmniCombatEnv::from_snapshot_json(&snapshot.to_string())
            .expect("validated schema-one snapshot migrates");
    }

    #[test]
    fn snapshot_with_missing_combat_rng_is_rejected() {
        pyo3::Python::initialize();
        let env = PyOmniCombatEnv::initial_fixture();
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&env.snapshot_json().expect("snapshot JSON")).expect("valid JSON");
        snapshot["state"]
            .as_object_mut()
            .expect("combat state object")
            .remove("card_random_rng");

        let error = PyOmniCombatEnv::from_snapshot_json(&snapshot.to_string())
            .err()
            .expect("missing authoritative RNG must fail");

        assert!(error.to_string().contains("invalid combat snapshot JSON"));
    }

    #[test]
    fn raw_run_state_with_contradictory_phase_is_rejected() {
        pyo3::Python::initialize();
        let env = PyOmniRunEnv::combat_fixture();
        let mut state: serde_json::Value =
            serde_json::from_str(&env.state_json().expect("state JSON")).expect("valid JSON");
        state["phase"] = serde_json::Value::String("Idle".to_owned());

        let error = PyOmniRunEnv::from_state_json_for_debugging(&state.to_string())
            .err()
            .expect("contradictory run phase must fail");

        assert!(error
            .to_string()
            .contains("combat state exists outside combat phase"));
    }

    #[test]
    fn exact_run_legal_actions_report_invalid_state() {
        let mut env = PyOmniRunEnv::map_fixture();
        env.state.phase = RunPhase::Shop;
        env.state.shop = None;

        assert!(env.exact_legal_actions().is_err());
    }

    #[test]
    fn run_snapshot_with_missing_phase_owner_is_rejected() {
        pyo3::Python::initialize();
        let env = PyOmniRunEnv::map_fixture();
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&env.snapshot_json().expect("snapshot JSON")).expect("valid JSON");
        snapshot["state"]["phase"] = serde_json::Value::String("Shop".to_owned());
        snapshot["state"]["shop"] = serde_json::Value::Null;

        let error = PyOmniRunEnv::from_snapshot_json(&snapshot.to_string())
            .err()
            .expect("missing authoritative screen must fail");

        assert!(error.to_string().contains("shop phase has no shop screen"));
    }

    #[test]
    fn schema_one_run_snapshot_migrates_when_state_is_valid() {
        let env = PyOmniRunEnv::map_fixture();
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&env.snapshot_json().expect("snapshot JSON")).expect("valid JSON");
        snapshot["schema_version"] = serde_json::Value::from(1);

        PyOmniRunEnv::from_snapshot_json(&snapshot.to_string())
            .expect("validated schema-one run snapshot migrates");
    }

    #[test]
    fn initial_fixture_round_trips_through_snapshot_json() {
        let env = PyOmniCombatEnv::initial_fixture();
        let restored =
            PyOmniCombatEnv::from_snapshot_json(&env.snapshot_json().expect("snapshot JSON"))
                .expect("snapshot restores");

        assert_eq!(
            restored.snapshot_hash().expect("restored hashes"),
            env.snapshot_hash().expect("fixture hashes")
        );
    }

    #[test]
    fn fixture_exposes_exact_legal_actions() {
        let env = PyOmniCombatEnv::initial_fixture();
        let actions = env.exact_legal_actions().expect("valid combat fixture");

        assert!(actions.iter().any(|action| action.kind() == "end_turn"));
        assert!(actions.iter().any(|action| action.card_id() == Some(1)));
    }

    #[test]
    fn stepping_updates_state_and_returns_transition() {
        let mut env = PyOmniCombatEnv::initial_fixture();
        let before = env.snapshot_hash().expect("hashes before");
        let result = env
            .step(&PyExactCombatAction::play_card(1, Some(1)))
            .expect("strike is legal");

        assert_ne!(result.snapshot_hash, before);
        assert_eq!(result.transition.previous_hash, before);
        assert_eq!(result.transition.resulting_hash, result.snapshot_hash);
        assert!(!result.transition.events_json.is_empty());
    }

    #[test]
    fn clone_branches_without_mutating_parent() {
        let env = PyOmniCombatEnv::initial_fixture();
        let parent_hash = env.snapshot_hash().expect("parent hashes");
        let mut child = env.clone_env();

        child
            .step(&PyExactCombatAction::play_card(1, Some(1)))
            .expect("child can step independently");

        assert_eq!(
            env.snapshot_hash().expect("parent still hashes"),
            parent_hash
        );
        assert_ne!(child.snapshot_hash().expect("child hashes"), parent_hash);
    }

    #[test]
    fn state_and_legal_action_inspection_do_not_mutate_hash() {
        let env = PyOmniCombatEnv::initial_fixture();
        let before = env.snapshot_hash().expect("hashes before");

        let _ = env.state_json().expect("state serializes");
        let _ = env.exact_legal_actions();

        assert_eq!(env.snapshot_hash().expect("hashes after"), before);
    }

    #[test]
    fn run_combat_fixture_exposes_combat_actions_and_steps() {
        let mut env = PyOmniRunEnv::combat_fixture();
        let before = env.snapshot_hash().expect("run hashes before");
        let actions = env.exact_legal_actions().expect("run legal actions");
        let strike = actions
            .iter()
            .find(|action| action.kind() == "play_card")
            .expect("combat fixture has a play action")
            .clone();

        let result = env.step(&strike).expect("run combat action applies");

        assert_eq!(result.transition.previous_hash, before);
        assert_ne!(result.snapshot_hash, before);
        assert_eq!(env.phase(), "combat");
    }

    #[test]
    fn unified_run_actions_use_public_combat_descriptors_and_one_step_boundary() {
        let mut env = PyOmniRunEnv::combat_fixture();
        let actions = env.legal_actions().expect("public actions");
        let strike = actions
            .iter()
            .find(|action| action.kind() == "play_hand_slot")
            .expect("combat fixture has a public play action")
            .clone();

        assert_eq!(strike.family(), "combat");
        assert_eq!(strike.revision(), 0);
        assert!(strike
            .public_choice_json()
            .expect("public descriptor serializes")
            .expect("combat action has a public descriptor")
            .contains("hand_slot"));

        env.step_action(&strike).expect("public action applies");

        assert_eq!(env.revision(), 1);
        assert_eq!(env.phase(), "combat");
    }

    #[test]
    fn unified_run_action_rejects_a_stale_revision_before_reuse() {
        pyo3::Python::initialize();
        let mut env = PyOmniRunEnv::combat_fixture();
        let end_turn = env
            .legal_actions()
            .expect("public actions")
            .into_iter()
            .find(|action| action.kind() == "end_turn")
            .expect("combat fixture can end turn");

        env.step_action(&end_turn).expect("first use applies");
        let error = env
            .step_action(&end_turn)
            .expect_err("old action must be stale");

        assert!(error.to_string().contains("public run decision is stale"));
    }

    #[test]
    fn unified_run_actions_exist_outside_combat_without_a_second_action_type() {
        let env = PyOmniRunEnv::map_fixture();
        let actions = env.legal_actions().expect("map actions");

        assert!(!actions.is_empty());
        assert!(actions.iter().all(|action| action.family() == "map"));
        assert!(actions.iter().all(|action| action
            .public_choice_json()
            .expect("optional JSON")
            .is_none()));
    }

    #[test]
    fn unified_run_observations_cover_event_and_map_screens() {
        let event = PyOmniRunEnv::new_ironclad("ABC123", Some(0)).expect("seeded run");
        let event_observation: serde_json::Value =
            serde_json::from_str(&event.observation_json().expect("event observation"))
                .expect("event JSON");
        assert_eq!(event_observation["kind"], "event");
        assert_eq!(event_observation["screen"]["event"], "Neow");
        assert!(event_observation["context"]["player_hp"].is_number());
        assert!(event_observation["screen"].get("event_data").is_none());

        let map = PyOmniRunEnv::map_fixture();
        let map_observation: serde_json::Value =
            serde_json::from_str(&map.observation_json().expect("map observation"))
                .expect("map JSON");
        assert_eq!(map_observation["kind"], "map");
        assert_eq!(
            map_observation["screen"]["reachable_nodes"],
            serde_json::json!([1, 2])
        );
        assert!(map_observation.to_string().find("card_id").is_none());
        let action = map
            .legal_actions()
            .expect("map actions")
            .into_iter()
            .next()
            .expect("map action");
        let descriptor: serde_json::Value =
            serde_json::from_str(&action.public_action_json()).expect("action descriptor");
        assert_eq!(descriptor["kind"], "choose_map_node");
        assert!(descriptor["node_slot"].is_number());
    }

    #[test]
    fn run_combat_exact_actions_expose_exhaust_select_after_elixir() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.potions = vec![Potion::Elixir];
        let elixir = env
            .exact_legal_actions()
            .expect("run legal actions")
            .into_iter()
            .find(|action| action.kind() == "use_potion")
            .expect("elixir is usable")
            .clone();

        env.step(&elixir).expect("elixir opens exhaust select");
        let actions = env.exact_legal_actions().expect("run legal actions");

        assert!(actions
            .iter()
            .any(|action| action.kind() == "choose_exhaust_select"));
        assert!(actions
            .iter()
            .any(|action| action.kind() == "confirm_exhaust_select"));
        assert_eq!(env.unsupported_reason(), None);
    }

    #[test]
    fn reward_exact_actions_expose_fruit_juice_without_combat_state() {
        let mut env = PyOmniRunEnv::combat_fixture();
        let combat = env.state.combat.as_mut().expect("combat fixture");
        combat.phase = CombatPhase::Won;
        for monster in &mut combat.monsters {
            monster.hp = 0;
            monster.alive = false;
        }
        sts_core::enter_reward_screen(&mut env.state).expect("fixture reward entry succeeds");
        env.state.player_hp = 75;
        env.state.player_max_hp = 80;
        env.state.potions = vec![Potion::Attack, Potion::FruitJuice];

        let fruit_juice = env
            .exact_legal_actions()
            .expect("run legal actions")
            .into_iter()
            .find(|action| action.kind() == "use_potion")
            .expect("Fruit Juice is usable from reward screen")
            .clone();

        env.step(&fruit_juice)
            .expect("reward-screen Fruit Juice applies");

        assert_eq!(env.state.player_hp, 80);
        assert_eq!(env.state.player_max_hp, 85);
        assert_eq!(env.state.potions, vec![Potion::Attack]);
    }

    #[test]
    fn reward_exact_take_potion_reward_steps_without_combat_state() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.phase = RunPhase::Reward;
        env.state.combat = None;
        env.state.reward = Some(sts_core::RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: Some(Potion::Ancient),
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: sts_core::CardRewardFlow::None,
        });
        env.state.potions.clear();

        let take_potion = env
            .exact_legal_actions()
            .expect("run legal actions")
            .into_iter()
            .find(|action| action.kind() == "take_potion_reward")
            .expect("potion reward can be taken")
            .clone();

        env.step(&take_potion)
            .expect("reward-screen potion reward is collected");

        assert_eq!(env.state.potions, vec![Potion::Ancient]);
        assert_eq!(
            env.state
                .reward
                .as_ref()
                .expect("reward remains")
                .potion_offer,
            None
        );
    }

    #[test]
    fn rust_beam_confirms_optional_exhaust_select_when_no_bad_card_is_available() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.potions = vec![Potion::Elixir];
        let elixir = env
            .exact_legal_actions()
            .expect("run legal actions")
            .into_iter()
            .find(|action| action.kind() == "use_potion")
            .expect("elixir is usable")
            .clone();

        env.step(&elixir).expect("elixir opens exhaust select");

        let recommendation = env
            .rust_beam_combat_search(12, Some("terminal_tactical"), Some(Vec::new()), 32)
            .expect("rust beam searches exhaust select");
        let best_action = recommendation
            .best_action
            .expect("rust beam recommends select action");

        assert_eq!(best_action.kind(), "confirm_exhaust_select");
    }

    #[test]
    fn rust_score_values_ten_gold_as_one_hp() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.gold = 0;
        let base = rust_run_score(&env.state, None, "terminal_tactical").expect("base score");

        env.state.gold = 10;
        let with_gold =
            rust_run_score(&env.state, None, "terminal_tactical").expect("gold-adjusted score");

        assert_eq!(with_gold - base, 22.0);
    }

    #[test]
    fn rust_score_values_one_potion_as_eight_hp() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.potions.clear();
        let base = rust_run_score(&env.state, None, "terminal_tactical").expect("base score");

        env.state.potions.push(Potion::Attack);
        let with_potion =
            rust_run_score(&env.state, None, "terminal_tactical").expect("potion-adjusted score");

        assert_eq!(with_potion - base, 176.0);
        assert_eq!(
            rust_action_penalty(&ExactRunActionKind::Run(RunAction::UsePotion {
                slot: 0,
                target: None,
            })),
            0.0
        );
    }

    #[test]
    fn run_map_fixture_exposes_map_actions_and_round_trips_snapshot() {
        let env = PyOmniRunEnv::map_fixture();
        let restored =
            PyOmniRunEnv::from_snapshot_json(&env.snapshot_json().expect("snapshot JSON"))
                .expect("snapshot restores");

        assert_eq!(
            restored.snapshot_hash().expect("restored hashes"),
            env.snapshot_hash().expect("run hashes")
        );
        assert!(env
            .exact_legal_actions()
            .expect("run legal actions")
            .iter()
            .any(|action| action.family() == "map"));
    }

    #[test]
    fn completed_run_round_trips_without_exposing_a_decision() {
        let mut env = PyOmniRunEnv::map_fixture();
        env.state.phase = RunPhase::Complete;
        let restored =
            PyOmniRunEnv::from_snapshot_json(&env.snapshot_json().expect("snapshot JSON"))
                .expect("completed snapshot restores");

        assert_eq!(restored.phase(), "complete");
        assert_eq!(restored.current_decision(), "complete");
        assert!(restored
            .exact_legal_actions()
            .expect("run legal actions")
            .is_empty());
        assert!(restored.unsupported_reason().is_none());
        assert_eq!(
            restored.snapshot_hash().expect("restored hashes"),
            env.snapshot_hash().expect("run hashes")
        );
    }

    #[test]
    fn seed_start_constructor_is_deterministic() {
        let first = PyOmniRunEnv::new_ironclad("TEST", Some(0)).expect("seed starts");
        let second = PyOmniRunEnv::new_ironclad("TEST", Some(0)).expect("seed starts");
        let other = PyOmniRunEnv::new_ironclad("OTHER", Some(0)).expect("seed starts");

        assert_eq!(first.phase(), "event");
        assert_eq!(first.current_decision(), "event");
        assert_eq!(
            first.snapshot_hash().expect("first hash"),
            second.snapshot_hash().expect("second hash")
        );
        assert_ne!(
            first.snapshot_hash().expect("first hash"),
            other.snapshot_hash().expect("other hash")
        );
        assert!(first
            .exact_legal_actions()
            .expect("run legal actions")
            .iter()
            .any(|action| action.family() == "event"));
    }

    #[test]
    fn seed_start_constructor_rejects_missing_identity_and_invalid_ascension() {
        assert!(PyOmniRunEnv::new_ironclad("", Some(0)).is_err());
        assert!(PyOmniRunEnv::new_ironclad("   ", Some(0)).is_err());
        assert!(PyOmniRunEnv::new_ironclad("TEST", Some(21)).is_err());
    }
}
