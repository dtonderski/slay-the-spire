use crate::{
    action::{CombatAction, EventAction, RestAction},
    map::MapAction,
    SimResult,
};
use serde::{Deserialize, Serialize};

use super::{
    apply_combat_action_on_run, apply_event_action, apply_map_action_on_run, apply_rest_action,
    apply_run_action, cancel_grid, confirm_grid, select_grid_card, RunAction, RunState,
};

/// One authoritative decision at any supported run boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunDecisionAction {
    Combat(CombatAction),
    Event(EventAction),
    GridSelect { index: usize },
    GridConfirm,
    GridCancel,
    Map(MapAction),
    Rest(RestAction),
    Run(RunAction),
}

/// Applies one top-level run decision and validates both sides of the boundary.
pub fn apply_run_decision_action(run: &RunState, action: RunDecisionAction) -> SimResult<RunState> {
    run.validate()?;
    let next = match action {
        RunDecisionAction::Combat(action) => apply_combat_action_on_run(run, action),
        RunDecisionAction::Event(action) => apply_event_action(run, action),
        RunDecisionAction::GridSelect { index } => select_grid_card(run, index),
        RunDecisionAction::GridConfirm => confirm_grid(run),
        RunDecisionAction::GridCancel => cancel_grid(run),
        RunDecisionAction::Map(action) => apply_map_action_on_run(run, action),
        RunDecisionAction::Rest(action) => apply_rest_action(run, action),
        RunDecisionAction::Run(action) => apply_run_action(run, action),
    }?;
    next.validate()?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{legal_map_actions_on_run, RunPhase, SimError};

    #[test]
    fn top_level_map_step_matches_the_specialized_transition() {
        let run = RunState::map_fixture();
        let action = legal_map_actions_on_run(&run)[0];

        assert_eq!(
            apply_run_decision_action(&run, RunDecisionAction::Map(action)),
            apply_map_action_on_run(&run, action)
        );
    }

    #[test]
    fn top_level_step_rejects_malformed_pre_state_before_routing() {
        let mut run = RunState::seeded_ironclad(22_079_335_079, 0);
        run.phase = RunPhase::Shop;
        run.event = None;
        run.shop = None;

        assert_eq!(
            apply_run_decision_action(&run, RunDecisionAction::Run(RunAction::Proceed)),
            Err(SimError::InvalidState("shop phase has no shop screen"))
        );
    }
}
