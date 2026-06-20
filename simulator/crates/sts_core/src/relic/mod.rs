use crate::action::InternalAction;
use crate::card::CardType;
use crate::combat::CombatState;
use crate::rng::{JavaRng, StsRng};
use serde::{Deserialize, Serialize};

use crate::ids::ContentId;

/// Strength granted by [Relic::Vajra] at combat start.
pub const VAJRA_STRENGTH: i32 = 1;
/// Dexterity granted by [Relic::OddlySmoothStone] at combat start.
pub const ODDLY_SMOOTH_STONE_DEXTERITY: i32 = 1;
/// Max HP granted by [Relic::Strawberry] on pickup.
pub const STRAWBERRY_MAX_HP: i32 = 7;
/// Energy per turn granted by [Relic::CoffeeDripper] on pickup.
pub const COFFEE_DRIPPER_ENERGY: i32 = 1;
/// Block granted by [Relic::Anchor] at combat start.
pub const ANCHOR_BLOCK: i32 = 10;
/// Cards played before [Relic::InkBottle] draws a card.
pub const INK_BOTTLE_THRESHOLD: u32 = 10;
/// Attacks played in one turn before [Relic::OrnamentalFan] grants block.
pub const ORNAMENTAL_FAN_THRESHOLD: u32 = 3;
/// Block granted by [Relic::OrnamentalFan] every third attack in a turn.
pub const ORNAMENTAL_FAN_BLOCK: i32 = 4;

/// Content id for [Relic::Vajra].
pub const VAJRA_ID: ContentId = ContentId::new(300);
/// Content id for [Relic::OddlySmoothStone].
pub const ODDLY_SMOOTH_STONE_ID: ContentId = ContentId::new(301);
/// Content id for [Relic::Strawberry].
pub const STRAWBERRY_ID: ContentId = ContentId::new(302);
/// Content id for [Relic::CoffeeDripper].
pub const COFFEE_DRIPPER_ID: ContentId = ContentId::new(303);
/// Content id for [Relic::Anchor].
pub const ANCHOR_ID: ContentId = ContentId::new(304);
/// Content id for [Relic::InkBottle].
pub const INK_BOTTLE_ID: ContentId = ContentId::new(305);
/// Content id for [Relic::OrnamentalFan].
pub const ORNAMENTAL_FAN_ID: ContentId = ContentId::new(306);
/// Content id for [Relic::IceCream].
pub const ICE_CREAM_ID: ContentId = ContentId::new(307);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RelicCounters {
    #[serde(default)]
    pub ink_bottle_cards_played: u32,
    #[serde(default)]
    pub ornamental_fan_attacks_this_turn: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelicTier {
    Common,
    Uncommon,
    Rare,
    Boss,
    Shop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelicKey {
    Whetstone,
    TheBoot,
    BloodVial,
    MealTicket,
    PenNib,
    Akabeko,
    Lantern,
    RegalPillow,
    BagOfPreparation,
    AncientTeaSet,
    SmilingMask,
    PotionBelt,
    PreservedInsect,
    Omamori,
    MawBank,
    ArtOfWar,
    ToyOrnithopter,
    CeramicFish,
    Vajra,
    CentennialPuzzle,
    Strawberry,
    HappyFlower,
    OddlySmoothStone,
    WarPaint,
    BronzeScales,
    JuzuBracelet,
    DreamCatcher,
    Nunchaku,
    TinyChest,
    Orichalcum,
    Anchor,
    BagOfMarbles,
    RedSkull,
    BottledTornado,
    Sundial,
    Kunai,
    Pear,
    BlueCandle,
    EternalFeather,
    StrikeDummy,
    SingingBowl,
    Matryoshka,
    InkBottle,
    TheCourier,
    FrozenEgg,
    OrnamentalFan,
    BottledLightning,
    GremlinHorn,
    HornCleat,
    ToxicEgg,
    LetterOpener,
    QuestionCard,
    BottledFlame,
    Shuriken,
    MoltenEgg,
    MeatOnTheBone,
    DarkstonePeriapt,
    MummifiedHand,
    Pantograph,
    WhiteBeastStatue,
    MercuryHourglass,
    SelfFormingClay,
    PaperPhrog,
    Ginger,
    OldCoin,
    BirdFacedUrn,
    UnceasingTop,
    Torii,
    StoneCalendar,
    Shovel,
    WingBoots,
    ThreadAndNeedle,
    Turnip,
    IceCream,
    Calipers,
    LizardTail,
    PrayerWheel,
    Girya,
    DeadBranch,
    DuVuDoll,
    Pocketwatch,
    Mango,
    IncenseBurner,
    GamblingChip,
    PeacePipe,
    CaptainsWheel,
    FossilizedHelix,
    TungstenRod,
    MagicFlower,
    CharonsAshes,
    ChampionBelt,
    FusionHammer,
    VelvetChoker,
    RunicDome,
    SlaversCollar,
    SneckoEye,
    PandorasBox,
    CursedKey,
    BustedCrown,
    Ectoplasm,
    TinyHouse,
    Sozu,
    PhilosophersStone,
    Astrolabe,
    BlackStar,
    SacredBark,
    EmptyCage,
    RunicPyramid,
    CallingBell,
    CoffeeDripper,
    BlackBlood,
    MarkOfPain,
    RunicCube,
    SlingOfCourage,
    HandDrill,
    Toolbox,
    ChemicalX,
    LeesWaffle,
    Orrery,
    DollysMirror,
    OrangePellets,
    PrismaticShard,
    ClockworkSouvenir,
    FrozenEye,
    TheAbacus,
    MedicalKit,
    Cauldron,
    StrangeSpoon,
    MembershipCard,
    Brimstone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelicPoolState {
    pub common: Vec<RelicKey>,
    pub uncommon: Vec<RelicKey>,
    pub rare: Vec<RelicKey>,
    pub shop: Vec<RelicKey>,
    pub boss: Vec<RelicKey>,
}

pub const IRONCLAD_COMMON_RELIC_POOL: [RelicKey; 33] = [
    RelicKey::Whetstone,
    RelicKey::TheBoot,
    RelicKey::BloodVial,
    RelicKey::MealTicket,
    RelicKey::PenNib,
    RelicKey::Akabeko,
    RelicKey::Lantern,
    RelicKey::RegalPillow,
    RelicKey::BagOfPreparation,
    RelicKey::AncientTeaSet,
    RelicKey::SmilingMask,
    RelicKey::PotionBelt,
    RelicKey::PreservedInsect,
    RelicKey::Omamori,
    RelicKey::MawBank,
    RelicKey::ArtOfWar,
    RelicKey::ToyOrnithopter,
    RelicKey::CeramicFish,
    RelicKey::Vajra,
    RelicKey::CentennialPuzzle,
    RelicKey::Strawberry,
    RelicKey::HappyFlower,
    RelicKey::OddlySmoothStone,
    RelicKey::WarPaint,
    RelicKey::BronzeScales,
    RelicKey::JuzuBracelet,
    RelicKey::DreamCatcher,
    RelicKey::Nunchaku,
    RelicKey::TinyChest,
    RelicKey::Orichalcum,
    RelicKey::Anchor,
    RelicKey::BagOfMarbles,
    RelicKey::RedSkull,
];

pub const IRONCLAD_UNCOMMON_RELIC_POOL: [RelicKey; 30] = [
    RelicKey::BottledTornado,
    RelicKey::Sundial,
    RelicKey::Kunai,
    RelicKey::Pear,
    RelicKey::BlueCandle,
    RelicKey::EternalFeather,
    RelicKey::StrikeDummy,
    RelicKey::SingingBowl,
    RelicKey::Matryoshka,
    RelicKey::InkBottle,
    RelicKey::TheCourier,
    RelicKey::FrozenEgg,
    RelicKey::OrnamentalFan,
    RelicKey::BottledLightning,
    RelicKey::GremlinHorn,
    RelicKey::HornCleat,
    RelicKey::ToxicEgg,
    RelicKey::LetterOpener,
    RelicKey::QuestionCard,
    RelicKey::BottledFlame,
    RelicKey::Shuriken,
    RelicKey::MoltenEgg,
    RelicKey::MeatOnTheBone,
    RelicKey::DarkstonePeriapt,
    RelicKey::MummifiedHand,
    RelicKey::Pantograph,
    RelicKey::WhiteBeastStatue,
    RelicKey::MercuryHourglass,
    RelicKey::SelfFormingClay,
    RelicKey::PaperPhrog,
];

pub const IRONCLAD_RARE_RELIC_POOL: [RelicKey; 28] = [
    RelicKey::Ginger,
    RelicKey::OldCoin,
    RelicKey::BirdFacedUrn,
    RelicKey::UnceasingTop,
    RelicKey::Torii,
    RelicKey::StoneCalendar,
    RelicKey::Shovel,
    RelicKey::WingBoots,
    RelicKey::ThreadAndNeedle,
    RelicKey::Turnip,
    RelicKey::IceCream,
    RelicKey::Calipers,
    RelicKey::LizardTail,
    RelicKey::PrayerWheel,
    RelicKey::Girya,
    RelicKey::DeadBranch,
    RelicKey::DuVuDoll,
    RelicKey::Pocketwatch,
    RelicKey::Mango,
    RelicKey::IncenseBurner,
    RelicKey::GamblingChip,
    RelicKey::PeacePipe,
    RelicKey::CaptainsWheel,
    RelicKey::FossilizedHelix,
    RelicKey::TungstenRod,
    RelicKey::MagicFlower,
    RelicKey::CharonsAshes,
    RelicKey::ChampionBelt,
];

pub const IRONCLAD_SHOP_RELIC_POOL: [RelicKey; 17] = [
    RelicKey::SlingOfCourage,
    RelicKey::HandDrill,
    RelicKey::Toolbox,
    RelicKey::ChemicalX,
    RelicKey::LeesWaffle,
    RelicKey::Orrery,
    RelicKey::DollysMirror,
    RelicKey::OrangePellets,
    RelicKey::PrismaticShard,
    RelicKey::ClockworkSouvenir,
    RelicKey::FrozenEye,
    RelicKey::TheAbacus,
    RelicKey::MedicalKit,
    RelicKey::Cauldron,
    RelicKey::StrangeSpoon,
    RelicKey::MembershipCard,
    RelicKey::Brimstone,
];

pub const IRONCLAD_BOSS_RELIC_POOL: [RelicKey; 22] = [
    RelicKey::FusionHammer,
    RelicKey::VelvetChoker,
    RelicKey::RunicDome,
    RelicKey::SlaversCollar,
    RelicKey::SneckoEye,
    RelicKey::PandorasBox,
    RelicKey::CursedKey,
    RelicKey::BustedCrown,
    RelicKey::Ectoplasm,
    RelicKey::TinyHouse,
    RelicKey::Sozu,
    RelicKey::PhilosophersStone,
    RelicKey::Astrolabe,
    RelicKey::BlackStar,
    RelicKey::SacredBark,
    RelicKey::EmptyCage,
    RelicKey::RunicPyramid,
    RelicKey::CallingBell,
    RelicKey::CoffeeDripper,
    RelicKey::BlackBlood,
    RelicKey::MarkOfPain,
    RelicKey::RunicCube,
];

pub fn initialize_ironclad_relic_pools(relic_rng: &mut StsRng) -> RelicPoolState {
    let mut common = IRONCLAD_COMMON_RELIC_POOL.to_vec();
    let mut uncommon = IRONCLAD_UNCOMMON_RELIC_POOL.to_vec();
    let mut rare = IRONCLAD_RARE_RELIC_POOL.to_vec();
    let mut shop = IRONCLAD_SHOP_RELIC_POOL.to_vec();
    let mut boss = IRONCLAD_BOSS_RELIC_POOL.to_vec();

    JavaRng::new(relic_rng.random_long()).collections_shuffle(&mut common);
    JavaRng::new(relic_rng.random_long()).collections_shuffle(&mut uncommon);
    JavaRng::new(relic_rng.random_long()).collections_shuffle(&mut rare);
    JavaRng::new(relic_rng.random_long()).collections_shuffle(&mut shop);
    JavaRng::new(relic_rng.random_long()).collections_shuffle(&mut boss);

    RelicPoolState {
        common,
        uncommon,
        rare,
        shop,
        boss,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relic {
    Vajra,
    OddlySmoothStone,
    Strawberry,
    CoffeeDripper,
    Anchor,
    InkBottle,
    OrnamentalFan,
    IceCream,
}

impl Relic {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Relic::Vajra => VAJRA_ID,
            Relic::OddlySmoothStone => ODDLY_SMOOTH_STONE_ID,
            Relic::Strawberry => STRAWBERRY_ID,
            Relic::CoffeeDripper => COFFEE_DRIPPER_ID,
            Relic::Anchor => ANCHOR_ID,
            Relic::InkBottle => INK_BOTTLE_ID,
            Relic::OrnamentalFan => ORNAMENTAL_FAN_ID,
            Relic::IceCream => ICE_CREAM_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == VAJRA_ID => Some(Relic::Vajra),
            id if id == ODDLY_SMOOTH_STONE_ID => Some(Relic::OddlySmoothStone),
            id if id == STRAWBERRY_ID => Some(Relic::Strawberry),
            id if id == COFFEE_DRIPPER_ID => Some(Relic::CoffeeDripper),
            id if id == ANCHOR_ID => Some(Relic::Anchor),
            id if id == INK_BOTTLE_ID => Some(Relic::InkBottle),
            id if id == ORNAMENTAL_FAN_ID => Some(Relic::OrnamentalFan),
            id if id == ICE_CREAM_ID => Some(Relic::IceCream),
            _ => None,
        }
    }
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) {
    for relic in relics {
        match relic {
            Relic::Vajra => {
                combat.player.powers.strength += VAJRA_STRENGTH;
            }
            Relic::OddlySmoothStone => {
                combat.player.powers.dexterity += ODDLY_SMOOTH_STONE_DEXTERITY;
            }
            Relic::Strawberry => {}
            Relic::CoffeeDripper => {}
            Relic::Anchor => {
                combat.player.block += ANCHOR_BLOCK;
            }
            Relic::InkBottle => {}
            Relic::OrnamentalFan => {}
            Relic::IceCream => {}
        }
    }
}

/// Whether player energy should carry over instead of refilling at turn start.
#[must_use]
pub fn preserves_energy_between_turns(relics: &[Relic]) -> bool {
    relics.contains(&Relic::IceCream)
}

pub fn reset_turn_relic_counters(state: &mut CombatState) {
    state.relic_counters.ornamental_fan_attacks_this_turn = 0;
}

#[must_use]
pub fn apply_on_card_play_relics(
    state: &mut CombatState,
    card_type: CardType,
) -> Vec<InternalAction> {
    let mut follow_ups = Vec::new();

    if state.relics.contains(&Relic::InkBottle) {
        state.relic_counters.ink_bottle_cards_played += 1;
        if state.relic_counters.ink_bottle_cards_played >= INK_BOTTLE_THRESHOLD {
            state.relic_counters.ink_bottle_cards_played = 0;
            follow_ups.push(InternalAction::DrawCards { count: 1 });
        }
    }

    if state.relics.contains(&Relic::OrnamentalFan) && card_type == CardType::Attack {
        state.relic_counters.ornamental_fan_attacks_this_turn += 1;
        if state.relic_counters.ornamental_fan_attacks_this_turn >= ORNAMENTAL_FAN_THRESHOLD {
            state.relic_counters.ornamental_fan_attacks_this_turn = 0;
            follow_ups.push(InternalAction::GainBlock {
                amount: ORNAMENTAL_FAN_BLOCK,
            });
        }
    }

    follow_ups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn ironclad_relic_pool_constants_match_target_sizes() {
        assert_eq!(IRONCLAD_COMMON_RELIC_POOL.len(), 33);
        assert_eq!(IRONCLAD_UNCOMMON_RELIC_POOL.len(), 30);
        assert_eq!(IRONCLAD_RARE_RELIC_POOL.len(), 28);
        assert_eq!(IRONCLAD_SHOP_RELIC_POOL.len(), 17);
        assert_eq!(IRONCLAD_BOSS_RELIC_POOL.len(), 22);
    }

    #[test]
    fn ironclad_relic_pool_initialization_consumes_five_relic_rng_draws() {
        let mut rng = StsRng::new(22_079_335_079);

        let pools = initialize_ironclad_relic_pools(&mut rng);

        assert_eq!(rng.counter(), 5);
        assert_eq!(pools.common.len(), 33);
        assert_eq!(pools.uncommon.len(), 30);
        assert_eq!(pools.rare.len(), 28);
        assert_eq!(pools.shop.len(), 17);
        assert_eq!(pools.boss.len(), 22);
    }

    #[test]
    fn ironclad_relic_pool_initialization_matches_codex04_prefixes() {
        let mut rng = StsRng::new(22_079_335_079);

        let pools = initialize_ironclad_relic_pools(&mut rng);

        assert_eq!(
            &pools.common[..8],
            &[
                RelicKey::ToyOrnithopter,
                RelicKey::BronzeScales,
                RelicKey::RegalPillow,
                RelicKey::SmilingMask,
                RelicKey::Orichalcum,
                RelicKey::Lantern,
                RelicKey::BagOfMarbles,
                RelicKey::Strawberry,
            ]
        );
        assert_eq!(
            &pools.uncommon[..8],
            &[
                RelicKey::MummifiedHand,
                RelicKey::MeatOnTheBone,
                RelicKey::Shuriken,
                RelicKey::LetterOpener,
                RelicKey::Sundial,
                RelicKey::TheCourier,
                RelicKey::FrozenEgg,
                RelicKey::SingingBowl,
            ]
        );
        assert_eq!(
            &pools.rare[..8],
            &[
                RelicKey::StoneCalendar,
                RelicKey::ChampionBelt,
                RelicKey::Ginger,
                RelicKey::CharonsAshes,
                RelicKey::PrayerWheel,
                RelicKey::CaptainsWheel,
                RelicKey::Torii,
                RelicKey::GamblingChip,
            ]
        );
        assert_eq!(
            &pools.shop[..8],
            &[
                RelicKey::Brimstone,
                RelicKey::HandDrill,
                RelicKey::Cauldron,
                RelicKey::Toolbox,
                RelicKey::MedicalKit,
                RelicKey::StrangeSpoon,
                RelicKey::LeesWaffle,
                RelicKey::TheAbacus,
            ]
        );
        assert_eq!(
            &pools.boss[..8],
            &[
                RelicKey::CoffeeDripper,
                RelicKey::SacredBark,
                RelicKey::BlackBlood,
                RelicKey::PhilosophersStone,
                RelicKey::RunicDome,
                RelicKey::RunicCube,
                RelicKey::SneckoEye,
                RelicKey::CallingBell,
            ]
        );
    }

    #[test]
    fn vajra_grants_one_strength_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Vajra]);

        assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
    }

    #[test]
    fn start_of_combat_relics_without_vajra_leaves_strength_unchanged() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[]);

        assert_eq!(combat.player.powers.strength, 0);
    }

    #[test]
    fn relic_round_trips_through_json() {
        let relic = Relic::Vajra;

        let json = serde_json::to_string(&relic).expect("relic serializes");
        let restored: Relic = serde_json::from_str(&json).expect("relic deserializes");

        assert_eq!(restored, relic);
    }

    #[test]
    fn oddly_smooth_stone_grants_one_dexterity_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::OddlySmoothStone]);

        assert_eq!(combat.player.powers.dexterity, ODDLY_SMOOTH_STONE_DEXTERITY);
    }

    #[test]
    fn relic_content_ids_map_both_ways() {
        assert_eq!(Relic::Vajra.content_id(), VAJRA_ID);
        assert_eq!(Relic::OddlySmoothStone.content_id(), ODDLY_SMOOTH_STONE_ID);
        assert_eq!(Relic::Strawberry.content_id(), STRAWBERRY_ID);
        assert_eq!(Relic::CoffeeDripper.content_id(), COFFEE_DRIPPER_ID);
        assert_eq!(Relic::Anchor.content_id(), ANCHOR_ID);
        assert_eq!(Relic::InkBottle.content_id(), INK_BOTTLE_ID);
        assert_eq!(Relic::OrnamentalFan.content_id(), ORNAMENTAL_FAN_ID);
        assert_eq!(Relic::IceCream.content_id(), ICE_CREAM_ID);
        assert_eq!(Relic::from_content_id(VAJRA_ID), Some(Relic::Vajra));
        assert_eq!(
            Relic::from_content_id(ODDLY_SMOOTH_STONE_ID),
            Some(Relic::OddlySmoothStone)
        );
        assert_eq!(
            Relic::from_content_id(STRAWBERRY_ID),
            Some(Relic::Strawberry)
        );
        assert_eq!(
            Relic::from_content_id(COFFEE_DRIPPER_ID),
            Some(Relic::CoffeeDripper)
        );
        assert_eq!(Relic::from_content_id(ANCHOR_ID), Some(Relic::Anchor));
        assert_eq!(
            Relic::from_content_id(INK_BOTTLE_ID),
            Some(Relic::InkBottle)
        );
        assert_eq!(
            Relic::from_content_id(ORNAMENTAL_FAN_ID),
            Some(Relic::OrnamentalFan)
        );
        assert_eq!(Relic::from_content_id(ICE_CREAM_ID), Some(Relic::IceCream));
        assert_eq!(Relic::from_content_id(ContentId::new(999)), None);
    }

    #[test]
    fn ice_cream_preserves_energy_between_turns_flag() {
        assert!(!preserves_energy_between_turns(&[]));
        assert!(preserves_energy_between_turns(&[Relic::IceCream]));
    }

    #[test]
    fn ink_bottle_increments_counter_on_card_play() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::InkBottle];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Skill);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ink_bottle_cards_played, 1);
    }

    #[test]
    fn ink_bottle_draws_after_ten_card_plays() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::InkBottle];
        combat.relic_counters.ink_bottle_cards_played = INK_BOTTLE_THRESHOLD - 1;

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(follow_ups, vec![InternalAction::DrawCards { count: 1 }]);
        assert_eq!(combat.relic_counters.ink_bottle_cards_played, 0);
    }

    #[test]
    fn ornamental_fan_increments_attack_counter() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 1);
    }

    #[test]
    fn ornamental_fan_ignores_non_attack_cards() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Skill);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn ornamental_fan_grants_block_on_third_attack() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::OrnamentalFan];
        combat.relic_counters.ornamental_fan_attacks_this_turn = ORNAMENTAL_FAN_THRESHOLD - 1;

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(
            follow_ups,
            vec![InternalAction::GainBlock {
                amount: ORNAMENTAL_FAN_BLOCK
            }]
        );
        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn reset_turn_relic_counters_clears_ornamental_fan_attacks() {
        let mut combat = CombatState::initial_fixture();
        combat.relic_counters.ornamental_fan_attacks_this_turn = 2;

        reset_turn_relic_counters(&mut combat);

        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
    }

    #[test]
    fn relic_counters_round_trip_through_json() {
        let counters = RelicCounters {
            ink_bottle_cards_played: 7,
            ornamental_fan_attacks_this_turn: 2,
        };

        let json = serde_json::to_string(&counters).expect("counters serialize");
        let restored: RelicCounters = serde_json::from_str(&json).expect("counters deserialize");

        assert_eq!(restored, counters);
    }

    #[test]
    fn anchor_grants_ten_block_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Anchor]);

        assert_eq!(combat.player.block, ANCHOR_BLOCK);
    }
}
