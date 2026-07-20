use sts_core::{
    content::ascension::AscensionConfig, FixedMap, MapNode, MapNodeId, RngStream, RoomKind,
    SimulatorRng,
};

const BRANCH_ROOM_OPTIONS_A0: [RoomKind; 3] = [RoomKind::Rest, RoomKind::Shop, RoomKind::Combat];
const BRANCH_ROOM_OPTIONS_A1: [RoomKind; 4] = [
    RoomKind::Rest,
    RoomKind::Shop,
    RoomKind::Combat,
    RoomKind::Elite,
];
const PRE_BOSS_ROOM_OPTIONS: [RoomKind; 2] = [RoomKind::Shop, RoomKind::Combat];

pub fn generate_placeholder_map(seed: u64, ascension: u8) -> (FixedMap, u64) {
    let mut rng = SimulatorRng::new(seed);
    let branch_options = if AscensionConfig::new(ascension).elite_rooms_enabled() {
        &BRANCH_ROOM_OPTIONS_A1[..]
    } else {
        &BRANCH_ROOM_OPTIONS_A0[..]
    };
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
