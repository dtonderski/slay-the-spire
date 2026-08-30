use pyo3::create_exception;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use sts_core::potion::IRONCLAD_POTION_POOL;
use sts_core::{
    apply_combat_action_with_events, apply_run_decision_action, fair_combat_observation,
    fair_run_observation, legal_combat_actions, legal_run_decision_actions, player_choices,
    potion_key, resolve_player_choice, restore_combat_snapshot_json, restore_run_snapshot_json,
    CardDefinition, CardId, CardKeywords, CardType, CardValues, CombatAction, CombatPhase,
    CombatState, DecisionRevision, EventAction, FairCombatObservation, FairObservationError,
    MapAction, MonsterId, PlayerChoice, PlayerChoiceError, PlayerChoiceRequest, Potion, Relic,
    RestAction, RunAction, RunDecisionAction, RunPhase, RunState, Snapshot, TargetRequirement,
    ALL_RELICS, SNAPSHOT_SCHEMA_VERSION,
};
use sts_search::{
    puct_search, CombatProxyConfig, FairLeafEvaluation, FairLeafEvaluator, PuctConfig,
    FAIR_LEAF_BATCH_SCHEMA, PRIVILEGED_PUCT_TEACHER_NAME, PRIVILEGED_PUCT_TEACHER_VERSION,
};

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
struct PublicStepWire {
    combat_outcome: Option<&'static str>,
    player_turn_advances: usize,
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
        let (combat_outcome, _) = apply_public_run_action(self, action)?;
        Ok(combat_outcome.map(str::to_owned))
    }

    pub fn step_action_json(&mut self, action: &PyAction) -> PyResult<String> {
        let (combat_outcome, player_turn_advances) = apply_public_run_action(self, action)?;
        to_json(&PublicStepWire {
            combat_outcome,
            player_turn_advances,
        })
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

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (evaluator, c_puct=1.5, simulation_budget=64, transition_budget=64, reward_config_json=None, episode_root_max_hp=None, episode_root_gold=None))]
    pub fn puct_search_json(
        &self,
        evaluator: Bound<'_, PyAny>,
        c_puct: f64,
        simulation_budget: usize,
        transition_budget: usize,
        reward_config_json: Option<&str>,
        episode_root_max_hp: Option<i32>,
        episode_root_gold: Option<i32>,
    ) -> PyResult<String> {
        puct_search_json(
            &self.state,
            evaluator.unbind(),
            c_puct,
            simulation_budget,
            transition_budget,
            reward_config_json,
            episode_root_max_hp,
            episode_root_gold,
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
    module.add_class::<PyFairCombatEnv>()?;
    module.add_class::<PyOmniCombatEnv>()?;
    module.add_class::<PyOmniRunEnv>()?;
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
    sts_core::sts_seed_long_to_string(seed)
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
    sts_core::try_sts_seed_string_to_long(seed)
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

fn classify_combat_state(state: &RunState) -> Option<&'static str> {
    let combat = state.combat.as_ref()?;
    if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
        Some("lost")
    } else if combat.phase == CombatPhase::Won {
        Some("won")
    } else {
        None
    }
}

fn classify_combat_episode_transition(
    before: &RunState,
    action: &RunDecisionAction,
    after: &RunState,
) -> Option<&'static str> {
    // Proceed is run-screen cleanup after an already terminal combat, not a
    // second combat outcome. In particular, lost -> Proceed must not become a win.
    if classify_combat_state(before).is_some() {
        return None;
    }
    if let Some(status) = classify_combat_state(after) {
        return Some(status);
    }
    if after.phase == RunPhase::Combat {
        return None;
    }
    let escaped = matches!(
        action,
        RunDecisionAction::Run(RunAction::UsePotion { slot, .. })
            if before.potion_at_slot(*slot) == Some(Potion::SmokeBomb)
    );
    if escaped {
        return Some("escaped");
    }
    // Victory clears the combat and opens the next run screen, so it is
    // recognized by exclusion. Guard the one case exclusion would get maximally
    // wrong and label a dead player's exit as the best possible outcome.
    if run_player_hp(after).0 <= 0 {
        return Some("lost");
    }
    Some("won")
}

fn player_turn_advances(before: &RunState, action: &RunDecisionAction, after: &RunState) -> usize {
    let (Some(before_combat), Some(after_combat)) = (before.combat.as_ref(), after.combat.as_ref())
    else {
        return 0;
    };
    if after_combat.phase != CombatPhase::WaitingForPlayer {
        return 0;
    }

    // This counter is authoritative when maintained, and preserves deltas
    // greater than one. It is not the sole signal because some relic sets do
    // not maintain it.
    let counter_delta = after_combat
        .relic_counters
        .player_turns_started
        .saturating_sub(before_combat.relic_counters.player_turns_started)
        as usize;
    if counter_delta > 0 {
        return counter_delta;
    }
    if matches!(action, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return 1;
    }

    // Conclude's PressEndTurnButtonAction is a modeled forced turn source just
    // like explicit END, but its accepted public action remains PlayCard.
    let forced_turn_card = match action {
        RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) => before_combat
            .piles
            .hand
            .iter()
            .find(|card| card.id == *card_id)
            .is_some_and(|card| card.content_id == sts_core::content::cards::CONCLUDE_ANY_COLOR_ID),
        _ => false,
    };
    if forced_turn_card {
        return 1;
    }

    // Time Warp can execute end_player_turn from the twelfth card transition
    // or from a later selection transition. Both are authoritative state
    // evidence that the forced turn completed.
    let time_warp_wrapped = before_combat.monsters.iter().any(|before_monster| {
        before_monster.alive
            && before_monster.content_id == sts_core::content::monsters::TIME_EATER_ID
            && before_monster.powers.time_warp == 11
            && after_combat.monsters.iter().any(|after_monster| {
                after_monster.content_id == before_monster.content_id
                    && after_monster.powers.time_warp == 0
            })
    });
    let forced_end_settled = (before_combat.time_warp_end_turn
        || before_combat.defer_time_warp_end_turn)
        && !after_combat.time_warp_end_turn
        && !after_combat.defer_time_warp_end_turn;
    usize::from(time_warp_wrapped || forced_end_settled)
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
    let mut terminal_status = classify_combat_state(root);
    let mut truncation_trigger = None;

    while terminal_status.is_none() {
        let observation = fair_combat_observation(&state).map_err(fair_observation_error)?;
        let revision = DecisionRevision::new(accepted_decisions as u64);
        let choice_set = player_choices(&state, revision).map_err(fair_choice_error)?;
        if choice_set.choices.is_empty() {
            return Err(DecisionUnavailableError::new_err(
                "beam clone reached an empty ongoing public decision",
            ));
        }
        let teacher = sts_search::beam_teacher_decision(
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
        player_turns = player_turns.saturating_add(player_turn_advances(&state, &action, &next));
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
        if player_turns > max_player_turns {
            truncation_trigger = Some("player_turns");
            state = next;
            break;
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
        teacher_name: "public_decision_replanning_beam",
        teacher_version: "replan_each_public_decision_v2",
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

#[derive(serde::Serialize)]
struct PuctSearchWire {
    teacher_name: &'static str,
    teacher_version: &'static str,
    selected_index: usize,
    visits: Vec<u64>,
    priors: Vec<f64>,
    value: f64,
    transitions: usize,
    completed_simulations: usize,
    unique_evaluations: usize,
    budget_exhausted: bool,
    choices: Vec<PlayerChoice>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FairLeafBatchResponse {
    schema: String,
    batch: Vec<FairLeafBatchItem>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FairLeafBatchItem {
    priors: Vec<f64>,
    value: f64,
}

struct CallbackLeafEvaluator {
    callback: Py<PyAny>,
    error: Option<PyErr>,
}

impl FairLeafEvaluator for CallbackLeafEvaluator {
    fn evaluate(
        &mut self,
        observation: &FairCombatObservation,
        choices: &[PlayerChoice],
    ) -> Result<FairLeafEvaluation, String> {
        Python::attach(|py| {
            let request = serde_json::json!({
                "schema": FAIR_LEAF_BATCH_SCHEMA,
                "batch": [{
                    "observation": observation,
                    "choices": choices,
                }],
            });
            let request_json =
                serde_json::to_string(&request).map_err(|error| error.to_string())?;
            let result = match self.callback.bind(py).call1((request_json,)) {
                Ok(result) => result,
                Err(error) => {
                    self.error = Some(error);
                    return Err("python evaluator failed".to_owned());
                }
            };
            let response_json: String = match result.extract::<String>() {
                Ok(response) => response,
                Err(error) => {
                    self.error = Some(error);
                    return Err("python evaluator failed".to_owned());
                }
            };
            parse_fair_leaf_response(&response_json)
        })
    }
}

fn parse_fair_leaf_response(response_json: &str) -> Result<FairLeafEvaluation, String> {
    let response: FairLeafBatchResponse =
        serde_json::from_str(response_json).map_err(|error| error.to_string())?;
    if response.schema != FAIR_LEAF_BATCH_SCHEMA {
        return Err(format!(
            "unsupported fair leaf response schema {}",
            response.schema
        ));
    }
    if response.batch.len() != 1 {
        return Err("naive PUCT evaluator requires batch size 1".to_owned());
    }
    Ok(FairLeafEvaluation {
        priors: response.batch[0].priors.clone(),
        value: response.batch[0].value,
    })
}

fn parse_reward_config(reward_config_json: Option<&str>) -> Result<CombatProxyConfig, String> {
    match reward_config_json {
        None | Some("") => {
            let config = CombatProxyConfig::default();
            config.validate().map_err(|error| error.to_string())?;
            Ok(config)
        }
        Some(json) => {
            let config: CombatProxyConfig =
                serde_json::from_str(json).map_err(|error| error.to_string())?;
            config.validate().map_err(|error| error.to_string())?;
            Ok(config)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn puct_search_json(
    root: &RunState,
    evaluator: Py<PyAny>,
    c_puct: f64,
    simulation_budget: usize,
    transition_budget: usize,
    reward_config_json: Option<&str>,
    episode_root_max_hp: Option<i32>,
    episode_root_gold: Option<i32>,
) -> PyResult<String> {
    let reward = parse_reward_config(reward_config_json).map_err(PyValueError::new_err)?;
    let (_, state_max_hp) = run_player_hp(root);
    let config = PuctConfig {
        c_puct,
        simulation_budget,
        transition_budget,
        reward,
        episode_root_max_hp: episode_root_max_hp.unwrap_or(state_max_hp),
        episode_root_gold: episode_root_gold.unwrap_or(root.gold),
    };
    let mut evaluator = CallbackLeafEvaluator {
        callback: evaluator,
        error: None,
    };
    let result = match puct_search(root, &config, &mut evaluator) {
        Ok(result) => result,
        Err(error) => {
            if let Some(callback_error) = evaluator.error.take() {
                return Err(callback_error);
            }
            return Err(puct_error(error));
        }
    };
    to_json(&PuctSearchWire {
        teacher_name: PRIVILEGED_PUCT_TEACHER_NAME,
        teacher_version: PRIVILEGED_PUCT_TEACHER_VERSION,
        selected_index: result.selected_index,
        visits: result.visits,
        priors: result.priors,
        value: result.value,
        transitions: result.transitions,
        completed_simulations: result.completed_simulations,
        unique_evaluations: result.unique_evaluations,
        budget_exhausted: result.budget_exhausted,
        choices: result.choices,
    })
}

fn puct_error(error: sts_search::PuctError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn apply_public_run_action(
    env: &mut PyOmniRunEnv,
    action: &PyAction,
) -> PyResult<(Option<&'static str>, usize)> {
    if action.revision != env.revision {
        return Err(StaleDecisionError::new_err("public run decision is stale"));
    }
    let is_currently_legal = public_run_actions(&env.state, env.revision)?
        .iter()
        .any(|candidate| candidate.action == action.action);
    if !is_currently_legal {
        return Err(InvalidChoiceError::new_err("public run action is invalid"));
    }
    let next = apply_exact_run_action(&env.state, &action.action)
        .map_err(|_| InvalidChoiceError::new_err("public run action is invalid"))?;
    let combat_outcome = if env.state.phase == RunPhase::Combat {
        classify_combat_episode_transition(&env.state, &action.action, &next)
    } else {
        None
    };
    let player_turn_advances = player_turn_advances(&env.state, &action.action, &next);
    let revision = env
        .revision
        .checked_next()
        .ok_or_else(|| PyRuntimeError::new_err("public decision revision exhausted"))?;
    env.state = next;
    env.revision = revision;
    Ok((combat_outcome, player_turn_advances))
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
    use sts_core::content::cards::{
        BASH_ID, CONCLUDE_ANY_COLOR_ID, DEFEND_R_ID, STRIKE_R_ID, THINKING_AHEAD_ID,
    };
    use sts_core::{CardInstance, Potion};

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

    #[test]
    fn terminal_cleanup_does_not_create_a_second_combat_outcome() {
        let mut lost = RunState::combat_fixture();
        let combat = lost.combat.as_mut().expect("combat fixture");
        combat.phase = CombatPhase::Lost;
        combat.player.hp = 0;
        lost.player_hp = 0;
        let after = lost.clone();

        assert_eq!(classify_combat_state(&lost), Some("lost"));
        assert_eq!(
            classify_combat_episode_transition(
                &lost,
                &RunDecisionAction::Run(RunAction::Proceed),
                &after,
            ),
            None
        );
    }

    #[test]
    fn a_dead_player_leaving_combat_is_not_classified_as_a_win() {
        let before = RunState::combat_fixture();
        let mut after = before.clone();
        after.combat = None;
        after.phase = RunPhase::Reward;
        after.player_hp = 0;

        assert_eq!(
            classify_combat_episode_transition(
                &before,
                &RunDecisionAction::Combat(CombatAction::EndTurn),
                &after,
            ),
            Some("lost")
        );
    }

    #[test]
    fn forced_time_warp_turn_evidence_covers_card_and_selection_paths() {
        let mut before_card = RunState::combat_fixture();
        let before_combat = before_card.combat.as_mut().expect("combat fixture");
        before_combat.monsters[0].content_id = sts_core::content::monsters::TIME_EATER_ID;
        before_combat.monsters[0].powers.time_warp = 11;
        let mut after_card = before_card.clone();
        after_card.combat.as_mut().expect("combat fixture").monsters[0]
            .powers
            .time_warp = 0;
        assert_eq!(
            player_turn_advances(
                &before_card,
                &RunDecisionAction::Combat(CombatAction::PlayCard {
                    card_id: sts_core::CardId::new(1),
                    target: None,
                }),
                &after_card,
            ),
            1
        );

        let mut before_selection = RunState::combat_fixture();
        before_selection
            .combat
            .as_mut()
            .expect("combat fixture")
            .time_warp_end_turn = true;
        let mut after_selection = before_selection.clone();
        after_selection
            .combat
            .as_mut()
            .expect("combat fixture")
            .time_warp_end_turn = false;
        assert_eq!(
            player_turn_advances(
                &before_selection,
                &RunDecisionAction::GridConfirm,
                &after_selection,
            ),
            1
        );
    }

    #[test]
    fn conclude_play_is_a_forced_turn_source() {
        let mut before = RunState::combat_fixture();
        let before_combat = before.combat.as_mut().expect("combat fixture");
        before_combat.player.energy = 1;
        before_combat.piles.hand = vec![CardInstance::new(CardId::new(100), CONCLUDE_ANY_COLOR_ID)];
        let after = before.clone();
        assert_eq!(
            player_turn_advances(
                &before,
                &RunDecisionAction::Combat(CombatAction::PlayCard {
                    card_id: CardId::new(100),
                    target: None,
                }),
                &after,
            ),
            1
        );
    }

    #[test]
    fn step_action_json_reports_the_same_forced_turn_delta_as_beam_clone() {
        let mut before = RunState::combat_fixture();
        {
            let combat = before.combat.as_mut().expect("combat fixture");
            combat.player.energy = 3;
            combat.piles.hand = vec![CardInstance::new(CardId::new(100), CONCLUDE_ANY_COLOR_ID)];
            combat.monsters[0].hp = 100;
            combat.monsters[0].max_hp = 100;
        }
        let mut env = PyOmniRunEnv {
            state: before.clone(),
            revision: DecisionRevision::new(0),
        };
        let conclude = env
            .legal_actions()
            .expect("public actions")
            .into_iter()
            .find(|action| action.kind() == "play_hand_slot")
            .expect("Conclude is playable");
        let payload = env
            .step_action_json(&conclude)
            .expect("public Conclude applies");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("step JSON");
        assert_eq!(
            parsed["player_turn_advances"],
            player_turn_advances(&before, &conclude.action, &env.state)
        );
        assert_eq!(parsed["player_turn_advances"], 1);
    }

    #[test]
    fn thinking_ahead_beam_clone_does_not_toggle_until_truncation() {
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.player.energy = 3;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), THINKING_AHEAD_ID),
                CardInstance::new(CardId::new(2), STRIKE_R_ID),
                CardInstance::new(CardId::new(3), DEFEND_R_ID),
                CardInstance::new(CardId::new(4), BASH_ID),
                CardInstance::new(CardId::new(5), STRIKE_R_ID),
            ];
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(6), STRIKE_R_ID),
                CardInstance::new(CardId::new(7), DEFEND_R_ID),
            ];
        }
        let opened = apply_run_decision_action(
            &run,
            RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            }),
        )
        .expect("Thinking Ahead opens hand select");
        assert!(opened
            .combat
            .as_ref()
            .and_then(|combat| combat.hand_select())
            .is_some());

        let payload = beam_clone_episode_json(&opened, 2, 8, 500, 8, 100, false)
            .expect("beam clone from Thinking Ahead select");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("episode JSON");
        assert_eq!(parsed["teacher_version"], "replan_each_public_decision_v2");
        let steps = parsed["steps"].as_array().expect("steps array");
        let selected_kinds: Vec<&str> = steps
            .iter()
            .map(|step| {
                let index = step["selected_index"].as_u64().expect("selected_index") as usize;
                step["choices"][index]["kind"]
                    .as_str()
                    .expect("choice kind")
            })
            .collect();
        let toggle_count = selected_kinds
            .iter()
            .filter(|kind| **kind == "toggle_visible_card")
            .count();
        assert!(
            toggle_count <= 1,
            "Thinking Ahead beam clone retargeted: {selected_kinds:?}"
        );
        assert!(
            selected_kinds.contains(&"confirm_selection"),
            "Thinking Ahead beam clone never confirmed: {selected_kinds:?}"
        );
    }
}
