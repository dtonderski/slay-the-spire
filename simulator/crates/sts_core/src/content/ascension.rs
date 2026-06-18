/// Ascension modifiers layered on top of base game rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AscensionConfig {
    pub level: u8,
}

impl AscensionConfig {
    pub const MAX_LEVEL: u8 = 20;

    #[must_use]
    pub fn new(level: u8) -> Self {
        Self {
            level: level.min(Self::MAX_LEVEL),
        }
    }

    #[must_use]
    pub fn elite_rooms_enabled(self) -> bool {
        self.level >= 1
    }

    /// Flat bonus added to normal enemy attack damage (A2+).
    #[must_use]
    pub fn normal_enemy_damage_bonus(self) -> i32 {
        if self.level >= 2 {
            2
        } else {
            0
        }
    }

    /// Percent bonus applied to enemy max HP (A7+).
    #[must_use]
    pub fn enemy_hp_bonus_percent(self) -> i32 {
        if self.level >= 7 {
            15
        } else {
            0
        }
    }

    #[must_use]
    pub fn ascenders_bane_in_deck(self) -> bool {
        self.level >= 10
    }

    /// Deadly enemies deal extra damage (A17+).
    #[must_use]
    pub fn deadly_enemies_damage_bonus(self) -> i32 {
        if self.level >= 17 {
            1
        } else {
            0
        }
    }

    #[must_use]
    pub fn double_boss(self) -> bool {
        self.level >= 20
    }

    #[must_use]
    pub fn scaled_enemy_hp(self, base_hp: i32) -> i32 {
        base_hp + base_hp * self.enemy_hp_bonus_percent() / 100
    }

    #[must_use]
    pub fn scaled_attack_damage(self, base_damage: i32) -> i32 {
        base_damage + self.normal_enemy_damage_bonus() + self.deadly_enemies_damage_bonus()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a0_has_no_modifiers() {
        let config = AscensionConfig::new(0);
        assert!(!config.elite_rooms_enabled());
        assert_eq!(config.normal_enemy_damage_bonus(), 0);
        assert_eq!(config.enemy_hp_bonus_percent(), 0);
        assert!(!config.ascenders_bane_in_deck());
        assert_eq!(config.deadly_enemies_damage_bonus(), 0);
        assert!(!config.double_boss());
    }

    #[test]
    fn a1_enables_elite_rooms() {
        assert!(AscensionConfig::new(1).elite_rooms_enabled());
    }

    #[test]
    fn a2_adds_normal_enemy_damage() {
        assert_eq!(AscensionConfig::new(2).normal_enemy_damage_bonus(), 2);
    }

    #[test]
    fn a7_scales_enemy_hp() {
        let config = AscensionConfig::new(7);
        assert_eq!(config.scaled_enemy_hp(100), 115);
    }

    #[test]
    fn a10_adds_ascenders_bane() {
        assert!(AscensionConfig::new(10).ascenders_bane_in_deck());
    }

    #[test]
    fn a17_adds_deadly_damage() {
        assert_eq!(AscensionConfig::new(17).deadly_enemies_damage_bonus(), 1);
    }

    #[test]
    fn a20_enables_double_boss() {
        assert!(AscensionConfig::new(20).double_boss());
    }

    #[test]
    fn ascension_config_round_trips_through_json() {
        let config = AscensionConfig::new(12);
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: AscensionConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, config);
    }
}
