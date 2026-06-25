use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlayerPowers {
    pub strength: i32,
    pub weak: i32,
    pub dexterity: i32,
    pub frail: i32,
    pub vulnerable: i32,
    pub ritual: i32,
    pub metallicize: i32,
    pub regen: i32,
    pub thorns: i32,
    pub plated_armor: i32,
    pub artifact: i32,
    pub feel_no_pain: i32,
    pub dark_embrace: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub barricade: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub evolve: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub berserk: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub rupture: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub brutality: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub combust: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub fire_breathing: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub corruption: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub buffer: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub intangible: i32,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MonsterPowers {
    pub vulnerable: i32,
    pub weak: i32,
    pub strength: i32,
    pub ritual: i32,
    pub spikes: i32,
    pub curl_up: i32,
    /// Gremlin Nob enrage stacks (Anger); each stack adds +1 attack damage.
    pub anger: i32,
    /// Lagavulin sleep stance block gain per turn.
    pub metallicize: i32,
}

/// Slay the Spire-style vulnerable bonus: attack damage is increased by 50%, floored.
#[must_use]
pub fn attack_damage_with_vulnerable(base: i32, vulnerable: i32) -> i32 {
    attack_damage_with_vulnerable_bonus(base, vulnerable, 1, 2)
}

#[must_use]
pub fn attack_damage_with_vulnerable_bonus(
    base: i32,
    vulnerable: i32,
    bonus_numerator: i32,
    bonus_denominator: i32,
) -> i32 {
    if vulnerable > 0 {
        base + base * bonus_numerator / bonus_denominator
    } else {
        base
    }
}

/// Player attack modifiers applied before target vulnerable:
/// 1. add strength and temp strength
/// 2. if weak, multiply by 0.75 and floor via integer `base * 3 / 4`
/// 3. apply target vulnerable
#[must_use]
pub fn calculate_attack_damage(
    base: i32,
    player: PlayerPowers,
    temp_strength: i32,
    target_vulnerable: i32,
) -> i32 {
    let with_strength = (base + player.strength + temp_strength).max(0);
    let with_weak = if player.weak > 0 {
        with_strength * 3 / 4
    } else {
        with_strength
    };

    attack_damage_with_vulnerable(with_weak, target_vulnerable)
}

/// Block from cards: add dexterity, then apply frail reduction (25%, floored).
#[must_use]
pub fn calculate_block(base: i32, player: PlayerPowers) -> i32 {
    let with_dexterity = (base + player.dexterity).max(0);
    if player.frail > 0 {
        with_dexterity * 3 / 4
    } else {
        with_dexterity
    }
}

pub fn apply_player_weak(powers: &mut PlayerPowers, amount: i32) {
    apply_player_debuff(powers, |powers| powers.weak += amount);
}

pub fn apply_player_vulnerable(powers: &mut PlayerPowers, amount: i32) {
    apply_player_debuff(powers, |powers| powers.vulnerable += amount);
}

pub fn apply_player_frail(powers: &mut PlayerPowers, amount: i32) {
    apply_player_debuff(powers, |powers| powers.frail += amount);
}

pub fn reduce_player_strength(powers: &mut PlayerPowers, amount: i32) {
    apply_player_debuff(powers, |powers| powers.strength -= amount);
}

pub fn reduce_player_dexterity(powers: &mut PlayerPowers, amount: i32) {
    apply_player_debuff(powers, |powers| powers.dexterity -= amount);
}

pub fn clear_player_debuffs(powers: &mut PlayerPowers) {
    if powers.strength < 0 {
        powers.strength = 0;
    }
    if powers.dexterity < 0 {
        powers.dexterity = 0;
    }
    powers.weak = 0;
    powers.frail = 0;
    powers.vulnerable = 0;
}

fn apply_player_debuff(powers: &mut PlayerPowers, apply: impl FnOnce(&mut PlayerPowers)) {
    if powers.artifact > 0 {
        powers.artifact -= 1;
    } else {
        apply(powers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulnerable_increases_attack_damage_by_fifty_percent_floored() {
        assert_eq!(attack_damage_with_vulnerable(6, 2), 9);
        assert_eq!(attack_damage_with_vulnerable(7, 1), 10);
        assert_eq!(attack_damage_with_vulnerable(6, 0), 6);
    }

    #[test]
    fn vulnerable_bonus_can_use_alternate_multiplier() {
        assert_eq!(attack_damage_with_vulnerable_bonus(8, 1, 3, 4), 14);
        assert_eq!(attack_damage_with_vulnerable_bonus(7, 1, 3, 4), 12);
        assert_eq!(attack_damage_with_vulnerable_bonus(8, 0, 3, 4), 8);
    }

    #[test]
    fn strength_modifies_attack_damage() {
        let player = PlayerPowers {
            strength: 2,
            ..Default::default()
        };

        assert_eq!(calculate_attack_damage(6, player, 0, 0), 8);
    }

    #[test]
    fn weak_modifies_outgoing_attack_damage_with_floor() {
        let player = PlayerPowers {
            weak: 1,
            ..Default::default()
        };

        assert_eq!(calculate_attack_damage(6, player, 0, 0), 4);
        assert_eq!(calculate_attack_damage(7, player, 0, 0), 5);
    }

    #[test]
    fn strength_and_weak_apply_in_order_before_vulnerable() {
        let player = PlayerPowers {
            strength: 1,
            weak: 1,
            ..Default::default()
        };

        assert_eq!(calculate_attack_damage(7, player, 0, 0), 6);
        assert_eq!(calculate_attack_damage(7, player, 0, 2), 9);
    }

    #[test]
    fn temp_strength_modifies_attack_damage() {
        let player = PlayerPowers::default();

        assert_eq!(calculate_attack_damage(6, player, 2, 0), 8);
    }

    #[test]
    fn dexterity_increases_block_from_cards() {
        let player = PlayerPowers {
            dexterity: 2,
            ..Default::default()
        };

        assert_eq!(calculate_block(5, player), 7);
    }

    #[test]
    fn player_powers_with_dexterity_round_trip_through_json() {
        let powers = PlayerPowers {
            dexterity: 3,
            ..Default::default()
        };
        let json = serde_json::to_string(&powers).expect("powers serialize");
        let restored: PlayerPowers = serde_json::from_str(&json).expect("powers deserialize");

        assert_eq!(restored, powers);
    }

    #[test]
    fn frail_reduces_block_from_cards_with_floor() {
        let player = PlayerPowers {
            frail: 1,
            ..Default::default()
        };

        assert_eq!(calculate_block(5, player), 3);
        assert_eq!(calculate_block(7, player), 5);
    }

    #[test]
    fn dexterity_and_frail_apply_in_order() {
        let player = PlayerPowers {
            dexterity: 2,
            frail: 1,
            ..Default::default()
        };

        assert_eq!(calculate_block(5, player), 5);
    }

    #[test]
    fn artifact_blocks_one_player_debuff() {
        let mut powers = PlayerPowers {
            artifact: 1,
            ..Default::default()
        };

        apply_player_weak(&mut powers, 2);
        apply_player_vulnerable(&mut powers, 3);

        assert_eq!(powers.artifact, 0);
        assert_eq!(powers.weak, 0);
        assert_eq!(powers.vulnerable, 3);
    }
}
