use crate::{RestAction, RunPhase, RunState, SimError, SimResult};

pub const REST_HEAL_PERCENT: i32 = 30;

#[must_use]
pub fn rest_heal_amount(max_hp: i32) -> i32 {
    max_hp * REST_HEAL_PERCENT / 100
}

#[must_use]
pub fn legal_rest_actions(run: &RunState) -> Vec<RestAction> {
    if run.phase != RunPhase::Rest {
        return Vec::new();
    }

    vec![RestAction::Heal]
}

pub fn validate_rest_action(run: &RunState, action: RestAction) -> SimResult<()> {
    if run.phase != RunPhase::Rest {
        return Err(SimError::IllegalAction("rest actions require rest phase"));
    }

    match action {
        RestAction::Heal if legal_rest_actions(run).contains(&action) => Ok(()),
        RestAction::Heal => Err(SimError::IllegalAction("heal is not available")),
    }
}

pub fn apply_rest_action(run: &RunState, action: RestAction) -> SimResult<RunState> {
    validate_rest_action(run, action)?;

    let mut next = run.clone();
    match action {
        RestAction::Heal => {
            let heal = rest_heal_amount(next.player_max_hp);
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.phase = RunPhase::Idle;
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::character::IRONCLAD_A0_BASE_HP, map::RoomKind, RunState};

    #[test]
    fn rest_heal_amount_floors_thirty_percent_of_max_hp() {
        assert_eq!(rest_heal_amount(80), 24);
        assert_eq!(rest_heal_amount(79), 23);
    }

    #[test]
    fn heal_caps_at_max_hp() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = IRONCLAD_A0_BASE_HP - 10;

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
        assert_eq!(after.phase, RunPhase::Idle);
    }

    #[test]
    fn heal_does_not_exceed_max_hp_when_near_full() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = IRONCLAD_A0_BASE_HP - 5;

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
    }

    #[test]
    fn rest_action_is_illegal_outside_rest_phase() {
        let run = RunState::map_fixture();

        let err = apply_rest_action(&run, RestAction::Heal).expect_err("not at rest");

        assert_eq!(
            err,
            SimError::IllegalAction("rest actions require rest phase")
        );
    }

    #[test]
    fn entering_rest_room_exposes_heal_action() {
        use crate::{apply_map_action_on_run, legal_rest_actions, MapAction, MapNodeId};

        let mut run = RunState::map_fixture();
        run.player_hp = 40;

        run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("enter rest room");

        assert_eq!(run.phase, RunPhase::Rest);
        assert_eq!(
            run.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Rest)
        );
        assert_eq!(legal_rest_actions(&run), vec![RestAction::Heal]);
    }
}
