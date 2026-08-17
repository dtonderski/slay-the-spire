use crate::action::InternalAction;
use crate::card::CardType;
use crate::combat::CombatState;
use crate::content::cards::upgrade_card_instance;
use crate::rng::{JavaRng, StsRng};
use crate::{SimError, SimResult};
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
/// Max HP granted by [Relic::TinyHouse] on pickup.
pub const TINY_HOUSE_MAX_HP: i32 = 5;
/// Gold granted by [Relic::TinyHouse] on pickup.
pub const TINY_HOUSE_GOLD: i32 = 50;
/// Card reward screens granted by [Relic::Orrery] on pickup.
pub const ORRERY_CARD_REWARDS: u8 = 5;
/// New reward items constructed by target Orrery.onEquip; the fifth visible
/// entry is already present on the room reward list.
pub const ORRERY_EAGER_CARD_REWARDS: u8 = 5;
/// Extra cards drawn each hand by [Relic::SneckoEye].
pub const SNECKO_EYE_DRAW: usize = 2;
/// Map jumps granted by [Relic::WingBoots] on pickup.
pub const WING_BOOTS_CHARGES: u8 = 3;
/// Chests that can grant an extra relic while [Relic::Matryoshka] is owned.
pub const MATRYOSHKA_MAX_CHESTS: u32 = 2;
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
/// Artifact granted by [Relic::ClockworkSouvenir] at combat start.
pub const CLOCKWORK_SOUVENIR_ARTIFACT: i32 = 1;
/// Temporary Strength granted by [Relic::MutagenicStrength] at combat start.
pub const MUTAGENIC_STRENGTH_AMOUNT: i32 = 3;
/// Strength granted by [Relic::RedSkull] while at or below half HP.
pub const RED_SKULL_STRENGTH: i32 = 3;
/// HP restored by [Relic::BloodyIdol] whenever gold is gained.
pub const BLOODY_IDOL_HEAL: i32 = 5;
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
/// Vigor granted by [Relic::Akabeko] at combat start (next Attack +N damage).
pub const AKABEKO_DAMAGE: i32 = 8;
/// Cards drawn after first HP loss each combat by [Relic::CentennialPuzzle].
pub const CENTENNIAL_PUZZLE_DRAW: usize = 3;
/// Cards drawn after each HP loss by [Relic::RunicCube].
pub const RUNIC_CUBE_DRAW: usize = 1;
/// Block granted by [Relic::TheAbacus] whenever the discard pile is shuffled into the draw pile.
pub const THE_ABACUS_BLOCK: i32 = 6;
/// Shuffles before [Relic::Sundial] grants energy.
pub const SUNDIAL_THRESHOLD: u32 = 3;
/// Energy granted by [Relic::Sundial] every third shuffle.
pub const SUNDIAL_ENERGY: i32 = 2;
/// Block granted by [Relic::SelfFormingClay] after HP loss.
pub const SELF_FORMING_CLAY_BLOCK: i32 = 3;
/// Temporary Wounds shuffled into the draw pile by [Relic::MarkOfPain] at battle start.
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
/// Attacks before [Relic::PenNib] doubles the next attack card's damage.
pub const PEN_NIB_THRESHOLD: u32 = 10;
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
/// Turns in the stable [Relic::IncenseBurner] counter cycle.
pub const INCENSE_BURNER_THRESHOLD: u32 = 6;
/// Rooms in the stable [Relic::TinyChest] counter cycle.
pub const TINY_CHEST_THRESHOLD: u32 = 4;
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
/// Gold granted by [Relic::SsserpentHead] when entering an event (`?`) room.
pub const SSSERPENT_HEAD_GOLD: i32 = 50;
/// Energy granted by [Relic::AncientTeaSet] in the next combat after entering a rest site.
pub const ANCIENT_TEA_SET_ENERGY: i32 = 2;
/// Block lost at turn transition with [Relic::Calipers] instead of losing all block.
pub const CALIPERS_BLOCK_LOSS: i32 = 15;
/// Max HP granted by [Relic::DarkstonePeriapt] whenever a curse is obtained.
pub const DARKSTONE_PERIAPT_MAX_HP: i32 = 6;
/// Strength granted by [Relic::DuVuDoll] per curse in the deck at combat start.
pub const DU_VU_DOLL_STRENGTH_PER_CURSE: i32 = 1;
/// Maximum Strength lifts stored by [Relic::Girya].
pub const GIRYA_MAX_LIFTS: u32 = 3;

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
/// Content id for [Relic::Whetstone].
pub const WHETSTONE_ID: ContentId = ContentId::new(377);
/// Content id for [Relic::WarPaint].
pub const WAR_PAINT_ID: ContentId = ContentId::new(378);
/// Content id for [Relic::Akabeko].
pub const AKABEKO_ID: ContentId = ContentId::new(379);
/// Content id for [Relic::CentennialPuzzle].
pub const CENTENNIAL_PUZZLE_ID: ContentId = ContentId::new(380);
/// Content id for [Relic::PenNib].
pub const PEN_NIB_ID: ContentId = ContentId::new(381);
/// Content id for [Relic::SelfFormingClay].
pub const SELF_FORMING_CLAY_ID: ContentId = ContentId::new(382);
/// Content id for [Relic::ClockworkSouvenir].
pub const CLOCKWORK_SOUVENIR_ID: ContentId = ContentId::new(383);
/// Content id for [Relic::RunicCube].
pub const RUNIC_CUBE_ID: ContentId = ContentId::new(384);
/// Content id for [Relic::TheAbacus].
pub const THE_ABACUS_ID: ContentId = ContentId::new(385);
/// Content id for [Relic::GremlinHorn].
pub const GREMLIN_HORN_ID: ContentId = ContentId::new(386);
/// Energy granted by [Relic::GremlinHorn] when a monster dies.
pub const GREMLIN_HORN_ENERGY: i32 = 1;
/// Cards drawn by [Relic::GremlinHorn] when a monster dies.
pub const GREMLIN_HORN_DRAW: usize = 1;
/// Content id for [Relic::Sundial].
pub const SUNDIAL_ID: ContentId = ContentId::new(387);
/// Content id for [Relic::CharonsAshes].
pub const CHARONS_ASHES_ID: ContentId = ContentId::new(388);
/// Damage dealt to all enemies by [Relic::CharonsAshes] when a card is exhausted.
pub const CHARONS_ASHES_DAMAGE: i32 = 3;
/// Content id for [Relic::BlueCandle].
pub const BLUE_CANDLE_ID: ContentId = ContentId::new(389);
/// HP lost when [Relic::BlueCandle] exhausts a Curse.
pub const BLUE_CANDLE_HP_LOSS: i32 = 1;
/// Content id for [Relic::MedicalKit].
pub const MEDICAL_KIT_ID: ContentId = ContentId::new(390);
/// Content id for [Relic::LizardTail].
pub const LIZARD_TAIL_ID: ContentId = ContentId::new(391);
/// Percent of max HP restored by [Relic::LizardTail] on lethal damage.
pub const LIZARD_TAIL_HEAL_PERCENT: i32 = 50;
/// Content id for [Relic::Pocketwatch].
pub const POCKETWATCH_ID: ContentId = ContentId::new(392);
/// Cards drawn by [Relic::Pocketwatch] after a turn with three or fewer cards played.
pub const POCKETWATCH_DRAW: usize = 3;
/// Maximum previous-turn card plays that trigger [Relic::Pocketwatch].
pub const POCKETWATCH_CARD_LIMIT: u32 = 3;
/// Content id for [Relic::HandDrill].
pub const HAND_DRILL_ID: ContentId = ContentId::new(393);
/// Vulnerable applied by [Relic::HandDrill] when an attack breaks monster block.
pub const HAND_DRILL_VULNERABLE: i32 = 2;
/// Content id for [Relic::BurningBlood].
pub const BURNING_BLOOD_ID: ContentId = ContentId::new(394);
/// Content id for [Relic::Circlet].
pub const CIRCLET_ID: ContentId = ContentId::new(395);
/// Content id for [Relic::RedCirclet].
pub const RED_CIRCLET_ID: ContentId = ContentId::new(396);
/// Content id for [Relic::RedMask].
pub const RED_MASK_ID: ContentId = ContentId::new(450);
/// Content id for [Relic::CultistMask].
pub const CULTIST_MASK_ID: ContentId = ContentId::new(451);
/// Content id for [Relic::FaceOfCleric].
pub const FACE_OF_CLERIC_ID: ContentId = ContentId::new(452);
/// Content id for [Relic::GremlinMask].
pub const GREMLIN_MASK_ID: ContentId = ContentId::new(453);
/// Content id for [Relic::NlothsMask].
pub const NLOTHS_MASK_ID: ContentId = ContentId::new(454);
/// Content id for [Relic::SsserpentHead].
pub const SSSERPENT_HEAD_ID: ContentId = ContentId::new(455);
/// Content id for [Relic::SacredBark].
pub const SACRED_BARK_ID: ContentId = ContentId::new(397);
/// Content id for [Relic::RunicPyramid].
pub const RUNIC_PYRAMID_ID: ContentId = ContentId::new(398);
/// Content id for [Relic::FrozenEye].
pub const FROZEN_EYE_ID: ContentId = ContentId::new(399);
/// Content id for [Relic::PeacePipe].
pub const PEACE_PIPE_ID: ContentId = ContentId::new(400);
/// Content id for [Relic::OrangePellets].
pub const ORANGE_PELLETS_ID: ContentId = ContentId::new(401);
/// Content id for [Relic::Girya].
pub const GIRYA_ID: ContentId = ContentId::new(402);
/// Content id for [Relic::UnceasingTop].
pub const UNCEASING_TOP_ID: ContentId = ContentId::new(403);
/// Cards drawn by [Relic::UnceasingTop] when the player's hand becomes empty.
pub const UNCEASING_TOP_DRAW: usize = 1;
/// Content id for [Relic::Shovel].
pub const SHOVEL_ID: ContentId = ContentId::new(404);
/// Content id for [Relic::FossilizedHelix].
pub const FOSSILIZED_HELIX_ID: ContentId = ContentId::new(405);
/// Buffer granted by [Relic::FossilizedHelix] at combat start.
pub const FOSSILIZED_HELIX_BUFFER: i32 = 1;
/// Content id for [Relic::BlackStar].
pub const BLACK_STAR_ID: ContentId = ContentId::new(406);
/// Content id for [Relic::Matryoshka].
pub const MATRYOSHKA_ID: ContentId = ContentId::new(407);
/// Content id for [Relic::EmptyCage].
pub const EMPTY_CAGE_ID: ContentId = ContentId::new(408);
/// Content id for [Relic::BottledFlame].
pub const BOTTLED_FLAME_ID: ContentId = ContentId::new(409);
/// Content id for [Relic::BottledLightning].
pub const BOTTLED_LIGHTNING_ID: ContentId = ContentId::new(410);
/// Content id for [Relic::BottledTornado].
pub const BOTTLED_TORNADO_ID: ContentId = ContentId::new(411);
/// Content id for [Relic::DollysMirror].
pub const DOLLYS_MIRROR_ID: ContentId = ContentId::new(412);
/// Content id for [Relic::PrayerWheel].
pub const PRAYER_WHEEL_ID: ContentId = ContentId::new(413);
/// Content id for [Relic::CrackedCore].
pub const CRACKED_CORE_ID: ContentId = ContentId::new(414);
/// Content id for [Relic::FrozenCore].
pub const FROZEN_CORE_ID: ContentId = ContentId::new(415);
/// Content id for [Relic::PureWater].
pub const PURE_WATER_ID: ContentId = ContentId::new(416);
/// Content id for [Relic::HolyWater].
pub const HOLY_WATER_ID: ContentId = ContentId::new(417);
/// Content id for [Relic::RingOfTheSnake].
pub const RING_OF_THE_SNAKE_ID: ContentId = ContentId::new(418);
/// Content id for [Relic::RingOfTheSerpent].
pub const RING_OF_THE_SERPENT_ID: ContentId = ContentId::new(419);
/// Content id for [Relic::Cauldron].
pub const CAULDRON_ID: ContentId = ContentId::new(420);
/// Random potion rolls granted by [Relic::Cauldron] on pickup.
pub const CAULDRON_POTIONS: usize = 5;
/// Content id for [Relic::TinyHouse].
pub const TINY_HOUSE_ID: ContentId = ContentId::new(421);
/// Content id for [Relic::DeadBranch].
pub const DEAD_BRANCH_ID: ContentId = ContentId::new(422);
/// Content id for [Relic::MummifiedHand].
pub const MUMMIFIED_HAND_ID: ContentId = ContentId::new(423);
/// Content id for [Relic::TheCourier].
pub const THE_COURIER_ID: ContentId = ContentId::new(424);
/// Content id for [Relic::IncenseBurner].
pub const INCENSE_BURNER_ID: ContentId = ContentId::new(425);
/// Content id for [Relic::CursedKey].
pub const CURSED_KEY_ID: ContentId = ContentId::new(426);
/// Content id for [Relic::TinyChest].
pub const TINY_CHEST_ID: ContentId = ContentId::new(427);
/// Content id for [Relic::Orrery].
pub const ORRERY_ID: ContentId = ContentId::new(428);
/// Content id for [Relic::SneckoEye].
pub const SNECKO_EYE_ID: ContentId = ContentId::new(429);
/// Content id for [Relic::StrangeSpoon].
pub const STRANGE_SPOON_ID: ContentId = ContentId::new(430);
/// Content id for [Relic::WingBoots].
pub const WING_BOOTS_ID: ContentId = ContentId::new(431);
/// Content id for [Relic::CallingBell].
pub const CALLING_BELL_ID: ContentId = ContentId::new(432);
/// Content id for [Relic::PandorasBox].
pub const PANDORAS_BOX_ID: ContentId = ContentId::new(433);
/// Content id for [Relic::Astrolabe].
pub const ASTROLABE_ID: ContentId = ContentId::new(434);
/// Content id for [Relic::GamblingChip].
pub const GAMBLING_CHIP_ID: ContentId = ContentId::new(435);
/// Content id for [Relic::Toolbox].
pub const TOOLBOX_ID: ContentId = ContentId::new(436);
/// Content id for [Relic::JuzuBracelet].
pub const JUZU_BRACELET_ID: ContentId = ContentId::new(437);
/// Content id for [Relic::PrismaticShard].
pub const PRISMATIC_SHARD_ID: ContentId = ContentId::new(438);
/// Content id for [Relic::MutagenicStrength].
pub const MUTAGENIC_STRENGTH_ID: ContentId = ContentId::new(439);
/// Content id for [Relic::WarpedTongs].
pub const WARPED_TONGS_ID: ContentId = ContentId::new(440);
/// Content id for [Relic::GoldenIdol].
pub const GOLDEN_IDOL_ID: ContentId = ContentId::new(441);
/// Content id for [Relic::BloodyIdol].
pub const BLOODY_IDOL_ID: ContentId = ContentId::new(442);
/// Content id for [Relic::Necronomicon].
pub const NECRONOMICON_ID: ContentId = ContentId::new(443);
/// Content id for [Relic::Enchiridion].
pub const ENCHIRIDION_ID: ContentId = ContentId::new(444);
/// Content id for [Relic::NilrysCodex].
pub const NILRYS_CODEX_ID: ContentId = ContentId::new(445);
/// Content id for [Relic::MarkOfBloom].
pub const MARK_OF_BLOOM_ID: ContentId = ContentId::new(446);
/// Content id for [Relic::SpiritPoop].
pub const SPIRIT_POOP_ID: ContentId = ContentId::new(447);
/// Content id for [Relic::OddMushroom].
pub const ODD_MUSHROOM_ID: ContentId = ContentId::new(448);
/// Content id for [Relic::NlothsGift].
pub const NLOTHS_GIFT_ID: ContentId = ContentId::new(449);
/// Content id for [Relic::NeowsLament].
pub const NEOWS_LAMENT_ID: ContentId = ContentId::new(456);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RelicCounters {
    #[serde(default, skip_serializing_if = "is_false")]
    pub lizard_tail_available: bool,
    /// Passive Fairy in a Bottle healing available during the current action.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub fairy_heal_percent: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fairy_consumed: bool,
    #[serde(default)]
    pub ink_bottle_cards_played: u32,
    #[serde(default)]
    pub ornamental_fan_attacks_this_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub nunchaku_attacks_played: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub pen_nib_attacks_played: u32,
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
    pub cards_played_last_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attacks_played_this_combat: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub centennial_puzzle_triggers: u32,
    /// Centennial Puzzle draw deferred across a multi-hit attack (addToBot).
    #[serde(default, skip_serializing_if = "is_false")]
    pub deferred_centennial_puzzle_draw: bool,
    /// Runic Cube draws deferred across a multi-hit attack (one per unblocked hit).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub deferred_runic_cube_draws: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attacks_played_last_turn: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub player_turns_started: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub happy_flower_turns: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub sundial_shuffles: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub orange_pellets_attack_played: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub orange_pellets_skill_played: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub orange_pellets_power_played: bool,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub incense_burner_counter: u32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub self_forming_clay_next_turn_block: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub red_skull_active: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub necronomicon_used_this_turn: bool,
}

impl RelicCounters {
    /// Whether a stable combat snapshot contains a counter value that target
    /// relic code would already have consumed and reset.
    #[must_use]
    pub(crate) fn has_out_of_bounds_stable_counter(&self) -> bool {
        [
            self.ink_bottle_cards_played,
            self.ornamental_fan_attacks_this_turn,
            self.nunchaku_attacks_played,
            self.pen_nib_attacks_played,
            self.shuriken_attacks_this_turn,
            self.kunai_attacks_this_turn,
            self.letter_opener_skills_this_turn,
            self.cards_played_this_turn,
            self.attacks_played_this_turn,
            self.cards_played_last_turn,
            self.attacks_played_this_combat,
            self.centennial_puzzle_triggers,
            self.attacks_played_last_turn,
            self.player_turns_started,
            self.happy_flower_turns,
            self.sundial_shuffles,
            self.incense_burner_counter,
        ]
        .into_iter()
        .any(|counter| counter > i32::MAX as u32)
            || self.ink_bottle_cards_played >= INK_BOTTLE_THRESHOLD
            || self.ornamental_fan_attacks_this_turn >= ORNAMENTAL_FAN_THRESHOLD
            || self.nunchaku_attacks_played >= NUNCHAKU_THRESHOLD
            || self.pen_nib_attacks_played >= PEN_NIB_THRESHOLD
            || self.shuriken_attacks_this_turn >= SHURIKEN_THRESHOLD
            || self.kunai_attacks_this_turn >= KUNAI_THRESHOLD
            || self.letter_opener_skills_this_turn >= LETTER_OPENER_THRESHOLD
            || self.centennial_puzzle_triggers > 1
            || self.happy_flower_turns >= HAPPY_FLOWER_THRESHOLD
            || self.incense_burner_counter >= INCENSE_BURNER_THRESHOLD
    }
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
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
pub enum RelicEffectStatus {
    /// The simulator models the relic's relevant behavior end to end.
    Modeled,
    /// Some behavior is modeled, but full fidelity has not been established.
    Partial,
    /// The identity is tracked, but its gameplay effect is unsupported.
    Unsupported,
    /// The relic is identity- or score-only for the simulator.
    IdentityOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelicDefinition {
    pub relic: Relic,
    pub content_id: ContentId,
    pub tier: Option<RelicTier>,
    pub trace_name: &'static str,
    pub aliases: &'static [&'static str],
    pub effect_status: RelicEffectStatus,
}

/// Source-compatible name for the former duplicate relic identity enum.
pub use Relic as RelicKey;

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
            RelicTier::Common => pop_relic(&mut self.common, tier, from_front),
            RelicTier::Uncommon if self.uncommon.is_empty() => {
                return self.return_random_relic_from(RelicTier::Rare, context, true);
            }
            RelicTier::Uncommon => pop_relic(&mut self.uncommon, tier, from_front),
            RelicTier::Rare if self.rare.is_empty() => RelicKey::Circlet,
            RelicTier::Rare => pop_relic(&mut self.rare, tier, from_front),
            RelicTier::Shop if self.shop.is_empty() => {
                return self.return_random_relic_from(RelicTier::Uncommon, context, true);
            }
            RelicTier::Shop => pop_relic(&mut self.shop, tier, from_front),
            RelicTier::Boss if self.boss.is_empty() => RelicKey::RedCirclet,
            RelicTier::Boss => pop_relic(&mut self.boss, tier, from_front),
        };

        if relic_can_spawn(relic, context) {
            relic
        } else {
            // STS AbstractDungeon.returnRandomRelicKey on !canSpawn calls
            // returnEndRandomRelicKey (front → end). returnEndRandomRelicKey on
            // !canSpawn calls itself (end → end). Both retries therefore pop from
            // the end; never preserve a front-only retry loop.
            self.return_random_relic_from(tier, context, false)
        }
    }
}

fn pop_relic(pool: &mut Vec<RelicKey>, tier: RelicTier, from_front: bool) -> RelicKey {
    if from_front || tier == RelicTier::Boss {
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
    use Relic::{
        AncientTeaSet, BlackBlood, BottledFlame, BottledLightning, BottledTornado, BurningBlood,
        CeramicFish, CrackedCore, DarkstonePeriapt, DreamCatcher, Ectoplasm, FrozenCore, FrozenEgg,
        Girya, HolyWater, JuzuBracelet, MawBank, MealTicket, MeatOnTheBone, MoltenEgg, OldCoin,
        Omamori, PeacePipe, PotionBelt, PrayerWheel, PreservedInsect, PureWater, QuestionCard,
        RegalPillow, RingOfTheSerpent, RingOfTheSnake, Shovel, SingingBowl, SmilingMask,
        TheCourier, TinyChest, ToxicEgg, WingBoots,
    };

    match relic {
        BottledFlame => context.has_non_basic_attack,
        BottledLightning => context.has_non_basic_skill,
        BottledTornado => context.has_power,
        BlackBlood => context.owned_relics.contains(&BurningBlood),
        FrozenCore => context.owned_relics.contains(&CrackedCore),
        BurningBlood => context.owned_relics.contains(&BurningBlood),
        Ectoplasm => context.floor_num <= 17,
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
        | PrayerWheel | QuestionCard | RegalPillow | SingingBowl | ToxicEgg => {
            context.floor_num <= 48
        }
        // Vanilla `Courier.canSpawn` rejects shop rooms even though the relic
        // remains eligible for ordinary rewards through floor 48. The target
        // `returnRandomRelicEnd` consumes this rejected tail offer before
        // returning the next eligible relic.
        TheCourier => context.floor_num <= 48 && !context.shop_room,
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
    BurningBlood,
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
    Whetstone,
    WarPaint,
    Akabeko,
    CentennialPuzzle,
    PenNib,
    SelfFormingClay,
    ClockworkSouvenir,
    RunicCube,
    TheAbacus,
    GremlinHorn,
    Sundial,
    CharonsAshes,
    BlueCandle,
    MedicalKit,
    LizardTail,
    Pocketwatch,
    HandDrill,
    RedMask,
    Circlet,
    RedCirclet,
    CultistMask,
    FaceOfCleric,
    GremlinMask,
    NlothsMask,
    SsserpentHead,
    SacredBark,
    RunicPyramid,
    FrozenEye,
    PeacePipe,
    OrangePellets,
    Girya,
    UnceasingTop,
    Shovel,
    FossilizedHelix,
    BlackStar,
    Matryoshka,
    EmptyCage,
    BottledFlame,
    BottledLightning,
    BottledTornado,
    DollysMirror,
    PrayerWheel,
    CrackedCore,
    FrozenCore,
    PureWater,
    HolyWater,
    RingOfTheSnake,
    RingOfTheSerpent,
    Cauldron,
    TinyHouse,
    DeadBranch,
    MummifiedHand,
    TheCourier,
    IncenseBurner,
    CursedKey,
    TinyChest,
    Orrery,
    SneckoEye,
    StrangeSpoon,
    WingBoots,
    CallingBell,
    PandorasBox,
    Astrolabe,
    GamblingChip,
    Toolbox,
    JuzuBracelet,
    PrismaticShard,
    MutagenicStrength,
    WarpedTongs,
    GoldenIdol,
    BloodyIdol,
    Necronomicon,
    Enchiridion,
    NilrysCodex,
    MarkOfBloom,
    SpiritPoop,
    OddMushroom,
    NlothsGift,
    NeowsLament,
}

pub const ALL_RELICS: &[Relic] = &[
    Relic::BurningBlood,
    Relic::BloodVial,
    Relic::Vajra,
    Relic::OddlySmoothStone,
    Relic::Strawberry,
    Relic::Pear,
    Relic::Mango,
    Relic::OldCoin,
    Relic::LeesWaffle,
    Relic::PotionBelt,
    Relic::Lantern,
    Relic::BagOfPreparation,
    Relic::BagOfMarbles,
    Relic::BronzeScales,
    Relic::ThreadAndNeedle,
    Relic::RedSkull,
    Relic::Nunchaku,
    Relic::ArtOfWar,
    Relic::Shuriken,
    Relic::Kunai,
    Relic::LetterOpener,
    Relic::HappyFlower,
    Relic::Orichalcum,
    Relic::HornCleat,
    Relic::CaptainsWheel,
    Relic::MercuryHourglass,
    Relic::StoneCalendar,
    Relic::MeatOnTheBone,
    Relic::QuestionCard,
    Relic::BlackBlood,
    Relic::MealTicket,
    Relic::RegalPillow,
    Relic::DreamCatcher,
    Relic::EternalFeather,
    Relic::Torii,
    Relic::TungstenRod,
    Relic::CeramicFish,
    Relic::MembershipCard,
    Relic::SmilingMask,
    Relic::Pantograph,
    Relic::Ginger,
    Relic::Turnip,
    Relic::MarkOfPain,
    Relic::MagicFlower,
    Relic::PaperPhrog,
    Relic::ChampionBelt,
    Relic::PreservedInsect,
    Relic::Omamori,
    Relic::SlingOfCourage,
    Relic::MawBank,
    Relic::AncientTeaSet,
    Relic::Calipers,
    Relic::SingingBowl,
    Relic::DarkstonePeriapt,
    Relic::DuVuDoll,
    Relic::FusionHammer,
    Relic::Sozu,
    Relic::BustedCrown,
    Relic::VelvetChoker,
    Relic::ToyOrnithopter,
    Relic::MoltenEgg,
    Relic::ToxicEgg,
    Relic::FrozenEgg,
    Relic::TheBoot,
    Relic::BirdFacedUrn,
    Relic::CoffeeDripper,
    Relic::Anchor,
    Relic::InkBottle,
    Relic::OrnamentalFan,
    Relic::IceCream,
    Relic::ChemicalX,
    Relic::PhilosophersStone,
    Relic::SlaversCollar,
    Relic::Ectoplasm,
    Relic::RunicDome,
    Relic::StrikeDummy,
    Relic::Brimstone,
    Relic::WhiteBeastStatue,
    Relic::Whetstone,
    Relic::WarPaint,
    Relic::Akabeko,
    Relic::CentennialPuzzle,
    Relic::PenNib,
    Relic::SelfFormingClay,
    Relic::ClockworkSouvenir,
    Relic::RunicCube,
    Relic::TheAbacus,
    Relic::GremlinHorn,
    Relic::Sundial,
    Relic::CharonsAshes,
    Relic::BlueCandle,
    Relic::MedicalKit,
    Relic::LizardTail,
    Relic::Pocketwatch,
    Relic::HandDrill,
    Relic::RedMask,
    Relic::Circlet,
    Relic::RedCirclet,
    Relic::CultistMask,
    Relic::FaceOfCleric,
    Relic::GremlinMask,
    Relic::NlothsMask,
    Relic::SsserpentHead,
    Relic::SacredBark,
    Relic::RunicPyramid,
    Relic::FrozenEye,
    Relic::PeacePipe,
    Relic::OrangePellets,
    Relic::Girya,
    Relic::UnceasingTop,
    Relic::Shovel,
    Relic::FossilizedHelix,
    Relic::BlackStar,
    Relic::Matryoshka,
    Relic::EmptyCage,
    Relic::BottledFlame,
    Relic::BottledLightning,
    Relic::BottledTornado,
    Relic::DollysMirror,
    Relic::PrayerWheel,
    Relic::CrackedCore,
    Relic::FrozenCore,
    Relic::PureWater,
    Relic::HolyWater,
    Relic::RingOfTheSnake,
    Relic::RingOfTheSerpent,
    Relic::Cauldron,
    Relic::TinyHouse,
    Relic::DeadBranch,
    Relic::MummifiedHand,
    Relic::TheCourier,
    Relic::IncenseBurner,
    Relic::CursedKey,
    Relic::TinyChest,
    Relic::Orrery,
    Relic::SneckoEye,
    Relic::StrangeSpoon,
    Relic::WingBoots,
    Relic::CallingBell,
    Relic::PandorasBox,
    Relic::Astrolabe,
    Relic::GamblingChip,
    Relic::Toolbox,
    Relic::JuzuBracelet,
    Relic::PrismaticShard,
    Relic::MutagenicStrength,
    Relic::WarpedTongs,
    Relic::GoldenIdol,
    Relic::BloodyIdol,
    Relic::Necronomicon,
    Relic::Enchiridion,
    Relic::NilrysCodex,
    Relic::MarkOfBloom,
    Relic::SpiritPoop,
    Relic::OddMushroom,
    Relic::NlothsGift,
    Relic::NeowsLament,
];

fn normalize_relic_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

impl Relic {
    #[must_use]
    pub fn definition(self) -> RelicDefinition {
        RelicDefinition {
            relic: self,
            content_id: self.content_id(),
            tier: self.tier(),
            trace_name: self.trace_name(),
            aliases: self.aliases(),
            effect_status: self.effect_status(),
        }
    }

    #[must_use]
    pub fn tier(self) -> Option<RelicTier> {
        if IRONCLAD_COMMON_RELIC_POOL.contains(&self) {
            Some(RelicTier::Common)
        } else if IRONCLAD_UNCOMMON_RELIC_POOL.contains(&self) {
            Some(RelicTier::Uncommon)
        } else if IRONCLAD_RARE_RELIC_POOL.contains(&self) {
            Some(RelicTier::Rare)
        } else if IRONCLAD_SHOP_RELIC_POOL.contains(&self) {
            Some(RelicTier::Shop)
        } else if IRONCLAD_BOSS_RELIC_POOL.contains(&self) {
            Some(RelicTier::Boss)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn effect_status(self) -> RelicEffectStatus {
        match self {
            Relic::MarkOfBloom | Relic::NeowsLament => RelicEffectStatus::Modeled,
            Relic::Circlet | Relic::RedCirclet | Relic::CultistMask | Relic::SpiritPoop => {
                RelicEffectStatus::IdentityOnly
            }
            Relic::CrackedCore
            | Relic::FrozenCore
            | Relic::PureWater
            | Relic::HolyWater
            | Relic::RingOfTheSnake
            | Relic::RingOfTheSerpent
            | Relic::FaceOfCleric
            | Relic::GremlinMask
            | Relic::NlothsMask
            | Relic::OddMushroom
            | Relic::NlothsGift => RelicEffectStatus::Unsupported,
            _ => RelicEffectStatus::Partial,
        }
    }

    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Relic::MarkOfBloom => &["Mark of Bloom"],
            Relic::MoltenEgg => &["Molten Egg 2"],
            Relic::FrozenEgg => &["Frozen Egg 2"],
            Relic::CultistMask => &["Cultist Mask"],
            Relic::FaceOfCleric => &["Cleric Face", "Face of Cleric"],
            Relic::GremlinMask => &["Gremlin Visage", "Gremlin Mask"],
            Relic::NlothsMask => &["N'loth's Hungry Face"],
            _ => &[],
        }
    }

    #[must_use]
    pub fn from_trace_name(name: &str) -> Option<Self> {
        let normalized = normalize_relic_name(name);
        ALL_RELICS.iter().copied().find(|relic| {
            normalize_relic_name(relic.trace_name()) == normalized
                || relic
                    .aliases()
                    .iter()
                    .any(|alias| normalize_relic_name(alias) == normalized)
        })
    }

    #[must_use]
    pub const fn trace_name(self) -> &'static str {
        match self {
            Relic::Akabeko => "Akabeko",
            Relic::CrackedCore => "Cracked Core",
            Relic::RingOfTheSnake => "Ring of the Snake",
            Relic::PureWater => "Pure Water",
            Relic::Vajra => "Vajra",
            Relic::BottledTornado => "Bottled Tornado",
            Relic::Sundial => "Sundial",
            Relic::TheCourier => "The Courier",
            Relic::OrnamentalFan => "Ornamental Fan",
            Relic::HornCleat => "Horn Cleat",
            Relic::BottledFlame => "Bottled Flame",
            Relic::DarkstonePeriapt => "Darkstone Periapt",
            Relic::MercuryHourglass => "Mercury Hourglass",
            Relic::OldCoin => "Old Coin",
            Relic::Shovel => "Shovel",
            Relic::Turnip => "Turnip",
            Relic::FrozenCore => "Frozen Core",
            Relic::RingOfTheSerpent => "Ring of the Serpent",
            Relic::HolyWater => "Holy Water",
            Relic::HandDrill => "Hand Drill",
            Relic::LeesWaffle => "Lee's Waffle",
            Relic::FrozenEye => "Frozen Eye",
            Relic::TheAbacus => "The Abacus",
            Relic::Necronomicon => "Necronomicon",
            Relic::Enchiridion => "Enchiridion",
            Relic::NilrysCodex => "Nilry's Codex",
            Relic::MutagenicStrength => "Mutagenic Strength",
            Relic::BloodyIdol => "Bloody Idol",
            Relic::MarkOfBloom => "Mark of the Bloom",
            Relic::SpiritPoop => "Spirit Poop",
            Relic::OddMushroom => "Odd Mushroom",
            Relic::NlothsGift => "N'loth's Gift",
            Relic::NeowsLament => "Neow's Lament",
            Relic::Circlet => "Circlet",
            Relic::RedCirclet => "Red Circlet",
            Relic::Anchor => "Anchor",
            Relic::TheBoot => "The Boot",
            Relic::TinyChest => "Tiny Chest",
            Relic::BagOfMarbles => "Bag of Marbles",
            Relic::BagOfPreparation => "Bag of Preparation",
            Relic::BurningBlood => "Burning Blood",
            Relic::BloodVial => "Blood Vial",
            Relic::RedSkull => "Red Skull",
            Relic::DreamCatcher => "Dream Catcher",
            Relic::Torii => "Torii",
            Relic::MoltenEgg => "Molten Egg",
            Relic::ToxicEgg => "Toxic Egg",
            Relic::FrozenEgg => "Frozen Egg",
            Relic::MummifiedHand => "Mummified Hand",
            Relic::CharonsAshes => "Charon's Ashes",
            Relic::CeramicFish => "Ceramic Fish",
            Relic::GamblingChip => "Gambling Chip",
            Relic::PenNib => "Pen Nib",
            Relic::MembershipCard => "Membership Card",
            Relic::Pantograph => "Pantograph",
            Relic::StrikeDummy => "Strike Dummy",
            Relic::WhiteBeastStatue => "White Beast Statue",
            Relic::SmilingMask => "Smiling Mask",
            Relic::Whetstone => "Whetstone",
            Relic::Orichalcum => "Orichalcum",
            Relic::BronzeScales => "Bronze Scales",
            Relic::Ginger => "Ginger",
            Relic::Strawberry => "Strawberry",
            Relic::TungstenRod => "Tungsten Rod",
            Relic::MagicFlower => "Magic Flower",
            Relic::ToyOrnithopter => "Toy Ornithopter",
            Relic::BirdFacedUrn => "Bird-Faced Urn",
            Relic::UnceasingTop => "Unceasing Top",
            Relic::Toolbox => "Toolbox",
            Relic::PotionBelt => "Potion Belt",
            Relic::RegalPillow => "Regal Pillow",
            Relic::Mango => "Mango",
            Relic::GremlinHorn => "Gremlin Horn",
            Relic::JuzuBracelet => "Juzu Bracelet",
            Relic::MawBank => "Maw Bank",
            Relic::Omamori => "Omamori",
            Relic::Lantern => "Lantern",
            Relic::AncientTeaSet => "Ancient Tea Set",
            Relic::Pocketwatch => "Pocketwatch",
            Relic::CentennialPuzzle => "Centennial Puzzle",
            Relic::OddlySmoothStone => "Oddly Smooth Stone",
            Relic::MeatOnTheBone => "Meat on the Bone",
            Relic::ClockworkSouvenir => "Clockwork Souvenir",
            Relic::Orrery => "Orrery",
            Relic::StoneCalendar => "Stone Calendar",
            Relic::IceCream => "Ice Cream",
            Relic::ChemicalX => "Chemical X",
            Relic::Calipers => "Calipers",
            Relic::QuestionCard => "Question Card",
            Relic::SingingBowl => "Singing Bowl",
            Relic::CursedKey => "Cursed Key",
            Relic::FusionHammer => "Fusion Hammer",
            Relic::VelvetChoker => "Velvet Choker",
            Relic::RunicDome => "Runic Dome",
            Relic::SlaversCollar => "Slaver's Collar",
            Relic::SneckoEye => "Snecko Eye",
            Relic::PandorasBox => "Pandora's Box",
            Relic::BustedCrown => "Busted Crown",
            Relic::Ectoplasm => "Ectoplasm",
            Relic::TinyHouse => "Tiny House",
            Relic::Cauldron => "Cauldron",
            Relic::Sozu => "Sozu",
            Relic::PhilosophersStone => "Philosopher's Stone",
            Relic::Astrolabe => "Astrolabe",
            Relic::BlackStar => "Black Star",
            Relic::SacredBark => "Sacred Bark",
            Relic::EmptyCage => "Empty Cage",
            Relic::RunicPyramid => "Runic Pyramid",
            Relic::CallingBell => "Calling Bell",
            Relic::CoffeeDripper => "Coffee Dripper",
            Relic::BlackBlood => "Black Blood",
            Relic::Brimstone => "Brimstone",
            Relic::RedMask => "Red Mask",
            Relic::EternalFeather => "Eternal Feather",
            Relic::Pear => "Pear",
            Relic::MarkOfPain => "Mark of Pain",
            Relic::RunicCube => "Runic Cube",
            Relic::DeadBranch => "Dead Branch",
            Relic::MealTicket => "Meal Ticket",
            Relic::PrismaticShard => "Prismatic Shard",
            Relic::ChampionBelt => "Champion Belt",
            Relic::GoldenIdol => "Golden Idol",
            Relic::DuVuDoll => "Du-Vu Doll",
            Relic::MedicalKit => "Medical Kit",
            Relic::WarPaint => "War Paint",
            Relic::LetterOpener => "Letter Opener",
            Relic::PreservedInsect => "Preserved Insect",
            Relic::SlingOfCourage => "Sling of Courage",
            Relic::ArtOfWar => "Art of War",
            Relic::PrayerWheel => "Prayer Wheel",
            Relic::CaptainsWheel => "Captain's Wheel",
            Relic::LizardTail => "Lizard Tail",
            Relic::Nunchaku => "Nunchaku",
            Relic::InkBottle => "Ink Bottle",
            Relic::Shuriken => "Shuriken",
            Relic::Kunai => "Kunai",
            Relic::HappyFlower => "Happy Flower",
            Relic::IncenseBurner => "Incense Burner",
            Relic::ThreadAndNeedle => "Thread and Needle",
            Relic::FossilizedHelix => "Fossilized Helix",
            Relic::PeacePipe => "Peace Pipe",
            Relic::PaperPhrog => "Paper Phrog",
            Relic::StrangeSpoon => "Strange Spoon",
            Relic::DollysMirror => "Dolly's Mirror",
            Relic::SelfFormingClay => "Self-Forming Clay",
            Relic::OrangePellets => "Orange Pellets",
            Relic::Matryoshka => "Matryoshka",
            Relic::BlueCandle => "Blue Candle",
            Relic::BottledLightning => "Bottled Lightning",
            Relic::WingBoots => "Wing Boots",
            Relic::CultistMask => "Cultist Headpiece",
            Relic::FaceOfCleric => "FaceOfCleric",
            Relic::GremlinMask => "GremlinMask",
            Relic::Girya => "Girya",
            Relic::NlothsMask => "NlothsMask",
            Relic::SsserpentHead => "Ssserpent Head",
            Relic::WarpedTongs => "Warped Tongs",
        }
    }

    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Relic::BurningBlood => BURNING_BLOOD_ID,
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
            Relic::Whetstone => WHETSTONE_ID,
            Relic::WarPaint => WAR_PAINT_ID,
            Relic::Akabeko => AKABEKO_ID,
            Relic::CentennialPuzzle => CENTENNIAL_PUZZLE_ID,
            Relic::PenNib => PEN_NIB_ID,
            Relic::SelfFormingClay => SELF_FORMING_CLAY_ID,
            Relic::ClockworkSouvenir => CLOCKWORK_SOUVENIR_ID,
            Relic::RunicCube => RUNIC_CUBE_ID,
            Relic::TheAbacus => THE_ABACUS_ID,
            Relic::GremlinHorn => GREMLIN_HORN_ID,
            Relic::Sundial => SUNDIAL_ID,
            Relic::CharonsAshes => CHARONS_ASHES_ID,
            Relic::BlueCandle => BLUE_CANDLE_ID,
            Relic::MedicalKit => MEDICAL_KIT_ID,
            Relic::LizardTail => LIZARD_TAIL_ID,
            Relic::Pocketwatch => POCKETWATCH_ID,
            Relic::HandDrill => HAND_DRILL_ID,
            Relic::RedMask => RED_MASK_ID,
            Relic::Circlet => CIRCLET_ID,
            Relic::RedCirclet => RED_CIRCLET_ID,
            Relic::CultistMask => CULTIST_MASK_ID,
            Relic::FaceOfCleric => FACE_OF_CLERIC_ID,
            Relic::GremlinMask => GREMLIN_MASK_ID,
            Relic::NlothsMask => NLOTHS_MASK_ID,
            Relic::SsserpentHead => SSSERPENT_HEAD_ID,
            Relic::SacredBark => SACRED_BARK_ID,
            Relic::RunicPyramid => RUNIC_PYRAMID_ID,
            Relic::FrozenEye => FROZEN_EYE_ID,
            Relic::PeacePipe => PEACE_PIPE_ID,
            Relic::OrangePellets => ORANGE_PELLETS_ID,
            Relic::Girya => GIRYA_ID,
            Relic::UnceasingTop => UNCEASING_TOP_ID,
            Relic::Shovel => SHOVEL_ID,
            Relic::FossilizedHelix => FOSSILIZED_HELIX_ID,
            Relic::BlackStar => BLACK_STAR_ID,
            Relic::Matryoshka => MATRYOSHKA_ID,
            Relic::EmptyCage => EMPTY_CAGE_ID,
            Relic::BottledFlame => BOTTLED_FLAME_ID,
            Relic::BottledLightning => BOTTLED_LIGHTNING_ID,
            Relic::BottledTornado => BOTTLED_TORNADO_ID,
            Relic::DollysMirror => DOLLYS_MIRROR_ID,
            Relic::PrayerWheel => PRAYER_WHEEL_ID,
            Relic::CrackedCore => CRACKED_CORE_ID,
            Relic::FrozenCore => FROZEN_CORE_ID,
            Relic::PureWater => PURE_WATER_ID,
            Relic::HolyWater => HOLY_WATER_ID,
            Relic::RingOfTheSnake => RING_OF_THE_SNAKE_ID,
            Relic::RingOfTheSerpent => RING_OF_THE_SERPENT_ID,
            Relic::Cauldron => CAULDRON_ID,
            Relic::TinyHouse => TINY_HOUSE_ID,
            Relic::DeadBranch => DEAD_BRANCH_ID,
            Relic::MummifiedHand => MUMMIFIED_HAND_ID,
            Relic::TheCourier => THE_COURIER_ID,
            Relic::IncenseBurner => INCENSE_BURNER_ID,
            Relic::CursedKey => CURSED_KEY_ID,
            Relic::TinyChest => TINY_CHEST_ID,
            Relic::Orrery => ORRERY_ID,
            Relic::SneckoEye => SNECKO_EYE_ID,
            Relic::StrangeSpoon => STRANGE_SPOON_ID,
            Relic::WingBoots => WING_BOOTS_ID,
            Relic::CallingBell => CALLING_BELL_ID,
            Relic::PandorasBox => PANDORAS_BOX_ID,
            Relic::Astrolabe => ASTROLABE_ID,
            Relic::GamblingChip => GAMBLING_CHIP_ID,
            Relic::Toolbox => TOOLBOX_ID,
            Relic::JuzuBracelet => JUZU_BRACELET_ID,
            Relic::PrismaticShard => PRISMATIC_SHARD_ID,
            Relic::MutagenicStrength => MUTAGENIC_STRENGTH_ID,
            Relic::WarpedTongs => WARPED_TONGS_ID,
            Relic::GoldenIdol => GOLDEN_IDOL_ID,
            Relic::BloodyIdol => BLOODY_IDOL_ID,
            Relic::Necronomicon => NECRONOMICON_ID,
            Relic::Enchiridion => ENCHIRIDION_ID,
            Relic::NilrysCodex => NILRYS_CODEX_ID,
            Relic::MarkOfBloom => MARK_OF_BLOOM_ID,
            Relic::SpiritPoop => SPIRIT_POOP_ID,
            Relic::OddMushroom => ODD_MUSHROOM_ID,
            Relic::NlothsGift => NLOTHS_GIFT_ID,
            Relic::NeowsLament => NEOWS_LAMENT_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == BURNING_BLOOD_ID => Some(Relic::BurningBlood),
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
            id if id == WHETSTONE_ID => Some(Relic::Whetstone),
            id if id == WAR_PAINT_ID => Some(Relic::WarPaint),
            id if id == AKABEKO_ID => Some(Relic::Akabeko),
            id if id == CENTENNIAL_PUZZLE_ID => Some(Relic::CentennialPuzzle),
            id if id == PEN_NIB_ID => Some(Relic::PenNib),
            id if id == SELF_FORMING_CLAY_ID => Some(Relic::SelfFormingClay),
            id if id == CLOCKWORK_SOUVENIR_ID => Some(Relic::ClockworkSouvenir),
            id if id == RUNIC_CUBE_ID => Some(Relic::RunicCube),
            id if id == THE_ABACUS_ID => Some(Relic::TheAbacus),
            id if id == GREMLIN_HORN_ID => Some(Relic::GremlinHorn),
            id if id == SUNDIAL_ID => Some(Relic::Sundial),
            id if id == CHARONS_ASHES_ID => Some(Relic::CharonsAshes),
            id if id == BLUE_CANDLE_ID => Some(Relic::BlueCandle),
            id if id == MEDICAL_KIT_ID => Some(Relic::MedicalKit),
            id if id == LIZARD_TAIL_ID => Some(Relic::LizardTail),
            id if id == POCKETWATCH_ID => Some(Relic::Pocketwatch),
            id if id == HAND_DRILL_ID => Some(Relic::HandDrill),
            id if id == RED_MASK_ID => Some(Relic::RedMask),
            id if id == CIRCLET_ID => Some(Relic::Circlet),
            id if id == RED_CIRCLET_ID => Some(Relic::RedCirclet),
            id if id == CULTIST_MASK_ID => Some(Relic::CultistMask),
            id if id == FACE_OF_CLERIC_ID => Some(Relic::FaceOfCleric),
            id if id == GREMLIN_MASK_ID => Some(Relic::GremlinMask),
            id if id == NLOTHS_MASK_ID => Some(Relic::NlothsMask),
            id if id == SSSERPENT_HEAD_ID => Some(Relic::SsserpentHead),
            id if id == SACRED_BARK_ID => Some(Relic::SacredBark),
            id if id == RUNIC_PYRAMID_ID => Some(Relic::RunicPyramid),
            id if id == FROZEN_EYE_ID => Some(Relic::FrozenEye),
            id if id == PEACE_PIPE_ID => Some(Relic::PeacePipe),
            id if id == ORANGE_PELLETS_ID => Some(Relic::OrangePellets),
            id if id == GIRYA_ID => Some(Relic::Girya),
            id if id == UNCEASING_TOP_ID => Some(Relic::UnceasingTop),
            id if id == SHOVEL_ID => Some(Relic::Shovel),
            id if id == FOSSILIZED_HELIX_ID => Some(Relic::FossilizedHelix),
            id if id == BLACK_STAR_ID => Some(Relic::BlackStar),
            id if id == MATRYOSHKA_ID => Some(Relic::Matryoshka),
            id if id == EMPTY_CAGE_ID => Some(Relic::EmptyCage),
            id if id == BOTTLED_FLAME_ID => Some(Relic::BottledFlame),
            id if id == BOTTLED_LIGHTNING_ID => Some(Relic::BottledLightning),
            id if id == BOTTLED_TORNADO_ID => Some(Relic::BottledTornado),
            id if id == DOLLYS_MIRROR_ID => Some(Relic::DollysMirror),
            id if id == PRAYER_WHEEL_ID => Some(Relic::PrayerWheel),
            id if id == CRACKED_CORE_ID => Some(Relic::CrackedCore),
            id if id == FROZEN_CORE_ID => Some(Relic::FrozenCore),
            id if id == PURE_WATER_ID => Some(Relic::PureWater),
            id if id == HOLY_WATER_ID => Some(Relic::HolyWater),
            id if id == RING_OF_THE_SNAKE_ID => Some(Relic::RingOfTheSnake),
            id if id == RING_OF_THE_SERPENT_ID => Some(Relic::RingOfTheSerpent),
            id if id == CAULDRON_ID => Some(Relic::Cauldron),
            id if id == TINY_HOUSE_ID => Some(Relic::TinyHouse),
            id if id == DEAD_BRANCH_ID => Some(Relic::DeadBranch),
            id if id == MUMMIFIED_HAND_ID => Some(Relic::MummifiedHand),
            id if id == THE_COURIER_ID => Some(Relic::TheCourier),
            id if id == INCENSE_BURNER_ID => Some(Relic::IncenseBurner),
            id if id == CURSED_KEY_ID => Some(Relic::CursedKey),
            id if id == TINY_CHEST_ID => Some(Relic::TinyChest),
            id if id == ORRERY_ID => Some(Relic::Orrery),
            id if id == SNECKO_EYE_ID => Some(Relic::SneckoEye),
            id if id == STRANGE_SPOON_ID => Some(Relic::StrangeSpoon),
            id if id == WING_BOOTS_ID => Some(Relic::WingBoots),
            id if id == CALLING_BELL_ID => Some(Relic::CallingBell),
            id if id == PANDORAS_BOX_ID => Some(Relic::PandorasBox),
            id if id == ASTROLABE_ID => Some(Relic::Astrolabe),
            id if id == GAMBLING_CHIP_ID => Some(Relic::GamblingChip),
            id if id == TOOLBOX_ID => Some(Relic::Toolbox),
            id if id == JUZU_BRACELET_ID => Some(Relic::JuzuBracelet),
            id if id == PRISMATIC_SHARD_ID => Some(Relic::PrismaticShard),
            id if id == MUTAGENIC_STRENGTH_ID => Some(Relic::MutagenicStrength),
            id if id == WARPED_TONGS_ID => Some(Relic::WarpedTongs),
            id if id == GOLDEN_IDOL_ID => Some(Relic::GoldenIdol),
            id if id == BLOODY_IDOL_ID => Some(Relic::BloodyIdol),
            id if id == NECRONOMICON_ID => Some(Relic::Necronomicon),
            id if id == ENCHIRIDION_ID => Some(Relic::Enchiridion),
            id if id == NILRYS_CODEX_ID => Some(Relic::NilrysCodex),
            id if id == MARK_OF_BLOOM_ID => Some(Relic::MarkOfBloom),
            id if id == SPIRIT_POOP_ID => Some(Relic::SpiritPoop),
            id if id == ODD_MUSHROOM_ID => Some(Relic::OddMushroom),
            id if id == NLOTHS_GIFT_ID => Some(Relic::NlothsGift),
            id if id == NEOWS_LAMENT_ID => Some(Relic::NeowsLament),
            _ => None,
        }
    }
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) -> SimResult<()> {
    for relic in relics {
        match relic {
            Relic::BurningBlood => {}
            Relic::SacredBark => {}
            Relic::RunicPyramid => {}
            Relic::FrozenEye => {}
            Relic::PeacePipe => {}
            Relic::OrangePellets => {}
            Relic::Girya => {}
            Relic::UnceasingTop => {}
            Relic::Shovel => {}
            Relic::BlackStar => {}
            Relic::Matryoshka => {}
            Relic::EmptyCage => {}
            Relic::BottledFlame => {}
            Relic::BottledLightning => {}
            Relic::BottledTornado => {}
            Relic::DollysMirror => {}
            Relic::PrayerWheel => {}
            Relic::CrackedCore => {}
            Relic::FrozenCore => {}
            Relic::PureWater => {}
            Relic::HolyWater => {}
            Relic::RingOfTheSnake => {}
            Relic::RingOfTheSerpent => {}
            Relic::Cauldron => {}
            Relic::TinyHouse => {}
            Relic::DeadBranch => {}
            Relic::MummifiedHand => {}
            Relic::TheCourier => {}
            Relic::IncenseBurner => {}
            Relic::CursedKey => {}
            Relic::TinyChest => {}
            Relic::Orrery => {}
            // Snecko Eye's atPreBattle hook applies Confusion once. The relic
            // does not itself randomize every later draw after that power is
            // removed (for example by Orange Pellets).
            Relic::SneckoEye => {
                crate::power::apply_player_confusion(&mut combat.player.powers)?;
            }
            Relic::StrangeSpoon => {}
            Relic::WingBoots => {}
            Relic::CallingBell => {}
            Relic::PandorasBox => {}
            Relic::Astrolabe => {}
            Relic::GamblingChip => {}
            Relic::Toolbox => {}
            Relic::JuzuBracelet => {}
            Relic::PrismaticShard => {}
            Relic::WarpedTongs => {}
            Relic::GoldenIdol => {}
            Relic::BloodyIdol => {}
            Relic::RedMask => {
                for monster in combat.monsters.iter_mut().filter(|monster| monster.alive) {
                    crate::power::apply_monster_weak(&mut monster.powers, 1)?;
                }
            }
            Relic::Necronomicon => {}
            Relic::Enchiridion => {}
            Relic::NilrysCodex => {}
            Relic::MutagenicStrength => {
                checked_add_relic_value(
                    &mut combat.player.temp_strength,
                    MUTAGENIC_STRENGTH_AMOUNT,
                )?;
            }
            Relic::FossilizedHelix => {
                checked_add_relic_value(&mut combat.player.powers.buffer, FOSSILIZED_HELIX_BUFFER)?;
            }
            Relic::BloodVial => {
                heal_combat_player_with_relics(combat, BLOOD_VIAL_HEAL)?;
            }
            Relic::Vajra => {
                checked_add_relic_value(&mut combat.player.powers.strength, VAJRA_STRENGTH)?;
            }
            Relic::OddlySmoothStone => {
                checked_add_relic_value(
                    &mut combat.player.powers.dexterity,
                    ODDLY_SMOOTH_STONE_DEXTERITY,
                )?;
            }
            Relic::Strawberry => {}
            Relic::Pear => {}
            Relic::Mango => {}
            Relic::OldCoin => {}
            Relic::LeesWaffle => {}
            Relic::PotionBelt => {}
            Relic::Lantern => {
                checked_add_relic_value(&mut combat.player.energy, LANTERN_ENERGY)?;
            }
            Relic::BagOfPreparation => {
                crate::combat::transition::player_draw_cards(combat, BAG_OF_PREPARATION_DRAW)?;
            }
            Relic::BagOfMarbles => {
                // Toolbox publishes its opening reward before the queued
                // atPreBattle hook. Settle Bag of Marbles with the deferred
                // opening draw instead of exposing Vulnerable on the reward
                // screen.
                if combat.pending_opening_hand_draw == 0 || !relics.contains(&Relic::Toolbox) {
                    for monster in combat.monsters.iter_mut().filter(|monster| monster.alive) {
                        apply_monster_vulnerable_with_relics(
                            &mut monster.powers,
                            relics,
                            BAG_OF_MARBLES_VULNERABLE,
                        )?;
                    }
                }
            }
            Relic::BronzeScales => {
                checked_add_relic_value(&mut combat.player.powers.thorns, BRONZE_SCALES_THORNS)?;
            }
            Relic::ThreadAndNeedle => {
                checked_add_relic_value(
                    &mut combat.player.powers.plated_armor,
                    THREAD_AND_NEEDLE_PLATED_ARMOR,
                )?;
            }
            Relic::ClockworkSouvenir => {
                checked_add_relic_value(
                    &mut combat.player.powers.artifact,
                    CLOCKWORK_SOUVENIR_ARTIFACT,
                )?;
            }
            Relic::RedSkull => {
                apply_start_of_combat_red_skull(combat)?;
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
                if combat.pending_opening_hand_draw > 0 {
                    checked_add_relic_value(
                        &mut combat.pending_opening_combat_block,
                        ANCHOR_BLOCK,
                    )?;
                } else {
                    checked_add_relic_value(&mut combat.player.block, ANCHOR_BLOCK)?;
                }
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
            Relic::Whetstone => {}
            Relic::WarPaint => {}
            Relic::Akabeko => {
                checked_add_relic_value(&mut combat.player.powers.vigor, AKABEKO_DAMAGE)?;
            }
            Relic::CentennialPuzzle => {}
            Relic::PenNib => {}
            Relic::SelfFormingClay => {}
            Relic::RunicCube => {}
            Relic::TheAbacus => {}
            Relic::GremlinHorn => {}
            Relic::Sundial => {}
            Relic::CharonsAshes => {}
            Relic::BlueCandle => {}
            Relic::MedicalKit => {}
            Relic::LizardTail => {}
            Relic::Pocketwatch => {}
            Relic::HandDrill => {}
            Relic::Circlet => {}
            Relic::RedCirclet => {}
            Relic::CultistMask => {}
            Relic::FaceOfCleric => {}
            Relic::GremlinMask => {}
            Relic::NlothsMask => {}
            Relic::SsserpentHead => {}
            Relic::MarkOfBloom => {}
            Relic::SpiritPoop => {}
            Relic::OddMushroom => {}
            Relic::NlothsGift => {}
            Relic::NeowsLament => {}
        }
    }

    apply_start_of_player_turn_relics(combat)?;
    Ok(())
}

pub fn apply_shuffle_relics(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    let mut follow_ups = Vec::new();
    if state.relics.contains(&Relic::TheAbacus) {
        // TheAbacus.onShuffle addToBot's GainBlockAction after EmptyDeckShuffle.
        // Warcry's later PutOnDeckAction therefore opens with block still 0
        // (FIDL01525).
        follow_ups.push(InternalAction::GainBlockDirect {
            amount: THE_ABACUS_BLOCK,
        });
    }
    if state.relics.contains(&Relic::Sundial) {
        state.relic_counters.sundial_shuffles =
            state.relic_counters.sundial_shuffles.wrapping_add(1);
        if state
            .relic_counters
            .sundial_shuffles
            .is_multiple_of(SUNDIAL_THRESHOLD)
        {
            // Sundial.onShuffle addToBot's GainEnergyAction after the shuffle.
            // Warcry's later PutOnDeckAction therefore opens with base energy
            // (FIDL01624).
            follow_ups.push(InternalAction::GainEnergy {
                amount: SUNDIAL_ENERGY,
            });
        }
    }
    Ok(follow_ups)
}

pub fn apply_monster_death_relics(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    if next.relics.contains(&Relic::GremlinHorn) {
        checked_add_relic_value(&mut next.player.energy, GREMLIN_HORN_ENERGY)?;
        crate::combat::transition::player_draw_cards(&mut next, GREMLIN_HORN_DRAW)?;
    }
    *state = next;
    Ok(())
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
    *hp = hp.saturating_add(heal).min(max_hp);
}

pub fn heal_combat_player_with_relics(state: &mut CombatState, base_heal: i32) -> SimResult<()> {
    if state.mark_of_bloom {
        return Ok(());
    }

    let mut next = state.clone();
    heal_player_in_combat_with_relics(
        &mut next.player.hp,
        next.player.max_hp,
        base_heal,
        &next.relics,
    );
    sync_red_skull_strength(&mut next)?;
    *state = next;
    Ok(())
}

pub fn apply_potion_use_relics_to_combat(combat: &mut CombatState) -> SimResult<()> {
    if combat.relics.contains(&Relic::ToyOrnithopter) {
        heal_combat_player_with_relics(combat, TOY_ORNITHOPTER_HEAL)?;
    }
    Ok(())
}

pub fn apply_player_hp_loss_relics(state: &mut CombatState, hp_loss: i32) -> SimResult<()> {
    apply_player_hp_loss_relics_with_draw_policy(state, hp_loss, HpLossDrawPolicy::Immediate)
}

/// Whether Centennial Puzzle / Runic Cube draw effects resolve immediately or
/// are counted for later settlement (multi-hit DamageAction sequences).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpLossDrawPolicy {
    /// Resolve draw relics now (single-hit attacks, card HP loss, etc.).
    Immediate,
    /// Mark counters / count Runic Cube events only. Caller settles draws after
    /// the full multi-hit attack finishes — matching addToBot ordering so mid-hit
    /// shuffles cannot grant Abacus block between stabs (aef32ab6).
    DeferDraws,
    /// Draw the trigger card immediately, but park Evolve/Fire Breathing
    /// callbacks behind the enclosing END queue's hand-discard action.
    QueueFollowUps,
    /// Post-discard Constricted loss: the source DrawCardAction is already
    /// behind the No Draw expiry and must not be suppressed by that flag.
    QueueFollowUpsBypassNoDraw,
}

pub fn apply_player_hp_loss_relics_with_draw_policy(
    state: &mut CombatState,
    hp_loss: i32,
    draw_policy: HpLossDrawPolicy,
) -> SimResult<()> {
    if hp_loss <= 0 {
        return Ok(());
    }
    let mut next = state.clone();
    if next.relics.contains(&Relic::CentennialPuzzle)
        && next.relic_counters.centennial_puzzle_triggers == 0
    {
        next.relic_counters.centennial_puzzle_triggers = 1;
        // CentennialPuzzle.onLoseHp addToBot's DrawCardAction; lethal damage
        // ends the fight before the bot runs.
        if next.player.hp > 0 {
            match draw_policy {
                HpLossDrawPolicy::Immediate => {
                    crate::combat::transition::player_draw_cards(
                        &mut next,
                        CENTENNIAL_PUZZLE_DRAW,
                    )?;
                }
                HpLossDrawPolicy::DeferDraws => {
                    next.relic_counters.deferred_centennial_puzzle_draw = true;
                }
                HpLossDrawPolicy::QueueFollowUps | HpLossDrawPolicy::QueueFollowUpsBypassNoDraw => {
                    let follow_ups = crate::combat::transition::
                        player_draw_cards_from_hp_loss_with_deferred_evolve_policy(
                            &mut next,
                            CENTENNIAL_PUZZLE_DRAW,
                            draw_policy == HpLossDrawPolicy::QueueFollowUpsBypassNoDraw,
                        )?;
                    next.pending_hp_loss_draw_follow_ups.extend(follow_ups);
                }
            }
        }
    }
    if next.relics.contains(&Relic::SelfFormingClay) {
        next.relic_counters.self_forming_clay_next_turn_block = next
            .relic_counters
            .self_forming_clay_next_turn_block
            .checked_add(SELF_FORMING_CLAY_BLOCK)
            .ok_or(SimError::InvalidState(
                "Self-Forming Clay block accumulation overflows i32",
            ))?;
    }
    if next.relics.contains(&Relic::RunicCube) {
        // RunicCube.wasHPLost addToTop's DrawCardAction. When the same damage
        // is lethal, GameActionManager never drains that bot entry — the death
        // screen keeps the pre-draw piles (a7f662aa8ed22115 END lethal vs
        // Lagavulin: Bash+ stays in draw, hand empty).
        match draw_policy {
            HpLossDrawPolicy::Immediate if next.player.hp > 0 => {
                crate::combat::transition::player_draw_cards(&mut next, RUNIC_CUBE_DRAW)?;
            }
            HpLossDrawPolicy::QueueFollowUps | HpLossDrawPolicy::QueueFollowUpsBypassNoDraw
                if next.player.hp > 0 =>
            {
                let follow_ups = crate::combat::transition::
                    player_draw_cards_from_hp_loss_with_deferred_evolve_policy(
                        &mut next,
                        RUNIC_CUBE_DRAW,
                        draw_policy == HpLossDrawPolicy::QueueFollowUpsBypassNoDraw,
                    )?;
                next.pending_hp_loss_draw_follow_ups.extend(follow_ups);
            }
            HpLossDrawPolicy::DeferDraws => {
                next.relic_counters.deferred_runic_cube_draws = next
                    .relic_counters
                    .deferred_runic_cube_draws
                    .checked_add(1)
                    .ok_or(SimError::InvalidState(
                        "deferred Runic Cube draw count overflows u32",
                    ))?;
            }
            HpLossDrawPolicy::QueueFollowUps | HpLossDrawPolicy::QueueFollowUpsBypassNoDraw => {}
            HpLossDrawPolicy::Immediate => {}
        }
    }
    sync_red_skull_strength(&mut next)?;
    *state = next;
    Ok(())
}

/// Resolve Centennial Puzzle / Runic Cube draws queued during a multi-hit attack.
pub fn settle_deferred_hp_loss_draw_relics(state: &mut CombatState) -> SimResult<()> {
    if state.player.hp <= 0 {
        state.relic_counters.deferred_centennial_puzzle_draw = false;
        state.relic_counters.deferred_runic_cube_draws = 0;
        return Ok(());
    }
    let mut next = state.clone();
    if next.relic_counters.deferred_centennial_puzzle_draw {
        next.relic_counters.deferred_centennial_puzzle_draw = false;
        if next.relics.contains(&Relic::CentennialPuzzle) {
            crate::combat::transition::player_draw_cards(&mut next, CENTENNIAL_PUZZLE_DRAW)?;
        }
    }
    let runic_draws = std::mem::take(&mut next.relic_counters.deferred_runic_cube_draws);
    if runic_draws > 0 && next.relics.contains(&Relic::RunicCube) {
        for _ in 0..runic_draws {
            if next.player.hp <= 0 {
                break;
            }
            crate::combat::transition::player_draw_cards(&mut next, RUNIC_CUBE_DRAW)?;
        }
    }
    *state = next;
    Ok(())
}

pub fn sync_red_skull_strength(state: &mut CombatState) -> SimResult<()> {
    sync_red_skull_strength_present(state, state.relics.contains(&Relic::RedSkull))
}

fn sync_red_skull_strength_present(state: &mut CombatState, has_red_skull: bool) -> SimResult<()> {
    if !has_red_skull {
        return Ok(());
    }

    let should_be_active = state.player.hp <= state.player.max_hp / 2;
    match (should_be_active, state.relic_counters.red_skull_active) {
        (true, false) => {
            state.player.powers.strength = state
                .player
                .powers
                .strength
                .checked_add(RED_SKULL_STRENGTH)
                .ok_or(SimError::InvalidState(
                    "Red Skull Strength activation overflows i32",
                ))?;
            state.relic_counters.red_skull_active = true;
        }
        (false, true) => {
            state.player.powers.strength = state
                .player
                .powers
                .strength
                .checked_sub(RED_SKULL_STRENGTH)
                .ok_or(SimError::InvalidState(
                    "Red Skull Strength removal underflows i32",
                ))?;
            state.relic_counters.red_skull_active = false;
        }
        _ => {}
    }
    Ok(())
}

pub fn apply_buffer_to_hp_loss(powers: &mut crate::power::PlayerPowers, hp_loss: i32) -> i32 {
    if hp_loss > 0 && powers.buffer > 0 {
        powers.buffer -= 1;
        0
    } else {
        hp_loss
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
    state.relic_counters.cards_played_last_turn = state.relic_counters.cards_played_this_turn;
    state.relic_counters.ornamental_fan_attacks_this_turn = 0;
    state.relic_counters.shuriken_attacks_this_turn = 0;
    state.relic_counters.kunai_attacks_this_turn = 0;
    state.relic_counters.letter_opener_skills_this_turn = 0;
    state.relic_counters.cards_played_this_turn = 0;
    state.relic_counters.attacks_played_this_turn = 0;
    state.relic_counters.necronomicon_used_this_turn = false;
    // Orange Pellets only counts card types played during the current turn.
    // Carrying Attack/Power flags into the next turn would falsely cleanse on
    // the first Skill (trace FIDL00025 / end-turn HP mismatch vs Constricted).
    state.relic_counters.orange_pellets_attack_played = false;
    state.relic_counters.orange_pellets_skill_played = false;
    state.relic_counters.orange_pellets_power_played = false;
}

pub fn apply_start_of_player_turn_relics(state: &mut CombatState) -> SimResult<()> {
    if !has_start_of_turn_relic(state) {
        return Ok(());
    }

    checked_increment_relic_counter(&mut state.relic_counters.player_turns_started)?;

    if state.relic_counters.self_forming_clay_next_turn_block > 0 {
        let block = state.relic_counters.self_forming_clay_next_turn_block;
        state.relic_counters.self_forming_clay_next_turn_block = 0;
        if state.player.no_block_turns == 0 {
            state
                .player
                .block
                .checked_add(block)
                .ok_or(SimError::InvalidState(
                    "combat integer addition overflows i32",
                ))?;
        }
        crate::combat::transition::apply_player_direct_block_gain(state, block)?;
    }

    if state.relics.contains(&Relic::HappyFlower) {
        checked_increment_relic_counter(&mut state.relic_counters.happy_flower_turns)?;
        if state.relic_counters.happy_flower_turns >= HAPPY_FLOWER_THRESHOLD {
            state.relic_counters.happy_flower_turns = 0;
            if state.relic_counters.player_turns_started == 1
                && state.relics.contains(&Relic::Toolbox)
            {
                checked_add_relic_value(
                    &mut state.pending_start_of_turn_relic_energy,
                    HAPPY_FLOWER_ENERGY,
                )?;
            } else {
                checked_add_relic_value(&mut state.player.energy, HAPPY_FLOWER_ENERGY)?;
            }
        }
    }

    if state.relics.contains(&Relic::ArtOfWar)
        && state.relic_counters.player_turns_started > 1
        && state.relic_counters.attacks_played_last_turn == 0
    {
        checked_add_relic_value(&mut state.player.energy, ART_OF_WAR_ENERGY)?;
    }

    match state.relic_counters.player_turns_started {
        HORN_CLEAT_TURN if state.relics.contains(&Relic::HornCleat) => {
            checked_add_relic_value(&mut state.player.block, HORN_CLEAT_BLOCK)?;
        }
        CAPTAINS_WHEEL_TURN if state.relics.contains(&Relic::CaptainsWheel) => {
            // Captain's Wheel's start-of-turn callback bypasses No Block just
            // like the target relic action, while Juggernaut still reacts to
            // the granted block (FIDL00244/FIDL01632).
            crate::combat::transition::apply_player_end_turn_automatic_block_gain(
                state,
                CAPTAINS_WHEEL_BLOCK,
            )?;
        }
        _ => {}
    }

    if state.relics.contains(&Relic::Brimstone) {
        checked_add_relic_value(&mut state.player.powers.strength, BRIMSTONE_PLAYER_STRENGTH)?;
        for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
            checked_add_relic_value(&mut monster.powers.strength, BRIMSTONE_MONSTER_STRENGTH)?;
        }
    }

    if state.relics.contains(&Relic::IncenseBurner) {
        checked_increment_relic_counter(&mut state.relic_counters.incense_burner_counter)?;
        if state.relic_counters.incense_burner_counter >= INCENSE_BURNER_THRESHOLD {
            state.relic_counters.incense_burner_counter = 0;
            checked_add_relic_value(&mut state.player.powers.intangible, 1)?;
        }
    }
    Ok(())
}

pub fn apply_start_of_player_turn_post_draw_relics(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&Relic::MercuryHourglass) {
        if matches!(
            state.decision,
            Some(crate::combat::CombatDecisionState::ToolboxCardReward { .. })
        ) {
            // Toolbox's ChooseOneColorless action sits in front of atTurnStart
            // relic DamageActions. Keep Maw / other enemies at full HP until
            // the colorless card is chosen (FIDL01367).
            checked_add_relic_value(
                &mut state.pending_start_of_turn_relic_damage,
                MERCURY_HOURGLASS_DAMAGE,
            )?;
        } else {
            deal_unmodified_damage_to_living_monsters(state, MERCURY_HOURGLASS_DAMAGE)?;
        }
    }

    if state.relics.contains(&Relic::Pocketwatch)
        && state.relic_counters.player_turns_started > 1
        && state.relic_counters.cards_played_last_turn <= POCKETWATCH_CARD_LIMIT
    {
        crate::combat::transition::player_draw_cards(state, POCKETWATCH_DRAW)?;
    }

    if state.relics.contains(&Relic::WarpedTongs) {
        upgrade_random_non_status_hand_card(state)?;
    }
    Ok(())
}

fn upgrade_random_non_status_hand_card(state: &mut CombatState) -> SimResult<()> {
    let mut upgradeable = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let definition = crate::content::cards::get_card_definition(card.content_id)?;
            (definition.card_type != CardType::Status
                && crate::content::cards::card_instance_is_upgradeable(card))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    if upgradeable.is_empty() {
        return Ok(());
    }

    let mut shuffle_rng = state.rng.shuffle_rng.clone();
    let shuffle_seed = shuffle_rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut upgradeable);

    let index = upgradeable[0];
    let upgraded = upgrade_card_instance(state.piles.hand[index])?.ok_or(
        SimError::InvalidState("Warped Tongs selected a non-upgradeable card"),
    )?;
    state.rng.shuffle_rng = shuffle_rng;
    state.piles.hand[index] = upgraded;
    Ok(())
}

fn has_start_of_turn_relic(state: &CombatState) -> bool {
    state.relics.iter().any(|relic| {
        matches!(
            relic,
            Relic::HappyFlower
                | Relic::ArtOfWar
                | Relic::Pocketwatch
                | Relic::HornCleat
                | Relic::CaptainsWheel
                | Relic::MercuryHourglass
                | Relic::StoneCalendar
                | Relic::Brimstone
                | Relic::IncenseBurner
                | Relic::SelfFormingClay
        )
    })
}

pub fn apply_orichalcum_end_of_player_turn(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&Relic::Orichalcum) && state.player.block == 0 {
        crate::combat::transition::apply_player_direct_block_gain(state, ORICHALCUM_BLOCK)?;
    }
    Ok(())
}

pub fn settle_pending_start_of_turn_relic_actions(state: &mut CombatState) -> SimResult<()> {
    let energy = state
        .player
        .energy
        .checked_add(state.pending_start_of_turn_relic_energy)
        .ok_or(SimError::InvalidState(
            "pending start-of-turn relic energy overflows i32",
        ))?;
    state.player.energy = energy;
    state.pending_start_of_turn_relic_energy = 0;
    if state.pending_start_of_turn_relic_damage > 0 {
        deal_unmodified_damage_to_living_monsters(state, state.pending_start_of_turn_relic_damage)?;
        state.pending_start_of_turn_relic_damage = 0;
    }
    Ok(())
}

/// Drain the opening actions that were queued behind Toolbox's card choice.
///
/// The target queues the opening draw before Anchor's start-of-combat block.
/// Clone first so a failed draw or checked block gain cannot consume the
/// pending queue marker.
pub fn settle_pending_opening_combat_actions(state: &mut CombatState) -> SimResult<()> {
    if state.pending_opening_hand_draw == 0 && state.pending_opening_combat_block == 0 {
        return Ok(());
    }
    let mut next = state.clone();
    let draw_count = next.pending_opening_hand_draw;
    if draw_count > 0 {
        crate::combat::transition::player_draw_cards(&mut next, draw_count)?;
        if next.relics.contains(&Relic::BagOfMarbles) {
            for monster in next.monsters.iter_mut().filter(|monster| monster.alive) {
                apply_monster_vulnerable_with_relics(
                    &mut monster.powers,
                    &next.relics,
                    BAG_OF_MARBLES_VULNERABLE,
                )?;
            }
        }
        // Warped Tongs is queued after the opening draw in the target. The
        // ordinary start-turn hook runs before a Toolbox-blocked draw and
        // therefore sees no hand to upgrade.
        if next.relics.contains(&Relic::WarpedTongs) {
            upgrade_random_non_status_hand_card(&mut next)?;
        }
    }
    if next.pending_opening_combat_block > 0 {
        checked_add_relic_value(&mut next.player.block, next.pending_opening_combat_block)?;
    }
    next.pending_opening_hand_draw = 0;
    next.pending_opening_combat_block = 0;
    *state = next;
    Ok(())
}

pub fn apply_end_of_player_turn_relics(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&Relic::StoneCalendar)
        && state.relic_counters.player_turns_started == STONE_CALENDAR_TURN
    {
        deal_unmodified_damage_to_living_monsters(state, STONE_CALENDAR_DAMAGE)?;
    }
    Ok(())
}

/// Open Nilry's Codex 3-card combat reward (caller pauses end-of-turn until closed).
pub fn open_nilrys_codex_card_reward(state: &mut CombatState) -> SimResult<()> {
    use crate::combat::state::CombatDecisionState;
    use crate::content::cards::get_card_definition;
    use crate::content::shop_pool::ironclad_combat_discovery_pool;
    use crate::ids::CardId;
    use crate::CardInstance;

    let pool: Vec<_> = ironclad_combat_discovery_pool()
        .iter()
        .copied()
        .filter(|content_id| get_card_definition(*content_id).is_some())
        .collect();
    if pool.len() < 3 {
        return Err(SimError::InvalidState(
            "Nilry's Codex card pool is smaller than 3 modeled cards",
        ));
    }
    let next_card_id = state.reserve_card_instance_ids(3)?;
    let mut choices = Vec::with_capacity(3);
    let rng = &mut state.rng.card_random_rng;
    while choices.len() < 3 {
        let index = rng.random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[index];
        if !choices.contains(&content_id) {
            choices.push(content_id);
        }
    }
    state.decision = Some(CombatDecisionState::NilrysCodexCardReward {
        choices: choices
            .into_iter()
            .enumerate()
            .map(|(index, content_id)| {
                CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
            })
            .collect(),
    });
    Ok(())
}

/// Park a Nilry choice without inserting into the draw pile (FIDL00451 first/second
/// offer frames where CommMod keeps pre-discard piles unchanged).
pub fn nilrys_codex_park_choice_without_insert(
    state: &mut CombatState,
    index: usize,
) -> SimResult<()> {
    use crate::combat::state::CombatDecisionState;

    let Some(CombatDecisionState::NilrysCodexCardReward { choices }) = state.decision.take() else {
        return Err(SimError::IllegalAction("no Nilry Codex reward is open"));
    };
    choices.get(index).ok_or(SimError::IllegalAction(
        "Nilry Codex choice index out of range",
    ))?;
    // The first offer is a pure UI pause in the two-step publication window;
    // its choice is not inserted. Only the second offer is committed when the
    // paused end-turn resumes.
    Ok(())
}

/// Closing the first Codex offer continues `callEndOfTurnActions` power
/// hooks that grant block (Plated Armor / Metallicize) while the hand is
/// still held (FIDL01486 CHOOSE 461: Thread and Needle +4).
pub fn nilrys_codex_apply_paused_end_turn_block_powers(state: &mut CombatState) -> SimResult<()> {
    if state.player.powers.metallicize > 0 {
        crate::combat::transition::apply_player_end_turn_automatic_block_gain(
            state,
            state.player.powers.metallicize,
        )?;
    }
    if state.player.powers.plated_armor > 0 {
        crate::combat::transition::apply_player_end_turn_automatic_block_gain(
            state,
            state.player.powers.plated_armor,
        )?;
    }
    state.time_warp_end_powers_applied = true;
    Ok(())
}

/// Insert every parked Nilry card into a random draw-pile spot.
pub fn nilrys_codex_flush_pending_draw_inserts(state: &mut CombatState) -> SimResult<()> {
    let pending = std::mem::take(&mut state.pending_nilrys_codex_draw_inserts);
    for content_id in pending {
        crate::combat::transition::add_generated_card_to_draw_pile_random_spot_public(
            state, content_id,
        )?;
    }
    Ok(())
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
) -> SimResult<bool> {
    if !relics.contains(&Relic::Ginger) {
        crate::power::apply_player_weak(powers, amount)
    } else {
        Ok(false)
    }
}

pub fn apply_player_frail_with_relics(
    powers: &mut crate::power::PlayerPowers,
    relics: &[Relic],
    amount: i32,
) -> SimResult<bool> {
    if !relics.contains(&Relic::Turnip) {
        crate::power::apply_player_frail(powers, amount)
    } else {
        Ok(false)
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
) -> SimResult<()> {
    let mut next = *powers;
    let applied = crate::power::apply_monster_vulnerable(&mut next, amount)?;
    if applied && relics.contains(&Relic::ChampionBelt) {
        crate::power::apply_monster_weak(&mut next, CHAMPION_BELT_WEAK)?;
    }
    *powers = next;
    Ok(())
}

pub fn apply_on_card_play_relics(
    state: &mut CombatState,
    card_type: CardType,
) -> SimResult<Vec<InternalAction>> {
    let mut follow_ups = Vec::new();

    checked_increment_relic_counter(&mut state.relic_counters.cards_played_this_turn)?;
    if state.relics.contains(&Relic::Akabeko) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.attacks_played_this_combat)?;
    }
    if state.relics.contains(&Relic::ArtOfWar) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.attacks_played_this_turn)?;
    }

    if state.relics.contains(&Relic::InkBottle) {
        checked_increment_relic_counter(&mut state.relic_counters.ink_bottle_cards_played)?;
        if state.relic_counters.ink_bottle_cards_played >= INK_BOTTLE_THRESHOLD {
            state.relic_counters.ink_bottle_cards_played = 0;
            follow_ups.push(InternalAction::DrawCardsFromInkBottle { count: 1 });
        }
    }

    if state.relics.contains(&Relic::OrnamentalFan) && card_type == CardType::Attack {
        checked_increment_relic_counter(
            &mut state.relic_counters.ornamental_fan_attacks_this_turn,
        )?;
        if state.relic_counters.ornamental_fan_attacks_this_turn >= ORNAMENTAL_FAN_THRESHOLD {
            state.relic_counters.ornamental_fan_attacks_this_turn = 0;
            follow_ups.push(InternalAction::GainBlockDirect {
                amount: ORNAMENTAL_FAN_BLOCK,
            });
        }
    }

    if state.relics.contains(&Relic::Nunchaku) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.nunchaku_attacks_played)?;
        if state.relic_counters.nunchaku_attacks_played >= NUNCHAKU_THRESHOLD {
            state.relic_counters.nunchaku_attacks_played = 0;
            follow_ups.push(InternalAction::GainEnergy {
                amount: NUNCHAKU_ENERGY,
            });
        }
    }

    if state.relics.contains(&Relic::PenNib) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.pen_nib_attacks_played)?;
        if state.relic_counters.pen_nib_attacks_played >= PEN_NIB_THRESHOLD {
            state.relic_counters.pen_nib_attacks_played = 0;
            // 10th attack deals double damage for this card (including a Double
            // Tap copy that is the wrapping play — FIDL00421).
            state.pen_nib_double_active = true;
        }
    }

    if state.relics.contains(&Relic::Shuriken) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.shuriken_attacks_this_turn)?;
        if state.relic_counters.shuriken_attacks_this_turn >= SHURIKEN_THRESHOLD {
            state.relic_counters.shuriken_attacks_this_turn = 0;
            follow_ups.push(InternalAction::GainStrength {
                amount: SHURIKEN_STRENGTH,
            });
        }
    }

    if state.relics.contains(&Relic::Kunai) && card_type == CardType::Attack {
        checked_increment_relic_counter(&mut state.relic_counters.kunai_attacks_this_turn)?;
        if state.relic_counters.kunai_attacks_this_turn >= KUNAI_THRESHOLD {
            state.relic_counters.kunai_attacks_this_turn = 0;
            follow_ups.push(InternalAction::GainDexterity {
                amount: KUNAI_DEXTERITY,
            });
        }
    }

    if state.relics.contains(&Relic::LetterOpener) && card_type == CardType::Skill {
        checked_increment_relic_counter(&mut state.relic_counters.letter_opener_skills_this_turn)?;
        if state.relic_counters.letter_opener_skills_this_turn >= LETTER_OPENER_THRESHOLD {
            state.relic_counters.letter_opener_skills_this_turn = 0;
            let targets = state
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .map(|monster| monster.id)
                .collect::<Vec<_>>();
            state.pending_letter_opener_blasts = state
                .pending_letter_opener_blasts
                .saturating_add(targets.len() as u32);
            follow_ups.extend(targets.into_iter().map(|target| {
                InternalAction::DealUnmodifiedDamage {
                    target,
                    amount: LETTER_OPENER_DAMAGE,
                }
            }));
        }
    }

    if state.relics.contains(&Relic::BirdFacedUrn) && card_type == CardType::Power {
        heal_combat_player_with_relics(state, BIRD_FACED_URN_HEAL)?;
    }

    if apply_orange_pellets_on_card_play(state, card_type) {
        follow_ups.push(InternalAction::ClearPlayerDebuffs);
    }

    Ok(follow_ups)
}

fn checked_increment_relic_counter(counter: &mut u32) -> SimResult<()> {
    if *counter >= i32::MAX as u32 {
        return Err(SimError::InvalidState(
            "combat relic counter exceeds the target signed range",
        ));
    }
    *counter += 1;
    Ok(())
}

fn checked_add_relic_value(value: &mut i32, amount: i32) -> SimResult<()> {
    *value = value.checked_add(amount).ok_or(SimError::InvalidState(
        "combat integer addition overflows i32",
    ))?;
    Ok(())
}

fn apply_start_of_combat_red_skull(state: &mut CombatState) -> SimResult<()> {
    sync_red_skull_strength_present(state, true)
}

fn apply_orange_pellets_on_card_play(state: &mut CombatState, card_type: CardType) -> bool {
    if !state.relics.contains(&Relic::OrangePellets) {
        return false;
    }

    match card_type {
        CardType::Attack => state.relic_counters.orange_pellets_attack_played = true,
        CardType::Skill => state.relic_counters.orange_pellets_skill_played = true,
        CardType::Power => state.relic_counters.orange_pellets_power_played = true,
        CardType::Status => {}
    }

    if state.relic_counters.orange_pellets_attack_played
        && state.relic_counters.orange_pellets_skill_played
        && state.relic_counters.orange_pellets_power_played
    {
        state.relic_counters.orange_pellets_attack_played = false;
        state.relic_counters.orange_pellets_skill_played = false;
        state.relic_counters.orange_pellets_power_played = false;
        // Standalone relic probes call this helper outside a card-use queue;
        // the live callback returns a deferred ClearPlayerDebuffs action.
        if state.card_in_use.is_none() {
            crate::power::clear_player_debuffs(&mut state.player.powers);
        }
        return true;
    }
    false
}

#[must_use]
pub fn can_play_card_with_relics(state: &CombatState) -> bool {
    !state.relics.contains(&Relic::VelvetChoker)
        || state.relic_counters.cards_played_this_turn < VELVET_CHOKER_CARD_LIMIT
}

#[must_use]
pub fn can_play_unplayable_card_with_relics(
    relics: &[Relic],
    card_type: CardType,
    content_id: ContentId,
) -> bool {
    if crate::content::cards::is_curse_content_id(content_id) {
        relics.contains(&Relic::BlueCandle)
    } else if card_type == CardType::Status {
        relics.contains(&Relic::MedicalKit)
    } else {
        false
    }
}

fn deal_unmodified_damage_to_living_monsters(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    let mut dead = Vec::new();
    for monster_id in targets {
        let killed = {
            let monster = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == monster_id)
                .expect("Stone Calendar target still exists");
            crate::combat::damage::deal_unmodified_damage_to_monster(monster, amount);
            !monster.alive
        };
        // Relic damage crosses Slime Boss's split threshold just like card and
        // power damage; the split must queue before the next end-turn phase.
        crate::content::monsters::check_slime_boss_split(state, monster_id);
        if killed {
            dead.push(monster_id);
        }
    }
    for monster_id in dead {
        crate::combat::transition::apply_monster_death_hooks(state, monster_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::{MonsterPowers, PlayerPowers};

    #[test]
    fn courier_is_rejected_in_shop_and_end_offer_falls_through() {
        let ordinary = RelicSpawnContext {
            floor_num: 28,
            shop_room: false,
            ..RelicSpawnContext::default()
        };
        assert!(relic_can_spawn(RelicKey::TheCourier, &ordinary));

        let shop = RelicSpawnContext {
            floor_num: 28,
            shop_room: true,
            has_non_basic_attack: true,
            ..RelicSpawnContext::default()
        };
        assert!(!relic_can_spawn(RelicKey::TheCourier, &shop));

        let mut pools = RelicPoolState {
            common: Vec::new(),
            uncommon: vec![RelicKey::BottledFlame, RelicKey::TheCourier],
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        };
        assert_eq!(
            pools.return_random_relic_end(RelicTier::Uncommon, &shop),
            RelicKey::BottledFlame
        );
        assert!(pools.uncommon.is_empty());
    }

    #[test]
    fn orange_pellets_type_flags_reset_each_turn_and_do_not_cross_cleanse() {
        // Source rule: Orange Pellets requires Attack + Skill + Power in the
        // *same* turn. Stale type flags from a prior turn must not complete a
        // cleanse (FIDL00025: Attack+Power turn 1, Constricted applied, then
        // Attack+Skill turn 2 must leave Constricted).
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::OrangePellets);
        state.player.powers.constricted = 0;

        apply_orange_pellets_on_card_play(&mut state, CardType::Attack);
        apply_orange_pellets_on_card_play(&mut state, CardType::Power);
        assert!(state.relic_counters.orange_pellets_attack_played);
        assert!(state.relic_counters.orange_pellets_power_played);
        assert!(!state.relic_counters.orange_pellets_skill_played);

        // Monster applies Constricted after the incomplete turn.
        state.player.powers.constricted = 10;

        reset_turn_relic_counters(&mut state);
        assert!(!state.relic_counters.orange_pellets_attack_played);
        assert!(!state.relic_counters.orange_pellets_skill_played);
        assert!(!state.relic_counters.orange_pellets_power_played);

        apply_orange_pellets_on_card_play(&mut state, CardType::Attack);
        apply_orange_pellets_on_card_play(&mut state, CardType::Skill);
        assert_eq!(
            state.player.powers.constricted, 10,
            "Attack+Skill alone must not cleanse without a same-turn Power"
        );

        apply_orange_pellets_on_card_play(&mut state, CardType::Power);
        assert_eq!(
            state.player.powers.constricted, 0,
            "same-turn Attack+Skill+Power cleanses Constricted"
        );
        assert!(!state.relic_counters.orange_pellets_attack_played);
        assert!(!state.relic_counters.orange_pellets_skill_played);
        assert!(!state.relic_counters.orange_pellets_power_played);
    }

    #[test]
    fn red_skull_activation_and_removal_fail_without_partial_state() {
        let mut activation = CombatState::initial_fixture();
        activation.relics.push(Relic::RedSkull);
        activation.player.hp = activation.player.max_hp / 2;
        activation.player.powers.strength = i32::MAX;
        let activation_before = activation.clone();

        assert_eq!(
            sync_red_skull_strength(&mut activation),
            Err(SimError::InvalidState(
                "Red Skull Strength activation overflows i32"
            ))
        );
        assert_eq!(activation, activation_before);

        let mut removal = CombatState::initial_fixture();
        removal.relics.push(Relic::RedSkull);
        removal.player.hp = removal.player.max_hp;
        removal.player.powers.strength = i32::MIN;
        removal.relic_counters.red_skull_active = true;
        let removal_before = removal.clone();

        assert_eq!(
            sync_red_skull_strength(&mut removal),
            Err(SimError::InvalidState(
                "Red Skull Strength removal underflows i32"
            ))
        );
        assert_eq!(removal, removal_before);
    }

    #[test]
    fn red_skull_removal_failure_rolls_back_direct_combat_healing() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::RedSkull);
        state.player.hp = state.player.max_hp / 2;
        state.player.powers.strength = i32::MIN;
        state.relic_counters.red_skull_active = true;
        let before = state.clone();

        assert_eq!(
            heal_combat_player_with_relics(&mut state, 1),
            Err(SimError::InvalidState(
                "Red Skull Strength removal underflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn player_debuff_immunity_relics_do_not_consume_artifact() {
        let mut powers = PlayerPowers {
            artifact: 1,
            ..PlayerPowers::default()
        };

        assert_eq!(
            apply_player_weak_with_relics(&mut powers, &[Relic::Ginger], 1),
            Ok(false)
        );
        assert_eq!(
            apply_player_frail_with_relics(&mut powers, &[Relic::Turnip], 1),
            Ok(false)
        );
        assert_eq!(powers.artifact, 1);
        assert_eq!(powers.weak, 0);
        assert_eq!(powers.frail, 0);
    }

    #[test]
    fn relic_vulnerable_respects_artifact() {
        let mut powers = MonsterPowers {
            artifact: 1,
            ..MonsterPowers::default()
        };

        apply_monster_vulnerable_with_relics(
            &mut powers,
            &[Relic::BagOfMarbles, Relic::ChampionBelt],
            1,
        )
        .expect("Artifact-blocked Vulnerable is valid");

        assert_eq!(powers.artifact, 0);
        assert_eq!(powers.vulnerable, 0);
        assert_eq!(powers.weak, 0);
    }

    #[test]
    fn champion_belt_applies_weak_after_vulnerable_lands() {
        let mut powers = MonsterPowers::default();

        apply_monster_vulnerable_with_relics(&mut powers, &[Relic::ChampionBelt], 2)
            .expect("representable Vulnerable and Weak are valid");

        assert_eq!(powers.vulnerable, 2);
        assert_eq!(powers.weak, CHAMPION_BELT_WEAK);
    }

    #[test]
    fn champion_belt_weak_overflow_rolls_back_vulnerable() {
        let mut powers = MonsterPowers {
            weak: i32::MAX,
            ..MonsterPowers::default()
        };
        let before = powers;

        assert_eq!(
            apply_monster_vulnerable_with_relics(&mut powers, &[Relic::ChampionBelt], 2),
            Err(SimError::InvalidState(
                "monster Weak application overflows i32"
            ))
        );
        assert_eq!(powers, before);
    }

    #[test]
    fn former_key_only_relics_have_canonical_identity_and_content() {
        for relic in [
            Relic::MarkOfBloom,
            Relic::SpiritPoop,
            Relic::OddMushroom,
            Relic::NlothsGift,
        ] {
            assert_eq!(relic.key(), relic);
            assert_eq!(Relic::from_key(relic), Some(relic));
            assert_eq!(Relic::from_content_id(relic.content_id()), Some(relic));
        }
    }

    #[test]
    fn relic_key_alias_preserves_wire_names() {
        use RelicKey::MarkOfBloom;

        let key: RelicKey = MarkOfBloom;

        assert_eq!(
            serde_json::to_string(&key).expect("relic identity serializes"),
            r#""MarkOfBloom""#
        );
        assert_eq!(
            serde_json::from_str::<RelicKey>(r#""NlothsGift""#)
                .expect("legacy key name deserializes"),
            Relic::NlothsGift
        );
    }

    #[test]
    fn canonical_relic_metadata_is_complete_and_self_consistent() {
        assert_eq!(ALL_RELICS.len(), 157);
        let mut relics = Vec::new();
        let mut content_ids = Vec::new();
        let mut names = Vec::new();

        for relic in ALL_RELICS.iter().copied() {
            let definition = relic.definition();
            assert_eq!(definition.relic, relic);
            assert_eq!(Relic::from_content_id(definition.content_id), Some(relic));
            assert_eq!(Relic::from_trace_name(definition.trace_name), Some(relic));
            assert!(!relics.contains(&relic), "duplicate relic {relic:?}");
            assert!(
                !content_ids.contains(&definition.content_id),
                "duplicate content id for {relic:?}"
            );
            let normalized_name = normalize_relic_name(definition.trace_name);
            assert!(
                !names.contains(&normalized_name),
                "duplicate trace name for {relic:?}"
            );
            for alias in definition.aliases {
                assert_eq!(Relic::from_trace_name(alias), Some(relic), "alias {alias}");
            }
            relics.push(relic);
            content_ids.push(definition.content_id);
            names.push(normalized_name);
        }

        for (tier, pool) in [
            (RelicTier::Common, IRONCLAD_COMMON_RELIC_POOL.as_slice()),
            (RelicTier::Uncommon, IRONCLAD_UNCOMMON_RELIC_POOL.as_slice()),
            (RelicTier::Rare, IRONCLAD_RARE_RELIC_POOL.as_slice()),
            (RelicTier::Shop, IRONCLAD_SHOP_RELIC_POOL.as_slice()),
            (RelicTier::Boss, IRONCLAD_BOSS_RELIC_POOL.as_slice()),
        ] {
            for relic in pool {
                assert_eq!(relic.definition().tier, Some(tier), "{relic:?}");
            }
        }
    }

    #[test]
    fn relic_effect_status_does_not_treat_recognition_as_support() {
        assert_eq!(
            Relic::MarkOfBloom.definition().effect_status,
            RelicEffectStatus::Modeled
        );
        assert_eq!(
            Relic::SpiritPoop.definition().effect_status,
            RelicEffectStatus::IdentityOnly
        );
        assert_eq!(
            Relic::OddMushroom.definition().effect_status,
            RelicEffectStatus::Unsupported
        );
        assert_eq!(
            Relic::NlothsGift.definition().effect_status,
            RelicEffectStatus::Unsupported
        );
    }
}
