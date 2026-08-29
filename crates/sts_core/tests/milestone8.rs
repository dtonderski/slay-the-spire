mod support;

use sts_core::{
    apply_map_action, legal_map_actions, reachable_nodes, MapAction, MapNodeId, MapRunState,
    RoomKind, RunState,
};
use support::generate_placeholder_map;

#[test]
fn milestone8_fixture_has_seven_nodes_with_expected_kinds() {
    let map = RunState::map_fixture().map.expect("fixture map").map;

    assert_eq!(map.nodes.len(), 7);
    assert_eq!(
        map.node(MapNodeId::new(0)).unwrap().room_kind,
        RoomKind::Combat
    );
    assert_eq!(
        map.node(MapNodeId::new(2)).unwrap().room_kind,
        RoomKind::Rest
    );
    assert_eq!(
        map.node(MapNodeId::new(6)).unwrap().room_kind,
        RoomKind::Boss
    );
}

#[test]
fn full_map_traversal_via_rest_branch_reaches_boss_at_floor_six() {
    let mut state = RunState::map_fixture().map.expect("fixture map state");
    let trace = [
        MapNodeId::new(2),
        MapNodeId::new(3),
        MapNodeId::new(4),
        MapNodeId::new(5),
        MapNodeId::new(6),
    ];

    for node_id in trace {
        let action = MapAction::ChooseNode { node_id };
        assert!(legal_map_actions(&state)
            .expect("valid fixture map")
            .contains(&action));
        state = apply_map_action(&state, action).expect("trace step applies");
    }

    assert_eq!(state.floor, 5);
    assert_eq!(state.act, 1);
    assert_eq!(state.current_node, MapNodeId::new(6));
    assert!(reachable_nodes(&state)
        .expect("valid terminal map state")
        .is_empty());
}

#[test]
fn generated_map_placeholder_is_deterministic_and_traversable() {
    let (map_a, _) = generate_placeholder_map(17, 0);
    let (map_b, _) = generate_placeholder_map(17, 0);

    assert_eq!(map_a, map_b);
    assert_eq!(map_a.nodes.len(), 7);

    let mut state = MapRunState {
        act: 1,
        floor: 0,
        current_node: MapNodeId::new(0),
        map: map_a,
    };
    let path = [
        MapNodeId::new(2),
        MapNodeId::new(3),
        MapNodeId::new(4),
        MapNodeId::new(5),
        MapNodeId::new(6),
    ];

    for node_id in path {
        let action = MapAction::ChooseNode { node_id };
        assert!(legal_map_actions(&state)
            .expect("valid generated map")
            .contains(&action));
        state = apply_map_action(&state, action).expect("generated map step applies");
    }

    assert_eq!(state.current_node, MapNodeId::new(6));
    assert_eq!(
        state.map.node(state.current_node).unwrap().room_kind,
        RoomKind::Boss
    );
}
