use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sts_core::{
    apply_combat_action_on_run, apply_combat_action_with_events, apply_event_action,
    apply_map_action_on_run, apply_rest_action, apply_run_action, legal_combat_actions,
    legal_event_actions, legal_map_actions_on_run, legal_rest_actions, legal_shop_actions, CardId,
    CombatAction, CombatPhase, CombatState, EventAction, MapAction, MonsterId, RestAction,
    RunAction, RunPhase, RunState, Snapshot, SNAPSHOT_SCHEMA_VERSION,
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
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
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
    let mut actions = Vec::new();

    if state.phase == RunPhase::Combat {
        if let Some(combat) = state.combat.as_ref() {
            actions.extend(
                legal_combat_actions(combat)
                    .into_iter()
                    .map(ExactRunActionKind::Combat),
            );
        }
    }

    if state.phase == RunPhase::Reward {
        actions.extend(
            legal_reward_actions(state)
                .into_iter()
                .map(ExactRunActionKind::Run),
        );
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
    }

    actions
        .into_iter()
        .map(|action| PyExactRunAction { action })
        .collect()
}

fn legal_reward_actions(state: &RunState) -> Vec<RunAction> {
    let mut candidates = vec![
        RunAction::SkipReward,
        RunAction::TakeGoldReward,
        RunAction::TakeStolenGoldReward,
        RunAction::TakePotionReward,
        RunAction::TakeRelicReward,
        RunAction::OpenCardReward,
        RunAction::SkipPotionReward,
        RunAction::TakeSingingBowlReward,
    ];
    if let Some(reward) = state.reward.as_ref() {
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

fn apply_exact_run_action(
    state: &RunState,
    action: &ExactRunActionKind,
) -> sts_core::SimResult<RunState> {
    match action {
        ExactRunActionKind::Combat(action) => apply_combat_action_on_run(state, action.clone()),
        ExactRunActionKind::Event(action) => apply_event_action(state, *action),
        ExactRunActionKind::Map(action) => apply_map_action_on_run(state, *action),
        ExactRunActionKind::Rest(action) => apply_rest_action(state, *action),
        ExactRunActionKind::Run(action) => apply_run_action(state, *action),
    }
}

fn run_action_json(action: &ExactRunActionKind) -> PyResult<String> {
    match action {
        ExactRunActionKind::Combat(action) => to_json(action),
        ExactRunActionKind::Event(action) => to_json(action),
        ExactRunActionKind::Map(action) => to_json(action),
        ExactRunActionKind::Rest(action) => to_json(action),
        ExactRunActionKind::Run(action) => to_json(action),
    }
}

fn run_action_family(action: &ExactRunActionKind) -> &'static str {
    match action {
        ExactRunActionKind::Combat(_) => "combat",
        ExactRunActionKind::Event(_) => "event",
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
        ExactRunActionKind::Map(MapAction::ChooseNode { .. }) => "choose_map_node",
        ExactRunActionKind::Rest(RestAction::Heal) => "rest_heal",
        ExactRunActionKind::Rest(RestAction::OpenSmith) => "rest_open_smith",
        ExactRunActionKind::Rest(RestAction::Smith { .. }) => "rest_smith",
        ExactRunActionKind::Rest(RestAction::RemoveCard { .. }) => "rest_remove_card",
        ExactRunActionKind::Rest(RestAction::Lift) => "rest_lift",
        ExactRunActionKind::Rest(RestAction::Dig) => "rest_dig",
        ExactRunActionKind::Run(RunAction::SkipReward) => "skip_reward",
        ExactRunActionKind::Run(RunAction::TakeCardReward { .. }) => "take_card_reward",
        ExactRunActionKind::Run(RunAction::TakeSingingBowlReward) => "take_singing_bowl_reward",
        ExactRunActionKind::Run(RunAction::TakeGoldReward) => "take_gold_reward",
        ExactRunActionKind::Run(RunAction::TakeStolenGoldReward) => "take_stolen_gold_reward",
        ExactRunActionKind::Run(RunAction::TakePotionReward) => "take_potion_reward",
        ExactRunActionKind::Run(RunAction::TakeRelicReward) => "take_relic_reward",
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
