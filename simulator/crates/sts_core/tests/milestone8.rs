use sts_core::{
    apply_map_action, legal_map_actions, milestone8_fixture, milestone8_map, reachable_nodes,
    MapAction, MapNodeId, RoomKind,
};

#[test]
fn milestone8_fixture_has_seven_nodes_with_expected_kinds() {
    let map = milestone8_map();

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
    let mut state = milestone8_fixture();
    let trace = [
        MapNodeId::new(2),
        MapNodeId::new(3),
        MapNodeId::new(4),
        MapNodeId::new(5),
        MapNodeId::new(6),
    ];

    for node_id in trace {
        let action = MapAction::ChooseNode { node_id };
        assert!(legal_map_actions(&state).contains(&action));
        state = apply_map_action(&state, action).expect("trace step applies");
    }

    assert_eq!(state.floor, 5);
    assert_eq!(state.act, 1);
    assert_eq!(state.current_node, MapNodeId::new(6));
    assert!(reachable_nodes(&state).is_empty());
}
