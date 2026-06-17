use crate::combat::MonsterState;

pub fn deal_unmodified_damage_to_monster(monster: &mut MonsterState, amount: i32) {
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    monster.hp -= amount - blocked;

    if monster.hp <= 0 {
        monster.alive = false;
    }
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
        };

        deal_unmodified_damage_to_monster(&mut monster, 6);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
        assert!(monster.alive);
    }
}
