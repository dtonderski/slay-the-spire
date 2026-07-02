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
