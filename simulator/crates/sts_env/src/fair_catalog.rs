//! Authoritative fair serialized content identities.
//!
//! Values are the exact strings currently emitted by fair observations. This
//! catalog is metadata only; it never reads gameplay state.

use crate::combat_observation::potion_key;
use serde::Serialize;
use sts_core::adapter_internals::{
    content::{
        cards::{public_card_definitions, THE_BOMB_TURNS},
        monsters::public_monster_definitions,
    },
    potion::Potion,
    relic::{Relic, ALL_RELICS},
    Event,
};

/// Closed fair content-identity catalogs used to generate Python StrEnums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FairContentCatalog {
    pub relics: Vec<&'static str>,
    pub potions: Vec<&'static str>,
    pub cards: Vec<&'static str>,
    pub monsters: Vec<&'static str>,
    pub events: Vec<&'static str>,
    pub powers: Vec<&'static str>,
    pub counters: Vec<&'static str>,
}

#[must_use]
pub fn fair_content_catalog() -> FairContentCatalog {
    assert_all_potions(Potion::Fire);
    assert_all_events(Event::Neow);
    FairContentCatalog {
        relics: unique_sorted(ALL_RELICS.iter().copied().map(Relic::trace_name)),
        potions: unique_sorted(ALL_POTIONS.iter().copied().map(potion_key)),
        cards: unique_sorted(public_card_definitions().map(|definition| definition.key)),
        monsters: unique_sorted(public_monster_definitions().map(|definition| definition.name)),
        events: unique_sorted(ALL_EVENTS.iter().copied().map(event_key)),
        powers: unique_sorted(fair_power_keys()),
        counters: unique_sorted(fair_counter_keys()),
    }
}

fn unique_sorted(values: impl IntoIterator<Item = &'static str>) -> Vec<&'static str> {
    let mut values: Vec<&'static str> = values.into_iter().collect();
    values.sort_unstable();
    values.dedup();
    values
}

macro_rules! closed_variants {
    ($ty:ty, $all:ident, $assert:ident, $($variant:ident),+ $(,)?) => {
        const $all: &[$ty] = &[$(<$ty>::$variant),+];
        const fn $assert(value: $ty) {
            match value {
                $(<$ty>::$variant => {},)+
            }
        }
    };
}

closed_variants!(
    Potion,
    ALL_POTIONS,
    assert_all_potions,
    Fire,
    Block,
    Fear,
    GamblersBrew,
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
);

closed_variants!(
    Event,
    ALL_EVENTS,
    assert_all_events,
    Neow,
    SpireHeart,
    AccursedBlacksmith,
    BonfireElementals,
    Designer,
    Duplicator,
    FountainOfCleansing,
    GoldenShrine,
    BigFish,
    TheCleric,
    DeadAdventurer,
    GoldenIdol,
    WingStatue,
    WorldOfGoop,
    TheSsssserpent,
    LivingWall,
    HypnotizingColoredMushrooms,
    ScrapOoze,
    ShiningLight,
    FaceTrader,
    Nloth,
    NoteForYourself,
    SecretPortal,
    TheJoust,
    WeMeetAgain,
    TheWomanInBlue,
    Transmorgrifier,
    Purifier,
    UpgradeShrine,
    WheelOfChange,
    MatchAndKeep,
    Addict,
    BackToBasics,
    Beggar,
    Colosseum,
    CursedTome,
    DrugDealer,
    ForgottenAltar,
    Ghosts,
    KnowingSkull,
    MaskedBandits,
    Nest,
    TheLibrary,
    TheMausoleum,
    Vampires,
    Lab,
    Falling,
    MindBloom,
    MoaiHead,
    MysteriousSphere,
    SensoryStone,
    TombOfLordRedMask,
    WindingHalls,
);

fn event_key(event: Event) -> &'static str {
    match event {
        Event::Neow => "Neow",
        Event::SpireHeart => "SpireHeart",
        Event::AccursedBlacksmith => "AccursedBlacksmith",
        Event::BonfireElementals => "BonfireElementals",
        Event::Designer => "Designer",
        Event::Duplicator => "Duplicator",
        Event::FountainOfCleansing => "FountainOfCleansing",
        Event::GoldenShrine => "GoldenShrine",
        Event::BigFish => "BigFish",
        Event::TheCleric => "TheCleric",
        Event::DeadAdventurer => "DeadAdventurer",
        Event::GoldenIdol => "GoldenIdol",
        Event::WingStatue => "WingStatue",
        Event::WorldOfGoop => "WorldOfGoop",
        Event::TheSsssserpent => "TheSsssserpent",
        Event::LivingWall => "LivingWall",
        Event::HypnotizingColoredMushrooms => "HypnotizingColoredMushrooms",
        Event::ScrapOoze => "ScrapOoze",
        Event::ShiningLight => "ShiningLight",
        Event::FaceTrader => "FaceTrader",
        Event::Nloth => "Nloth",
        Event::NoteForYourself => "NoteForYourself",
        Event::SecretPortal => "SecretPortal",
        Event::TheJoust => "TheJoust",
        Event::WeMeetAgain => "WeMeetAgain",
        Event::TheWomanInBlue => "TheWomanInBlue",
        Event::Transmorgrifier => "Transmorgrifier",
        Event::Purifier => "Purifier",
        Event::UpgradeShrine => "UpgradeShrine",
        Event::WheelOfChange => "WheelOfChange",
        Event::MatchAndKeep => "MatchAndKeep",
        Event::Addict => "Addict",
        Event::BackToBasics => "BackToBasics",
        Event::Beggar => "Beggar",
        Event::Colosseum => "Colosseum",
        Event::CursedTome => "CursedTome",
        Event::DrugDealer => "DrugDealer",
        Event::ForgottenAltar => "ForgottenAltar",
        Event::Ghosts => "Ghosts",
        Event::KnowingSkull => "KnowingSkull",
        Event::MaskedBandits => "MaskedBandits",
        Event::Nest => "Nest",
        Event::TheLibrary => "TheLibrary",
        Event::TheMausoleum => "TheMausoleum",
        Event::Vampires => "Vampires",
        Event::Lab => "Lab",
        Event::Falling => "Falling",
        Event::MindBloom => "MindBloom",
        Event::MoaiHead => "MoaiHead",
        Event::MysteriousSphere => "MysteriousSphere",
        Event::SensoryStone => "SensoryStone",
        Event::TombOfLordRedMask => "TombOfLordRedMask",
        Event::WindingHalls => "WindingHalls",
    }
}

fn fair_power_keys() -> Vec<&'static str> {
    let mut keys = Vec::from(FAIR_STATIC_POWER_KEYS);
    let bomb_turns = usize::try_from(THE_BOMB_TURNS).expect("The Bomb turns fit usize");
    keys.extend(FAIR_BOMB_POWER_KEYS.iter().copied().take(bomb_turns));
    keys
}

/// Static power keys actually emitted by fair combat projection.
const FAIR_STATIC_POWER_KEYS: &[&str] = &[
    "after_image",
    "anger",
    "artifact",
    "barricade",
    "berserk",
    "brutality",
    "buffer",
    "combust",
    "combust_damage",
    "confusion",
    "constricted",
    "corruption",
    "creative_ai",
    "curl_up",
    "dark_embrace",
    "demon_form",
    "dexterity",
    "double_tap",
    "duplication",
    "entangled",
    "evolve",
    "explosive",
    "fasting",
    "feel_no_pain",
    "fire_breathing",
    "flight",
    "focus",
    "frail",
    "hex",
    "intangible",
    "juggernaut",
    "like_water",
    "lock_on",
    "lose_dexterity",
    "lose_strength",
    "magnetism",
    "malleable",
    "mantra",
    "mark",
    "mayhem",
    "metallicize",
    "mode_shift",
    "nirvana",
    "no_block",
    "no_draw",
    "painful_stabs",
    "panache",
    "panache_cards_played",
    "plated_armor",
    "poison",
    "rage",
    "regen",
    "restore_strength",
    "ritual",
    "rupture",
    "sadistic_nature",
    "slow",
    "spikes",
    "spore_cloud",
    "static_discharge",
    "storm",
    "strength",
    "strength_up",
    "temporary_thorns",
    "thorns",
    "vigor",
    "vulnerable",
    "weak",
    "wrath",
];

const FAIR_BOMB_POWER_KEYS: &[&str] = &["bomb_1_damage", "bomb_2_damage", "bomb_3_damage"];

fn fair_counter_keys() -> Vec<&'static str> {
    Vec::from(FAIR_COUNTER_KEYS)
}

const FAIR_COUNTER_KEYS: &[&str] = &[
    "active",
    "armed",
    "attack_played",
    "attacks",
    "attacks_last_turn",
    "attacks_played_this_turn",
    "attacks_this_combat",
    "attacks_this_turn",
    "available",
    "cards",
    "cards_discarded_this_turn",
    "cards_last_turn",
    "cards_played_this_turn",
    "cards_this_turn",
    "charges",
    "charges_remaining",
    "chests_remaining",
    "combats_remaining",
    "lifts",
    "next_turn_block",
    "player_turns_started",
    "power_played",
    "rooms",
    "shuffles",
    "skill_played",
    "skills_this_turn",
    "triggers",
    "turns",
    "used_this_turn",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_are_nonempty_unique_and_sorted() {
        let catalog = fair_content_catalog();
        for (name, values) in [
            ("relics", &catalog.relics),
            ("potions", &catalog.potions),
            ("cards", &catalog.cards),
            ("monsters", &catalog.monsters),
            ("events", &catalog.events),
            ("powers", &catalog.powers),
            ("counters", &catalog.counters),
        ] {
            assert!(!values.is_empty(), "{name} catalog is empty");
            let mut sorted = values.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(values, &sorted, "{name} catalog is not unique and sorted");
            for value in values {
                assert!(!value.is_empty(), "{name} contains an empty identity");
            }
        }
        assert_all_potions(Potion::Fire);
        assert_all_events(Event::Neow);
        assert_eq!(ALL_POTIONS.len(), catalog.potions.len());
        assert_eq!(ALL_EVENTS.len(), catalog.events.len());
        assert_eq!(ALL_RELICS.len(), catalog.relics.len());
        assert!(catalog.powers.contains(&"bomb_3_damage"));
        assert!(!catalog.powers.contains(&"bomb_4_damage"));
        assert!(catalog.potions.contains(&"fire"));
        assert!(catalog.relics.contains(&"Burning Blood"));
        assert!(catalog.cards.contains(&"Strike_R"));
        assert!(catalog.monsters.contains(&"Jaw Worm"));
        assert!(catalog.events.contains(&"Neow"));
    }
}
