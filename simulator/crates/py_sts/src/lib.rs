use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sts_core::combat::ExhaustSelectPurpose;
use sts_core::{
    apply_combat_action_on_run, apply_combat_action_with_events, apply_event_action,
    apply_map_action_on_run, apply_rest_action, apply_run_action as apply_core_run_action,
    cancel_grid, confirm_grid, leave_shop_room, legal_combat_actions, legal_event_actions,
    legal_map_actions_on_run, legal_rest_actions, legal_shop_actions, select_grid_card,
    validate_potion_action, CardId, CombatAction, CombatPhase, CombatState, EventAction, MapAction,
    MonsterId, MonsterIntent, Potion, RestAction, RunAction, RunPhase, RunState, Snapshot,
    SNAPSHOT_SCHEMA_VERSION,
};

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

#[derive(Clone)]
enum ExactRunActionKind {
    Combat(CombatAction),
    Event(EventAction),
    GridSelect { index: usize },
    GridConfirm,
    GridCancel,
    LeaveShopRoom,
    Map(MapAction),
    Rest(RestAction),
    Run(RunAction),
}

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
    pub fn from_state_json(json: &str) -> PyResult<Self> {
        let state = serde_json::from_str(json).map_err(|error| {
            PyValueError::new_err(format!("invalid combat state JSON: {error}"))
        })?;
        Ok(Self { state })
    }

    #[staticmethod]
    pub fn from_snapshot_json(json: &str) -> PyResult<Self> {
        let snapshot: Snapshot<CombatState> = serde_json::from_str(json).map_err(|error| {
            PyValueError::new_err(format!("invalid combat snapshot JSON: {error}"))
        })?;
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(PyValueError::new_err(format!(
                "unsupported snapshot schema version: expected {}, got {}",
                SNAPSHOT_SCHEMA_VERSION, snapshot.schema_version
            )));
        }
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

    pub fn exact_legal_actions(&self) -> Vec<PyExactCombatAction> {
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
        let transition = apply_combat_action_with_events(&self.state, action.action.clone())
            .map_err(|error| {
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
            exact_legal_actions: self.exact_legal_actions(),
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
}

#[pymethods]
impl PyOmniRunEnv {
    #[staticmethod]
    pub fn combat_fixture() -> Self {
        Self {
            state: RunState::combat_fixture(),
        }
    }

    #[staticmethod]
    pub fn map_fixture() -> Self {
        Self {
            state: RunState::map_fixture(),
        }
    }

    #[staticmethod]
    pub fn new_ironclad(seed: Option<&str>, ascension: Option<u8>) -> PyResult<Self> {
        let ascension = ascension.unwrap_or(0);
        let state = match seed {
            Some(seed) => RunState::placeholder_seeded_ironclad(stable_seed(seed), ascension),
            None => RunState::combat_fixture_with_ascension(ascension),
        };
        Ok(Self { state })
    }

    #[staticmethod]
    pub fn from_state_json(json: &str) -> PyResult<Self> {
        let state = serde_json::from_str(json)
            .map_err(|error| PyValueError::new_err(format!("invalid run state JSON: {error}")))?;
        Ok(Self { state })
    }

    #[staticmethod]
    pub fn from_communication_mod_state_json(json: &str) -> PyResult<Self> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|error| {
            PyValueError::new_err(format!("invalid CommunicationMod state JSON: {error}"))
        })?;
        let message = if value.get("game_state").is_some() {
            value
        } else {
            serde_json::json!({ "game_state": value })
        };
        let state = sts_verify::run_state_from_observed_message(&message).ok_or_else(|| {
            PyValueError::new_err("CommunicationMod state is not a supported observed state")
        })?;
        Ok(Self { state })
    }

    #[staticmethod]
    pub fn from_snapshot_json(json: &str) -> PyResult<Self> {
        let snapshot: Snapshot<RunState> = serde_json::from_str(json).map_err(|error| {
            PyValueError::new_err(format!("invalid run snapshot JSON: {error}"))
        })?;
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(PyValueError::new_err(format!(
                "unsupported snapshot schema version: expected {}, got {}",
                SNAPSHOT_SCHEMA_VERSION, snapshot.schema_version
            )));
        }
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
        run_snapshot(&self.state).canonical_json().map_err(|error| {
            PyRuntimeError::new_err(format!("snapshot serialization failed: {error:?}"))
        })
    }

    pub fn snapshot_hash(&self) -> PyResult<String> {
        run_snapshot_hash(&self.state)
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

    pub fn exact_legal_actions(&self) -> Vec<PyExactRunAction> {
        exact_run_legal_actions(&self.state)
    }

    pub fn step(&mut self, action: &PyExactRunAction) -> PyResult<PyExactRunStepResult> {
        let previous_hash = run_snapshot_hash(&self.state)?;
        let action_json = run_action_json(&action.action)?;
        let next = apply_exact_run_action(&self.state, &action.action).map_err(|error| {
            PyValueError::new_err(format!("illegal exact run action: {error:?}"))
        })?;
        let resulting_hash = run_snapshot_hash(&next)?;

        self.state = next;

        Ok(PyExactRunStepResult {
            state_json: self.state_json()?,
            snapshot_json: self.snapshot_json()?,
            snapshot_hash: resulting_hash.clone(),
            phase: self.phase(),
            current_decision: self.current_decision(),
            exact_legal_actions: self.exact_legal_actions(),
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
            "OmniRunEnv(phase={}, snapshot_hash={})",
            self.phase(),
            self.snapshot_hash()?
        ))
    }
}

#[pymodule]
fn sts_omni(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyExactCombatAction>()?;
    module.add_class::<PyExactRunAction>()?;
    module.add_class::<PyDebugTransition>()?;
    module.add_class::<PyExactStepResult>()?;
    module.add_class::<PyExactRunStepResult>()?;
    module.add_class::<PyRustSearchRecommendation>()?;
    module.add_class::<PyOmniCombatEnv>()?;
    module.add_class::<PyOmniRunEnv>()?;
    Ok(())
}

fn exact_legal_actions(state: &CombatState) -> Vec<PyExactCombatAction> {
    if is_terminal(state.phase) {
        return Vec::new();
    }
    legal_combat_actions(state)
        .into_iter()
        .map(|action| PyExactCombatAction { action })
        .collect()
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

fn stable_seed(seed: &str) -> u64 {
    if let Ok(value) = seed.parse::<u64>() {
        return value;
    }
    sts_verify::sts_seed_string_to_long(seed) as u64
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

fn exact_run_legal_actions(state: &RunState) -> Vec<PyExactRunAction> {
    exact_run_legal_action_kinds(state)
        .into_iter()
        .map(|action| PyExactRunAction { action })
        .collect()
}

fn exact_run_legal_action_kinds(state: &RunState) -> Vec<ExactRunActionKind> {
    let mut actions = Vec::new();

    if let Some(grid) = state.card_grid.as_ref() {
        actions.extend(
            (0..grid.cards.len())
                .filter(|index| select_grid_card(state, *index).is_ok())
                .map(|index| ExactRunActionKind::GridSelect { index }),
        );
        if confirm_grid(state).is_ok() {
            actions.push(ExactRunActionKind::GridConfirm);
        }
        if cancel_grid(state).is_ok() {
            actions.push(ExactRunActionKind::GridCancel);
        }
        return actions;
    }

    if state.phase == RunPhase::Combat {
        if let Some(combat) = state.combat.as_ref() {
            let select_actions = legal_combat_select_actions_on_run(state, combat);
            if !select_actions.is_empty() {
                actions.extend(select_actions.into_iter().map(ExactRunActionKind::Run));
                return actions;
            }
            let combat_actions = legal_combat_actions(combat)
                .into_iter()
                .map(ExactRunActionKind::Combat);
            if combat.duplication_potion_pending {
                actions.extend(
                    combat_actions.filter(|action| apply_exact_run_action(state, action).is_ok()),
                );
            } else {
                actions.extend(combat_actions);
            }
            actions.extend(
                legal_potion_actions_on_run(state)
                    .into_iter()
                    .map(ExactRunActionKind::Run),
            );
        }
    }

    if state.phase == RunPhase::Reward {
        actions.extend(
            legal_reward_actions(state)
                .into_iter()
                .map(ExactRunActionKind::Run),
        );
        actions.extend(
            legal_potion_actions_on_run(state)
                .into_iter()
                .map(ExactRunActionKind::Run),
        );
    }

    if state.phase == RunPhase::Treasure {
        for action in [RunAction::OpenChest, RunAction::Proceed] {
            if apply_core_run_action(state, action).is_ok() {
                actions.push(ExactRunActionKind::Run(action));
            }
        }
    }

    if state.phase == RunPhase::Idle {
        actions.extend(
            legal_map_actions_on_run(state)
                .into_iter()
                .map(ExactRunActionKind::Map),
        );
    }

    if state.phase == RunPhase::Rest {
        actions.extend(
            legal_rest_actions(state)
                .into_iter()
                .map(ExactRunActionKind::Rest),
        );
    }

    if state.phase == RunPhase::Event {
        actions.extend(
            legal_event_actions(state)
                .into_iter()
                .map(ExactRunActionKind::Event),
        );
    }

    if state.phase == RunPhase::Shop {
        actions.extend(
            legal_shop_actions(state)
                .into_iter()
                .map(ExactRunActionKind::Run),
        );
        if state.shop.is_none() && state.card_grid.is_none() {
            actions.push(ExactRunActionKind::LeaveShopRoom);
        }
    }

    actions
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
        let actions = filtered_run_actions(&current, allowed_potions.as_deref());
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
            best_first_action = Some(action.clone());
        }
        principal_variation.push(action.clone());
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
            let actions = filtered_run_actions(&node.state, allowed_potions.as_deref());
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
                principal_variation.push(action.clone());
                let child = RustBeamNode {
                    state: next_state,
                    first_action: node.first_action.clone().or_else(|| Some(action)),
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
) -> Vec<ExactRunActionKind> {
    let actions: Vec<_> = exact_run_legal_action_kinds(state)
        .into_iter()
        .filter(|action| rust_action_allowed(state, action, allowed_potions))
        .collect();
    preferred_select_actions(state, &actions).unwrap_or(actions)
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
        return Some(vec![confirm.clone()]);
    }
    if let Some(action) = preferred_bad_exhaust_action(state, actions) {
        return Some(vec![action]);
    }
    Some(vec![confirm.clone()])
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
    let Some(select) = combat.exhaust_select.as_ref() else {
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
    let before = combat.exhaust_select.as_ref()?;
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
        let Some(after) = next_combat.exhaust_select.as_ref() else {
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
            return Some(action.clone());
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
        ExactRunActionKind::Run(RunAction::UsePotion { .. }) => 5_000.0,
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
    let player_hp = f64::from(combat.player.hp);
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
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrail { damage, .. }
        | MonsterIntent::AttackHealSelf { damage }
        | MonsterIntent::AttackAddWoundsToDiscard { damage, .. }
        | MonsterIntent::AttackAddSlimedToDiscard { damage, .. }
        | MonsterIntent::AttackStealGold { damage, .. } => damage,
        MonsterIntent::AttackMultiple { damage, hits } => damage * hits,
        MonsterIntent::AddBurnToDiscard { damage, .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => damage,
        _ => 0,
    }
}

fn legal_combat_select_actions_on_run(state: &RunState, combat: &CombatState) -> Vec<RunAction> {
    if let Some(choices) = combat
        .potion_card_reward
        .as_ref()
        .or(combat.toolbox_card_reward.as_ref())
        .or(combat.discovery_card_reward.as_ref())
    {
        return (0..choices.len())
            .map(|index| RunAction::ChooseCombatCardReward { index })
            .filter(|action| apply_core_run_action(state, *action).is_ok())
            .collect();
    }

    let mut candidates = Vec::new();
    if combat.hand_select.is_some() {
        candidates.extend(
            (0..combat.piles.hand.len()).map(|index| RunAction::ChooseHandSelect { index }),
        );
        candidates.push(RunAction::ConfirmHandSelect);
    }
    if combat.draw_select.is_some() {
        candidates.extend(
            (0..combat.piles.draw_pile.len()).map(|index| RunAction::ChooseDrawSelect { index }),
        );
        candidates.push(RunAction::ConfirmDrawSelect);
    }
    if combat.discard_select.is_some() {
        candidates.extend(
            (0..combat.piles.discard_pile.len())
                .map(|index| RunAction::ChooseDiscardSelect { index }),
        );
        candidates.push(RunAction::ConfirmDiscardSelect);
    }
    if combat.exhaust_select.is_some() {
        candidates.extend(
            (0..combat.piles.hand.len()).map(|index| RunAction::ChooseExhaustSelect { index }),
        );
        candidates.push(RunAction::ConfirmExhaustSelect);
    }
    candidates
        .into_iter()
        .filter(|action| apply_core_run_action(state, *action).is_ok())
        .collect()
}

fn legal_reward_actions(state: &RunState) -> Vec<RunAction> {
    let mut candidates = vec![
        RunAction::SkipReward,
        RunAction::CloseCardReward,
        RunAction::TakeGoldReward,
        RunAction::TakeStolenGoldReward,
        RunAction::TakePotionReward,
        RunAction::TakeRelicReward,
        RunAction::Proceed,
        RunAction::OpenCardReward,
        RunAction::SkipPotionReward,
        RunAction::TakeSingingBowlReward,
    ];
    if let Some(reward) = state.reward.as_ref() {
        candidates.extend(
            (0..reward.boss_relic_choices.len())
                .map(|index| RunAction::ChooseBossRelicReward { index }),
        );
        candidates.extend(
            reward
                .choices
                .iter()
                .map(|choice| RunAction::TakeCardReward { card_id: choice.id }),
        );
    }
    candidates
        .into_iter()
        .filter(|action| state.validate_reward_action(*action).is_ok())
        .collect()
}

fn legal_potion_actions_on_run(state: &RunState) -> Vec<RunAction> {
    state
        .occupied_potion_slots()
        .into_iter()
        .flat_map(|(slot, potion)| potion_use_candidates(slot, potion, state.combat.as_ref()))
        .filter(|action| validate_potion_action(state, *action).is_ok())
        .collect()
}

fn potion_use_candidates(
    slot: usize,
    potion: Potion,
    combat: Option<&CombatState>,
) -> Vec<RunAction> {
    if potion.requires_target() {
        let Some(combat) = combat else {
            return Vec::new();
        };
        return combat
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| RunAction::UsePotion {
                slot,
                target: Some(monster.id),
            })
            .collect();
    }
    vec![RunAction::UsePotion { slot, target: None }]
}

fn apply_exact_run_action(
    state: &RunState,
    action: &ExactRunActionKind,
) -> sts_core::SimResult<RunState> {
    match action {
        ExactRunActionKind::Combat(action) => apply_combat_action_on_run(state, action.clone()),
        ExactRunActionKind::Event(action) => apply_event_action(state, *action),
        ExactRunActionKind::GridSelect { index } => select_grid_card(state, *index),
        ExactRunActionKind::GridConfirm => confirm_grid(state),
        ExactRunActionKind::GridCancel => cancel_grid(state),
        ExactRunActionKind::LeaveShopRoom => {
            let mut next = state.clone();
            leave_shop_room(&mut next);
            Ok(next)
        }
        ExactRunActionKind::Map(action) => apply_map_action_on_run(state, *action),
        ExactRunActionKind::Rest(action) => apply_rest_action(state, *action),
        ExactRunActionKind::Run(action) => apply_core_run_action(state, *action),
    }
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
        ExactRunActionKind::LeaveShopRoom => to_json(&serde_json::json!("LeaveShopRoom")),
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
        ExactRunActionKind::LeaveShopRoom => "shop",
        ExactRunActionKind::Map(_) => "map",
        ExactRunActionKind::Rest(_) => "rest",
        ExactRunActionKind::Run(_) => "run",
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
        ExactRunActionKind::LeaveShopRoom => "leave_shop_room",
        ExactRunActionKind::Map(MapAction::ChooseNode { .. }) => "choose_map_node",
        ExactRunActionKind::Rest(RestAction::Heal) => "rest_heal",
        ExactRunActionKind::Rest(RestAction::OpenSmith) => "rest_open_smith",
        ExactRunActionKind::Rest(RestAction::Smith { .. }) => "rest_smith",
        ExactRunActionKind::Rest(RestAction::RemoveCard { .. }) => "rest_remove_card",
        ExactRunActionKind::Rest(RestAction::Lift) => "rest_lift",
        ExactRunActionKind::Rest(RestAction::Dig) => "rest_dig",
        ExactRunActionKind::Rest(RestAction::Proceed) => "rest_proceed",
        ExactRunActionKind::Run(RunAction::SkipReward) => "skip_reward",
        ExactRunActionKind::Run(RunAction::CloseCardReward) => "close_card_reward",
        ExactRunActionKind::Run(RunAction::TakeCardReward { .. }) => "take_card_reward",
        ExactRunActionKind::Run(RunAction::TakeSingingBowlReward) => "take_singing_bowl_reward",
        ExactRunActionKind::Run(RunAction::TakeGoldReward) => "take_gold_reward",
        ExactRunActionKind::Run(RunAction::TakeStolenGoldReward) => "take_stolen_gold_reward",
        ExactRunActionKind::Run(RunAction::TakePotionReward) => "take_potion_reward",
        ExactRunActionKind::Run(RunAction::TakeRelicReward) => "take_relic_reward",
        ExactRunActionKind::Run(RunAction::ChooseBossRelicReward { .. }) => {
            "choose_boss_relic_reward"
        }
        ExactRunActionKind::Run(RunAction::Proceed) => "proceed",
        ExactRunActionKind::Run(RunAction::OpenChest) => "open_chest",
        ExactRunActionKind::Run(RunAction::OpenCardReward) => "open_card_reward",
        ExactRunActionKind::Run(RunAction::SkipPotionReward) => "skip_potion_reward",
        ExactRunActionKind::Run(RunAction::BuyShopCard { .. }) => "buy_shop_card",
        ExactRunActionKind::Run(RunAction::BuyShopRelic { .. }) => "buy_shop_relic",
        ExactRunActionKind::Run(RunAction::BuyShopPotion { .. }) => "buy_shop_potion",
        ExactRunActionKind::Run(RunAction::UsePotion { .. }) => "use_potion",
        ExactRunActionKind::Run(RunAction::DiscardPotion { .. }) => "discard_potion",
        ExactRunActionKind::Run(RunAction::ChooseCombatCardReward { .. }) => {
            "choose_combat_card_reward"
        }
        ExactRunActionKind::Run(RunAction::ChooseHandSelect { .. }) => "choose_hand_select",
        ExactRunActionKind::Run(RunAction::ConfirmHandSelect) => "confirm_hand_select",
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
    }
}

fn run_unsupported_reason(state: &RunState) -> Option<&'static str> {
    if exact_run_legal_actions(state).is_empty() {
        Some("no exact run legal-action adapter for current decision")
    } else {
        None
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
        let actions = env.exact_legal_actions();

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
        let actions = env.exact_legal_actions();
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
    fn run_combat_exact_actions_expose_exhaust_select_after_elixir() {
        let mut env = PyOmniRunEnv::combat_fixture();
        env.state.potions = vec![Potion::Elixir];
        let elixir = env
            .exact_legal_actions()
            .into_iter()
            .find(|action| action.kind() == "use_potion")
            .expect("elixir is usable")
            .clone();

        env.step(&elixir).expect("elixir opens exhaust select");
        let actions = env.exact_legal_actions();

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
        env.state.phase = RunPhase::Reward;
        env.state.combat = None;
        env.state.player_hp = 75;
        env.state.player_max_hp = 80;
        env.state.potions = vec![Potion::Attack, Potion::FruitJuice];

        let fruit_juice = env
            .exact_legal_actions()
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
            choices: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: Some(Potion::Ancient),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });
        env.state.potions.clear();

        let take_potion = env
            .exact_legal_actions()
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
            .iter()
            .any(|action| action.family() == "map"));
    }

    #[test]
    fn seed_start_constructor_uses_placeholder_generated_map() {
        let first = PyOmniRunEnv::new_ironclad(Some("TEST"), Some(0)).expect("seed starts");
        let second = PyOmniRunEnv::new_ironclad(Some("TEST"), Some(0)).expect("seed starts");
        let other = PyOmniRunEnv::new_ironclad(Some("OTHER"), Some(0)).expect("seed starts");

        assert_eq!(first.phase(), "idle");
        assert_eq!(first.current_decision(), "map");
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
            .iter()
            .any(|action| action.family() == "map"));
    }
}
