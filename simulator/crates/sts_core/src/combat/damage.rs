use crate::{
    combat::{MonsterState, PlayerState},
    content::monsters::{
        large_acid_slime_on_hp_damage, DARKLING_ID, GREMLIN_WARRIOR_ID, TRANSIENT_ID,
    },
    ids::{CardId, MonsterId},
    power::PlayerPowers,
    relic::Relic,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackDamageResult {
    pub hp_damage: i32,
    pub broke_block: bool,
    pub malleable_block: Option<i32>,
}

pub fn deal_unmodified_damage_to_monster(monster: &mut MonsterState, amount: i32) -> i32 {
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    let hp_damage = monster.hp.max(0).min(amount - blocked);
    monster.hp -= hp_damage;

    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    } else if hp_damage > 0 && monster.powers.curl_up > 0 {
        monster.block += monster.powers.curl_up;
        monster.powers.curl_up = 0;
    }
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);

    hp_damage
}

fn deal_attack_damage_to_monster(
    monster: &mut MonsterState,
    relics: &[Relic],
    amount: i32,
) -> AttackDamageResult {
    let amount = if monster.powers.flight > 0 {
        amount / 2
    } else {
        amount
    };
    let block_before = monster.block;
    let blocked = monster.block.min(amount);
    monster.block -= blocked;
    let unblocked =
        crate::relic::apply_attack_damage_relics_to_unblocked_damage(relics, amount - blocked);
    let hp_damage = monster.hp.max(0).min(unblocked);
    monster.hp -= hp_damage;

    if monster.hp <= 0 {
        monster.hp = 0;
        monster.alive = false;
        monster.block = 0;
        if monster.content_id == DARKLING_ID {
            monster.escaped = true;
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            monster.powers = Default::default();
        }
    } else if hp_damage > 0 && monster.powers.curl_up > 0 {
        monster.block += monster.powers.curl_up;
        monster.powers.curl_up = 0;
    }
    let malleable_block = if monster.alive && hp_damage > 0 && monster.powers.malleable > 0 {
        let amount = monster.powers.malleable;
        monster.powers.malleable += 1;
        Some(amount)
    } else {
        None
    };
    if monster.alive
        && hp_damage > 0
        && monster.content_id == GREMLIN_WARRIOR_ID
        && monster.powers.anger > 0
    {
        monster.powers.strength += monster.powers.anger;
    }
    if monster.alive && hp_damage > 0 && monster.powers.flight > 0 {
        monster.powers.flight -= 1;
        if monster.powers.flight == 0 {
            monster.intent = crate::MonsterIntent::Stun;
        }
    }
    reduce_monster_plated_armor_after_hp_damage(monster, hp_damage);
    large_acid_slime_on_hp_damage(monster, hp_damage);
    transient_shifting_on_hp_damage(monster, hp_damage);

    AttackDamageResult {
        hp_damage,
        broke_block: block_before > 0 && blocked == block_before,
        malleable_block,
    }
}

fn reduce_monster_plated_armor_after_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if !monster.alive || hp_damage <= 0 || monster.powers.plated_armor <= 0 {
        return;
    }

    monster.powers.plated_armor -= 1;
    if monster.powers.plated_armor == 0 {
        monster.intent = crate::MonsterIntent::Stun;
    }
}

fn transient_shifting_on_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if !monster.alive || hp_damage <= 0 || monster.content_id != TRANSIENT_ID {
        return;
    }

    monster.powers.strength -= hp_damage;
    monster.temp_strength_down += hp_damage;
}

pub fn deal_damage_info_to_monster(
    monster: &mut MonsterState,
    info: DamageInfo,
    player: PlayerPowers,
    temp_strength: i32,
    relics: &[Relic],
) -> i32 {
    deal_damage_info_to_monster_with_result(monster, info, player, temp_strength, relics).hp_damage
}

pub fn deal_damage_info_to_monster_with_result(
    monster: &mut MonsterState,
    info: DamageInfo,
    player: PlayerPowers,
    temp_strength: i32,
    relics: &[Relic],
) -> AttackDamageResult {
    let amount = calculate_player_attack_damage(
        info.amount,
        player,
        temp_strength,
        monster.powers.vulnerable,
        relics,
    );
    deal_attack_damage_to_monster(monster, relics, amount)
}

fn calculate_player_attack_damage(
    base: i32,
    player: PlayerPowers,
    temp_strength: i32,
    target_vulnerable: i32,
    relics: &[Relic],
) -> i32 {
    let mut amount = (base + player.strength + temp_strength).max(0) as f64;
    if player.weak > 0 {
        amount *= 0.75;
    }
    if target_vulnerable > 0 {
        amount *= if relics.contains(&Relic::PaperPhrog) {
            1.75
        } else {
            1.5
        };
    }
    amount.floor().max(0.0) as i32
}

/// Reflects thorns-style spikes damage to the player after an attack hits the monster.
pub fn reflect_spikes_to_player(player: &mut PlayerState, relics: &[Relic], spikes: i32) -> i32 {
    if spikes <= 0 {
        return 0;
    }

    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(player, spikes);
    let blocked = player.block.min(incoming);
    player.block -= blocked;
    let mitigated = crate::relic::mitigate_hp_loss(relics, incoming - blocked);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut player.powers, mitigated);
    player.hp -= hp_loss;
    hp_loss
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::turn_powers::monster_attack_damage,
        content::monsters::{
            monster_state, FIXED_SIMPLE_MONSTER_ID, GREMLIN_WARRIOR_A0, TRANSIENT_A0,
        },
        MonsterId,
    };

    #[test]
    fn curl_up_does_not_leave_block_on_lethal_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 1,
            max_hp: 1,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                curl_up: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 5);

        assert!(!monster.alive);
        assert_eq!(monster.hp, 0);
        assert_eq!(monster.block, 0);
    }

    #[test]
    fn overkill_unmodified_damage_clamps_monster_hp_to_zero() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 2,
            max_hp: 2,
            block: 0,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        let hp_damage = deal_unmodified_damage_to_monster(&mut monster, 5);

        assert_eq!(hp_damage, 5);
        assert!(!monster.alive);
        assert_eq!(monster.hp, 0);
    }

    #[test]
    fn damage_consumes_block_before_hp() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            max_hp: 10,
            block: 4,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 6);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
        assert!(monster.alive);
    }

    #[test]
    fn unmodified_damage_does_not_trigger_curl_up_without_hp_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            max_hp: 10,
            block: 6,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                curl_up: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 5);

        assert_eq!(monster.block, 1);
        assert_eq!(monster.powers.curl_up, 3);
        assert_eq!(monster.hp, 10);
    }

    #[test]
    fn attack_hp_damage_queues_malleable_block_and_increment() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                malleable: 3,
                malleable_base: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 5,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 5);
        assert_eq!(result.malleable_block, Some(3));
        assert_eq!(monster.hp, 15);
        assert_eq!(monster.block, 0);
        assert_eq!(monster.powers.malleable, 4);
    }

    #[test]
    fn weak_and_vulnerable_round_after_all_attack_modifiers() {
        let amount = calculate_player_attack_damage(
            3,
            PlayerPowers {
                strength: 6,
                weak: 1,
                ..Default::default()
            },
            0,
            1,
            &[],
        );

        assert_eq!(amount, 10);
    }

    #[test]
    fn attack_hp_damage_triggers_gremlin_warrior_anger_strength() {
        let mut monster = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(1));
        assert_eq!(monster.powers.anger, 1);
        assert_eq!(monster_attack_damage(&monster, 4), 4);

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 5,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 5);
        assert_eq!(monster.powers.strength, 1);
        assert_eq!(monster_attack_damage(&monster, 4), 5);

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(2)),
                target: MonsterId::new(1),
                amount: 5,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 5);
        assert_eq!(monster.powers.strength, 2);
        assert_eq!(monster_attack_damage(&monster, 4), 6);
    }

    #[test]
    fn transient_shifting_reduces_strength_until_monster_turn_cleanup() {
        let mut monster = monster_state(&TRANSIENT_A0, MonsterId::new(1));

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 16,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 16);
        assert_eq!(monster.hp, 983);
        assert_eq!(monster.powers.strength, -16);
        assert_eq!(monster.temp_strength_down, 16);
        assert_eq!(monster_attack_damage(&monster, 30), 14);
    }

    #[test]
    fn flight_halves_attack_damage_and_loses_one_stack_on_nonlethal_hp_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                flight: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 7,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 3);
        assert_eq!(monster.hp, 17);
        assert_eq!(monster.powers.flight, 2);
        assert_eq!(monster.intent, crate::MonsterIntent::Attack { damage: 6 });
    }

    #[test]
    fn flight_grounding_sets_stun_intent() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                flight: 1,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 6,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(monster.powers.flight, 0);
        assert_eq!(monster.intent, crate::MonsterIntent::Stun);
    }

    #[test]
    fn lethal_attack_damage_does_not_reduce_flight() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 3,
            max_hp: 3,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                flight: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 6,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert!(!monster.alive);
        assert_eq!(monster.hp, 0);
        assert_eq!(monster.powers.flight, 3);
    }

    #[test]
    fn overkill_attack_damage_clamps_monster_hp_to_zero() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 2,
            max_hp: 2,
            block: 0,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        let result = deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 5,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(result.hp_damage, 5);
        assert!(!monster.alive);
        assert_eq!(monster.hp, 0);
    }

    #[test]
    fn unmodified_damage_bypasses_flight() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                flight: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        let hp_damage = deal_unmodified_damage_to_monster(&mut monster, 7);

        assert_eq!(hp_damage, 7);
        assert_eq!(monster.hp, 13);
        assert_eq!(monster.powers.flight, 3);
    }

    #[test]
    fn unmodified_damage_does_not_reduce_monster_plated_armor() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                plated_armor: 1,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 7);

        assert_eq!(monster.hp, 13);
        assert_eq!(monster.powers.plated_armor, 1);
        assert_eq!(monster.intent, crate::MonsterIntent::Attack { damage: 6 });
    }

    #[test]
    fn monster_plated_armor_break_sets_stun_intent() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                plated_armor: 1,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 6,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert_eq!(monster.powers.plated_armor, 0);
        assert_eq!(monster.intent, crate::MonsterIntent::Stun);
    }

    #[test]
    fn lethal_damage_does_not_trigger_monster_plated_armor_break_stun() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 6,
            max_hp: 6,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                plated_armor: 1,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 6,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert!(!monster.alive);
        assert_eq!(monster.powers.plated_armor, 1);
        assert_eq!(monster.intent, crate::MonsterIntent::Attack { damage: 6 });
    }

    #[test]
    fn lethal_attack_damage_does_not_trigger_malleable_block() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 5,
            max_hp: 5,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                malleable: 3,
                malleable_base: 3,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_damage_info_to_monster_with_result(
            &mut monster,
            DamageInfo {
                source: DamageSource::Card(CardId::new(1)),
                target: MonsterId::new(1),
                amount: 5,
            },
            PlayerPowers::default(),
            0,
            &[],
        );

        assert!(!monster.alive);
        assert_eq!(monster.block, 0);
        assert_eq!(monster.powers.malleable, 3);
    }

    #[test]
    fn damage_info_preserves_block_and_hp_math() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            max_hp: 10,
            block: 4,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 6,
        };

        deal_damage_info_to_monster(&mut monster, info, PlayerPowers::default(), 0, &[]);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
    }

    #[test]
    fn the_boot_increases_small_unblocked_attack_damage_after_block() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 4,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 6,
        };

        deal_damage_info_to_monster(
            &mut monster,
            info,
            PlayerPowers::default(),
            0,
            &[Relic::TheBoot],
        );

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 15);
    }

    #[test]
    fn unmodified_damage_does_not_use_the_boot() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 4,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 6);

        assert_eq!(monster.hp, 18);
    }

    #[test]
    fn strength_increases_dealt_attack_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 6,
        };

        deal_damage_info_to_monster(
            &mut monster,
            info,
            PlayerPowers {
                strength: 2,
                ..Default::default()
            },
            0,
            &[],
        );

        assert_eq!(monster.hp, 12);
    }

    #[test]
    fn weak_reduces_dealt_attack_damage_with_floor() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            max_hp: 20,
            block: 0,
            alive: true,
            escaped: false,
            powers: Default::default(),
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 7,
        };

        deal_damage_info_to_monster(
            &mut monster,
            info,
            PlayerPowers {
                weak: 1,
                ..Default::default()
            },
            0,
            &[],
        );

        assert_eq!(monster.hp, 15);
    }

    #[test]
    fn paper_phrog_increases_vulnerable_bonus_damage_to_seventy_five_percent() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 30,
            max_hp: 30,
            block: 0,
            alive: true,
            escaped: false,
            powers: crate::MonsterPowers {
                vulnerable: 1,
                ..Default::default()
            },
            temp_strength_down: 0,
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            rolled_attack_damage: None,
            stolen_gold: 0,
            move_history: Vec::new(),
            gremlin_leader_slot: None,
            stasis_card: None,
            initial_intent_locked: false,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 8,
        };

        deal_damage_info_to_monster(
            &mut monster,
            info,
            PlayerPowers::default(),
            0,
            &[Relic::PaperPhrog],
        );

        assert_eq!(monster.hp, 16);
    }

    #[test]
    fn spikes_reflect_damage_to_player_through_block() {
        let mut player = PlayerState {
            hp: 20,
            max_hp: 80,
            block: 1,
            energy: 3,
            max_energy: 3,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        };

        reflect_spikes_to_player(&mut player, &[], 3);

        assert_eq!(player.block, 0);
        assert_eq!(player.hp, 18);
    }

    #[test]
    fn intangible_caps_spikes_hp_loss() {
        let mut player = PlayerState {
            hp: 20,
            max_hp: 80,
            block: 0,
            energy: 3,
            max_energy: 3,
            powers: PlayerPowers {
                intangible: 1,
                ..Default::default()
            },
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        };

        let hp_loss = reflect_spikes_to_player(&mut player, &[], 3);

        assert_eq!(hp_loss, 1);
        assert_eq!(player.hp, 19);
    }

    #[test]
    fn tungsten_rod_reduces_spikes_hp_loss() {
        let mut player = PlayerState {
            hp: 20,
            max_hp: 80,
            block: 1,
            energy: 3,
            max_energy: 3,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        };

        reflect_spikes_to_player(&mut player, &[crate::Relic::TungstenRod], 3);

        assert_eq!(player.block, 0);
        assert_eq!(player.hp, 19);
    }

    #[test]
    fn buffer_prevents_next_spikes_hp_loss_after_block() {
        let mut player = PlayerState {
            hp: 20,
            max_hp: 80,
            block: 1,
            energy: 3,
            max_energy: 3,
            powers: PlayerPowers {
                buffer: 1,
                ..Default::default()
            },
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        };

        reflect_spikes_to_player(&mut player, &[], 3);

        assert_eq!(player.block, 0);
        assert_eq!(player.hp, 20);
        assert_eq!(player.powers.buffer, 0);
    }
}
