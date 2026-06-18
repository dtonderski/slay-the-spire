use sts_core::{
    apply_map_action_on_run, apply_rest_action, content::character::IRONCLAD_A0_BASE_HP,
    legal_map_actions_on_run, legal_rest_actions, rest_heal_amount, MapAction, MapNodeId,
    RestAction, RoomKind, RunPhase, RunState, SimError,
};

#[test]
fn rest_heal_restores_thirty_percent_max_hp_floored() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.player_hp = 30;
    run.player_max_hp = 80;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

    assert_eq!(rest_heal_amount(80), 24);
    assert_eq!(after.player_hp, 54);
    assert_eq!(after.phase, RunPhase::Idle);
}

#[test]
fn rest_heal_caps_at_max_hp() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Rest;
    run.player_hp = IRONCLAD_A0_BASE_HP - 3;

    let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

    assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
}

#[test]
fn entering_rest_room_exposes_heal_and_blocks_map_actions() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;

    assert!(!legal_map_actions_on_run(&run).is_empty());

    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");

    assert_eq!(run.phase, RunPhase::Rest);
    assert_eq!(
        run.map
            .as_ref()
            .and_then(|map| map.map.node(map.current_node))
            .map(|node| node.room_kind),
        Some(RoomKind::Rest)
    );
    assert_eq!(legal_rest_actions(&run), vec![RestAction::Heal]);
    assert!(legal_map_actions_on_run(&run).is_empty());
}

#[test]
fn heal_then_map_traversal_continues() {
    let mut run = RunState::map_fixture();
    run.player_hp = 40;

    run = apply_map_action_on_run(
        &run,
        MapAction::ChooseNode {
            node_id: MapNodeId::new(2),
        },
    )
    .expect("enter rest");
    run = apply_rest_action(&run, RestAction::Heal).expect("heal");

    assert_eq!(run.phase, RunPhase::Idle);
    assert_eq!(run.player_hp, 64);
    assert_eq!(
        legal_map_actions_on_run(&run),
        vec![MapAction::ChooseNode {
            node_id: MapNodeId::new(3)
        }]
    );
}

#[test]
fn rest_heal_is_illegal_outside_rest_phase() {
    let run = RunState::map_fixture();

    let err = apply_rest_action(&run, RestAction::Heal).expect_err("not at rest");

    assert_eq!(
        err,
        SimError::IllegalAction("rest actions require rest phase")
    );
}
