use crate::ids::ContentId;
use serde::{Deserialize, Serialize};

pub const MAX_POTIONS: usize = 3;

pub const FIRE_POTION_DAMAGE: i32 = 20;
pub const BLOCK_POTION_BLOCK: i32 = 12;
pub const FEAR_POTION_WEAK: i32 = 3;
pub const BLOOD_POTION_HEAL_PERCENT: i32 = 20;
pub const HEART_OF_IRON_METALLICIZE: i32 = 6;
pub const CULTIST_POTION_RITUAL: i32 = 1;
pub const DEXTERITY_POTION_DEXTERITY: i32 = 2;
pub const ENERGY_POTION_ENERGY: i32 = 2;
pub const ESSENCE_OF_STEEL_PLATED_ARMOR: i32 = 4;
pub const EXPLOSIVE_POTION_DAMAGE: i32 = 10;
pub const LIQUID_BRONZE_THORNS: i32 = 3;
pub const REGEN_POTION_REGEN: i32 = 5;
pub const STRENGTH_POTION_STRENGTH: i32 = 2;
pub const FLEX_POTION_TEMP_STRENGTH: i32 = 5;
pub const SPEED_POTION_TEMP_DEXTERITY: i32 = 5;
pub const WEAK_POTION_WEAK: i32 = 3;
pub const FRUIT_JUICE_MAX_HP: i32 = 5;
pub const SWIFT_POTION_DRAW: usize = 3;
pub const GAMBLE_POTION_WIN_GOLD: i32 = 50;
pub const GAMBLE_POTION_LOSS_GOLD: i32 = 50;

/// Content id for [Potion::Fire].
pub const FIRE_POTION_ID: ContentId = ContentId::new(200);
/// Content id for [Potion::Block].
pub const BLOCK_POTION_ID: ContentId = ContentId::new(201);
/// Content id for [Potion::Fear].
pub const FEAR_POTION_ID: ContentId = ContentId::new(202);
/// Content id for [Potion::Gamble].
pub const GAMBLE_POTION_ID: ContentId = ContentId::new(203);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Potion {
    Fire,
    Block,
    Fear,
    Gamble,
    Blood,
    Elixir,
    HeartOfIron,
    Dexterity,
    Energy,
    Explosive,
    Strength,
    Swift,
    Weak,
    Attack,
    Skill,
    Power,
    Colorless,
    Flex,
    Speed,
    BlessingOfTheForge,
    Regen,
    Ancient,
    LiquidBronze,
    EssenceOfSteel,
    Duplication,
    DistilledChaos,
    LiquidMemories,
    Cultist,
    FruitJuice,
    SneckoOil,
    Fairy,
    SmokeBomb,
    EntropicBrew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotionRarity {
    Common,
    Uncommon,
    Rare,
}

pub const IRONCLAD_POTION_POOL: [Potion; 33] = [
    Potion::Blood,
    Potion::Elixir,
    Potion::HeartOfIron,
    Potion::Block,
    Potion::Dexterity,
    Potion::Energy,
    Potion::Explosive,
    Potion::Fire,
    Potion::Strength,
    Potion::Swift,
    Potion::Weak,
    Potion::Fear,
    Potion::Attack,
    Potion::Skill,
    Potion::Power,
    Potion::Colorless,
    Potion::Flex,
    Potion::Speed,
    Potion::BlessingOfTheForge,
    Potion::Regen,
    Potion::Ancient,
    Potion::LiquidBronze,
    Potion::Gamble,
    Potion::EssenceOfSteel,
    Potion::Duplication,
    Potion::DistilledChaos,
    Potion::LiquidMemories,
    Potion::Cultist,
    Potion::FruitJuice,
    Potion::SneckoOil,
    Potion::Fairy,
    Potion::SmokeBomb,
    Potion::EntropicBrew,
];

impl Potion {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Potion::Fire => FIRE_POTION_ID,
            Potion::Block => BLOCK_POTION_ID,
            Potion::Fear => FEAR_POTION_ID,
            Potion::Gamble => GAMBLE_POTION_ID,
            _ => ContentId::new(1_000 + self.target_ordinal() as u64),
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == FIRE_POTION_ID => Some(Potion::Fire),
            id if id == BLOCK_POTION_ID => Some(Potion::Block),
            id if id == FEAR_POTION_ID => Some(Potion::Fear),
            id if id == GAMBLE_POTION_ID => Some(Potion::Gamble),
            id => Potion::from_target_ordinal((id.get().checked_sub(1_000)?) as u8),
        }
    }

    #[must_use]
    pub fn rarity(self) -> PotionRarity {
        match self {
            Potion::Attack
            | Potion::BlessingOfTheForge
            | Potion::Block
            | Potion::Blood
            | Potion::Colorless
            | Potion::Dexterity
            | Potion::Energy
            | Potion::Explosive
            | Potion::Fear
            | Potion::Fire
            | Potion::Flex
            | Potion::Speed
            | Potion::Strength
            | Potion::Swift
            | Potion::Weak
            | Potion::Power => PotionRarity::Common,
            Potion::Ancient
            | Potion::DistilledChaos
            | Potion::Duplication
            | Potion::Elixir
            | Potion::EssenceOfSteel
            | Potion::Gamble
            | Potion::LiquidBronze
            | Potion::LiquidMemories
            | Potion::Regen
            | Potion::Skill => PotionRarity::Uncommon,
            Potion::Cultist
            | Potion::EntropicBrew
            | Potion::Fairy
            | Potion::FruitJuice
            | Potion::HeartOfIron
            | Potion::SmokeBomb
            | Potion::SneckoOil => PotionRarity::Rare,
        }
    }

    fn target_ordinal(self) -> u8 {
        match self {
            Potion::Ancient => 3,
            Potion::Attack => 4,
            Potion::BlessingOfTheForge => 5,
            Potion::Block => 6,
            Potion::Blood => 7,
            Potion::Colorless => 9,
            Potion::Cultist => 10,
            Potion::Dexterity => 12,
            Potion::DistilledChaos => 13,
            Potion::Duplication => 14,
            Potion::Elixir => 15,
            Potion::Energy => 16,
            Potion::EntropicBrew => 17,
            Potion::EssenceOfSteel => 19,
            Potion::Explosive => 20,
            Potion::Fairy => 21,
            Potion::Fear => 22,
            Potion::Fire => 23,
            Potion::Flex => 24,
            Potion::FruitJuice => 26,
            Potion::Gamble => 27,
            Potion::HeartOfIron => 29,
            Potion::LiquidBronze => 30,
            Potion::LiquidMemories => 31,
            Potion::Power => 34,
            Potion::Regen => 35,
            Potion::Skill => 36,
            Potion::SmokeBomb => 37,
            Potion::SneckoOil => 38,
            Potion::Speed => 39,
            Potion::Strength => 41,
            Potion::Swift => 42,
            Potion::Weak => 43,
        }
    }

    fn from_target_ordinal(ordinal: u8) -> Option<Self> {
        match ordinal {
            3 => Some(Potion::Ancient),
            4 => Some(Potion::Attack),
            5 => Some(Potion::BlessingOfTheForge),
            6 => Some(Potion::Block),
            7 => Some(Potion::Blood),
            9 => Some(Potion::Colorless),
            10 => Some(Potion::Cultist),
            12 => Some(Potion::Dexterity),
            13 => Some(Potion::DistilledChaos),
            14 => Some(Potion::Duplication),
            15 => Some(Potion::Elixir),
            16 => Some(Potion::Energy),
            17 => Some(Potion::EntropicBrew),
            19 => Some(Potion::EssenceOfSteel),
            20 => Some(Potion::Explosive),
            21 => Some(Potion::Fairy),
            22 => Some(Potion::Fear),
            23 => Some(Potion::Fire),
            24 => Some(Potion::Flex),
            26 => Some(Potion::FruitJuice),
            27 => Some(Potion::Gamble),
            29 => Some(Potion::HeartOfIron),
            30 => Some(Potion::LiquidBronze),
            31 => Some(Potion::LiquidMemories),
            34 => Some(Potion::Power),
            35 => Some(Potion::Regen),
            36 => Some(Potion::Skill),
            37 => Some(Potion::SmokeBomb),
            38 => Some(Potion::SneckoOil),
            39 => Some(Potion::Speed),
            41 => Some(Potion::Strength),
            42 => Some(Potion::Swift),
            43 => Some(Potion::Weak),
            _ => None,
        }
    }

    #[must_use]
    pub fn requires_target(self) -> bool {
        matches!(self, Potion::Fire | Potion::Fear | Potion::Weak)
    }

    #[must_use]
    pub fn requires_combat(self) -> bool {
        matches!(
            self,
            Potion::Fire
                | Potion::Fear
                | Potion::Block
                | Potion::Weak
                | Potion::Blood
                | Potion::HeartOfIron
                | Potion::Cultist
                | Potion::Dexterity
                | Potion::Energy
                | Potion::EssenceOfSteel
                | Potion::Explosive
                | Potion::LiquidBronze
                | Potion::Regen
                | Potion::Strength
                | Potion::Flex
                | Potion::Speed
                | Potion::Swift
                | Potion::BlessingOfTheForge
                | Potion::Power
        )
    }

    #[must_use]
    pub fn uses_rng(self) -> bool {
        matches!(self, Potion::Gamble)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_potion_round_trips_through_json() {
        let potion = Potion::Fire;

        let json = serde_json::to_string(&potion).expect("potion serializes");
        let restored: Potion = serde_json::from_str(&json).expect("potion deserializes");

        assert_eq!(restored, potion);
    }

    #[test]
    fn fire_potion_content_id_maps_both_ways() {
        assert_eq!(Potion::Fire.content_id(), FIRE_POTION_ID);
        assert_eq!(Potion::from_content_id(FIRE_POTION_ID), Some(Potion::Fire));
        assert_eq!(Potion::from_content_id(ContentId::new(999)), None);
    }

    #[test]
    fn block_potion_round_trips_through_json() {
        let potion = Potion::Block;

        let json = serde_json::to_string(&potion).expect("potion serializes");
        let restored: Potion = serde_json::from_str(&json).expect("potion deserializes");

        assert_eq!(restored, potion);
    }

    #[test]
    fn block_potion_content_id_maps_both_ways() {
        assert_eq!(Potion::Block.content_id(), BLOCK_POTION_ID);
        assert_eq!(
            Potion::from_content_id(BLOCK_POTION_ID),
            Some(Potion::Block)
        );
    }

    #[test]
    fn block_potion_does_not_require_target() {
        assert!(Potion::Fire.requires_target());
        assert!(!Potion::Block.requires_target());
    }

    #[test]
    fn fear_potion_content_id_maps_both_ways() {
        assert_eq!(Potion::Fear.content_id(), FEAR_POTION_ID);
        assert_eq!(Potion::from_content_id(FEAR_POTION_ID), Some(Potion::Fear));
    }

    #[test]
    fn gamble_potion_uses_rng() {
        assert!(Potion::Gamble.uses_rng());
        assert!(!Potion::Block.uses_rng());
    }

    #[test]
    fn ironclad_pool_matches_target_pool_size() {
        assert_eq!(IRONCLAD_POTION_POOL.len(), 33);
        let unique: std::collections::BTreeSet<_> = IRONCLAD_POTION_POOL.iter().copied().collect();
        assert_eq!(unique.len(), IRONCLAD_POTION_POOL.len());
    }

    #[test]
    fn ironclad_pool_preserves_target_order_prefix() {
        assert_eq!(
            &IRONCLAD_POTION_POOL[..8],
            &[
                Potion::Blood,
                Potion::Elixir,
                Potion::HeartOfIron,
                Potion::Block,
                Potion::Dexterity,
                Potion::Energy,
                Potion::Explosive,
                Potion::Fire,
            ]
        );
    }
}
