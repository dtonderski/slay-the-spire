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
pub fn placeholder_generated_map_fixture(seed: u64) -> MapRunState {
    placeholder_generated_map_fixture_for_ascension(seed, 0)
}

#[must_use]
pub fn placeholder_generated_map_fixture_for_ascension(seed: u64, ascension: u8) -> MapRunState {
    let (map, _) = generate_map_placeholder(seed, ascension);

    MapRunState {
        act: 1,
        floor: 0,
        current_node: MapNodeId::new(0),
        map,
    }
}

/// Compatibility wrapper for [`placeholder_generated_map_fixture`].
///
/// Fidelity: [`crate::FidelityCategory::Placeholder`]. This uses the
/// simulator-only placeholder map generator and is not target-game map parity.
#[must_use]
pub fn generated_map_fixture(seed: u64) -> MapRunState {
    placeholder_generated_map_fixture(seed)
}

/// Compatibility wrapper for [`placeholder_generated_map_fixture_for_ascension`].
///
/// Fidelity: [`crate::FidelityCategory::Placeholder`].
#[must_use]
pub fn generated_map_fixture_for_ascension(seed: u64, ascension: u8) -> MapRunState {
    placeholder_generated_map_fixture_for_ascension(seed, ascension)
}
