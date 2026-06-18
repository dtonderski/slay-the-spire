use crate::{
    error::{SimError, SimResult},
    ids::MapNodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomKind {
    Combat,
    Rest,
    Boss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapNode {
    pub id: MapNodeId,
    pub act: u8,
    pub room_kind: RoomKind,
    pub children: Vec<MapNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedMap {
    pub nodes: Vec<MapNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRunState {
    pub act: u8,
    pub floor: u32,
    pub current_node: MapNodeId,
    pub map: FixedMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapAction {
    ChooseNode { node_id: MapNodeId },
}

impl FixedMap {
    pub fn node(&self, id: MapNodeId) -> Option<&MapNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn children_of(&self, id: MapNodeId) -> SimResult<&[MapNodeId]> {
        self.node(id)
            .map(|node| node.children.as_slice())
            .ok_or(SimError::UnknownMapNode(id))
    }
}

pub fn reachable_nodes(state: &MapRunState) -> Vec<MapNodeId> {
    state
        .map
        .children_of(state.current_node)
        .map(|children| children.to_vec())
        .unwrap_or_default()
}

pub fn legal_map_actions(state: &MapRunState) -> Vec<MapAction> {
    reachable_nodes(state)
        .into_iter()
        .map(|node_id| MapAction::ChooseNode { node_id })
        .collect()
}

pub fn validate_map_action(state: &MapRunState, action: MapAction) -> SimResult<()> {
    match action {
        MapAction::ChooseNode { node_id } => {
            if reachable_nodes(state).contains(&node_id) {
                Ok(())
            } else {
                Err(SimError::IllegalAction("map node is not reachable"))
            }
        }
    }
}

pub fn apply_map_action(state: &MapRunState, action: MapAction) -> SimResult<MapRunState> {
    validate_map_action(state, action)?;

    let MapAction::ChooseNode { node_id } = action;
    let target = state
        .map
        .node(node_id)
        .ok_or(SimError::UnknownMapNode(node_id))?;

    Ok(MapRunState {
        act: target.act,
        floor: state.floor + 1,
        current_node: node_id,
        map: state.map.clone(),
    })
}

/// Seven-node act-1 fixture: combat start, branch, merge, rest, combat, boss.
#[must_use]
pub fn milestone8_map() -> FixedMap {
    FixedMap {
        nodes: vec![
            MapNode {
                id: MapNodeId::new(0),
                act: 1,
                room_kind: RoomKind::Combat,
                children: vec![MapNodeId::new(1), MapNodeId::new(2)],
            },
            MapNode {
                id: MapNodeId::new(1),
                act: 1,
                room_kind: RoomKind::Combat,
                children: vec![MapNodeId::new(3)],
            },
            MapNode {
                id: MapNodeId::new(2),
                act: 1,
                room_kind: RoomKind::Rest,
                children: vec![MapNodeId::new(3)],
            },
            MapNode {
                id: MapNodeId::new(3),
                act: 1,
                room_kind: RoomKind::Combat,
                children: vec![MapNodeId::new(4)],
            },
            MapNode {
                id: MapNodeId::new(4),
                act: 1,
                room_kind: RoomKind::Rest,
                children: vec![MapNodeId::new(5)],
            },
            MapNode {
                id: MapNodeId::new(5),
                act: 1,
                room_kind: RoomKind::Combat,
                children: vec![MapNodeId::new(6)],
            },
            MapNode {
                id: MapNodeId::new(6),
                act: 1,
                room_kind: RoomKind::Boss,
                children: vec![],
            },
        ],
    }
}

#[must_use]
pub fn milestone8_fixture() -> MapRunState {
    MapRunState {
        act: 1,
        floor: 0,
        current_node: MapNodeId::new(0),
        map: milestone8_map(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_node_exposes_two_reachable_children() {
        let state = milestone8_fixture();

        assert_eq!(
            reachable_nodes(&state),
            vec![MapNodeId::new(1), MapNodeId::new(2)]
        );
        assert_eq!(
            legal_map_actions(&state),
            vec![
                MapAction::ChooseNode {
                    node_id: MapNodeId::new(1)
                },
                MapAction::ChooseNode {
                    node_id: MapNodeId::new(2)
                },
            ]
        );
    }

    #[test]
    fn unreachable_node_is_rejected() {
        let state = milestone8_fixture();

        assert_eq!(
            validate_map_action(
                &state,
                MapAction::ChooseNode {
                    node_id: MapNodeId::new(3)
                }
            ),
            Err(SimError::IllegalAction("map node is not reachable"))
        );
    }

    #[test]
    fn left_branch_traversal_updates_floor_and_act() {
        let mut state = milestone8_fixture();
        let path = [
            MapNodeId::new(1),
            MapNodeId::new(3),
            MapNodeId::new(4),
            MapNodeId::new(5),
            MapNodeId::new(6),
        ];

        for (step, node_id) in path.iter().enumerate() {
            let action = MapAction::ChooseNode { node_id: *node_id };
            assert!(legal_map_actions(&state).contains(&action));
            state = apply_map_action(&state, action).expect("branch step applies");
            assert_eq!(state.floor, (step + 1) as u32);
            assert_eq!(state.act, 1);
            assert_eq!(state.current_node, *node_id);
        }

        assert_eq!(
            state.map.node(state.current_node).unwrap().room_kind,
            RoomKind::Boss
        );
        assert!(reachable_nodes(&state).is_empty());
    }

    #[test]
    fn right_branch_rejoins_and_reaches_boss() {
        let mut state = milestone8_fixture();

        state = apply_map_action(
            &state,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("rest branch applies");
        assert_eq!(state.current_node, MapNodeId::new(2));
        assert_eq!(
            state.map.node(state.current_node).unwrap().room_kind,
            RoomKind::Rest
        );

        state = apply_map_action(
            &state,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(3),
            },
        )
        .expect("merge applies");

        assert_eq!(state.floor, 2);
        assert_eq!(reachable_nodes(&state), vec![MapNodeId::new(4)]);
    }

    #[test]
    fn legal_action_generation_does_not_mutate_state() {
        let state = milestone8_fixture();
        let before = state.clone();

        let _actions = legal_map_actions(&state);

        assert_eq!(state, before);
    }
}
