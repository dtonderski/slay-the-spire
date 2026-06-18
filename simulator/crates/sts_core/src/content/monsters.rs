use crate::{
    combat::turn_powers::monster_attack_damage,
    combat::{MonsterIntent, MonsterState},
    ids::{ContentId, MonsterId},
    power::MonsterPowers,
};

pub const FIXED_SIMPLE_MONSTER_ID: ContentId = ContentId::new(100);
pub const CULTIST_ID: ContentId = ContentId::new(101);
pub const JAW_WORM_ID: ContentId = ContentId::new(102);
pub const GREMLIN_NOB_ID: ContentId = ContentId::new(103);
pub const RED_LOUSE_ID: ContentId = ContentId::new(104);
pub const GREEN_LOUSE_ID: ContentId = ContentId::new(105);
pub const SPIKE_SLIME_ID: ContentId = ContentId::new(106);
pub const ACID_SLIME_ID: ContentId = ContentId::new(107);
pub const LAGAVULIN_ID: ContentId = ContentId::new(108);

const RED_LOUSE_CURL_BLOCK: i32 = 3;
const RED_LOUSE_BITE_DAMAGE: i32 = 6;

const GREEN_LOUSE_CURL_BLOCK: i32 = 3;
const GREEN_LOUSE_BITE_DAMAGE: i32 = 6;
const GREEN_LOUSE_SPIKES: i32 = 3;

const SPIKE_SLIME_LICK_WEAK: i32 = 1;
const SPIKE_SLIME_SPIT_DAMAGE: i32 = 7;

const ACID_SLIME_ATTACK_DAMAGE: i32 = 6;
const ACID_SLIME_WEAK: i32 = 1;

const LAGAVULIN_SLEEP_TURNS: u32 = 3;
const LAGAVULIN_SIPHON_STRENGTH: i32 = 2;
const LAGAVULIN_SIPHON_DEXTERITY: i32 = 2;
const LAGAVULIN_ATTACK_DAMAGE: i32 = 18;

const GREMLIN_NOB_BITE_DAMAGE: i32 = 6;
const GREMLIN_NOB_SKULL_BASH_DAMAGE: i32 = 14;
const GREMLIN_NOB_RUSH_DAMAGE: i32 = 10;

const JAW_WORM_CHOMP_DAMAGE: i32 = 11;
const JAW_WORM_THRASH_DAMAGE: i32 = 7;
const JAW_WORM_THRASH_BLOCK: i32 = 5;
const JAW_WORM_BELLOW_STRENGTH: i32 = 3;
const JAW_WORM_BELLOW_BLOCK: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterDefinition {
    pub content_id: ContentId,
    pub name: &'static str,
    pub hp: i32,
    pub attack_damage: i32,
    pub ritual_amount: i32,
    /// Weak applied to the player when they play a skill card while this monster is alive.
    pub enrage_weak_on_skill: i32,
    /// Spikes applied at combat start (thorns on attack).
    pub starting_spikes: i32,
    /// Turns spent asleep before acting (Lagavulin).
    pub starting_sleep_turns: u32,
}

pub const FIXED_SIMPLE_MONSTER: MonsterDefinition = MonsterDefinition {
    content_id: FIXED_SIMPLE_MONSTER_ID,
    name: "Fixed Simple Monster",
    hp: 40,
    attack_damage: 6,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Cultist at ascension 0, simplified: 50 HP, Ritual 2 on first turn, then 6-damage attacks.
pub const CULTIST_A0: MonsterDefinition = MonsterDefinition {
    content_id: CULTIST_ID,
    name: "Cultist",
    hp: 50,
    attack_damage: 6,
    ritual_amount: 2,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Jaw Worm at ascension 0, simplified: 42 HP (within 40–44), three-move cycle.
pub const JAW_WORM_A0: MonsterDefinition = MonsterDefinition {
    content_id: JAW_WORM_ID,
    name: "Jaw Worm",
    hp: 42,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Gremlin Nob at ascension 0, simplified: 82 HP, enrages on skill play, 6/14/10 attack cycle.
pub const GREMLIN_NOB_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_NOB_ID,
    name: "Gremlin Nob",
    hp: 82,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 2,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Red Louse at ascension 0, simplified: 11 HP (within 11–12), Curl/Bite two-move cycle.
pub const RED_LOUSE_A0: MonsterDefinition = MonsterDefinition {
    content_id: RED_LOUSE_ID,
    name: "Red Louse",
    hp: 11,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Green Louse at ascension 0: 12 HP, Spikes 3, Curl/Bite cycle.
pub const GREEN_LOUSE_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREEN_LOUSE_ID,
    name: "Green Louse",
    hp: 12,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: GREEN_LOUSE_SPIKES,
    starting_sleep_turns: 0,
};

/// Act 1 Lagavulin at ascension 0: 109 HP, sleeps 3 turns then siphons and attacks.
pub const LAGAVULIN_A0: MonsterDefinition = MonsterDefinition {
    content_id: LAGAVULIN_ID,
    name: "Lagavulin",
    hp: 109,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: LAGAVULIN_SLEEP_TURNS,
};

/// Act 1 Spike Slime at ascension 0: 14 HP, Lick (weak) / Spit (attack) cycle.
pub const SPIKE_SLIME_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIKE_SLIME_ID,
    name: "Spike Slime",
    hp: 14,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

/// Act 1 Acid Slime (small) at ascension 0: 12 HP, attack then apply weak cycle.
pub const ACID_SLIME_A0: MonsterDefinition = MonsterDefinition {
    content_id: ACID_SLIME_ID,
    name: "Acid Slime (S)",
    hp: 12,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
};

#[must_use]
pub fn get_monster_definition(content_id: ContentId) -> Option<&'static MonsterDefinition> {
    match content_id {
        FIXED_SIMPLE_MONSTER_ID => Some(&FIXED_SIMPLE_MONSTER),
        CULTIST_ID => Some(&CULTIST_A0),
        JAW_WORM_ID => Some(&JAW_WORM_A0),
        GREMLIN_NOB_ID => Some(&GREMLIN_NOB_A0),
        RED_LOUSE_ID => Some(&RED_LOUSE_A0),
        GREEN_LOUSE_ID => Some(&GREEN_LOUSE_A0),
        SPIKE_SLIME_ID => Some(&SPIKE_SLIME_A0),
        ACID_SLIME_ID => Some(&ACID_SLIME_A0),
        LAGAVULIN_ID => Some(&LAGAVULIN_A0),
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
        powers: MonsterPowers {
            spikes: definition.starting_spikes,
            ..MonsterPowers::default()
        },
        content_id: definition.content_id,
        moves_executed: 0,
        sleep_turns_remaining: definition.starting_sleep_turns,
        has_siphoned: false,
        intent: prepare_monster_intent_for_monster(
            definition,
            0,
            definition.starting_sleep_turns,
            false,
        ),
    }
}

#[must_use]
pub fn prepare_monster_intent(monster: &MonsterState) -> MonsterIntent {
    let definition = get_monster_definition(monster.content_id).unwrap_or(&FIXED_SIMPLE_MONSTER);
    prepare_monster_intent_for_monster(
        definition,
        monster.moves_executed,
        monster.sleep_turns_remaining,
        monster.has_siphoned,
    )
}

#[must_use]
fn prepare_monster_intent_for_monster(
    definition: &MonsterDefinition,
    moves_executed: u32,
    sleep_turns_remaining: u32,
    has_siphoned: bool,
) -> MonsterIntent {
    if definition.content_id == LAGAVULIN_ID {
        return lagavulin_intent(sleep_turns_remaining, has_siphoned);
    }
    prepare_monster_intent_for(definition, moves_executed)
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
        JAW_WORM_ID => jaw_worm_intent(moves_executed),
        GREMLIN_NOB_ID => gremlin_nob_intent(moves_executed),
        RED_LOUSE_ID => red_louse_intent(moves_executed),
        GREEN_LOUSE_ID => green_louse_intent(moves_executed),
        SPIKE_SLIME_ID => spike_slime_intent(moves_executed),
        ACID_SLIME_ID => acid_slime_intent(moves_executed),
        _ => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
    }
}

/// Deterministic Red Louse move cycle: Curl → Bite, keyed on `moves_executed`.
#[must_use]
fn red_louse_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Block {
            block: RED_LOUSE_CURL_BLOCK,
        },
        _ => MonsterIntent::Attack {
            damage: RED_LOUSE_BITE_DAMAGE,
        },
    }
}

#[must_use]
fn green_louse_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Block {
            block: GREEN_LOUSE_CURL_BLOCK,
        },
        _ => MonsterIntent::Attack {
            damage: GREEN_LOUSE_BITE_DAMAGE,
        },
    }
}

#[must_use]
fn spike_slime_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::ApplyPlayerWeak {
            amount: SPIKE_SLIME_LICK_WEAK,
        },
        _ => MonsterIntent::Attack {
            damage: SPIKE_SLIME_SPIT_DAMAGE,
        },
    }
}

#[must_use]
fn acid_slime_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Attack {
            damage: ACID_SLIME_ATTACK_DAMAGE,
        },
        _ => MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        },
    }
}

#[must_use]
fn lagavulin_intent(sleep_turns_remaining: u32, has_siphoned: bool) -> MonsterIntent {
    if sleep_turns_remaining > 0 {
        MonsterIntent::Sleep
    } else if !has_siphoned {
        MonsterIntent::SiphonPlayer {
            strength: LAGAVULIN_SIPHON_STRENGTH,
            dexterity: LAGAVULIN_SIPHON_DEXTERITY,
        }
    } else {
        MonsterIntent::Attack {
            damage: LAGAVULIN_ATTACK_DAMAGE,
        }
    }
}

/// Wakes a sleeping Lagavulin when damaged and updates its intent for the current turn.
pub fn wake_lagavulin_on_damage(monster: &mut MonsterState) {
    if monster.content_id == LAGAVULIN_ID && monster.sleep_turns_remaining > 0 {
        monster.sleep_turns_remaining = 0;
        monster.intent = lagavulin_intent(monster.sleep_turns_remaining, monster.has_siphoned);
    }
}

/// Deterministic Gremlin Nob move cycle: Bite → Skull Bash → Rush, keyed on `moves_executed`.
#[must_use]
fn gremlin_nob_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 3 {
        0 => MonsterIntent::Attack {
            damage: GREMLIN_NOB_BITE_DAMAGE,
        },
        1 => MonsterIntent::Attack {
            damage: GREMLIN_NOB_SKULL_BASH_DAMAGE,
        },
        _ => MonsterIntent::Attack {
            damage: GREMLIN_NOB_RUSH_DAMAGE,
        },
    }
}

/// Deterministic Jaw Worm move cycle: Chomp → Thrash → Bellow, keyed on `moves_executed`.
#[must_use]
fn jaw_worm_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 3 {
        0 => MonsterIntent::Attack {
            damage: JAW_WORM_CHOMP_DAMAGE,
        },
        1 => MonsterIntent::AttackAndBlock {
            damage: JAW_WORM_THRASH_DAMAGE,
            block: JAW_WORM_THRASH_BLOCK,
        },
        _ => MonsterIntent::StrengthAndBlock {
            strength: JAW_WORM_BELLOW_STRENGTH,
            block: JAW_WORM_BELLOW_BLOCK,
        },
    }
}

/// Execute the monster's current intent and return player damage dealt this turn.
pub fn apply_monster_intent(monster: &mut MonsterState, player: &mut crate::PlayerState) -> i32 {
    let damage = match monster.intent {
        MonsterIntent::Attack { damage } => monster_attack_damage(monster, damage),
        MonsterIntent::Block { block } => {
            monster.block += block;
            0
        }
        MonsterIntent::Ritual { amount } => {
            monster.powers.ritual += amount;
            0
        }
        MonsterIntent::AttackAndBlock { damage, block } => {
            monster.block += block;
            monster_attack_damage(monster, damage)
        }
        MonsterIntent::StrengthAndBlock { strength, block } => {
            monster.powers.strength += strength;
            monster.block += block;
            0
        }
        MonsterIntent::ApplyPlayerWeak { amount } => {
            player.powers.weak += amount;
            0
        }
        MonsterIntent::Sleep => {
            if monster.sleep_turns_remaining > 0 {
                monster.sleep_turns_remaining -= 1;
            }
            0
        }
        MonsterIntent::SiphonPlayer {
            strength,
            dexterity,
        } => {
            player.powers.strength = (player.powers.strength - strength).max(0);
            player.powers.dexterity = (player.powers.dexterity - dexterity).max(0);
            monster.has_siphoned = true;
            0
        }
    };
    monster.moves_executed += 1;
    damage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::MonsterIntent, power::PlayerPowers, PlayerState};

    fn dummy_player() -> PlayerState {
        PlayerState {
            hp: 80,
            max_hp: 80,
            block: 0,
            energy: 3,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
        }
    }

    fn apply_intent(monster: &mut MonsterState) -> i32 {
        let mut player = dummy_player();
        apply_monster_intent(monster, &mut player)
    }

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

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.powers.ritual, 2);
        assert_eq!(monster.moves_executed, 1);
    }

    #[test]
    fn cultist_attack_intent_deals_six_plus_strength() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));
        monster.powers.strength = 2;
        monster.intent = MonsterIntent::Attack { damage: 6 };

        assert_eq!(apply_intent(&mut monster), 8);
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

    #[test]
    fn jaw_worm_has_forty_two_hp() {
        assert_eq!(JAW_WORM_A0.hp, 42);
    }

    #[test]
    fn jaw_worm_starts_with_chomp_intent() {
        let monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
    }

    #[test]
    fn jaw_worm_move_selection_cycles_chomp_thrash_bellow() {
        let definition = &JAW_WORM_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::AttackAndBlock {
                damage: JAW_WORM_THRASH_DAMAGE,
                block: JAW_WORM_THRASH_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2),
            MonsterIntent::StrengthAndBlock {
                strength: JAW_WORM_BELLOW_STRENGTH,
                block: JAW_WORM_BELLOW_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3),
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
    }

    #[test]
    fn jaw_worm_chomp_deals_eleven_plus_strength() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.powers.strength = 3;
        monster.intent = MonsterIntent::Attack {
            damage: JAW_WORM_CHOMP_DAMAGE,
        };

        assert_eq!(apply_intent(&mut monster), 14);
    }

    #[test]
    fn jaw_worm_thrash_deals_damage_and_gains_block() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackAndBlock {
            damage: JAW_WORM_THRASH_DAMAGE,
            block: JAW_WORM_THRASH_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 7);
        assert_eq!(monster.block, 5);
    }

    #[test]
    fn jaw_worm_bellow_gains_strength_and_block() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::StrengthAndBlock {
            strength: JAW_WORM_BELLOW_STRENGTH,
            block: JAW_WORM_BELLOW_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.powers.strength, 3);
        assert_eq!(monster.block, 6);
    }

    #[test]
    fn gremlin_nob_has_eighty_two_hp() {
        assert_eq!(GREMLIN_NOB_A0.hp, 82);
    }

    #[test]
    fn gremlin_nob_starts_with_bite_intent() {
        let monster = monster_state(&GREMLIN_NOB_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_BITE_DAMAGE
            }
        );
    }

    #[test]
    fn gremlin_nob_move_selection_cycles_bite_skull_bash_rush() {
        let definition = &GREMLIN_NOB_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_BITE_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_SKULL_BASH_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_RUSH_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_BITE_DAMAGE
            }
        );
    }

    #[test]
    fn gremlin_nob_enrages_on_skill() {
        assert_eq!(GREMLIN_NOB_A0.enrage_weak_on_skill, 2);
    }

    #[test]
    fn red_louse_has_eleven_hp() {
        assert_eq!(RED_LOUSE_A0.hp, 11);
    }

    #[test]
    fn red_louse_starts_with_curl_intent() {
        let monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
            }
        );
    }

    #[test]
    fn red_louse_move_selection_cycles_curl_bite() {
        let definition = &RED_LOUSE_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::Attack {
                damage: RED_LOUSE_BITE_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2),
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
            }
        );
    }

    #[test]
    fn red_louse_curl_gains_block() {
        let mut monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Block {
            block: RED_LOUSE_CURL_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.block, 3);
    }

    #[test]
    fn red_louse_bite_deals_six_damage() {
        let mut monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Attack {
            damage: RED_LOUSE_BITE_DAMAGE,
        };

        assert_eq!(apply_intent(&mut monster), 6);
    }

    #[test]
    fn green_louse_has_twelve_hp_and_starting_spikes() {
        let monster = monster_state(&GREEN_LOUSE_A0, MonsterId::new(1));

        assert_eq!(GREEN_LOUSE_A0.hp, 12);
        assert_eq!(monster.powers.spikes, GREEN_LOUSE_SPIKES);
    }

    #[test]
    fn green_louse_move_selection_cycles_curl_bite() {
        let definition = &GREEN_LOUSE_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Block {
                block: GREEN_LOUSE_CURL_BLOCK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::Attack {
                damage: GREEN_LOUSE_BITE_DAMAGE
            }
        );
    }

    #[test]
    fn spike_slime_has_fourteen_hp_and_lick_spit_cycle() {
        let definition = &SPIKE_SLIME_A0;

        assert_eq!(SPIKE_SLIME_A0.hp, 14);
        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::ApplyPlayerWeak {
                amount: SPIKE_SLIME_LICK_WEAK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::Attack {
                damage: SPIKE_SLIME_SPIT_DAMAGE
            }
        );
    }

    #[test]
    fn spike_slime_lick_applies_weak_to_player() {
        let mut monster = monster_state(&SPIKE_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak {
            amount: SPIKE_SLIME_LICK_WEAK,
        };
        let mut player = dummy_player();

        assert_eq!(apply_monster_intent(&mut monster, &mut player), 0);
        assert_eq!(player.powers.weak, 1);
    }

    #[test]
    fn acid_slime_has_twelve_hp_and_attack_weak_cycle() {
        let definition = &ACID_SLIME_A0;

        assert_eq!(ACID_SLIME_A0.hp, 12);
        assert_eq!(
            prepare_monster_intent_for(definition, 0),
            MonsterIntent::Attack {
                damage: ACID_SLIME_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1),
            MonsterIntent::ApplyPlayerWeak {
                amount: ACID_SLIME_WEAK
            }
        );
    }

    #[test]
    fn acid_slime_weak_applies_weak_to_player() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        };
        let mut player = dummy_player();

        assert_eq!(apply_monster_intent(&mut monster, &mut player), 0);
        assert_eq!(player.powers.weak, 1);
    }

    #[test]
    fn lagavulin_has_one_hundred_nine_hp_and_starts_asleep() {
        let monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));

        assert_eq!(LAGAVULIN_A0.hp, 109);
        assert_eq!(monster.sleep_turns_remaining, LAGAVULIN_SLEEP_TURNS);
        assert_eq!(monster.intent, MonsterIntent::Sleep);
    }

    #[test]
    fn lagavulin_intent_progresses_sleep_siphon_attack() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        assert_eq!(monster.intent, MonsterIntent::Sleep);

        monster.sleep_turns_remaining = 0;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::SiphonPlayer {
                strength: LAGAVULIN_SIPHON_STRENGTH,
                dexterity: LAGAVULIN_SIPHON_DEXTERITY,
            }
        );

        monster.has_siphoned = true;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: LAGAVULIN_ATTACK_DAMAGE
            }
        );
    }

    #[test]
    fn lagavulin_sleep_decrements_remaining_turns() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Sleep;

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.sleep_turns_remaining, 2);
    }

    #[test]
    fn lagavulin_siphon_reduces_player_strength_and_dexterity() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.sleep_turns_remaining = 0;
        monster.intent = MonsterIntent::SiphonPlayer {
            strength: LAGAVULIN_SIPHON_STRENGTH,
            dexterity: LAGAVULIN_SIPHON_DEXTERITY,
        };
        let mut player = dummy_player();
        player.powers.strength = 3;
        player.powers.dexterity = 2;

        assert_eq!(apply_monster_intent(&mut monster, &mut player), 0);
        assert_eq!(player.powers.strength, 1);
        assert_eq!(player.powers.dexterity, 0);
        assert!(monster.has_siphoned);
    }

    #[test]
    fn lagavulin_wake_on_damage_clears_sleep_and_sets_siphon_intent() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));

        wake_lagavulin_on_damage(&mut monster);

        assert_eq!(monster.sleep_turns_remaining, 0);
        assert_eq!(
            monster.intent,
            MonsterIntent::SiphonPlayer {
                strength: LAGAVULIN_SIPHON_STRENGTH,
                dexterity: LAGAVULIN_SIPHON_DEXTERITY,
            }
        );
    }
}
