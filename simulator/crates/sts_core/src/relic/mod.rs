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
/// Max HP granted by [Relic::Pear] on pickup.
pub const PEAR_MAX_HP: i32 = 10;
/// Max HP granted by [Relic::Mango] on pickup.
pub const MANGO_MAX_HP: i32 = 14;
/// Max HP granted by [Relic::LeesWaffle] on pickup.
pub const LEES_WAFFLE_MAX_HP: i32 = 7;
/// Gold granted by [Relic::OldCoin] on pickup.
pub const OLD_COIN_GOLD: i32 = 300;
/// Extra potion slots granted by [Relic::PotionBelt] on pickup.
pub const POTION_BELT_SLOTS: usize = 2;
/// HP healed by [Relic::BloodVial] at combat start.
pub const BLOOD_VIAL_HEAL: i32 = 2;
/// Energy granted by [Relic::Lantern] at combat start.
pub const LANTERN_ENERGY: i32 = 1;
/// Cards drawn by [Relic::BagOfPreparation] at combat start.
pub const BAG_OF_PREPARATION_DRAW: usize = 2;
/// Vulnerable applied by [Relic::BagOfMarbles] at combat start.
pub const BAG_OF_MARBLES_VULNERABLE: i32 = 1;
/// Thorns granted by [Relic::BronzeScales] at combat start.
pub const BRONZE_SCALES_THORNS: i32 = 3;
/// Plated Armor granted by [Relic::ThreadAndNeedle] at combat start.
pub const THREAD_AND_NEEDLE_PLATED_ARMOR: i32 = 4;
/// Strength granted by [Relic::RedSkull] while starting combat at or below half HP.
pub const RED_SKULL_STRENGTH: i32 = 3;
/// Energy per turn granted by [Relic::CoffeeDripper] on pickup.
pub const COFFEE_DRIPPER_ENERGY: i32 = 1;
/// Energy per turn granted by [Relic::MarkOfPain] on pickup.
pub const MARK_OF_PAIN_ENERGY: i32 = 1;
/// Energy per turn granted by [Relic::FusionHammer] on pickup.
pub const FUSION_HAMMER_ENERGY: i32 = 1;
/// Energy per turn granted by [Relic::Sozu] on pickup.
pub const SOZU_ENERGY: i32 = 1;
/// Energy per turn granted by [Relic::BustedCrown] on pickup.
pub const BUSTED_CROWN_ENERGY: i32 = 1;
/// Fewer card reward choices shown by [Relic::BustedCrown].
pub const BUSTED_CROWN_CARD_REWARD_REDUCTION: usize = 2;
/// Extra card reward choice shown by [Relic::QuestionCard].
pub const QUESTION_CARD_REWARD_BONUS: usize = 1;
/// Curses prevented by [Relic::Omamori].
pub const OMAMORI_CHARGES: u32 = 2;
/// Energy per turn granted by [Relic::VelvetChoker] on pickup.
pub const VELVET_CHOKER_ENERGY: i32 = 1;
/// Maximum cards playable per turn with [Relic::VelvetChoker].
pub const VELVET_CHOKER_CARD_LIMIT: u32 = 6;
/// HP healed by [Relic::ToyOrnithopter] when a potion is used.
pub const TOY_ORNITHOPTER_HEAL: i32 = 5;
/// HP healed by [Relic::BirdFacedUrn] when a Power is played.
pub const BIRD_FACED_URN_HEAL: i32 = 2;
/// Maximum unblocked attack damage that [Relic::TheBoot] increases.
pub const THE_BOOT_MAX_DAMAGE: i32 = 4;
/// Unblocked attack damage after [Relic::TheBoot] increase.
pub const THE_BOOT_DAMAGE: i32 = 5;
/// Wounds added to the deck by [Relic::MarkOfPain] on pickup.
pub const MARK_OF_PAIN_WOUNDS: usize = 2;
/// Block granted by [Relic::Anchor] at combat start.
pub const ANCHOR_BLOCK: i32 = 10;
/// Cards played before [Relic::InkBottle] draws a card.
pub const INK_BOTTLE_THRESHOLD: u32 = 10;
/// Attacks played in one turn before [Relic::OrnamentalFan] grants block.
pub const ORNAMENTAL_FAN_THRESHOLD: u32 = 3;
/// Block granted by [Relic::OrnamentalFan] every third attack in a turn.
pub const ORNAMENTAL_FAN_BLOCK: i32 = 4;
/// Attacks before [Relic::Nunchaku] grants energy.
pub const NUNCHAKU_THRESHOLD: u32 = 10;
/// Energy granted by [Relic::Nunchaku].
pub const NUNCHAKU_ENERGY: i32 = 1;
/// Attacks in one turn before [Relic::Shuriken] grants strength.
pub const SHURIKEN_THRESHOLD: u32 = 3;
/// Strength granted by [Relic::Shuriken].
pub const SHURIKEN_STRENGTH: i32 = 1;
/// Attacks in one turn before [Relic::Kunai] grants dexterity.
pub const KUNAI_THRESHOLD: u32 = 3;
/// Dexterity granted by [Relic::Kunai].
pub const KUNAI_DEXTERITY: i32 = 1;
/// Skills in one turn before [Relic::LetterOpener] deals damage.
pub const LETTER_OPENER_THRESHOLD: u32 = 3;
/// Damage dealt by [Relic::LetterOpener] to all enemies.
pub const LETTER_OPENER_DAMAGE: i32 = 5;
/// Turns before [Relic::HappyFlower] grants energy.
pub const HAPPY_FLOWER_THRESHOLD: u32 = 3;
/// Energy granted by [Relic::HappyFlower].
pub const HAPPY_FLOWER_ENERGY: i32 = 1;
/// Energy granted by [Relic::ArtOfWar] after a turn with no Attacks played.
pub const ART_OF_WAR_ENERGY: i32 = 1;
/// Block granted by [Relic::Orichalcum] when ending the turn with no block.
pub const ORICHALCUM_BLOCK: i32 = 6;
/// Player turn when [Relic::HornCleat] grants block.
pub const HORN_CLEAT_TURN: u32 = 2;
/// Block granted by [Relic::HornCleat].
pub const HORN_CLEAT_BLOCK: i32 = 14;
/// Player turn when [Relic::CaptainsWheel] grants block.
pub const CAPTAINS_WHEEL_TURN: u32 = 3;
/// Block granted by [Relic::CaptainsWheel].
pub const CAPTAINS_WHEEL_BLOCK: i32 = 18;
/// Damage dealt by [Relic::MercuryHourglass] to all enemies each turn.
pub const MERCURY_HOURGLASS_DAMAGE: i32 = 3;
/// Player turn when [Relic::StoneCalendar] deals damage.
pub const STONE_CALENDAR_TURN: u32 = 7;
/// Damage dealt by [Relic::StoneCalendar] to all enemies.
pub const STONE_CALENDAR_DAMAGE: i32 = 52;
/// HP healed by [Relic::BlackBlood] after combat victory.
pub const BLACK_BLOOD_HEAL: i32 = 12;
/// HP healed by [Relic::MeatOnTheBone] after combat victory at or below half HP.
pub const MEAT_ON_THE_BONE_HEAL: i32 = 12;
/// HP healed by [Relic::MealTicket] when entering a shop.
pub const MEAL_TICKET_HEAL: i32 = 15;
/// Extra HP healed by [Relic::RegalPillow] when resting.
pub const REGAL_PILLOW_HEAL: i32 = 15;
/// HP healed by [Relic::EternalFeather] per five cards in the deck when resting.
pub const ETERNAL_FEATHER_HEAL_PER_FIVE_CARDS: i32 = 3;
/// Maximum unblocked attack damage that [Relic::Torii] reduces.
pub const TORII_MAX_DAMAGE: i32 = 5;
/// Attack damage after [Relic::Torii] reduction.
pub const TORII_REDUCED_DAMAGE: i32 = 1;
/// HP loss prevented by [Relic::TungstenRod].
pub const TUNGSTEN_ROD_REDUCTION: i32 = 1;
/// Gold granted by [Relic::CeramicFish] whenever a card is added to the deck.
pub const CERAMIC_FISH_GOLD: i32 = 9;
/// HP healed by [Relic::Pantograph] at the start of boss combat.
pub const PANTOGRAPH_HEAL: i32 = 25;
/// Numerator for [Relic::MagicFlower]'s 50% Ironclad healing increase.
pub const MAGIC_FLOWER_HEAL_NUMERATOR: i32 = 3;
/// Denominator for [Relic::MagicFlower]'s 50% Ironclad healing increase.
pub const MAGIC_FLOWER_HEAL_DENOMINATOR: i32 = 2;
/// Numerator for [Relic::PaperPhrog]'s Vulnerable bonus damage increase.
pub const PAPER_PHROG_VULNERABLE_BONUS_NUMERATOR: i32 = 3;
/// Denominator for [Relic::PaperPhrog]'s Vulnerable bonus damage increase.
pub const PAPER_PHROG_VULNERABLE_BONUS_DENOMINATOR: i32 = 4;
/// Weak applied by [Relic::ChampionBelt] whenever the player applies Vulnerable.
pub const CHAMPION_BELT_WEAK: i32 = 1;
/// Numerator for [Relic::PreservedInsect]'s elite HP multiplier.
pub const PRESERVED_INSECT_HP_NUMERATOR: i32 = 3;
/// Denominator for [Relic::PreservedInsect]'s elite HP multiplier.
pub const PRESERVED_INSECT_HP_DENOMINATOR: i32 = 4;
/// Strength granted by [Relic::SlingOfCourage] in elite combats.
pub const SLING_OF_COURAGE_STRENGTH: i32 = 2;
/// Gold granted by [Relic::MawBank] when entering a floor before it breaks.
pub const MAW_BANK_GOLD: i32 = 12;
/// Energy granted by [Relic::AncientTeaSet] in the next combat after entering a rest site.
pub const ANCIENT_TEA_SET_ENERGY: i32 = 2;
/// Block lost at turn transition with [Relic::Calipers] instead of losing all block.
pub const CALIPERS_BLOCK_LOSS: i32 = 15;
/// Max HP granted by [Relic::DarkstonePeriapt] whenever a curse is obtained.
pub const DARKSTONE_PERIAPT_MAX_HP: i32 = 6;
/// Strength granted by [Relic::DuVuDoll] per curse in the deck at combat start.
pub const DU_VU_DOLL_STRENGTH_PER_CURSE: i32 = 1;

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
/// Content id for [Relic::BloodVial].
pub const BLOOD_VIAL_ID: ContentId = ContentId::new(308);
/// Content id for [Relic::Pear].
pub const PEAR_ID: ContentId = ContentId::new(309);
/// Content id for [Relic::Mango].
pub const MANGO_ID: ContentId = ContentId::new(310);
/// Content id for [Relic::OldCoin].
pub const OLD_COIN_ID: ContentId = ContentId::new(311);
/// Content id for [Relic::LeesWaffle].
pub const LEES_WAFFLE_ID: ContentId = ContentId::new(312);
/// Content id for [Relic::PotionBelt].
pub const POTION_BELT_ID: ContentId = ContentId::new(313);
/// Content id for [Relic::Lantern].
pub const LANTERN_ID: ContentId = ContentId::new(314);
/// Content id for [Relic::BagOfPreparation].
pub const BAG_OF_PREPARATION_ID: ContentId = ContentId::new(315);
/// Content id for [Relic::BagOfMarbles].
pub const BAG_OF_MARBLES_ID: ContentId = ContentId::new(316);
/// Content id for [Relic::BronzeScales].
pub const BRONZE_SCALES_ID: ContentId = ContentId::new(317);
/// Content id for [Relic::ThreadAndNeedle].
pub const THREAD_AND_NEEDLE_ID: ContentId = ContentId::new(318);
/// Content id for [Relic::RedSkull].
pub const RED_SKULL_ID: ContentId = ContentId::new(319);
/// Content id for [Relic::Nunchaku].
pub const NUNCHAKU_ID: ContentId = ContentId::new(320);
/// Content id for [Relic::Shuriken].
pub const SHURIKEN_ID: ContentId = ContentId::new(321);
/// Content id for [Relic::Kunai].
pub const KUNAI_ID: ContentId = ContentId::new(322);
/// Content id for [Relic::LetterOpener].
pub const LETTER_OPENER_ID: ContentId = ContentId::new(323);
/// Content id for [Relic::HappyFlower].
pub const HAPPY_FLOWER_ID: ContentId = ContentId::new(324);
/// Content id for [Relic::Orichalcum].
pub const ORICHALCUM_ID: ContentId = ContentId::new(325);
/// Content id for [Relic::HornCleat].
pub const HORN_CLEAT_ID: ContentId = ContentId::new(326);
/// Content id for [Relic::CaptainsWheel].
pub const CAPTAINS_WHEEL_ID: ContentId = ContentId::new(327);
/// Content id for [Relic::MercuryHourglass].
pub const MERCURY_HOURGLASS_ID: ContentId = ContentId::new(328);
/// Content id for [Relic::StoneCalendar].
pub const STONE_CALENDAR_ID: ContentId = ContentId::new(329);
/// Content id for [Relic::MeatOnTheBone].
pub const MEAT_ON_THE_BONE_ID: ContentId = ContentId::new(330);
/// Content id for [Relic::BlackBlood].
pub const BLACK_BLOOD_ID: ContentId = ContentId::new(331);
/// Content id for [Relic::MealTicket].
pub const MEAL_TICKET_ID: ContentId = ContentId::new(332);
/// Content id for [Relic::RegalPillow].
pub const REGAL_PILLOW_ID: ContentId = ContentId::new(333);
/// Content id for [Relic::DreamCatcher].
pub const DREAM_CATCHER_ID: ContentId = ContentId::new(334);
/// Content id for [Relic::EternalFeather].
pub const ETERNAL_FEATHER_ID: ContentId = ContentId::new(335);
/// Content id for [Relic::Torii].
pub const TORII_ID: ContentId = ContentId::new(336);
/// Content id for [Relic::TungstenRod].
pub const TUNGSTEN_ROD_ID: ContentId = ContentId::new(337);
/// Content id for [Relic::CeramicFish].
pub const CERAMIC_FISH_ID: ContentId = ContentId::new(338);
/// Content id for [Relic::MembershipCard].
pub const MEMBERSHIP_CARD_ID: ContentId = ContentId::new(339);
/// Content id for [Relic::SmilingMask].
pub const SMILING_MASK_ID: ContentId = ContentId::new(340);
/// Content id for [Relic::Pantograph].
pub const PANTOGRAPH_ID: ContentId = ContentId::new(341);
/// Content id for [Relic::Ginger].
pub const GINGER_ID: ContentId = ContentId::new(342);
/// Content id for [Relic::Turnip].
pub const TURNIP_ID: ContentId = ContentId::new(343);
/// Content id for [Relic::MarkOfPain].
pub const MARK_OF_PAIN_ID: ContentId = ContentId::new(344);
/// Content id for [Relic::MagicFlower].
pub const MAGIC_FLOWER_ID: ContentId = ContentId::new(345);
/// Content id for [Relic::PaperPhrog].
pub const PAPER_PHROG_ID: ContentId = ContentId::new(346);
/// Content id for [Relic::ChampionBelt].
pub const CHAMPION_BELT_ID: ContentId = ContentId::new(347);
/// Content id for [Relic::PreservedInsect].
pub const PRESERVED_INSECT_ID: ContentId = ContentId::new(348);
/// Content id for [Relic::DarkstonePeriapt].
pub const DARKSTONE_PERIAPT_ID: ContentId = ContentId::new(349);
/// Content id for [Relic::DuVuDoll].
pub const DU_VU_DOLL_ID: ContentId = ContentId::new(350);
/// Content id for [Relic::FusionHammer].
pub const FUSION_HAMMER_ID: ContentId = ContentId::new(351);
/// Content id for [Relic::Sozu].
pub const SOZU_ID: ContentId = ContentId::new(352);
/// Content id for [Relic::BustedCrown].
pub const BUSTED_CROWN_ID: ContentId = ContentId::new(353);
/// Content id for [Relic::VelvetChoker].
pub const VELVET_CHOKER_ID: ContentId = ContentId::new(354);
/// Content id for [Relic::ToyOrnithopter].
pub const TOY_ORNITHOPTER_ID: ContentId = ContentId::new(355);
/// Content id for [Relic::MoltenEgg].
pub const MOLTEN_EGG_ID: ContentId = ContentId::new(356);
/// Content id for [Relic::ToxicEgg].
pub const TOXIC_EGG_ID: ContentId = ContentId::new(357);
/// Content id for [Relic::FrozenEgg].
pub const FROZEN_EGG_ID: ContentId = ContentId::new(358);
/// Content id for [Relic::TheBoot].
pub const THE_BOOT_ID: ContentId = ContentId::new(359);
/// Content id for [Relic::BirdFacedUrn].
pub const BIRD_FACED_URN_ID: ContentId = ContentId::new(360);
/// Content id for [Relic::ArtOfWar].
pub const ART_OF_WAR_ID: ContentId = ContentId::new(361);
/// Content id for [Relic::QuestionCard].
pub const QUESTION_CARD_ID: ContentId = ContentId::new(362);
/// Content id for [Relic::Omamori].
pub const OMAMORI_ID: ContentId = ContentId::new(363);
/// Content id for [Relic::SlingOfCourage].
pub const SLING_OF_COURAGE_ID: ContentId = ContentId::new(364);
/// Content id for [Relic::MawBank].
pub const MAW_BANK_ID: ContentId = ContentId::new(365);
/// Content id for [Relic::AncientTeaSet].
pub const ANCIENT_TEA_SET_ID: ContentId = ContentId::new(366);
/// Content id for [Relic::Calipers].
pub const CALIPERS_ID: ContentId = ContentId::new(367);
/// Content id for [Relic::SingingBowl].
pub const SINGING_BOWL_ID: ContentId = ContentId::new(368);
/// Max HP granted by [Relic::SingingBowl] when skipping a card reward.
pub const SINGING_BOWL_MAX_HP: i32 = 2;
/// Content id for [Relic::ChemicalX].
pub const CHEMICAL_X_ID: ContentId = ContentId::new(369);
/// Extra X value granted by [Relic::ChemicalX].
pub const CHEMICAL_X_BONUS_X: i32 = 2;
/// Content id for [Relic::PhilosophersStone].
pub const PHILOSOPHERS_STONE_ID: ContentId = ContentId::new(370);
/// Energy per turn granted by [Relic::PhilosophersStone] on pickup.
pub const PHILOSOPHERS_STONE_ENERGY: i32 = 1;
/// Strength granted to monsters by [Relic::PhilosophersStone] at combat start.
pub const PHILOSOPHERS_STONE_MONSTER_STRENGTH: i32 = 1;
/// Content id for [Relic::SlaversCollar].
pub const SLAVERS_COLLAR_ID: ContentId = ContentId::new(371);
/// Energy per turn granted by [Relic::SlaversCollar] during elite and boss combats.
pub const SLAVERS_COLLAR_ENERGY: i32 = 1;
/// Content id for [Relic::Ectoplasm].
pub const ECTOPLASM_ID: ContentId = ContentId::new(372);
/// Energy per turn granted by [Relic::Ectoplasm] on pickup.
pub const ECTOPLASM_ENERGY: i32 = 1;
/// Content id for [Relic::RunicDome].
pub const RUNIC_DOME_ID: ContentId = ContentId::new(373);
/// Energy per turn granted by [Relic::RunicDome] on pickup.
pub const RUNIC_DOME_ENERGY: i32 = 1;
/// Content id for [Relic::StrikeDummy].
pub const STRIKE_DUMMY_ID: ContentId = ContentId::new(374);
/// Extra damage granted by [Relic::StrikeDummy] to Strike cards.
pub const STRIKE_DUMMY_DAMAGE: i32 = 3;
/// Content id for [Relic::Brimstone].
pub const BRIMSTONE_ID: ContentId = ContentId::new(375);
/// Strength granted to the player by [Relic::Brimstone] at the start of each player turn.
pub const BRIMSTONE_PLAYER_STRENGTH: i32 = 2;
/// Strength granted to each enemy by [Relic::Brimstone] at the start of each player turn.
pub const BRIMSTONE_MONSTER_STRENGTH: i32 = 1;
/// Content id for [Relic::WhiteBeastStatue].
pub const WHITE_BEAST_STATUE_ID: ContentId = ContentId::new(376);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RelicCounters {
    #[serde(default)]
    pub ink_bottle_cards_played: u32,
    #[serde(default)]
    pub ornamental_fan_attacks_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub nunchaku_attacks_played: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub shuriken_attacks_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub kunai_attacks_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub letter_opener_skills_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub cards_played_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attacks_played_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attacks_played_last_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub player_turns_started: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub happy_flower_turns: u32,
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
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
    BurningBlood,
    CrackedCore,
    RingOfTheSnake,
    PureWater,
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
    FrozenCore,
    RingOfTheSerpent,
    HolyWater,
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
    Circlet,
    RedCirclet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelicPoolState {
    pub common: Vec<RelicKey>,
    pub uncommon: Vec<RelicKey>,
    pub rare: Vec<RelicKey>,
    pub shop: Vec<RelicKey>,
    pub boss: Vec<RelicKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelicSpawnContext {
    pub floor_num: i32,
    pub shop_room: bool,
    pub owned_relics: Vec<RelicKey>,
    pub has_non_basic_attack: bool,
    pub has_non_basic_skill: bool,
    pub has_power: bool,
}

impl Default for RelicSpawnContext {
    fn default() -> Self {
        Self {
            floor_num: 1,
            shop_room: false,
            owned_relics: Vec::new(),
            has_non_basic_attack: false,
            has_non_basic_skill: false,
            has_power: false,
        }
    }
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

impl RelicPoolState {
    pub fn remove_relic(&mut self, key: RelicKey) {
        remove_relic_from_pool(&mut self.common, key);
        remove_relic_from_pool(&mut self.uncommon, key);
        remove_relic_from_pool(&mut self.rare, key);
        remove_relic_from_pool(&mut self.shop, key);
        remove_relic_from_pool(&mut self.boss, key);
    }

    pub fn return_random_relic(
        &mut self,
        tier: RelicTier,
        context: &RelicSpawnContext,
    ) -> RelicKey {
        self.return_random_relic_from(tier, context, true)
    }

    pub fn return_random_relic_end(
        &mut self,
        tier: RelicTier,
        context: &RelicSpawnContext,
    ) -> RelicKey {
        self.return_random_relic_from(tier, context, false)
    }

    pub fn return_random_screenless_relic(
        &mut self,
        tier: RelicTier,
        context: &RelicSpawnContext,
    ) -> RelicKey {
        loop {
            let relic = self.return_random_relic(tier, context);
            if !matches!(
                relic,
                RelicKey::BottledFlame
                    | RelicKey::BottledLightning
                    | RelicKey::BottledTornado
                    | RelicKey::Whetstone
            ) {
                return relic;
            }
        }
    }

    fn return_random_relic_from(
        &mut self,
        tier: RelicTier,
        context: &RelicSpawnContext,
        from_front: bool,
    ) -> RelicKey {
        let relic = match tier {
            RelicTier::Common if self.common.is_empty() => {
                return self.return_random_relic_from(RelicTier::Uncommon, context, true);
            }
            RelicTier::Common => pop_relic(&mut self.common, from_front),
            RelicTier::Uncommon if self.uncommon.is_empty() => {
                return self.return_random_relic_from(RelicTier::Rare, context, true);
            }
            RelicTier::Uncommon => pop_relic(&mut self.uncommon, from_front),
            RelicTier::Rare if self.rare.is_empty() => RelicKey::Circlet,
            RelicTier::Rare => pop_relic(&mut self.rare, from_front),
            RelicTier::Shop if self.shop.is_empty() => {
                return self.return_random_relic_from(RelicTier::Uncommon, context, true);
            }
            RelicTier::Shop => pop_relic(&mut self.shop, from_front),
            RelicTier::Boss if self.boss.is_empty() => RelicKey::RedCirclet,
            RelicTier::Boss => pop_relic(&mut self.boss, from_front),
        };

        if relic_can_spawn(relic, context) {
            relic
        } else {
            self.return_random_relic_from(tier, context, false)
        }
    }
}

fn pop_relic(pool: &mut Vec<RelicKey>, from_front: bool) -> RelicKey {
    if from_front {
        pool.remove(0)
    } else {
        pool.pop().expect("pool checked non-empty")
    }
}

fn remove_relic_from_pool(pool: &mut Vec<RelicKey>, key: RelicKey) {
    if let Some(index) = pool.iter().position(|candidate| *candidate == key) {
        pool.remove(index);
    }
}

#[must_use]
pub fn relic_can_spawn(relic: RelicKey, context: &RelicSpawnContext) -> bool {
    use RelicKey::{
        AncientTeaSet, BlackBlood, BottledFlame, BottledLightning, BottledTornado, BurningBlood,
        CeramicFish, CrackedCore, DarkstonePeriapt, DreamCatcher, FrozenCore, FrozenEgg, Girya,
        HolyWater, JuzuBracelet, MawBank, MealTicket, MeatOnTheBone, MoltenEgg, OldCoin, Omamori,
        PeacePipe, PotionBelt, PrayerWheel, PreservedInsect, PureWater, QuestionCard, RegalPillow,
        RingOfTheSerpent, RingOfTheSnake, Shovel, SingingBowl, SmilingMask, TheCourier, TinyChest,
        ToxicEgg, WingBoots,
    };

    match relic {
        BottledFlame => context.has_non_basic_attack,
        BottledLightning => context.has_non_basic_skill,
        BottledTornado => context.has_power,
        BlackBlood => context.owned_relics.contains(&BurningBlood),
        FrozenCore => context.owned_relics.contains(&CrackedCore),
        BurningBlood => context.owned_relics.contains(&BurningBlood),
        RingOfTheSerpent => context.owned_relics.contains(&RingOfTheSnake),
        HolyWater => context.owned_relics.contains(&PureWater),
        TinyChest => context.floor_num <= 35,
        WingBoots | RelicKey::Matryoshka => context.floor_num <= 40,
        Girya | PeacePipe | Shovel => {
            context.floor_num < 48 && campfire_relic_count(&context.owned_relics) < 2
        }
        MawBank | OldCoin | SmilingMask => context.floor_num <= 48 && !context.shop_room,
        AncientTeaSet | CeramicFish | DarkstonePeriapt | DreamCatcher | FrozenEgg
        | JuzuBracelet | MealTicket | MeatOnTheBone | MoltenEgg | Omamori | PotionBelt
        | PrayerWheel | QuestionCard | RegalPillow | SingingBowl | TheCourier | ToxicEgg => {
            context.floor_num <= 48
        }
        PreservedInsect => context.floor_num <= 52,
        _ => true,
    }
}

fn campfire_relic_count(owned: &[RelicKey]) -> usize {
    owned
        .iter()
        .filter(|relic| {
            matches!(
                relic,
                RelicKey::Girya | RelicKey::PeacePipe | RelicKey::Shovel
            )
        })
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relic {
    BloodVial,
    Vajra,
    OddlySmoothStone,
    Strawberry,
    Pear,
    Mango,
    OldCoin,
    LeesWaffle,
    PotionBelt,
    Lantern,
    BagOfPreparation,
    BagOfMarbles,
    BronzeScales,
    ThreadAndNeedle,
    RedSkull,
    Nunchaku,
    ArtOfWar,
    Shuriken,
    Kunai,
    LetterOpener,
    HappyFlower,
    Orichalcum,
    HornCleat,
    CaptainsWheel,
    MercuryHourglass,
    StoneCalendar,
    MeatOnTheBone,
    QuestionCard,
    BlackBlood,
    MealTicket,
    RegalPillow,
    DreamCatcher,
    EternalFeather,
    Torii,
    TungstenRod,
    CeramicFish,
    MembershipCard,
    SmilingMask,
    Pantograph,
    Ginger,
    Turnip,
    MarkOfPain,
    MagicFlower,
    PaperPhrog,
    ChampionBelt,
    PreservedInsect,
    Omamori,
    SlingOfCourage,
    MawBank,
    AncientTeaSet,
    Calipers,
    SingingBowl,
    DarkstonePeriapt,
    DuVuDoll,
    FusionHammer,
    Sozu,
    BustedCrown,
    VelvetChoker,
    ToyOrnithopter,
    MoltenEgg,
    ToxicEgg,
    FrozenEgg,
    TheBoot,
    BirdFacedUrn,
    CoffeeDripper,
    Anchor,
    InkBottle,
    OrnamentalFan,
    IceCream,
    ChemicalX,
    PhilosophersStone,
    SlaversCollar,
    Ectoplasm,
    RunicDome,
    StrikeDummy,
    Brimstone,
    WhiteBeastStatue,
}

impl Relic {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Relic::BloodVial => BLOOD_VIAL_ID,
            Relic::Vajra => VAJRA_ID,
            Relic::OddlySmoothStone => ODDLY_SMOOTH_STONE_ID,
            Relic::Strawberry => STRAWBERRY_ID,
            Relic::Pear => PEAR_ID,
            Relic::Mango => MANGO_ID,
            Relic::OldCoin => OLD_COIN_ID,
            Relic::LeesWaffle => LEES_WAFFLE_ID,
            Relic::PotionBelt => POTION_BELT_ID,
            Relic::Lantern => LANTERN_ID,
            Relic::BagOfPreparation => BAG_OF_PREPARATION_ID,
            Relic::BagOfMarbles => BAG_OF_MARBLES_ID,
            Relic::BronzeScales => BRONZE_SCALES_ID,
            Relic::ThreadAndNeedle => THREAD_AND_NEEDLE_ID,
            Relic::RedSkull => RED_SKULL_ID,
            Relic::Nunchaku => NUNCHAKU_ID,
            Relic::ArtOfWar => ART_OF_WAR_ID,
            Relic::Shuriken => SHURIKEN_ID,
            Relic::Kunai => KUNAI_ID,
            Relic::LetterOpener => LETTER_OPENER_ID,
            Relic::HappyFlower => HAPPY_FLOWER_ID,
            Relic::Orichalcum => ORICHALCUM_ID,
            Relic::HornCleat => HORN_CLEAT_ID,
            Relic::CaptainsWheel => CAPTAINS_WHEEL_ID,
            Relic::MercuryHourglass => MERCURY_HOURGLASS_ID,
            Relic::StoneCalendar => STONE_CALENDAR_ID,
            Relic::MeatOnTheBone => MEAT_ON_THE_BONE_ID,
            Relic::QuestionCard => QUESTION_CARD_ID,
            Relic::BlackBlood => BLACK_BLOOD_ID,
            Relic::MealTicket => MEAL_TICKET_ID,
            Relic::RegalPillow => REGAL_PILLOW_ID,
            Relic::DreamCatcher => DREAM_CATCHER_ID,
            Relic::EternalFeather => ETERNAL_FEATHER_ID,
            Relic::Torii => TORII_ID,
            Relic::TungstenRod => TUNGSTEN_ROD_ID,
            Relic::CeramicFish => CERAMIC_FISH_ID,
            Relic::MembershipCard => MEMBERSHIP_CARD_ID,
            Relic::SmilingMask => SMILING_MASK_ID,
            Relic::Pantograph => PANTOGRAPH_ID,
            Relic::Ginger => GINGER_ID,
            Relic::Turnip => TURNIP_ID,
            Relic::MarkOfPain => MARK_OF_PAIN_ID,
            Relic::MagicFlower => MAGIC_FLOWER_ID,
            Relic::PaperPhrog => PAPER_PHROG_ID,
            Relic::ChampionBelt => CHAMPION_BELT_ID,
            Relic::PreservedInsect => PRESERVED_INSECT_ID,
            Relic::Omamori => OMAMORI_ID,
            Relic::SlingOfCourage => SLING_OF_COURAGE_ID,
            Relic::MawBank => MAW_BANK_ID,
            Relic::AncientTeaSet => ANCIENT_TEA_SET_ID,
            Relic::Calipers => CALIPERS_ID,
            Relic::SingingBowl => SINGING_BOWL_ID,
            Relic::DarkstonePeriapt => DARKSTONE_PERIAPT_ID,
            Relic::DuVuDoll => DU_VU_DOLL_ID,
            Relic::FusionHammer => FUSION_HAMMER_ID,
            Relic::Sozu => SOZU_ID,
            Relic::BustedCrown => BUSTED_CROWN_ID,
            Relic::VelvetChoker => VELVET_CHOKER_ID,
            Relic::ToyOrnithopter => TOY_ORNITHOPTER_ID,
            Relic::MoltenEgg => MOLTEN_EGG_ID,
            Relic::ToxicEgg => TOXIC_EGG_ID,
            Relic::FrozenEgg => FROZEN_EGG_ID,
            Relic::TheBoot => THE_BOOT_ID,
            Relic::BirdFacedUrn => BIRD_FACED_URN_ID,
            Relic::CoffeeDripper => COFFEE_DRIPPER_ID,
            Relic::Anchor => ANCHOR_ID,
            Relic::InkBottle => INK_BOTTLE_ID,
            Relic::OrnamentalFan => ORNAMENTAL_FAN_ID,
            Relic::IceCream => ICE_CREAM_ID,
            Relic::ChemicalX => CHEMICAL_X_ID,
            Relic::PhilosophersStone => PHILOSOPHERS_STONE_ID,
            Relic::SlaversCollar => SLAVERS_COLLAR_ID,
            Relic::Ectoplasm => ECTOPLASM_ID,
            Relic::RunicDome => RUNIC_DOME_ID,
            Relic::StrikeDummy => STRIKE_DUMMY_ID,
            Relic::Brimstone => BRIMSTONE_ID,
            Relic::WhiteBeastStatue => WHITE_BEAST_STATUE_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == BLOOD_VIAL_ID => Some(Relic::BloodVial),
            id if id == VAJRA_ID => Some(Relic::Vajra),
            id if id == ODDLY_SMOOTH_STONE_ID => Some(Relic::OddlySmoothStone),
            id if id == STRAWBERRY_ID => Some(Relic::Strawberry),
            id if id == PEAR_ID => Some(Relic::Pear),
            id if id == MANGO_ID => Some(Relic::Mango),
            id if id == OLD_COIN_ID => Some(Relic::OldCoin),
            id if id == LEES_WAFFLE_ID => Some(Relic::LeesWaffle),
            id if id == POTION_BELT_ID => Some(Relic::PotionBelt),
            id if id == LANTERN_ID => Some(Relic::Lantern),
            id if id == BAG_OF_PREPARATION_ID => Some(Relic::BagOfPreparation),
            id if id == BAG_OF_MARBLES_ID => Some(Relic::BagOfMarbles),
            id if id == BRONZE_SCALES_ID => Some(Relic::BronzeScales),
            id if id == THREAD_AND_NEEDLE_ID => Some(Relic::ThreadAndNeedle),
            id if id == RED_SKULL_ID => Some(Relic::RedSkull),
            id if id == NUNCHAKU_ID => Some(Relic::Nunchaku),
            id if id == ART_OF_WAR_ID => Some(Relic::ArtOfWar),
            id if id == SHURIKEN_ID => Some(Relic::Shuriken),
            id if id == KUNAI_ID => Some(Relic::Kunai),
            id if id == LETTER_OPENER_ID => Some(Relic::LetterOpener),
            id if id == HAPPY_FLOWER_ID => Some(Relic::HappyFlower),
            id if id == ORICHALCUM_ID => Some(Relic::Orichalcum),
            id if id == HORN_CLEAT_ID => Some(Relic::HornCleat),
            id if id == CAPTAINS_WHEEL_ID => Some(Relic::CaptainsWheel),
            id if id == MERCURY_HOURGLASS_ID => Some(Relic::MercuryHourglass),
            id if id == STONE_CALENDAR_ID => Some(Relic::StoneCalendar),
            id if id == MEAT_ON_THE_BONE_ID => Some(Relic::MeatOnTheBone),
            id if id == QUESTION_CARD_ID => Some(Relic::QuestionCard),
            id if id == BLACK_BLOOD_ID => Some(Relic::BlackBlood),
            id if id == MEAL_TICKET_ID => Some(Relic::MealTicket),
            id if id == REGAL_PILLOW_ID => Some(Relic::RegalPillow),
            id if id == DREAM_CATCHER_ID => Some(Relic::DreamCatcher),
            id if id == ETERNAL_FEATHER_ID => Some(Relic::EternalFeather),
            id if id == TORII_ID => Some(Relic::Torii),
            id if id == TUNGSTEN_ROD_ID => Some(Relic::TungstenRod),
            id if id == CERAMIC_FISH_ID => Some(Relic::CeramicFish),
            id if id == MEMBERSHIP_CARD_ID => Some(Relic::MembershipCard),
            id if id == SMILING_MASK_ID => Some(Relic::SmilingMask),
            id if id == PANTOGRAPH_ID => Some(Relic::Pantograph),
            id if id == GINGER_ID => Some(Relic::Ginger),
            id if id == TURNIP_ID => Some(Relic::Turnip),
            id if id == MARK_OF_PAIN_ID => Some(Relic::MarkOfPain),
            id if id == MAGIC_FLOWER_ID => Some(Relic::MagicFlower),
            id if id == PAPER_PHROG_ID => Some(Relic::PaperPhrog),
            id if id == CHAMPION_BELT_ID => Some(Relic::ChampionBelt),
            id if id == PRESERVED_INSECT_ID => Some(Relic::PreservedInsect),
            id if id == OMAMORI_ID => Some(Relic::Omamori),
            id if id == SLING_OF_COURAGE_ID => Some(Relic::SlingOfCourage),
            id if id == MAW_BANK_ID => Some(Relic::MawBank),
            id if id == ANCIENT_TEA_SET_ID => Some(Relic::AncientTeaSet),
            id if id == CALIPERS_ID => Some(Relic::Calipers),
            id if id == SINGING_BOWL_ID => Some(Relic::SingingBowl),
            id if id == DARKSTONE_PERIAPT_ID => Some(Relic::DarkstonePeriapt),
            id if id == DU_VU_DOLL_ID => Some(Relic::DuVuDoll),
            id if id == FUSION_HAMMER_ID => Some(Relic::FusionHammer),
            id if id == SOZU_ID => Some(Relic::Sozu),
            id if id == BUSTED_CROWN_ID => Some(Relic::BustedCrown),
            id if id == VELVET_CHOKER_ID => Some(Relic::VelvetChoker),
            id if id == TOY_ORNITHOPTER_ID => Some(Relic::ToyOrnithopter),
            id if id == MOLTEN_EGG_ID => Some(Relic::MoltenEgg),
            id if id == TOXIC_EGG_ID => Some(Relic::ToxicEgg),
            id if id == FROZEN_EGG_ID => Some(Relic::FrozenEgg),
            id if id == THE_BOOT_ID => Some(Relic::TheBoot),
            id if id == BIRD_FACED_URN_ID => Some(Relic::BirdFacedUrn),
            id if id == COFFEE_DRIPPER_ID => Some(Relic::CoffeeDripper),
            id if id == ANCHOR_ID => Some(Relic::Anchor),
            id if id == INK_BOTTLE_ID => Some(Relic::InkBottle),
            id if id == ORNAMENTAL_FAN_ID => Some(Relic::OrnamentalFan),
            id if id == ICE_CREAM_ID => Some(Relic::IceCream),
            id if id == CHEMICAL_X_ID => Some(Relic::ChemicalX),
            id if id == PHILOSOPHERS_STONE_ID => Some(Relic::PhilosophersStone),
            id if id == SLAVERS_COLLAR_ID => Some(Relic::SlaversCollar),
            id if id == ECTOPLASM_ID => Some(Relic::Ectoplasm),
            id if id == RUNIC_DOME_ID => Some(Relic::RunicDome),
            id if id == STRIKE_DUMMY_ID => Some(Relic::StrikeDummy),
            id if id == BRIMSTONE_ID => Some(Relic::Brimstone),
            id if id == WHITE_BEAST_STATUE_ID => Some(Relic::WhiteBeastStatue),
            _ => None,
        }
    }
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) {
    for relic in relics {
        match relic {
            Relic::BloodVial => {
                heal_player_in_combat_with_relics(
                    &mut combat.player.hp,
                    combat.player.max_hp,
                    BLOOD_VIAL_HEAL,
                    relics,
                );
            }
            Relic::Vajra => {
                combat.player.powers.strength += VAJRA_STRENGTH;
            }
            Relic::OddlySmoothStone => {
                combat.player.powers.dexterity += ODDLY_SMOOTH_STONE_DEXTERITY;
            }
            Relic::Strawberry => {}
            Relic::Pear => {}
            Relic::Mango => {}
            Relic::OldCoin => {}
            Relic::LeesWaffle => {}
            Relic::PotionBelt => {}
            Relic::Lantern => {
                combat.player.energy += LANTERN_ENERGY;
            }
            Relic::BagOfPreparation => {
                crate::combat::transition::player_draw_cards(combat, BAG_OF_PREPARATION_DRAW);
            }
            Relic::BagOfMarbles => {
                for monster in combat.monsters.iter_mut().filter(|monster| monster.alive) {
                    apply_monster_vulnerable_with_relics(
                        &mut monster.powers,
                        relics,
                        BAG_OF_MARBLES_VULNERABLE,
                    );
                }
            }
            Relic::BronzeScales => {
                combat.player.powers.thorns += BRONZE_SCALES_THORNS;
            }
            Relic::ThreadAndNeedle => {
                combat.player.powers.plated_armor += THREAD_AND_NEEDLE_PLATED_ARMOR;
            }
            Relic::RedSkull => {
                if combat.player.hp * 2 <= combat.player.max_hp {
                    combat.player.powers.strength += RED_SKULL_STRENGTH;
                }
            }
            Relic::Nunchaku => {}
            Relic::ArtOfWar => {}
            Relic::Shuriken => {}
            Relic::Kunai => {}
            Relic::LetterOpener => {}
            Relic::HappyFlower => {}
            Relic::Orichalcum => {}
            Relic::HornCleat => {}
            Relic::CaptainsWheel => {}
            Relic::MercuryHourglass => {}
            Relic::StoneCalendar => {}
            Relic::MeatOnTheBone => {}
            Relic::QuestionCard => {}
            Relic::BlackBlood => {}
            Relic::MealTicket => {}
            Relic::RegalPillow => {}
            Relic::DreamCatcher => {}
            Relic::EternalFeather => {}
            Relic::Torii => {}
            Relic::TungstenRod => {}
            Relic::CeramicFish => {}
            Relic::MembershipCard => {}
            Relic::SmilingMask => {}
            Relic::Pantograph => {}
            Relic::Ginger => {}
            Relic::Turnip => {}
            Relic::MarkOfPain => {}
            Relic::MagicFlower => {}
            Relic::PaperPhrog => {}
            Relic::ChampionBelt => {}
            Relic::PreservedInsect => {}
            Relic::Omamori => {}
            Relic::SlingOfCourage => {}
            Relic::MawBank => {}
            Relic::AncientTeaSet => {}
            Relic::Calipers => {}
            Relic::SingingBowl => {}
            Relic::DarkstonePeriapt => {}
            Relic::DuVuDoll => {}
            Relic::FusionHammer => {}
            Relic::Sozu => {}
            Relic::BustedCrown => {}
            Relic::VelvetChoker => {}
            Relic::ToyOrnithopter => {}
            Relic::MoltenEgg => {}
            Relic::ToxicEgg => {}
            Relic::FrozenEgg => {}
            Relic::TheBoot => {}
            Relic::BirdFacedUrn => {}
            Relic::CoffeeDripper => {}
            Relic::Anchor => {
                combat.player.block += ANCHOR_BLOCK;
            }
            Relic::InkBottle => {}
            Relic::OrnamentalFan => {}
            Relic::IceCream => {}
            Relic::ChemicalX => {}
            Relic::PhilosophersStone => {}
            Relic::SlaversCollar => {}
            Relic::Ectoplasm => {}
            Relic::RunicDome => {}
            Relic::StrikeDummy => {}
            Relic::Brimstone => {}
            Relic::WhiteBeastStatue => {}
        }
    }

    apply_start_of_player_turn_relics(combat);
}

#[must_use]
pub fn combat_healing_amount_with_relics(base_heal: i32, relics: &[Relic]) -> i32 {
    if base_heal <= 0 {
        return base_heal;
    }
    if relics.contains(&Relic::MagicFlower) {
        (base_heal * MAGIC_FLOWER_HEAL_NUMERATOR + MAGIC_FLOWER_HEAL_DENOMINATOR / 2)
            / MAGIC_FLOWER_HEAL_DENOMINATOR
    } else {
        base_heal
    }
}

pub fn heal_player_in_combat_with_relics(
    hp: &mut i32,
    max_hp: i32,
    base_heal: i32,
    relics: &[Relic],
) {
    let heal = combat_healing_amount_with_relics(base_heal, relics);
    *hp = (*hp + heal).min(max_hp);
}

pub fn apply_potion_use_relics_to_combat(combat: &mut CombatState) {
    if combat.relics.contains(&Relic::ToyOrnithopter) {
        heal_player_in_combat_with_relics(
            &mut combat.player.hp,
            combat.player.max_hp,
            TOY_ORNITHOPTER_HEAL,
            &combat.relics,
        );
    }
}

pub fn apply_potion_use_relics_to_run_hp(hp: &mut i32, max_hp: i32, relics: &[Relic]) {
    if relics.contains(&Relic::ToyOrnithopter) {
        *hp = (*hp + TOY_ORNITHOPTER_HEAL).min(max_hp);
    }
}

/// Whether player energy should carry over instead of refilling at turn start.
#[must_use]
pub fn preserves_energy_between_turns(relics: &[Relic]) -> bool {
    relics.contains(&Relic::IceCream)
}

pub fn reset_turn_relic_counters(state: &mut CombatState) {
    state.relic_counters.attacks_played_last_turn = state.relic_counters.attacks_played_this_turn;
    state.relic_counters.ornamental_fan_attacks_this_turn = 0;
    state.relic_counters.shuriken_attacks_this_turn = 0;
    state.relic_counters.kunai_attacks_this_turn = 0;
    state.relic_counters.letter_opener_skills_this_turn = 0;
    state.relic_counters.cards_played_this_turn = 0;
    state.relic_counters.attacks_played_this_turn = 0;
}

pub fn apply_start_of_player_turn_relics(state: &mut CombatState) {
    if !has_start_of_turn_relic(state) {
        return;
    }

    state.relic_counters.player_turns_started += 1;

    if state.relics.contains(&Relic::HappyFlower) {
        state.relic_counters.happy_flower_turns += 1;
        if state.relic_counters.happy_flower_turns >= HAPPY_FLOWER_THRESHOLD {
            state.relic_counters.happy_flower_turns = 0;
            state.player.energy += HAPPY_FLOWER_ENERGY;
        }
    }

    if state.relics.contains(&Relic::ArtOfWar)
        && state.relic_counters.player_turns_started > 1
        && state.relic_counters.attacks_played_last_turn == 0
    {
        state.player.energy += ART_OF_WAR_ENERGY;
    }

    match state.relic_counters.player_turns_started {
        HORN_CLEAT_TURN if state.relics.contains(&Relic::HornCleat) => {
            state.player.block += HORN_CLEAT_BLOCK;
        }
        CAPTAINS_WHEEL_TURN if state.relics.contains(&Relic::CaptainsWheel) => {
            state.player.block += CAPTAINS_WHEEL_BLOCK;
        }
        _ => {}
    }

    if state.relics.contains(&Relic::MercuryHourglass) {
        deal_unmodified_damage_to_living_monsters(state, MERCURY_HOURGLASS_DAMAGE);
    }

    if state.relics.contains(&Relic::Brimstone) {
        state.player.powers.strength += BRIMSTONE_PLAYER_STRENGTH;
        for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
            monster.powers.strength += BRIMSTONE_MONSTER_STRENGTH;
        }
    }
}

fn has_start_of_turn_relic(state: &CombatState) -> bool {
    state.relics.iter().any(|relic| {
        matches!(
            relic,
            Relic::HappyFlower
                | Relic::ArtOfWar
                | Relic::HornCleat
                | Relic::CaptainsWheel
                | Relic::MercuryHourglass
                | Relic::StoneCalendar
                | Relic::Brimstone
        )
    })
}

pub fn apply_end_of_player_turn_relics(state: &mut CombatState) {
    if state.relics.contains(&Relic::Orichalcum) && state.player.block == 0 {
        state.player.block += ORICHALCUM_BLOCK;
    }

    if state.relics.contains(&Relic::StoneCalendar)
        && state.relic_counters.player_turns_started == STONE_CALENDAR_TURN
    {
        deal_unmodified_damage_to_living_monsters(state, STONE_CALENDAR_DAMAGE);
    }
}

#[must_use]
pub fn mitigate_unblocked_attack_damage(relics: &[Relic], amount: i32) -> i32 {
    let mut mitigated = amount;
    if relics.contains(&Relic::Torii) && (1..=TORII_MAX_DAMAGE).contains(&mitigated) {
        mitigated = TORII_REDUCED_DAMAGE;
    }
    mitigate_hp_loss(relics, mitigated)
}

#[must_use]
pub fn mitigate_hp_loss(relics: &[Relic], amount: i32) -> i32 {
    let mut mitigated = amount.max(0);
    if relics.contains(&Relic::TungstenRod) {
        mitigated = (mitigated - TUNGSTEN_ROD_REDUCTION).max(0);
    }
    mitigated
}

#[must_use]
pub fn apply_attack_damage_relics_to_unblocked_damage(relics: &[Relic], amount: i32) -> i32 {
    if relics.contains(&Relic::TheBoot) && (1..=THE_BOOT_MAX_DAMAGE).contains(&amount) {
        THE_BOOT_DAMAGE
    } else {
        amount
    }
}

pub fn apply_player_weak_with_relics(
    powers: &mut crate::power::PlayerPowers,
    relics: &[Relic],
    amount: i32,
) {
    if !relics.contains(&Relic::Ginger) {
        crate::power::apply_player_weak(powers, amount);
    }
}

pub fn apply_player_frail_with_relics(
    powers: &mut crate::power::PlayerPowers,
    relics: &[Relic],
    amount: i32,
) {
    if !relics.contains(&Relic::Turnip) {
        crate::power::apply_player_frail(powers, amount);
    }
}

#[must_use]
pub fn attack_damage_with_vulnerable_relics(base: i32, vulnerable: i32, relics: &[Relic]) -> i32 {
    if relics.contains(&Relic::PaperPhrog) {
        crate::power::attack_damage_with_vulnerable_bonus(
            base,
            vulnerable,
            PAPER_PHROG_VULNERABLE_BONUS_NUMERATOR,
            PAPER_PHROG_VULNERABLE_BONUS_DENOMINATOR,
        )
    } else {
        crate::power::attack_damage_with_vulnerable(base, vulnerable)
    }
}

pub fn strike_damage_with_relics(relics: &[Relic], base: i32) -> i32 {
    if relics.contains(&Relic::StrikeDummy) {
        base + STRIKE_DUMMY_DAMAGE
    } else {
        base
    }
}

pub fn apply_monster_vulnerable_with_relics(
    powers: &mut crate::power::MonsterPowers,
    relics: &[Relic],
    amount: i32,
) {
    if amount <= 0 {
        return;
    }
    powers.vulnerable += amount;
    if relics.contains(&Relic::ChampionBelt) {
        powers.weak += CHAMPION_BELT_WEAK;
    }
}

#[must_use]
pub fn apply_on_card_play_relics(
    state: &mut CombatState,
    card_type: CardType,
) -> Vec<InternalAction> {
    let mut follow_ups = Vec::new();

    state.relic_counters.cards_played_this_turn += 1;
    if state.relics.contains(&Relic::ArtOfWar) && card_type == CardType::Attack {
        state.relic_counters.attacks_played_this_turn += 1;
    }

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

    if state.relics.contains(&Relic::Nunchaku) && card_type == CardType::Attack {
        state.relic_counters.nunchaku_attacks_played += 1;
        if state.relic_counters.nunchaku_attacks_played >= NUNCHAKU_THRESHOLD {
            state.relic_counters.nunchaku_attacks_played = 0;
            state.player.energy += NUNCHAKU_ENERGY;
        }
    }

    if state.relics.contains(&Relic::Shuriken) && card_type == CardType::Attack {
        state.relic_counters.shuriken_attacks_this_turn += 1;
        if state.relic_counters.shuriken_attacks_this_turn >= SHURIKEN_THRESHOLD {
            state.relic_counters.shuriken_attacks_this_turn = 0;
            state.player.powers.strength += SHURIKEN_STRENGTH;
        }
    }

    if state.relics.contains(&Relic::Kunai) && card_type == CardType::Attack {
        state.relic_counters.kunai_attacks_this_turn += 1;
        if state.relic_counters.kunai_attacks_this_turn >= KUNAI_THRESHOLD {
            state.relic_counters.kunai_attacks_this_turn = 0;
            state.player.powers.dexterity += KUNAI_DEXTERITY;
        }
    }

    if state.relics.contains(&Relic::LetterOpener) && card_type == CardType::Skill {
        state.relic_counters.letter_opener_skills_this_turn += 1;
        if state.relic_counters.letter_opener_skills_this_turn >= LETTER_OPENER_THRESHOLD {
            state.relic_counters.letter_opener_skills_this_turn = 0;
            deal_unmodified_damage_to_living_monsters(state, LETTER_OPENER_DAMAGE);
        }
    }

    if state.relics.contains(&Relic::BirdFacedUrn) && card_type == CardType::Power {
        heal_player_in_combat_with_relics(
            &mut state.player.hp,
            state.player.max_hp,
            BIRD_FACED_URN_HEAL,
            &state.relics,
        );
    }

    follow_ups
}

#[must_use]
pub fn can_play_card_with_relics(state: &CombatState) -> bool {
    !state.relics.contains(&Relic::VelvetChoker)
        || state.relic_counters.cards_played_this_turn < VELVET_CHOKER_CARD_LIMIT
}

fn deal_unmodified_damage_to_living_monsters(state: &mut CombatState, amount: i32) {
    for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
        crate::combat::damage::deal_unmodified_damage_to_monster(monster, amount);
    }
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
    fn return_random_relic_pops_from_front_for_requested_tier() {
        let mut pools = RelicPoolState {
            common: vec![RelicKey::Anchor, RelicKey::Vajra],
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        };

        let relic = pools.return_random_relic(RelicTier::Common, &RelicSpawnContext::default());

        assert_eq!(relic, RelicKey::Anchor);
        assert_eq!(pools.common, vec![RelicKey::Vajra]);
    }

    #[test]
    fn return_random_relic_end_pops_from_back_for_requested_tier() {
        let mut pools = RelicPoolState {
            common: vec![RelicKey::Anchor, RelicKey::Vajra],
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        };

        let relic = pools.return_random_relic_end(RelicTier::Common, &RelicSpawnContext::default());

        assert_eq!(relic, RelicKey::Vajra);
        assert_eq!(pools.common, vec![RelicKey::Anchor]);
    }

    #[test]
    fn remove_relic_prunes_key_from_any_pool() {
        let mut pools = RelicPoolState {
            common: vec![RelicKey::Anchor],
            uncommon: vec![RelicKey::ToxicEgg],
            rare: Vec::new(),
            shop: vec![RelicKey::MembershipCard],
            boss: Vec::new(),
        };

        pools.remove_relic(RelicKey::ToxicEgg);
        pools.remove_relic(RelicKey::MembershipCard);

        assert_eq!(pools.common, vec![RelicKey::Anchor]);
        assert!(pools.uncommon.is_empty());
        assert!(pools.shop.is_empty());
    }

    #[test]
    fn return_random_relic_falls_through_empty_common_and_uncommon_pools() {
        let mut pools = RelicPoolState {
            common: Vec::new(),
            uncommon: vec![RelicKey::InkBottle],
            rare: vec![RelicKey::IceCream],
            shop: Vec::new(),
            boss: Vec::new(),
        };

        let relic = pools.return_random_relic(RelicTier::Common, &RelicSpawnContext::default());

        assert_eq!(relic, RelicKey::InkBottle);
        assert!(pools.uncommon.is_empty());
        assert_eq!(pools.rare, vec![RelicKey::IceCream]);
    }

    #[test]
    fn return_random_relic_uses_circlets_for_empty_rare_and_boss_pools() {
        let mut pools = RelicPoolState {
            common: Vec::new(),
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        };

        assert_eq!(
            pools.return_random_relic(RelicTier::Rare, &RelicSpawnContext::default()),
            RelicKey::Circlet
        );
        assert_eq!(
            pools.return_random_relic(RelicTier::Boss, &RelicSpawnContext::default()),
            RelicKey::RedCirclet
        );
    }

    #[test]
    fn rejected_relic_is_discarded_then_retry_pops_from_back() {
        let mut pools = RelicPoolState {
            common: vec![RelicKey::TinyChest, RelicKey::Anchor, RelicKey::Vajra],
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        };
        let context = RelicSpawnContext {
            floor_num: 36,
            ..RelicSpawnContext::default()
        };

        let relic = pools.return_random_relic(RelicTier::Common, &context);

        assert_eq!(relic, RelicKey::Vajra);
        assert_eq!(pools.common, vec![RelicKey::Anchor]);
    }

    #[test]
    fn relic_spawn_filters_match_target_floor_shop_and_owned_gates() {
        let mut context = RelicSpawnContext {
            floor_num: 49,
            shop_room: true,
            owned_relics: vec![RelicKey::Girya, RelicKey::PeacePipe],
            has_non_basic_attack: false,
            has_non_basic_skill: false,
            has_power: false,
        };

        assert!(!relic_can_spawn(RelicKey::MawBank, &context));
        assert!(!relic_can_spawn(RelicKey::PotionBelt, &context));
        assert!(!relic_can_spawn(RelicKey::Shovel, &context));
        assert!(!relic_can_spawn(RelicKey::BottledFlame, &context));

        context.floor_num = 20;
        context.shop_room = false;
        context.owned_relics.clear();
        context.has_non_basic_attack = true;

        assert!(relic_can_spawn(RelicKey::MawBank, &context));
        assert!(relic_can_spawn(RelicKey::PotionBelt, &context));
        assert!(relic_can_spawn(RelicKey::Shovel, &context));
        assert!(relic_can_spawn(RelicKey::BottledFlame, &context));
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
    fn blood_vial_heals_two_at_combat_start_capped_by_max_hp() {
        let mut combat = CombatState::initial_fixture();
        combat.player.hp = 70;

        apply_start_of_combat_relics(&mut combat, &[Relic::BloodVial]);
        assert_eq!(combat.player.hp, 70 + BLOOD_VIAL_HEAL);

        apply_start_of_combat_relics(&mut combat, &[Relic::BloodVial]);
        combat.player.hp = combat.player.max_hp - 1;
        apply_start_of_combat_relics(&mut combat, &[Relic::BloodVial]);
        assert_eq!(combat.player.hp, combat.player.max_hp);
    }

    #[test]
    fn magic_flower_rounds_combat_healing_half_up() {
        assert_eq!(
            combat_healing_amount_with_relics(2, &[Relic::MagicFlower]),
            3
        );
        assert_eq!(
            combat_healing_amount_with_relics(5, &[Relic::MagicFlower]),
            8
        );
        assert_eq!(
            combat_healing_amount_with_relics(25, &[Relic::MagicFlower]),
            38
        );
        assert_eq!(combat_healing_amount_with_relics(5, &[]), 5);
    }

    #[test]
    fn magic_flower_increases_blood_vial_combat_healing() {
        let mut combat = CombatState::initial_fixture();
        combat.player.hp = 70;

        apply_start_of_combat_relics(&mut combat, &[Relic::BloodVial, Relic::MagicFlower]);

        assert_eq!(combat.player.hp, 70 + 3);
    }

    #[test]
    fn lantern_grants_one_energy_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Lantern]);

        assert_eq!(
            combat.player.energy,
            combat.player.max_energy + LANTERN_ENERGY
        );
    }

    #[test]
    fn bag_of_preparation_draws_two_at_combat_start() {
        let mut combat = CombatState::initial_fixture();
        let hand_before = combat.piles.hand.len();
        let draw_before = combat.piles.draw_pile.len();
        let expected_draw = BAG_OF_PREPARATION_DRAW.min(draw_before);

        apply_start_of_combat_relics(&mut combat, &[Relic::BagOfPreparation]);

        assert_eq!(combat.piles.hand.len(), hand_before + expected_draw);
        assert_eq!(combat.piles.draw_pile.len(), draw_before - expected_draw);
    }

    #[test]
    fn bag_of_marbles_applies_vulnerable_to_living_monsters() {
        let mut combat = CombatState::initial_fixture();
        combat
            .monsters
            .push(crate::content::monsters::monster_state(
                &crate::content::monsters::CULTIST_A0,
                crate::MonsterId::new(2),
            ));
        combat.monsters[1].alive = false;

        apply_start_of_combat_relics(&mut combat, &[Relic::BagOfMarbles]);

        assert_eq!(
            combat.monsters[0].powers.vulnerable,
            BAG_OF_MARBLES_VULNERABLE
        );
        assert_eq!(combat.monsters[1].powers.vulnerable, 0);
    }

    #[test]
    fn champion_belt_adds_weak_when_bag_of_marbles_applies_vulnerable() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::BagOfMarbles, Relic::ChampionBelt]);

        assert_eq!(
            combat.monsters[0].powers.vulnerable,
            BAG_OF_MARBLES_VULNERABLE
        );
        assert_eq!(combat.monsters[0].powers.weak, CHAMPION_BELT_WEAK);
    }

    #[test]
    fn defensive_start_relics_grant_player_powers() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::BronzeScales, Relic::ThreadAndNeedle]);

        assert_eq!(combat.player.powers.thorns, BRONZE_SCALES_THORNS);
        assert_eq!(
            combat.player.powers.plated_armor,
            THREAD_AND_NEEDLE_PLATED_ARMOR
        );
    }

    #[test]
    fn red_skull_grants_strength_only_at_or_below_half_hp() {
        let mut high_hp = CombatState::initial_fixture();
        high_hp.player.hp = high_hp.player.max_hp / 2 + 1;
        apply_start_of_combat_relics(&mut high_hp, &[Relic::RedSkull]);
        assert_eq!(high_hp.player.powers.strength, 0);

        let mut low_hp = CombatState::initial_fixture();
        low_hp.player.hp = low_hp.player.max_hp / 2;
        apply_start_of_combat_relics(&mut low_hp, &[Relic::RedSkull]);
        assert_eq!(low_hp.player.powers.strength, RED_SKULL_STRENGTH);
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
        assert_eq!(Relic::BloodVial.content_id(), BLOOD_VIAL_ID);
        assert_eq!(Relic::Pear.content_id(), PEAR_ID);
        assert_eq!(Relic::Mango.content_id(), MANGO_ID);
        assert_eq!(Relic::OldCoin.content_id(), OLD_COIN_ID);
        assert_eq!(Relic::LeesWaffle.content_id(), LEES_WAFFLE_ID);
        assert_eq!(Relic::PotionBelt.content_id(), POTION_BELT_ID);
        assert_eq!(Relic::Lantern.content_id(), LANTERN_ID);
        assert_eq!(Relic::BagOfPreparation.content_id(), BAG_OF_PREPARATION_ID);
        assert_eq!(Relic::BagOfMarbles.content_id(), BAG_OF_MARBLES_ID);
        assert_eq!(Relic::BronzeScales.content_id(), BRONZE_SCALES_ID);
        assert_eq!(Relic::ThreadAndNeedle.content_id(), THREAD_AND_NEEDLE_ID);
        assert_eq!(Relic::RedSkull.content_id(), RED_SKULL_ID);
        assert_eq!(Relic::Nunchaku.content_id(), NUNCHAKU_ID);
        assert_eq!(Relic::Shuriken.content_id(), SHURIKEN_ID);
        assert_eq!(Relic::Kunai.content_id(), KUNAI_ID);
        assert_eq!(Relic::LetterOpener.content_id(), LETTER_OPENER_ID);
        assert_eq!(Relic::HappyFlower.content_id(), HAPPY_FLOWER_ID);
        assert_eq!(Relic::Orichalcum.content_id(), ORICHALCUM_ID);
        assert_eq!(Relic::HornCleat.content_id(), HORN_CLEAT_ID);
        assert_eq!(Relic::CaptainsWheel.content_id(), CAPTAINS_WHEEL_ID);
        assert_eq!(Relic::MercuryHourglass.content_id(), MERCURY_HOURGLASS_ID);
        assert_eq!(Relic::StoneCalendar.content_id(), STONE_CALENDAR_ID);
        assert_eq!(Relic::MeatOnTheBone.content_id(), MEAT_ON_THE_BONE_ID);
        assert_eq!(Relic::BlackBlood.content_id(), BLACK_BLOOD_ID);
        assert_eq!(Relic::MealTicket.content_id(), MEAL_TICKET_ID);
        assert_eq!(Relic::RegalPillow.content_id(), REGAL_PILLOW_ID);
        assert_eq!(Relic::DreamCatcher.content_id(), DREAM_CATCHER_ID);
        assert_eq!(Relic::EternalFeather.content_id(), ETERNAL_FEATHER_ID);
        assert_eq!(Relic::Torii.content_id(), TORII_ID);
        assert_eq!(Relic::TungstenRod.content_id(), TUNGSTEN_ROD_ID);
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
        assert_eq!(
            Relic::from_content_id(BLOOD_VIAL_ID),
            Some(Relic::BloodVial)
        );
        assert_eq!(Relic::from_content_id(PEAR_ID), Some(Relic::Pear));
        assert_eq!(Relic::from_content_id(MANGO_ID), Some(Relic::Mango));
        assert_eq!(Relic::from_content_id(OLD_COIN_ID), Some(Relic::OldCoin));
        assert_eq!(
            Relic::from_content_id(LEES_WAFFLE_ID),
            Some(Relic::LeesWaffle)
        );
        assert_eq!(
            Relic::from_content_id(POTION_BELT_ID),
            Some(Relic::PotionBelt)
        );
        assert_eq!(Relic::from_content_id(LANTERN_ID), Some(Relic::Lantern));
        assert_eq!(
            Relic::from_content_id(BAG_OF_PREPARATION_ID),
            Some(Relic::BagOfPreparation)
        );
        assert_eq!(
            Relic::from_content_id(BAG_OF_MARBLES_ID),
            Some(Relic::BagOfMarbles)
        );
        assert_eq!(
            Relic::from_content_id(BRONZE_SCALES_ID),
            Some(Relic::BronzeScales)
        );
        assert_eq!(
            Relic::from_content_id(THREAD_AND_NEEDLE_ID),
            Some(Relic::ThreadAndNeedle)
        );
        assert_eq!(Relic::from_content_id(RED_SKULL_ID), Some(Relic::RedSkull));
        assert_eq!(Relic::from_content_id(NUNCHAKU_ID), Some(Relic::Nunchaku));
        assert_eq!(Relic::from_content_id(SHURIKEN_ID), Some(Relic::Shuriken));
        assert_eq!(Relic::from_content_id(KUNAI_ID), Some(Relic::Kunai));
        assert_eq!(
            Relic::from_content_id(LETTER_OPENER_ID),
            Some(Relic::LetterOpener)
        );
        assert_eq!(
            Relic::from_content_id(HAPPY_FLOWER_ID),
            Some(Relic::HappyFlower)
        );
        assert_eq!(
            Relic::from_content_id(ORICHALCUM_ID),
            Some(Relic::Orichalcum)
        );
        assert_eq!(
            Relic::from_content_id(HORN_CLEAT_ID),
            Some(Relic::HornCleat)
        );
        assert_eq!(
            Relic::from_content_id(CAPTAINS_WHEEL_ID),
            Some(Relic::CaptainsWheel)
        );
        assert_eq!(
            Relic::from_content_id(MERCURY_HOURGLASS_ID),
            Some(Relic::MercuryHourglass)
        );
        assert_eq!(
            Relic::from_content_id(STONE_CALENDAR_ID),
            Some(Relic::StoneCalendar)
        );
        assert_eq!(
            Relic::from_content_id(MEAT_ON_THE_BONE_ID),
            Some(Relic::MeatOnTheBone)
        );
        assert_eq!(
            Relic::from_content_id(BLACK_BLOOD_ID),
            Some(Relic::BlackBlood)
        );
        assert_eq!(
            Relic::from_content_id(MEAL_TICKET_ID),
            Some(Relic::MealTicket)
        );
        assert_eq!(
            Relic::from_content_id(REGAL_PILLOW_ID),
            Some(Relic::RegalPillow)
        );
        assert_eq!(
            Relic::from_content_id(DREAM_CATCHER_ID),
            Some(Relic::DreamCatcher)
        );
        assert_eq!(
            Relic::from_content_id(ETERNAL_FEATHER_ID),
            Some(Relic::EternalFeather)
        );
        assert_eq!(Relic::from_content_id(TORII_ID), Some(Relic::Torii));
        assert_eq!(
            Relic::from_content_id(TUNGSTEN_ROD_ID),
            Some(Relic::TungstenRod)
        );
        assert_eq!(Relic::from_content_id(ContentId::new(999)), None);
        assert_eq!(Relic::FusionHammer.content_id(), FUSION_HAMMER_ID);
        assert_eq!(
            Relic::from_content_id(FUSION_HAMMER_ID),
            Some(Relic::FusionHammer)
        );
        assert_eq!(Relic::Sozu.content_id(), SOZU_ID);
        assert_eq!(Relic::from_content_id(SOZU_ID), Some(Relic::Sozu));
        assert_eq!(Relic::BustedCrown.content_id(), BUSTED_CROWN_ID);
        assert_eq!(
            Relic::from_content_id(BUSTED_CROWN_ID),
            Some(Relic::BustedCrown)
        );
        assert_eq!(Relic::VelvetChoker.content_id(), VELVET_CHOKER_ID);
        assert_eq!(
            Relic::from_content_id(VELVET_CHOKER_ID),
            Some(Relic::VelvetChoker)
        );
        assert_eq!(Relic::ToyOrnithopter.content_id(), TOY_ORNITHOPTER_ID);
        assert_eq!(
            Relic::from_content_id(TOY_ORNITHOPTER_ID),
            Some(Relic::ToyOrnithopter)
        );
        assert_eq!(Relic::MoltenEgg.content_id(), MOLTEN_EGG_ID);
        assert_eq!(
            Relic::from_content_id(MOLTEN_EGG_ID),
            Some(Relic::MoltenEgg)
        );
        assert_eq!(Relic::ToxicEgg.content_id(), TOXIC_EGG_ID);
        assert_eq!(Relic::from_content_id(TOXIC_EGG_ID), Some(Relic::ToxicEgg));
        assert_eq!(Relic::FrozenEgg.content_id(), FROZEN_EGG_ID);
        assert_eq!(
            Relic::from_content_id(FROZEN_EGG_ID),
            Some(Relic::FrozenEgg)
        );
        assert_eq!(Relic::TheBoot.content_id(), THE_BOOT_ID);
        assert_eq!(Relic::from_content_id(THE_BOOT_ID), Some(Relic::TheBoot));
        assert_eq!(Relic::BirdFacedUrn.content_id(), BIRD_FACED_URN_ID);
        assert_eq!(
            Relic::from_content_id(BIRD_FACED_URN_ID),
            Some(Relic::BirdFacedUrn)
        );
        assert_eq!(Relic::ArtOfWar.content_id(), ART_OF_WAR_ID);
        assert_eq!(Relic::from_content_id(ART_OF_WAR_ID), Some(Relic::ArtOfWar));
        assert_eq!(Relic::QuestionCard.content_id(), QUESTION_CARD_ID);
        assert_eq!(
            Relic::from_content_id(QUESTION_CARD_ID),
            Some(Relic::QuestionCard)
        );
        assert_eq!(Relic::Omamori.content_id(), OMAMORI_ID);
        assert_eq!(Relic::from_content_id(OMAMORI_ID), Some(Relic::Omamori));
        assert_eq!(Relic::SlingOfCourage.content_id(), SLING_OF_COURAGE_ID);
        assert_eq!(
            Relic::from_content_id(SLING_OF_COURAGE_ID),
            Some(Relic::SlingOfCourage)
        );
        assert_eq!(Relic::MawBank.content_id(), MAW_BANK_ID);
        assert_eq!(Relic::from_content_id(MAW_BANK_ID), Some(Relic::MawBank));
        assert_eq!(Relic::AncientTeaSet.content_id(), ANCIENT_TEA_SET_ID);
        assert_eq!(
            Relic::from_content_id(ANCIENT_TEA_SET_ID),
            Some(Relic::AncientTeaSet)
        );
        assert_eq!(Relic::Calipers.content_id(), CALIPERS_ID);
        assert_eq!(Relic::from_content_id(CALIPERS_ID), Some(Relic::Calipers));
        assert_eq!(Relic::SingingBowl.content_id(), SINGING_BOWL_ID);
        assert_eq!(
            Relic::from_content_id(SINGING_BOWL_ID),
            Some(Relic::SingingBowl)
        );
        assert_eq!(Relic::ChemicalX.content_id(), CHEMICAL_X_ID);
        assert_eq!(
            Relic::from_content_id(CHEMICAL_X_ID),
            Some(Relic::ChemicalX)
        );
        assert_eq!(Relic::PhilosophersStone.content_id(), PHILOSOPHERS_STONE_ID);
        assert_eq!(
            Relic::from_content_id(PHILOSOPHERS_STONE_ID),
            Some(Relic::PhilosophersStone)
        );
        assert_eq!(Relic::SlaversCollar.content_id(), SLAVERS_COLLAR_ID);
        assert_eq!(
            Relic::from_content_id(SLAVERS_COLLAR_ID),
            Some(Relic::SlaversCollar)
        );
        assert_eq!(Relic::Ectoplasm.content_id(), ECTOPLASM_ID);
        assert_eq!(Relic::from_content_id(ECTOPLASM_ID), Some(Relic::Ectoplasm));
        assert_eq!(Relic::RunicDome.content_id(), RUNIC_DOME_ID);
        assert_eq!(
            Relic::from_content_id(RUNIC_DOME_ID),
            Some(Relic::RunicDome)
        );
        assert_eq!(Relic::StrikeDummy.content_id(), STRIKE_DUMMY_ID);
        assert_eq!(
            Relic::from_content_id(STRIKE_DUMMY_ID),
            Some(Relic::StrikeDummy)
        );
        assert_eq!(Relic::Brimstone.content_id(), BRIMSTONE_ID);
        assert_eq!(Relic::from_content_id(BRIMSTONE_ID), Some(Relic::Brimstone));
        assert_eq!(Relic::WhiteBeastStatue.content_id(), WHITE_BEAST_STATUE_ID);
        assert_eq!(
            Relic::from_content_id(WHITE_BEAST_STATUE_ID),
            Some(Relic::WhiteBeastStatue)
        );
    }

    #[test]
    fn ice_cream_preserves_energy_between_turns_flag() {
        assert!(!preserves_energy_between_turns(&[]));
        assert!(preserves_energy_between_turns(&[Relic::IceCream]));
    }

    #[test]
    fn torii_reduces_small_unblocked_attack_damage_before_tungsten_rod() {
        assert_eq!(mitigate_unblocked_attack_damage(&[Relic::Torii], 5), 1);
        assert_eq!(mitigate_unblocked_attack_damage(&[Relic::Torii], 6), 6);
        assert_eq!(
            mitigate_unblocked_attack_damage(&[Relic::Torii, Relic::TungstenRod], 5),
            0
        );
    }

    #[test]
    fn tungsten_rod_reduces_non_attack_hp_loss_by_one() {
        assert_eq!(mitigate_hp_loss(&[Relic::TungstenRod], 3), 2);
        assert_eq!(mitigate_hp_loss(&[Relic::TungstenRod], 1), 0);
        assert_eq!(mitigate_hp_loss(&[], 3), 3);
    }

    #[test]
    fn the_boot_increases_small_unblocked_attack_damage_to_five() {
        assert_eq!(
            apply_attack_damage_relics_to_unblocked_damage(&[Relic::TheBoot], 1),
            THE_BOOT_DAMAGE
        );
        assert_eq!(
            apply_attack_damage_relics_to_unblocked_damage(&[Relic::TheBoot], 4),
            THE_BOOT_DAMAGE
        );
        assert_eq!(
            apply_attack_damage_relics_to_unblocked_damage(&[Relic::TheBoot], 5),
            5
        );
        assert_eq!(apply_attack_damage_relics_to_unblocked_damage(&[], 4), 4);
    }

    #[test]
    fn toy_ornithopter_heals_on_potion_use_in_combat() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::ToyOrnithopter];
        combat.player.hp = 70;

        apply_potion_use_relics_to_combat(&mut combat);

        assert_eq!(combat.player.hp, 70 + TOY_ORNITHOPTER_HEAL);
    }

    #[test]
    fn toy_ornithopter_combat_heal_uses_magic_flower() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::ToyOrnithopter, Relic::MagicFlower];
        combat.player.hp = 60;

        apply_potion_use_relics_to_combat(&mut combat);

        assert_eq!(combat.player.hp, 68);
    }

    #[test]
    fn toy_ornithopter_noncombat_heal_caps_at_max_hp() {
        let mut hp = 78;

        apply_potion_use_relics_to_run_hp(&mut hp, 80, &[Relic::ToyOrnithopter]);

        assert_eq!(hp, 80);
    }

    #[test]
    fn bird_faced_urn_heals_when_power_is_played() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::BirdFacedUrn];
        combat.player.hp = 70;

        let follow_ups = apply_on_card_play_relics(&mut combat, CardType::Power);

        assert!(follow_ups.is_empty());
        assert_eq!(combat.player.hp, 70 + BIRD_FACED_URN_HEAL);
    }

    #[test]
    fn bird_faced_urn_ignores_non_power_cards() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::BirdFacedUrn];
        combat.player.hp = 70;

        let _ = apply_on_card_play_relics(&mut combat, CardType::Skill);

        assert_eq!(combat.player.hp, 70);
    }

    #[test]
    fn bird_faced_urn_combat_heal_uses_magic_flower() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::BirdFacedUrn, Relic::MagicFlower];
        combat.player.hp = 70;

        let _ = apply_on_card_play_relics(&mut combat, CardType::Power);

        assert_eq!(combat.player.hp, 73);
    }

    #[test]
    fn ginger_prevents_player_weak_without_consuming_artifact() {
        let mut powers = crate::power::PlayerPowers {
            artifact: 1,
            ..Default::default()
        };

        apply_player_weak_with_relics(&mut powers, &[Relic::Ginger], 2);

        assert_eq!(powers.weak, 0);
        assert_eq!(powers.artifact, 1);
    }

    #[test]
    fn turnip_prevents_player_frail_without_consuming_artifact() {
        let mut powers = crate::power::PlayerPowers {
            artifact: 1,
            ..Default::default()
        };

        apply_player_frail_with_relics(&mut powers, &[Relic::Turnip], 2);

        assert_eq!(powers.frail, 0);
        assert_eq!(powers.artifact, 1);
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
    fn nunchaku_grants_energy_on_tenth_attack_without_turn_reset() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::Nunchaku];
        combat.player.energy = 0;

        for _ in 0..9 {
            let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        }
        reset_turn_relic_counters(&mut combat);
        assert_eq!(combat.player.energy, 0);
        assert_eq!(combat.relic_counters.nunchaku_attacks_played, 9);

        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        assert_eq!(combat.player.energy, NUNCHAKU_ENERGY);
        assert_eq!(combat.relic_counters.nunchaku_attacks_played, 0);
    }

    #[test]
    fn shuriken_grants_strength_on_third_attack_this_turn() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::Shuriken];

        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        assert_eq!(combat.player.powers.strength, 0);

        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        assert_eq!(combat.player.powers.strength, SHURIKEN_STRENGTH);
        assert_eq!(combat.relic_counters.shuriken_attacks_this_turn, 0);
    }

    #[test]
    fn kunai_grants_dexterity_on_third_attack_this_turn() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::Kunai];

        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(combat.player.powers.dexterity, KUNAI_DEXTERITY);
        assert_eq!(combat.relic_counters.kunai_attacks_this_turn, 0);
    }

    #[test]
    fn letter_opener_deals_damage_to_all_living_monsters_on_third_skill() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::LetterOpener];
        combat
            .monsters
            .push(crate::content::monsters::monster_state(
                &crate::content::monsters::CULTIST_A0,
                crate::MonsterId::new(2),
            ));
        combat.monsters[1].alive = false;
        let hp_before = combat.monsters[0].hp;

        let _ = apply_on_card_play_relics(&mut combat, CardType::Skill);
        let _ = apply_on_card_play_relics(&mut combat, CardType::Skill);
        assert_eq!(combat.monsters[0].hp, hp_before);

        let _ = apply_on_card_play_relics(&mut combat, CardType::Skill);
        assert_eq!(combat.monsters[0].hp, hp_before - LETTER_OPENER_DAMAGE);
        assert_eq!(
            combat.monsters[1].hp,
            crate::content::monsters::CULTIST_A0.hp
        );
        assert_eq!(combat.relic_counters.letter_opener_skills_this_turn, 0);
    }

    #[test]
    fn card_play_relic_hook_counts_cards_played_this_turn() {
        let mut combat = CombatState::initial_fixture();

        let _ = apply_on_card_play_relics(&mut combat, CardType::Skill);
        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);

        assert_eq!(combat.relic_counters.cards_played_this_turn, 2);
    }

    #[test]
    fn art_of_war_does_not_grant_energy_on_first_player_turn() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::ArtOfWar];
        combat.player.energy = 3;

        apply_start_of_player_turn_relics(&mut combat);

        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.relic_counters.player_turns_started, 1);
    }

    #[test]
    fn art_of_war_grants_energy_after_turn_with_no_attacks() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::ArtOfWar];
        combat.relic_counters.player_turns_started = 1;
        combat.relic_counters.attacks_played_this_turn = 0;
        combat.player.energy = 3;

        reset_turn_relic_counters(&mut combat);
        apply_start_of_player_turn_relics(&mut combat);

        assert_eq!(combat.player.energy, 3 + ART_OF_WAR_ENERGY);
        assert_eq!(combat.relic_counters.attacks_played_last_turn, 0);
    }

    #[test]
    fn art_of_war_skips_energy_after_turn_with_attack() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::ArtOfWar];
        combat.relic_counters.player_turns_started = 1;
        combat.player.energy = 3;

        let _ = apply_on_card_play_relics(&mut combat, CardType::Attack);
        reset_turn_relic_counters(&mut combat);
        apply_start_of_player_turn_relics(&mut combat);

        assert_eq!(combat.player.energy, 3);
        assert_eq!(combat.relic_counters.attacks_played_last_turn, 1);
    }

    #[test]
    fn turn_reset_clears_turn_scoped_card_play_relic_counters() {
        let mut combat = CombatState::initial_fixture();
        combat.relic_counters.ornamental_fan_attacks_this_turn = 2;
        combat.relic_counters.shuriken_attacks_this_turn = 2;
        combat.relic_counters.kunai_attacks_this_turn = 2;
        combat.relic_counters.letter_opener_skills_this_turn = 2;
        combat.relic_counters.cards_played_this_turn = 6;
        combat.relic_counters.attacks_played_this_turn = 4;
        combat.relic_counters.nunchaku_attacks_played = 9;
        combat.relic_counters.player_turns_started = 3;
        combat.relic_counters.happy_flower_turns = 2;

        reset_turn_relic_counters(&mut combat);

        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.shuriken_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.kunai_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.letter_opener_skills_this_turn, 0);
        assert_eq!(combat.relic_counters.cards_played_this_turn, 0);
        assert_eq!(combat.relic_counters.attacks_played_this_turn, 0);
        assert_eq!(combat.relic_counters.attacks_played_last_turn, 4);
        assert_eq!(combat.relic_counters.nunchaku_attacks_played, 9);
        assert_eq!(combat.relic_counters.player_turns_started, 3);
        assert_eq!(combat.relic_counters.happy_flower_turns, 2);
    }

    #[test]
    fn happy_flower_grants_energy_every_third_player_turn() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::HappyFlower];
        combat.player.energy = 0;

        apply_start_of_player_turn_relics(&mut combat);
        apply_start_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.energy, 0);
        assert_eq!(combat.relic_counters.happy_flower_turns, 2);

        apply_start_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.energy, HAPPY_FLOWER_ENERGY);
        assert_eq!(combat.relic_counters.happy_flower_turns, 0);
    }

    #[test]
    fn defensive_turn_relics_grant_block_on_target_turns() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::HornCleat, Relic::CaptainsWheel];

        apply_start_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.block, 0);

        apply_start_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.block, HORN_CLEAT_BLOCK);

        combat.player.block = 0;
        apply_start_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.block, CAPTAINS_WHEEL_BLOCK);
    }

    #[test]
    fn mercury_hourglass_damages_all_living_monsters_at_turn_start() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::MercuryHourglass];
        combat
            .monsters
            .push(crate::content::monsters::monster_state(
                &crate::content::monsters::CULTIST_A0,
                crate::MonsterId::new(2),
            ));
        combat.monsters[1].alive = false;
        let living_hp = combat.monsters[0].hp;
        let dead_hp = combat.monsters[1].hp;

        apply_start_of_player_turn_relics(&mut combat);

        assert_eq!(combat.monsters[0].hp, living_hp - MERCURY_HOURGLASS_DAMAGE);
        assert_eq!(combat.monsters[1].hp, dead_hp);
    }

    #[test]
    fn brimstone_grants_player_and_living_monsters_strength_at_turn_start() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::Brimstone];
        combat
            .monsters
            .push(crate::content::monsters::monster_state(
                &crate::content::monsters::CULTIST_A0,
                crate::MonsterId::new(2),
            ));
        combat.monsters[1].alive = false;

        apply_start_of_player_turn_relics(&mut combat);

        assert_eq!(combat.player.powers.strength, BRIMSTONE_PLAYER_STRENGTH);
        assert_eq!(
            combat.monsters[0].powers.strength,
            BRIMSTONE_MONSTER_STRENGTH
        );
        assert_eq!(combat.monsters[1].powers.strength, 0);
    }

    #[test]
    fn orichalcum_grants_block_only_when_ending_with_no_block() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::Orichalcum];
        combat.player.block = 0;

        apply_end_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.block, ORICHALCUM_BLOCK);

        combat.player.block = 2;
        apply_end_of_player_turn_relics(&mut combat);
        assert_eq!(combat.player.block, 2);
    }

    #[test]
    fn stone_calendar_damages_all_living_monsters_on_seventh_turn_end() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::StoneCalendar];
        combat.relic_counters.player_turns_started = STONE_CALENDAR_TURN - 1;
        let hp_before = combat.monsters[0].hp;

        apply_end_of_player_turn_relics(&mut combat);
        assert_eq!(combat.monsters[0].hp, hp_before);

        combat.relic_counters.player_turns_started = STONE_CALENDAR_TURN;
        apply_end_of_player_turn_relics(&mut combat);
        assert_eq!(combat.monsters[0].hp, hp_before - STONE_CALENDAR_DAMAGE);
    }

    #[test]
    fn run_combat_entry_counts_first_player_turn_for_turn_relics() {
        let mut combat = CombatState::initial_fixture();
        combat.relics = vec![Relic::HappyFlower, Relic::MercuryHourglass];
        let hp_before = combat.monsters[0].hp;

        apply_start_of_combat_relics(&mut combat, &[Relic::HappyFlower, Relic::MercuryHourglass]);

        assert_eq!(combat.relic_counters.player_turns_started, 1);
        assert_eq!(combat.relic_counters.happy_flower_turns, 1);
        assert_eq!(combat.monsters[0].hp, hp_before - MERCURY_HOURGLASS_DAMAGE);
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
            nunchaku_attacks_played: 9,
            shuriken_attacks_this_turn: 1,
            kunai_attacks_this_turn: 2,
            letter_opener_skills_this_turn: 1,
            cards_played_this_turn: 5,
            attacks_played_this_turn: 3,
            attacks_played_last_turn: 1,
            player_turns_started: 6,
            happy_flower_turns: 2,
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
