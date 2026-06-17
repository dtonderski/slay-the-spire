use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulnerable_increases_attack_damage_by_fifty_percent_floored() {
        assert_eq!(attack_damage_with_vulnerable(6, 2), 9);
        assert_eq!(attack_damage_with_vulnerable(7, 1), 10);
        assert_eq!(attack_damage_with_vulnerable(6, 0), 6);
    }
}
