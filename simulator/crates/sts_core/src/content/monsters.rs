use crate::MonsterId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedMonsterDefinition {
    pub id: MonsterId,
    pub name: &'static str,
    pub attack_damage: i32,
}

pub const FIXED_SIMPLE_MONSTER: FixedMonsterDefinition = FixedMonsterDefinition {
    id: MonsterId::new(1),
    name: "Fixed Simple Monster",
    attack_damage: 6,
};
