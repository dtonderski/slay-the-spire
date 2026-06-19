use crate::{ids::MapNodeId, rng::StsRng};

use super::{FixedMap, MapNode, MapRunState, RoomKind};

const EXORDIUM_ROWS: usize = 15;
const EXORDIUM_WIDTH: usize = 7;
const EXORDIUM_PATHS: usize = 6;
const ACT_1_SEED_OFFSET: i64 = 1;
const EXORDIUM_SHOP_ROOM_CHANCE: f32 = 0.05;
const EXORDIUM_REST_ROOM_CHANCE: f32 = 0.12;
const EXORDIUM_TREASURE_ROOM_CHANCE: f32 = 0.0;
const EXORDIUM_EVENT_ROOM_CHANCE: f32 = 0.22;
const EXORDIUM_ELITE_ROOM_CHANCE: f32 = 0.08;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExordiumMapTopology {
    pub first_row_choices: Vec<i32>,
    pub first_row_room_kind: RoomKind,
    pub fixed_room_rows: Vec<ExordiumFixedRoomRow>,
    pub room_type_counts: ExordiumRoomTypeCounts,
    pub pre_shuffle_room_list: Vec<RoomKind>,
    pub shuffled_room_list: Vec<RoomKind>,
    pub assigned_rooms: Vec<ExordiumAssignedRoom>,
    pub map_rng_counter: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExordiumMapChoiceStep {
    pub floor: usize,
    pub x: i32,
    pub next_choices: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExordiumFixedRoomRow {
    pub row: usize,
    pub room_kind: RoomKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExordiumRoomTypeCounts {
    pub assignable_connected_nodes: usize,
    pub shops: usize,
    pub rests: usize,
    pub treasures: usize,
    pub elites: usize,
    pub events: usize,
    pub combats: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExordiumAssignedRoom {
    pub row: usize,
    pub x: i32,
    pub room_kind: RoomKind,
    pub children: Vec<ExordiumMapChild>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExordiumMapChild {
    pub row: usize,
    pub x: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TopologyNode {
    x: i32,
    y: i32,
    edges: Vec<TopologyEdge>,
    parents: Vec<(usize, usize)>,
    room_kind: Option<RoomKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TopologyEdge {
    src_x: i32,
    src_y: i32,
    dst_x: i32,
    dst_y: i32,
}

#[must_use]
pub fn generate_exordium_map_topology(seed: i64) -> ExordiumMapTopology {
    let mut generator = TargetMapGenerator::new(seed + ACT_1_SEED_OFFSET);
    generator.create_paths(EXORDIUM_PATHS);
    generator.filter_redundant_edges_from_first_row();
    generator.topology()
}

#[must_use]
pub fn generate_exordium_fixed_map(seed: i64) -> MapRunState {
    let topology = generate_exordium_map_topology(seed);
    let boss_id = exordium_boss_node_id();
    let mut nodes = Vec::with_capacity(topology.assigned_rooms.len() + 2);
    nodes.push(MapNode {
        id: exordium_root_node_id(),
        act: 1,
        room_kind: RoomKind::Event,
        children: topology
            .first_row_choices
            .iter()
            .copied()
            .map(|x| exordium_map_node_id(0, x))
            .collect(),
    });

    nodes.extend(topology.assigned_rooms.iter().map(|room| {
        MapNode {
            id: exordium_map_node_id(room.row, room.x),
            act: 1,
            room_kind: room.room_kind,
            children: room
                .children
                .iter()
                .map(|child| {
                    if child.row >= EXORDIUM_ROWS {
                        boss_id
                    } else {
                        exordium_map_node_id(child.row, child.x)
                    }
                })
                .collect(),
        }
    }));

    nodes.push(MapNode {
        id: boss_id,
        act: 1,
        room_kind: RoomKind::Boss,
        children: Vec::new(),
    });

    MapRunState {
        act: 1,
        floor: 0,
        current_node: exordium_root_node_id(),
        map: FixedMap { nodes },
    }
}

#[must_use]
pub fn generate_exordium_map_choices_after_path(
    seed: i64,
    path_xs: &[i32],
) -> Vec<ExordiumMapChoiceStep> {
    let mut generator = TargetMapGenerator::new(seed + ACT_1_SEED_OFFSET);
    generator.create_paths(EXORDIUM_PATHS);
    generator.filter_redundant_edges_from_first_row();
    generator.choices_after_path(path_xs)
}

struct TargetMapGenerator {
    rng: StsRng,
    grid: Vec<Vec<TopologyNode>>,
}

impl TargetMapGenerator {
    fn new(map_seed: i64) -> Self {
        let grid = (0..EXORDIUM_ROWS)
            .map(|y| {
                (0..EXORDIUM_WIDTH)
                    .map(|x| TopologyNode {
                        x: x as i32,
                        y: y as i32,
                        edges: Vec::new(),
                        parents: Vec::new(),
                        room_kind: None,
                    })
                    .collect()
            })
            .collect();
        Self {
            rng: StsRng::new(map_seed),
            grid,
        }
    }

    fn create_paths(&mut self, path_count: usize) {
        let max_x = EXORDIUM_WIDTH as i32 - 1;
        let mut first_x = -1;
        for path_index in 0..path_count {
            let mut x = self.rand_range(0, max_x);
            if path_index == 0 {
                first_x = x;
            }
            while x == first_x && path_index == 1 {
                x = self.rand_range(0, max_x);
            }
            self.create_path(TopologyEdge {
                src_x: x,
                src_y: -1,
                dst_x: x,
                dst_y: 0,
            });
        }
    }

    fn create_path(&mut self, edge: TopologyEdge) {
        let src = (edge.dst_y as usize, edge.dst_x as usize);
        if edge.dst_y + 1 >= self.grid.len() as i32 {
            self.grid[src.0][src.1].edges.push(TopologyEdge {
                src_x: edge.dst_x,
                src_y: edge.dst_y,
                dst_x: 3,
                dst_y: edge.dst_y + 2,
            });
            self.sort_edges(src);
            return;
        }

        let max_x = self.grid[edge.dst_y as usize].len() as i32 - 1;
        let (min_step, max_step) = if edge.dst_x == 0 {
            (0, 1)
        } else if edge.dst_x == max_x {
            (-1, 0)
        } else {
            (-1, 1)
        };

        let mut dst_x = edge.dst_x + self.rand_range(min_step, max_step);
        let dst_y = edge.dst_y + 1;
        let mut dst = (dst_y as usize, dst_x as usize);
        let min_distance = 3;
        let max_distance = 5;

        for other_parent in self.grid[dst.0][dst.1].parents.clone() {
            if other_parent == src {
                continue;
            }
            if let Some(ancestor) = self.common_ancestor(other_parent, src, max_distance) {
                let distance = dst_y - self.grid[ancestor.0][ancestor.1].y;
                if distance < min_distance {
                    dst_x = self.adjust_close_merge_destination(edge.dst_x, src, dst, max_x);
                    dst = (dst_y as usize, dst_x as usize);
                }
            }
        }

        if edge.dst_x != 0 {
            let left = (edge.dst_y as usize, edge.dst_x as usize - 1);
            if let Some(max_edge) = self.max_edge(left) {
                if max_edge.dst_x > dst_x {
                    dst_x = max_edge.dst_x;
                }
            }
        }
        if edge.dst_x < max_x {
            let right = (edge.dst_y as usize, edge.dst_x as usize + 1);
            if let Some(min_edge) = self.min_edge(right) {
                if min_edge.dst_x < dst_x {
                    dst_x = min_edge.dst_x;
                }
            }
        }

        dst = (dst_y as usize, dst_x as usize);
        let next_edge = TopologyEdge {
            src_x: edge.dst_x,
            src_y: edge.dst_y,
            dst_x,
            dst_y,
        };
        self.grid[src.0][src.1].edges.push(next_edge);
        self.sort_edges(src);
        self.grid[dst.0][dst.1].parents.push(src);
        self.create_path(next_edge);
    }

    fn adjust_close_merge_destination(
        &mut self,
        src_x: i32,
        src: (usize, usize),
        dst: (usize, usize),
        max_x: i32,
    ) -> i32 {
        let dst_node = &self.grid[dst.0][dst.1];
        let src_node = &self.grid[src.0][src.1];
        if dst_node.x > src_node.x {
            let adjusted = src_x + self.rand_range(-1, 0);
            if adjusted < 0 {
                src_x
            } else {
                adjusted
            }
        } else if dst_node.x == src_node.x {
            let adjusted = src_x + self.rand_range(-1, 1);
            if adjusted > max_x {
                src_x - 1
            } else if adjusted < 0 {
                src_x + 1
            } else {
                adjusted
            }
        } else {
            let adjusted = src_x + self.rand_range(0, 1);
            if adjusted > max_x {
                src_x
            } else {
                adjusted
            }
        }
    }

    fn filter_redundant_edges_from_first_row(&mut self) {
        let mut seen = Vec::new();
        let mut duplicates = Vec::new();
        for x in 0..self.grid[0].len() {
            for edge in &self.grid[0][x].edges {
                if seen.iter().any(|seen_edge: &TopologyEdge| {
                    edge.dst_x == seen_edge.dst_x && edge.dst_y == seen_edge.dst_y
                }) {
                    duplicates.push(*edge);
                }
                seen.push(*edge);
            }
            self.grid[0][x]
                .edges
                .retain(|edge| !duplicates.contains(edge));
            duplicates.clear();
        }
    }

    fn common_ancestor(
        &self,
        mut left: (usize, usize),
        mut right: (usize, usize),
        max_distance: i32,
    ) -> Option<(usize, usize)> {
        let first = &self.grid[left.0][left.1];
        let second = &self.grid[right.0][right.1];
        if first.x >= second.y {
            (left, right) = (right, left);
        }

        let start_y = self.grid[left.0][left.1].y;
        for y in (start_y - max_distance..=start_y).rev() {
            if y < 0 {
                break;
            }
            let left_parents = &self.grid[left.0][left.1].parents;
            let right_parents = &self.grid[right.0][right.1].parents;
            if left_parents.is_empty() || right_parents.is_empty() {
                return None;
            }
            left = self.node_with_max_x(left_parents);
            right = self.node_with_min_x(right_parents);
            if left == right {
                return Some(left);
            }
        }
        None
    }

    fn node_with_max_x(&self, nodes: &[(usize, usize)]) -> (usize, usize) {
        *nodes
            .iter()
            .max_by_key(|(y, x)| self.grid[*y][*x].x)
            .expect("non-empty node list")
    }

    fn node_with_min_x(&self, nodes: &[(usize, usize)]) -> (usize, usize) {
        *nodes
            .iter()
            .min_by_key(|(y, x)| self.grid[*y][*x].x)
            .expect("non-empty node list")
    }

    fn min_edge(&mut self, node: (usize, usize)) -> Option<TopologyEdge> {
        self.sort_edges(node);
        self.grid[node.0][node.1].edges.first().copied()
    }

    fn max_edge(&mut self, node: (usize, usize)) -> Option<TopologyEdge> {
        self.sort_edges(node);
        self.grid[node.0][node.1].edges.last().copied()
    }

    fn sort_edges(&mut self, node: (usize, usize)) {
        self.grid[node.0][node.1]
            .edges
            .sort_by_key(|edge| (edge.dst_x, edge.dst_y));
    }

    fn topology(mut self) -> ExordiumMapTopology {
        let first_row_choices = self.grid[0]
            .iter()
            .filter(|node| !node.edges.is_empty())
            .map(|node| node.x)
            .collect();
        let room_type_counts = self.room_type_counts();
        let pre_shuffle_room_list = pre_shuffle_room_list(room_type_counts);
        let mut shuffled_room_list = pre_shuffle_room_list.clone();
        shuffle_room_list(&mut self.rng, &mut shuffled_room_list);
        let shuffled_room_list_report = shuffled_room_list.clone();
        self.assign_fixed_rows();
        self.assign_rooms_to_nodes(&mut shuffled_room_list);
        self.last_minute_node_checker();
        let assigned_rooms = self.assigned_rooms();
        ExordiumMapTopology {
            first_row_choices,
            first_row_room_kind: RoomKind::Combat,
            fixed_room_rows: target_fixed_room_rows(),
            room_type_counts,
            pre_shuffle_room_list,
            shuffled_room_list: shuffled_room_list_report,
            assigned_rooms,
            map_rng_counter: self.rng.counter(),
        }
    }

    fn room_type_counts(&self) -> ExordiumRoomTypeCounts {
        let assignable_connected_nodes = self.assignable_connected_node_count();
        let shops = java_round(assignable_connected_nodes as f32 * EXORDIUM_SHOP_ROOM_CHANCE);
        let rests = java_round(assignable_connected_nodes as f32 * EXORDIUM_REST_ROOM_CHANCE);
        let treasures =
            java_round(assignable_connected_nodes as f32 * EXORDIUM_TREASURE_ROOM_CHANCE);
        let elites = java_round(assignable_connected_nodes as f32 * EXORDIUM_ELITE_ROOM_CHANCE);
        let events = java_round(assignable_connected_nodes as f32 * EXORDIUM_EVENT_ROOM_CHANCE);
        let combats = assignable_connected_nodes
            .saturating_sub(shops)
            .saturating_sub(rests)
            .saturating_sub(treasures)
            .saturating_sub(elites)
            .saturating_sub(events);

        ExordiumRoomTypeCounts {
            assignable_connected_nodes,
            shops,
            rests,
            treasures,
            elites,
            events,
            combats,
        }
    }

    fn assignable_connected_node_count(&self) -> usize {
        self.grid
            .iter()
            .enumerate()
            .flat_map(|(row_index, row)| {
                row.iter()
                    .filter(move |node| node.has_edges() && row_index != EXORDIUM_ROWS - 2)
            })
            .count()
    }

    fn choices_after_path(&self, path_xs: &[i32]) -> Vec<ExordiumMapChoiceStep> {
        let mut steps = Vec::with_capacity(path_xs.len());
        for (floor, x) in path_xs.iter().copied().enumerate() {
            let Some(node) = self
                .grid
                .get(floor)
                .and_then(|row| row.iter().find(|node| node.x == x))
            else {
                break;
            };
            let next_choices = node.edges.iter().map(|edge| edge.dst_x).collect();
            steps.push(ExordiumMapChoiceStep {
                floor: floor + 1,
                x,
                next_choices,
            });
        }
        steps
    }

    fn rand_range(&mut self, min_inclusive: i32, max_inclusive: i32) -> i32 {
        min_inclusive + self.rng.random_int(max_inclusive - min_inclusive)
    }

    fn assign_fixed_rows(&mut self) {
        self.assign_row_as_room_type(EXORDIUM_ROWS - 1, RoomKind::Rest);
        self.assign_row_as_room_type(0, RoomKind::Combat);
        self.assign_row_as_room_type(8, RoomKind::Treasure);
    }

    fn assign_row_as_room_type(&mut self, row: usize, room_kind: RoomKind) {
        for node in &mut self.grid[row] {
            if node.room_kind.is_none() {
                node.room_kind = Some(room_kind);
            }
        }
    }

    fn assign_rooms_to_nodes(&mut self, room_list: &mut Vec<RoomKind>) {
        for row_index in 0..self.grid.len() {
            for node_index in (0..self.grid[row_index].len()).rev() {
                if !self.grid[row_index][node_index].has_edges()
                    || self.grid[row_index][node_index].room_kind.is_some()
                {
                    continue;
                }
                if let Some(room_index) =
                    self.next_room_type_index_according_to_rules((row_index, node_index), room_list)
                {
                    let room_kind = room_list.remove(room_index);
                    self.grid[row_index][node_index].room_kind = Some(room_kind);
                }
            }
        }
    }

    fn next_room_type_index_according_to_rules(
        &self,
        node: (usize, usize),
        room_list: &[RoomKind],
    ) -> Option<usize> {
        if self.grid[node.0][node.1].y == 1 {
            return room_list
                .iter()
                .position(|room_kind| *room_kind == RoomKind::Combat);
        }

        let parents = &self.grid[node.0][node.1].parents;
        let siblings = self.siblings(parents, node);
        for (index, room_kind) in room_list.iter().copied().enumerate() {
            if !self.rule_assignable_to_row(node, room_kind) {
                continue;
            }
            if !self.rule_parent_matches(parents, room_kind)
                && !self.rule_sibling_matches(&siblings, room_kind)
            {
                return Some(index);
            }
            if self.grid[node.0][node.1].y == 0 {
                return Some(index);
            }
        }
        None
    }

    fn siblings(&self, parents: &[(usize, usize)], node: (usize, usize)) -> Vec<(usize, usize)> {
        let mut siblings = Vec::new();
        for parent in parents {
            for edge in &self.grid[parent.0][parent.1].edges {
                let sibling = (edge.dst_y as usize, edge.dst_x as usize);
                if sibling != node {
                    siblings.push(sibling);
                }
            }
        }
        siblings
    }

    fn rule_assignable_to_row(&self, node: (usize, usize), room_kind: RoomKind) -> bool {
        let y = self.grid[node.0][node.1].y;
        if y <= 4 && matches!(room_kind, RoomKind::Rest | RoomKind::Elite | RoomKind::Shop) {
            return false;
        }
        if y >= 13 && room_kind == RoomKind::Rest {
            return false;
        }
        true
    }

    fn rule_parent_matches(&self, parents: &[(usize, usize)], room_kind: RoomKind) -> bool {
        if !matches!(
            room_kind,
            RoomKind::Rest | RoomKind::Treasure | RoomKind::Shop | RoomKind::Elite
        ) {
            return false;
        }
        parents
            .iter()
            .any(|parent| self.grid[parent.0][parent.1].room_kind == Some(room_kind))
    }

    fn rule_sibling_matches(&self, siblings: &[(usize, usize)], room_kind: RoomKind) -> bool {
        if !matches!(
            room_kind,
            RoomKind::Rest | RoomKind::Combat | RoomKind::Event | RoomKind::Elite | RoomKind::Shop
        ) {
            return false;
        }
        siblings
            .iter()
            .any(|sibling| self.grid[sibling.0][sibling.1].room_kind == Some(room_kind))
    }

    fn last_minute_node_checker(&mut self) {
        for row in &mut self.grid {
            for node in row {
                if node.has_edges() && node.room_kind.is_none() {
                    node.room_kind = Some(RoomKind::Combat);
                }
            }
        }
    }

    fn assigned_rooms(&self) -> Vec<ExordiumAssignedRoom> {
        self.grid
            .iter()
            .flat_map(|row| {
                row.iter().filter_map(|node| {
                    node.room_kind.map(|room_kind| ExordiumAssignedRoom {
                        row: node.y as usize,
                        x: node.x,
                        room_kind,
                        children: node
                            .edges
                            .iter()
                            .map(|edge| ExordiumMapChild {
                                row: edge.dst_y as usize,
                                x: edge.dst_x,
                            })
                            .collect(),
                    })
                })
            })
            .collect()
    }
}

fn exordium_root_node_id() -> MapNodeId {
    MapNodeId::new(0)
}

fn exordium_boss_node_id() -> MapNodeId {
    MapNodeId::new(1 + EXORDIUM_ROWS as u64 * EXORDIUM_WIDTH as u64)
}

fn exordium_map_node_id(row: usize, x: i32) -> MapNodeId {
    MapNodeId::new(1 + row as u64 * EXORDIUM_WIDTH as u64 + x as u64)
}

impl TopologyNode {
    fn has_edges(&self) -> bool {
        !self.edges.is_empty()
    }
}

fn java_round(value: f32) -> usize {
    value.round() as usize
}

fn target_fixed_room_rows() -> Vec<ExordiumFixedRoomRow> {
    vec![
        ExordiumFixedRoomRow {
            row: 0,
            room_kind: RoomKind::Combat,
        },
        ExordiumFixedRoomRow {
            row: 8,
            room_kind: RoomKind::Treasure,
        },
        ExordiumFixedRoomRow {
            row: 14,
            room_kind: RoomKind::Rest,
        },
    ]
}

fn pre_shuffle_room_list(counts: ExordiumRoomTypeCounts) -> Vec<RoomKind> {
    let mut rooms = Vec::with_capacity(counts.assignable_connected_nodes);
    rooms.extend(std::iter::repeat_n(RoomKind::Shop, counts.shops));
    rooms.extend(std::iter::repeat_n(RoomKind::Rest, counts.rests));
    rooms.extend(std::iter::repeat_n(RoomKind::Treasure, counts.treasures));
    rooms.extend(std::iter::repeat_n(RoomKind::Elite, counts.elites));
    rooms.extend(std::iter::repeat_n(RoomKind::Event, counts.events));
    rooms.extend(std::iter::repeat_n(RoomKind::Combat, counts.combats));
    rooms
}

fn shuffle_room_list(rng: &mut StsRng, rooms: &mut [RoomKind]) {
    for i in (2..=rooms.len()).rev() {
        let j = rng.raw_next_int(i as i32) as usize;
        rooms.swap(i - 1, j);
    }
}

#[cfg(test)]
mod tests {
    use crate::{apply_map_action, legal_map_actions, reachable_nodes, MapAction};

    use super::*;

    #[test]
    fn exordium_first_map_choices_match_captured_seed_prefixes() {
        assert_eq!(
            generate_exordium_map_topology(1_957_307_888_551).first_row_choices,
            vec![1, 2]
        );
        assert_eq!(
            generate_exordium_map_topology(22_079_335_079).first_row_choices,
            vec![0, 2, 4, 5]
        );
    }

    #[test]
    fn exordium_map_rng_is_seeded_with_act_offset() {
        let topology = generate_exordium_map_topology(22_079_335_079);

        assert_eq!(topology.map_rng_counter, 95);
    }

    #[test]
    fn exordium_first_row_is_assigned_monster_room() {
        assert_eq!(
            generate_exordium_map_topology(22_079_335_079).first_row_room_kind,
            RoomKind::Combat
        );
    }

    #[test]
    fn exordium_fixed_room_rows_match_target_generate_map_order() {
        assert_eq!(
            generate_exordium_map_topology(22_079_335_079).fixed_room_rows,
            vec![
                ExordiumFixedRoomRow {
                    row: 0,
                    room_kind: RoomKind::Combat,
                },
                ExordiumFixedRoomRow {
                    row: 8,
                    room_kind: RoomKind::Treasure,
                },
                ExordiumFixedRoomRow {
                    row: 14,
                    room_kind: RoomKind::Rest,
                },
            ]
        );
    }

    #[test]
    fn exordium_room_type_counts_match_target_generate_room_types() {
        assert_eq!(
            generate_exordium_map_topology(22_079_335_079).room_type_counts,
            ExordiumRoomTypeCounts {
                assignable_connected_nodes: 57,
                shops: 3,
                rests: 7,
                treasures: 0,
                elites: 5,
                events: 13,
                combats: 29,
            }
        );
        assert_eq!(
            generate_exordium_map_topology(1_957_307_888_551).room_type_counts,
            ExordiumRoomTypeCounts {
                assignable_connected_nodes: 54,
                shops: 3,
                rests: 6,
                treasures: 0,
                elites: 4,
                events: 12,
                combats: 29,
            }
        );
    }

    #[test]
    fn exordium_pre_shuffle_room_list_matches_target_generate_room_types_order() {
        let codex04 = generate_exordium_map_topology(22_079_335_079);
        assert_eq!(codex04.pre_shuffle_room_list.len(), 57);
        assert_eq!(
            codex04
                .pre_shuffle_room_list
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>(),
            vec![RoomKind::Shop, RoomKind::Shop, RoomKind::Shop]
        );
        assert_eq!(codex04.pre_shuffle_room_list[3..10], [RoomKind::Rest; 7]);
        assert_eq!(codex04.pre_shuffle_room_list[10..15], [RoomKind::Elite; 5]);
        assert_eq!(codex04.pre_shuffle_room_list[15..28], [RoomKind::Event; 13]);
        assert_eq!(
            codex04.pre_shuffle_room_list[28..57],
            [RoomKind::Combat; 29]
        );

        let verify01 = generate_exordium_map_topology(1_957_307_888_551);
        assert_eq!(verify01.pre_shuffle_room_list.len(), 54);
        assert_eq!(verify01.pre_shuffle_room_list[0..3], [RoomKind::Shop; 3]);
        assert_eq!(verify01.pre_shuffle_room_list[3..9], [RoomKind::Rest; 6]);
        assert_eq!(verify01.pre_shuffle_room_list[9..13], [RoomKind::Elite; 4]);
        assert_eq!(
            verify01.pre_shuffle_room_list[13..25],
            [RoomKind::Event; 12]
        );
        assert_eq!(
            verify01.pre_shuffle_room_list[25..54],
            [RoomKind::Combat; 29]
        );
    }

    #[test]
    fn exordium_room_shuffle_uses_raw_randomxs128_without_wrapper_counter() {
        let topology = generate_exordium_map_topology(22_079_335_079);

        assert_eq!(topology.map_rng_counter, 95);
        assert_eq!(topology.shuffled_room_list.len(), 57);
        assert_eq!(
            topology
                .shuffled_room_list
                .iter()
                .take(12)
                .copied()
                .collect::<Vec<_>>(),
            vec![
                RoomKind::Rest,
                RoomKind::Combat,
                RoomKind::Event,
                RoomKind::Combat,
                RoomKind::Combat,
                RoomKind::Rest,
                RoomKind::Rest,
                RoomKind::Elite,
                RoomKind::Elite,
                RoomKind::Shop,
                RoomKind::Combat,
                RoomKind::Event,
            ]
        );
    }

    #[test]
    fn exordium_room_assignment_matches_codex04_captured_path_prefix() {
        let topology = generate_exordium_map_topology(22_079_335_079);
        let room_at = |row, x| {
            topology
                .assigned_rooms
                .iter()
                .find(|room| room.row == row && room.x == x)
                .map(|room| room.room_kind)
        };

        assert_eq!(room_at(0, 2), Some(RoomKind::Combat));
        assert_eq!(room_at(1, 3), Some(RoomKind::Combat));
        assert_eq!(room_at(2, 2), Some(RoomKind::Combat));
        assert_eq!(room_at(2, 3), Some(RoomKind::Event));
    }

    #[test]
    fn exordium_fixed_map_traverses_codex04_captured_prefix() {
        let mut state = generate_exordium_fixed_map(22_079_335_079);

        assert_eq!(
            legal_map_actions(&state),
            vec![
                MapAction::ChooseNode {
                    node_id: exordium_map_node_id(0, 0)
                },
                MapAction::ChooseNode {
                    node_id: exordium_map_node_id(0, 2)
                },
                MapAction::ChooseNode {
                    node_id: exordium_map_node_id(0, 4)
                },
                MapAction::ChooseNode {
                    node_id: exordium_map_node_id(0, 5)
                },
            ]
        );

        state = apply_map_action(
            &state,
            MapAction::ChooseNode {
                node_id: exordium_map_node_id(0, 2),
            },
        )
        .expect("first captured node is reachable");
        assert_eq!(state.floor, 1);
        assert_eq!(
            state
                .map
                .node(state.current_node)
                .map(|node| node.room_kind),
            Some(RoomKind::Combat)
        );
        assert_eq!(reachable_nodes(&state), vec![exordium_map_node_id(1, 3)]);

        state = apply_map_action(
            &state,
            MapAction::ChooseNode {
                node_id: exordium_map_node_id(1, 3),
            },
        )
        .expect("second captured node is reachable");
        assert_eq!(state.floor, 2);
        assert_eq!(
            state
                .map
                .node(state.current_node)
                .map(|node| node.room_kind),
            Some(RoomKind::Combat)
        );
        assert_eq!(
            reachable_nodes(&state),
            vec![exordium_map_node_id(2, 2), exordium_map_node_id(2, 3)]
        );
        assert_eq!(
            state
                .map
                .node(exordium_map_node_id(2, 2))
                .map(|node| node.room_kind),
            Some(RoomKind::Combat)
        );
        assert_eq!(
            state
                .map
                .node(exordium_map_node_id(2, 3))
                .map(|node| node.room_kind),
            Some(RoomKind::Event)
        );
    }

    #[test]
    fn exordium_reachable_choices_match_codex04_captured_path_prefix() {
        let choices = generate_exordium_map_choices_after_path(22_079_335_079, &[2, 3]);

        assert_eq!(
            choices,
            vec![
                ExordiumMapChoiceStep {
                    floor: 1,
                    x: 2,
                    next_choices: vec![3],
                },
                ExordiumMapChoiceStep {
                    floor: 2,
                    x: 3,
                    next_choices: vec![2, 3],
                },
            ]
        );
    }
}
