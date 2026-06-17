use crate::{
    combat::MonsterState,
    ids::{CardId, MonsterId},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageInfo {
    pub source: DamageSource,
    pub target: MonsterId,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageSource {
    Card(CardId),
}

pub fn deal_unmodified_damage_to_monster(monster: &mut MonsterState, amount: i32) {
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    monster.hp -= amount - blocked;

    if monster.hp <= 0 {
        monster.alive = false;
    }
}

pub fn deal_damage_info_to_monster(monster: &mut MonsterState, info: DamageInfo) {
    deal_unmodified_damage_to_monster(monster, info.amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MonsterId;

    #[test]
    fn damage_consumes_block_before_hp() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            block: 4,
            alive: true,
            powers: Default::default(),
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 6);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
        assert!(monster.alive);
    }

    #[test]
    fn damage_info_preserves_block_and_hp_math() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            block: 4,
            alive: true,
            powers: Default::default(),
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 6,
        };

        deal_damage_info_to_monster(&mut monster, info);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
    }
}
