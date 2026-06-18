use crate::{
    map::{apply_map_action, legal_map_actions, validate_map_action, MapAction, RoomKind},
    RunPhase, RunState, SimError, SimResult,
};

fn current_room_kind(run: &RunState) -> Option<RoomKind> {
    run.map.as_ref().and_then(|map_state| {
        map_state
            .map
            .node(map_state.current_node)
            .map(|node| node.room_kind)
    })
}

pub fn legal_map_actions_on_run(run: &RunState) -> Vec<MapAction> {
    if run.phase != RunPhase::Idle {
        return Vec::new();
    }

    run.map.as_ref().map(legal_map_actions).unwrap_or_default()
}

pub fn validate_map_action_on_run(run: &RunState, action: MapAction) -> SimResult<()> {
    if run.phase != RunPhase::Idle {
        return Err(SimError::IllegalAction("map actions require idle phase"));
    }

    let map_state = run
        .map
        .as_ref()
        .ok_or(SimError::InvalidState("map state is missing"))?;

    validate_map_action(map_state, action)
}

pub fn apply_map_action_on_run(run: &RunState, action: MapAction) -> SimResult<RunState> {
    validate_map_action_on_run(run, action)?;

    let map_state = run.map.as_ref().expect("validated map state");
    let next_map = apply_map_action(map_state, action)?;

    let mut next = run.clone();
    next.map = Some(next_map);

    if current_room_kind(&next) == Some(RoomKind::Rest) {
        next.phase = RunPhase::Rest;
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ids::MapNodeId, map::RoomKind};

    #[test]
    fn map_actions_require_idle_phase() {
        let run = RunState::combat_fixture();

        let err = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect_err("combat blocks map");

        assert_eq!(
            err,
            SimError::IllegalAction("map actions require idle phase")
        );
    }

    #[test]
    fn entering_rest_node_transitions_to_rest_phase() {
        let run = RunState::map_fixture();

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("choose rest node");

        assert_eq!(next.phase, RunPhase::Rest);
        assert_eq!(
            next.map.as_ref().expect("map").current_node,
            MapNodeId::new(2)
        );
        assert_eq!(
            next.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Rest)
        );
    }

    #[test]
    fn entering_combat_node_stays_idle() {
        let run = RunState::map_fixture();

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("choose combat node");

        assert_eq!(next.phase, RunPhase::Idle);
    }
}
