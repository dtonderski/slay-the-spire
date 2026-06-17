use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlayerPowers {
    pub strength: i32,
    pub weak: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MonsterPowers {
    pub vulnerable: i32,
}

/// Slay the Spire-style vulnerable bonus: attack damage is increased by 50%, floored.
#[must_use]
pub fn attack_damage_with_vulnerable(base: i32, vulnerable: i32) -> i32 {
    if vulnerable > 0 {
        base + base / 2
    } else {
        base
    }
}

/// Player attack modifiers applied before target vulnerable:
/// 1. add strength
/// 2. if weak, multiply by 0.75 and floor via integer `base * 3 / 4`
/// 3. apply target vulnerable
#[must_use]
pub fn calculate_attack_damage(base: i32, player: PlayerPowers, target_vulnerable: i32) -> i32 {
    let with_strength = base + player.strength;
    let with_weak = if player.weak > 0 {
        with_strength * 3 / 4
    } else {
        with_strength
    };

    attack_damage_with_vulnerable(with_weak, target_vulnerable)
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
    fn strength_modifies_attack_damage() {
        let player = PlayerPowers {
            strength: 2,
            weak: 0,
        };

        assert_eq!(calculate_attack_damage(6, player, 0), 8);
    }

    #[test]
    fn weak_modifies_outgoing_attack_damage_with_floor() {
        let player = PlayerPowers {
            strength: 0,
            weak: 1,
        };

        assert_eq!(calculate_attack_damage(6, player, 0), 4);
        assert_eq!(calculate_attack_damage(7, player, 0), 5);
    }

    #[test]
    fn strength_and_weak_apply_in_order_before_vulnerable() {
        let player = PlayerPowers {
            strength: 1,
            weak: 1,
        };

        assert_eq!(calculate_attack_damage(7, player, 0), 6);
        assert_eq!(calculate_attack_damage(7, player, 2), 9);
    }
}
