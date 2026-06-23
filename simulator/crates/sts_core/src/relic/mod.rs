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
    Shuriken,
    Kunai,
    LetterOpener,
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
            Relic::Shuriken => SHURIKEN_ID,
            Relic::Kunai => KUNAI_ID,
            Relic::LetterOpener => LETTER_OPENER_ID,
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
            id if id == SHURIKEN_ID => Some(Relic::Shuriken),
            id if id == KUNAI_ID => Some(Relic::Kunai),
            id if id == LETTER_OPENER_ID => Some(Relic::LetterOpener),
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
            Relic::BloodVial => {
                combat.player.hp = (combat.player.hp + BLOOD_VIAL_HEAL).min(combat.player.max_hp);
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
                    monster.powers.vulnerable += BAG_OF_MARBLES_VULNERABLE;
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
            Relic::Shuriken => {}
            Relic::Kunai => {}
            Relic::LetterOpener => {}
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
    state.relic_counters.shuriken_attacks_this_turn = 0;
    state.relic_counters.kunai_attacks_this_turn = 0;
    state.relic_counters.letter_opener_skills_this_turn = 0;
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
            for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
                crate::combat::damage::deal_unmodified_damage_to_monster(
                    monster,
                    LETTER_OPENER_DAMAGE,
                );
            }
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
    fn turn_reset_clears_turn_scoped_card_play_relic_counters() {
        let mut combat = CombatState::initial_fixture();
        combat.relic_counters.ornamental_fan_attacks_this_turn = 2;
        combat.relic_counters.shuriken_attacks_this_turn = 2;
        combat.relic_counters.kunai_attacks_this_turn = 2;
        combat.relic_counters.letter_opener_skills_this_turn = 2;
        combat.relic_counters.nunchaku_attacks_played = 9;

        reset_turn_relic_counters(&mut combat);

        assert_eq!(combat.relic_counters.ornamental_fan_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.shuriken_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.kunai_attacks_this_turn, 0);
        assert_eq!(combat.relic_counters.letter_opener_skills_this_turn, 0);
        assert_eq!(combat.relic_counters.nunchaku_attacks_played, 9);
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
