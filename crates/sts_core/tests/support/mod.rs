use sts_core::{content::ascension::AscensionConfig, FixedMap, MapNode, MapNodeId, RoomKind};

const BRANCH_ROOM_OPTIONS_A0: [RoomKind; 3] = [RoomKind::Rest, RoomKind::Shop, RoomKind::Combat];
const BRANCH_ROOM_OPTIONS_A1: [RoomKind; 4] = [
    RoomKind::Rest,
    RoomKind::Shop,
    RoomKind::Combat,
    RoomKind::Elite,
];
const PRE_BOSS_ROOM_OPTIONS: [RoomKind; 2] = [RoomKind::Shop, RoomKind::Combat];

struct PlaceholderMapRng(u64);

impl PlaceholderMapRng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        (value as usize) % bound
    }
}

pub fn generate_placeholder_map(seed: u64, ascension: u8) -> (FixedMap, u64) {
    let mut rng = PlaceholderMapRng::new(seed);
    let branch_options = if AscensionConfig::new(ascension).elite_rooms_enabled() {
        &BRANCH_ROOM_OPTIONS_A1[..]
    } else {
        &BRANCH_ROOM_OPTIONS_A0[..]
    };
    let branch_room = branch_options[rng.next_usize(branch_options.len())];
    let pre_boss_room = PRE_BOSS_ROOM_OPTIONS[rng.next_usize(PRE_BOSS_ROOM_OPTIONS.len())];
    let final_seed = rng.0;

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
