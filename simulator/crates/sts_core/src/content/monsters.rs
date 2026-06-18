use crate::{
    combat::turn_powers::monster_attack_damage,
    combat::{MonsterIntent, MonsterState},
    ids::{ContentId, MonsterId},
    power::MonsterPowers,
};

pub const FIXED_SIMPLE_MONSTER_ID: ContentId = ContentId::new(100);
pub const CULTIST_ID: ContentId = ContentId::new(101);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterDefinition {
    pub content_id: ContentId,
    pub name: &'static str,
    pub hp: i32,
    pub attack_damage: i32,
    pub ritual_amount: i32,
}

pub const FIXED_SIMPLE_MONSTER: MonsterDefinition = MonsterDefinition {
    content_id: FIXED_SIMPLE_MONSTER_ID,
    name: "Fixed Simple Monster",
    hp: 40,
    attack_damage: 6,
    ritual_amount: 0,
};

/// Act 1 Cultist at ascension 0, simplified: 50 HP, Ritual 2 on first turn, then 6-damage attacks.
pub const CULTIST_A0: MonsterDefinition = MonsterDefinition {
    content_id: CULTIST_ID,
    name: "Cultist",
    hp: 50,
    attack_damage: 6,
    ritual_amount: 2,
};

#[must_use]
pub fn get_monster_definition(content_id: ContentId) -> Option<&'static MonsterDefinition> {
    match content_id {
        FIXED_SIMPLE_MONSTER_ID => Some(&FIXED_SIMPLE_MONSTER),
        CULTIST_ID => Some(&CULTIST_A0),
        _ => None,
    }
}

#[must_use]
pub fn monster_state(definition: &MonsterDefinition, id: MonsterId) -> MonsterState {
    MonsterState {
        id,
        hp: definition.hp,
        block: 0,
        alive: true,
        powers: MonsterPowers::default(),
        content_id: definition.content_id,
        moves_executed: 0,
        intent: prepare_monster_intent_for(definition, 0),
    }
}

#[must_use]
pub fn prepare_monster_intent(monster: &MonsterState) -> MonsterIntent {
    let definition = get_monster_definition(monster.content_id).unwrap_or(&FIXED_SIMPLE_MONSTER);
    prepare_monster_intent_for(definition, monster.moves_executed)
}

#[must_use]
pub fn prepare_monster_intent_for(
    definition: &MonsterDefinition,
    moves_executed: u32,
) -> MonsterIntent {
    match definition.content_id {
        CULTIST_ID if moves_executed == 0 => MonsterIntent::Ritual {
            amount: definition.ritual_amount,
        },
        _ => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
    }
}

/// Execute the monster's current intent and return player damage dealt this turn.
pub fn apply_monster_intent(monster: &mut MonsterState) -> i32 {
    let damage = match monster.intent {
        MonsterIntent::Attack { damage } => monster_attack_damage(monster, damage),
        MonsterIntent::Ritual { amount } => {
            monster.powers.ritual += amount;
            0
        }
    };
    monster.moves_executed += 1;
    damage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cultist_has_fifty_hp() {
        assert_eq!(CULTIST_A0.hp, 50);
    }

    #[test]
    fn cultist_starts_with_ritual_intent() {
        let monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Ritual {
                amount: CULTIST_A0.ritual_amount
            }
        );
    }

    #[test]
    fn cultist_move_selection_ritual_then_attack() {
        let definition = &CULTIST_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Ritual { amount: 2 }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::Attack { damage: 6 }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2),
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn cultist_prepare_monster_intent_tracks_moves_executed() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Ritual { amount: 2 }
        );

        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn cultist_ritual_intent_grants_ritual_power() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(apply_monster_intent(&mut monster), 0);
        assert_eq!(monster.powers.ritual, 2);
        assert_eq!(monster.moves_executed, 1);
    }

    #[test]
    fn cultist_attack_intent_deals_six_plus_strength() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));
        monster.powers.strength = 2;
        monster.intent = MonsterIntent::Attack { damage: 6 };

        assert_eq!(apply_monster_intent(&mut monster), 8);
    }

    #[test]
    fn fixed_simple_monster_always_attacks() {
        let monster = monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1));

        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&FIXED_SIMPLE_MONSTER, 5),
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage
            }
        );
    }
}
