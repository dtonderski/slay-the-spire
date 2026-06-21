use crate::{
    combat::{MonsterState, PlayerState},
    ids::{CardId, MonsterId},
    power::{calculate_attack_damage, PlayerPowers},
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
        monster.block = 0;
    } else if monster.powers.curl_up > 0 {
        monster.block += monster.powers.curl_up;
        monster.powers.curl_up = 0;
    }
}

pub fn deal_damage_info_to_monster(
    monster: &mut MonsterState,
    info: DamageInfo,
    player: PlayerPowers,
    temp_strength: i32,
) {
    let amount = calculate_attack_damage(
        info.amount,
        player,
        temp_strength,
        monster.powers.vulnerable,
    );
    deal_unmodified_damage_to_monster(monster, amount);
}

/// Reflects thorns-style spikes damage to the player after an attack hits the monster.
pub fn reflect_spikes_to_player(player: &mut PlayerState, spikes: i32) {
    if spikes <= 0 {
        return;
    }

    let blocked = player.block.min(spikes);
    player.block -= blocked;
    player.hp -= spikes - blocked;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::monsters::FIXED_SIMPLE_MONSTER_ID, MonsterId};

    #[test]
    fn curl_up_does_not_leave_block_on_lethal_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 1,
            block: 0,
            alive: true,
            powers: crate::MonsterPowers {
                curl_up: 3,
                ..Default::default()
            },
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            rolled_attack_damage: None,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };

        deal_unmodified_damage_to_monster(&mut monster, 5);

        assert!(!monster.alive);
        assert_eq!(monster.block, 0);
    }

    #[test]
    fn damage_consumes_block_before_hp() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 10,
            block: 4,
            alive: true,
            powers: Default::default(),
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            rolled_attack_damage: None,
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
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            rolled_attack_damage: None,
            intent: crate::MonsterIntent::Attack { damage: 6 },
        };
        let info = DamageInfo {
            source: DamageSource::Card(CardId::new(1)),
            target: MonsterId::new(1),
            amount: 6,
        };

        deal_damage_info_to_monster(&mut monster, info, PlayerPowers::default(), 0);

        assert_eq!(monster.block, 0);
        assert_eq!(monster.hp, 8);
    }

    #[test]
    fn strength_increases_dealt_attack_damage() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            block: 0,
            alive: true,
            powers: Default::default(),
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            rolled_attack_damage: None,
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
        );

        assert_eq!(monster.hp, 12);
    }

    #[test]
    fn weak_reduces_dealt_attack_damage_with_floor() {
        let mut monster = MonsterState {
            id: MonsterId::new(1),
            hp: 20,
            block: 0,
            alive: true,
            powers: Default::default(),
            content_id: FIXED_SIMPLE_MONSTER_ID,
            moves_executed: 0,
            sleep_turns_remaining: 0,
            has_siphoned: false,
            split_triggered: false,
            defensive_turns_remaining: 0,
            rolled_attack_damage: None,
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
        );

        assert_eq!(monster.hp, 15);
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
        };

        reflect_spikes_to_player(&mut player, 3);

        assert_eq!(player.block, 0);
        assert_eq!(player.hp, 18);
    }
}
