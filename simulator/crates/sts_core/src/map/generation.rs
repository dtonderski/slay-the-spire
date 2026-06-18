use super::{FixedMap, MapNode, MapRunState, RoomKind};
use crate::{
    content::ascension::AscensionConfig,
    ids::MapNodeId,
    rng::{RngStream, SimulatorRng},
};

const BRANCH_ROOM_OPTIONS_A0: [RoomKind; 3] = [RoomKind::Rest, RoomKind::Shop, RoomKind::Combat];
const BRANCH_ROOM_OPTIONS_A1: [RoomKind; 4] = [
    RoomKind::Rest,
    RoomKind::Shop,
    RoomKind::Combat,
    RoomKind::Elite,
];
const PRE_BOSS_ROOM_OPTIONS: [RoomKind; 2] = [RoomKind::Shop, RoomKind::Combat];

fn branch_room_options(ascension: u8) -> &'static [RoomKind] {
    if AscensionConfig::new(ascension).elite_rooms_enabled() {
        &BRANCH_ROOM_OPTIONS_A1
    } else {
        &BRANCH_ROOM_OPTIONS_A0
    }
}

/// Deterministic placeholder map generator. Topology matches [super::milestone8_map] but
/// branch and pre-boss room kinds vary by seed. Not claimed to match in-game generation.
#[must_use]
pub fn generate_map_placeholder(seed: u64, ascension: u8) -> (FixedMap, u64) {
    let mut rng = SimulatorRng::new(seed);
    let branch_options = branch_room_options(ascension);
    let branch_room =
        branch_options[rng.next_usize(RngStream::MapRoom, "branch_room", branch_options.len())];
    let pre_boss_room = PRE_BOSS_ROOM_OPTIONS[rng.next_usize(
        RngStream::MapRoom,
        "pre_boss_room",
        PRE_BOSS_ROOM_OPTIONS.len(),
    )];
    let final_seed = rng.seed_state();

    let map = FixedMap {
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
                room_kind: branch_room,
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
                room_kind: pre_boss_room,
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
    };

    (map, final_seed)
}

#[must_use]
pub fn generated_map_fixture(seed: u64) -> MapRunState {
    generated_map_fixture_for_ascension(seed, 0)
}

#[must_use]
pub fn generated_map_fixture_for_ascension(seed: u64, ascension: u8) -> MapRunState {
    let (map, _) = generate_map_placeholder(seed, ascension);

    MapRunState {
        act: 1,
        floor: 0,
        current_node: MapNodeId::new(0),
        map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{apply_map_action, legal_map_actions, milestone8_map, MapAction};

    #[test]
    fn generate_map_placeholder_is_deterministic_for_seed() {
        let (first, first_seed) = generate_map_placeholder(42, 0);
        let (second, second_seed) = generate_map_placeholder(42, 0);

        assert_eq!(first, second);
        assert_eq!(first_seed, second_seed);
    }

    #[test]
    fn generate_map_placeholder_varies_room_kinds_by_seed() {
        let (map_a, _) = generate_map_placeholder(1, 0);
        let (map_b, _) = generate_map_placeholder(99, 0);

        let branch_a = map_a.node(MapNodeId::new(2)).unwrap().room_kind;
        let branch_b = map_b.node(MapNodeId::new(2)).unwrap().room_kind;
        let pre_boss_a = map_a.node(MapNodeId::new(4)).unwrap().room_kind;
        let pre_boss_b = map_b.node(MapNodeId::new(4)).unwrap().room_kind;

        assert!(
            branch_a != branch_b || pre_boss_a != pre_boss_b,
            "expected at least one room kind to differ across seeds"
        );
    }

    #[test]
    fn a1_maps_can_include_elite_rooms() {
        let has_elite = (0..64).any(|seed| {
            generate_map_placeholder(seed, 1)
                .0
                .nodes
                .iter()
                .any(|node| node.room_kind == RoomKind::Elite)
        });
        assert!(has_elite);
    }

    #[test]
    fn generated_map_reaches_boss_along_either_branch() {
        for seed in [0_u64, 7, 42, 99] {
            let mut state = generated_map_fixture(seed);
            let left_path = [
                MapNodeId::new(1),
                MapNodeId::new(3),
                MapNodeId::new(4),
                MapNodeId::new(5),
                MapNodeId::new(6),
            ];

            for node_id in left_path {
                let action = MapAction::ChooseNode { node_id };
                assert!(legal_map_actions(&state).contains(&action));
                state = apply_map_action(&state, action).expect("left branch step applies");
            }

            assert_eq!(state.current_node, MapNodeId::new(6));
            assert_eq!(
                state.map.node(state.current_node).unwrap().room_kind,
                RoomKind::Boss
            );
        }
    }

    #[test]
    fn generated_map_matches_fixed_fixture_when_seed_selects_same_rooms() {
        let fixed = milestone8_map();
        let fixed_branch = fixed.node(MapNodeId::new(2)).unwrap().room_kind;
        let fixed_pre_boss = fixed.node(MapNodeId::new(4)).unwrap().room_kind;

        let generated = (0_u64..10_000)
            .map(|seed| generate_map_placeholder(seed, 0).0)
            .find(|map| {
                let branch = map.node(MapNodeId::new(2)).unwrap().room_kind;
                let pre_boss = map.node(MapNodeId::new(4)).unwrap().room_kind;
                branch == fixed_branch && pre_boss == fixed_pre_boss
            })
            .expect("failed to find matching seed for fixture rooms");

        assert_eq!(generated, fixed);
    }
}
