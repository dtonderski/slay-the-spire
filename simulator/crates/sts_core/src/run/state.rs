use crate::{
    card::{CardInstance, CardType},
    combat::state::BASE_PLAYER_ENERGY,
    combat::CombatState,
    content::ascension::AscensionConfig,
    content::cards::{
        card_type_and_rarity, get_card_definition, is_basic_starter_card, is_curse_content_id,
        upgrade_content_id,
    },
    content::character::IRONCLAD_A0_BASE_HP,
    content::shop_pool::colorless_discovery_card_choices,
    ids::{CardId, ContentId, MonsterId},
    map::{generate_target_fixed_map, milestone8_fixture, MapRunState, RoomKind, TargetMapAct},
    potion::{Potion, MAX_POTIONS},
    relic::{
        apply_start_of_combat_relics, initialize_ironclad_relic_pools, Relic, RelicKey,
        RelicPoolState, RelicSpawnContext, ANCIENT_TEA_SET_ENERGY, BUSTED_CROWN_ENERGY,
        CAULDRON_POTIONS, CERAMIC_FISH_GOLD, COFFEE_DRIPPER_ENERGY, DARKSTONE_PERIAPT_MAX_HP,
        DU_VU_DOLL_STRENGTH_PER_CURSE, ECTOPLASM_ENERGY, ETERNAL_FEATHER_HEAL_PER_FIVE_CARDS,
        FUSION_HAMMER_ENERGY, LEES_WAFFLE_MAX_HP, MANGO_MAX_HP, MARK_OF_PAIN_ENERGY, MAW_BANK_GOLD,
        OLD_COIN_GOLD, OMAMORI_CHARGES, ORRERY_CARD_REWARDS, PANTOGRAPH_HEAL, PEAR_MAX_HP,
        PHILOSOPHERS_STONE_ENERGY, PHILOSOPHERS_STONE_MONSTER_STRENGTH, POTION_BELT_SLOTS,
        PRESERVED_INSECT_HP_DENOMINATOR, PRESERVED_INSECT_HP_NUMERATOR, RUNIC_DOME_ENERGY,
        SLAVERS_COLLAR_ENERGY, SLING_OF_COURAGE_STRENGTH, SNECKO_EYE_ENERGY, SOZU_ENERGY,
        STRAWBERRY_MAX_HP, TINY_HOUSE_GOLD, TINY_HOUSE_HEAL, TINY_HOUSE_MAX_HP,
        VELVET_CHOKER_ENERGY, WING_BOOTS_CHARGES,
    },
    rng::JavaRng,
    rng::StsRng,
    SimError, SimResult,
};
use serde::{Deserialize, Serialize};

pub const STARTING_GOLD: i32 = 99;

fn default_energy_per_turn() -> i32 {
    BASE_PLAYER_ENERGY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        ANGER_ID, BATTLE_TRANCE_ID, CLEAVE_ID, FEEL_NO_PAIN_ID, INFLAME_ID, INFLAME_PLUS_ID,
        POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, SEEING_RED_ID, SEEING_RED_PLUS_ID,
        SHRUG_IT_OFF_ID, STRIKE_R_ID, WARCRY_ID, WOUND_ID,
    };
    use crate::ids::MapNodeId;

    #[test]
    fn ensure_ironclad_relic_pools_initializes_once_and_advances_counter() {
        let mut run = RunState::map_fixture();
        run.relic_rng_seed = 22_079_335_079;

        run.ensure_ironclad_relic_pools();
        let first = run.relic_pools.clone().expect("relic pools");

        assert_eq!(run.relic_rng_counter, 5);
        assert_eq!(first.common.first(), Some(&RelicKey::ToyOrnithopter));

        run.ensure_ironclad_relic_pools();

        assert_eq!(run.relic_rng_counter, 5);
        assert_eq!(run.relic_pools, Some(first));
    }

    #[test]
    fn rng_stream_accessors_preserve_flat_run_state_fields() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 100;
        run.card_rng_counter = 1;
        run.card_random_rng_counter = 2;
        run.event_rng_seed = 200;
        run.event_rng_counter = 3;
        run.merchant_rng_seed = 300;
        run.merchant_rng_counter = 4;
        run.misc_rng_seed = 400;
        run.misc_rng_counter = 5;
        run.potion_rng_seed = 500;
        run.potion_rng_counter = 6;
        run.relic_rng_seed = 600;
        run.relic_rng_counter = 7;
        run.treasure_rng_seed = 700;
        run.treasure_rng_counter = 8;
        run.current_floor = 9;

        assert_eq!(
            run.rng_stream_state(RunRngStream::CardReward),
            RunRngStreamState {
                seed: 100,
                counter: 1
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::CardRandom),
            RunRngStreamState {
                seed: 109,
                counter: 2
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Event),
            RunRngStreamState {
                seed: 200,
                counter: 3
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Merchant),
            RunRngStreamState {
                seed: 300,
                counter: 4
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Misc),
            RunRngStreamState {
                seed: 400,
                counter: 5
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Potion),
            RunRngStreamState {
                seed: 500,
                counter: 6
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Relic),
            RunRngStreamState {
                seed: 600,
                counter: 7
            }
        );
        assert_eq!(
            run.rng_stream_state(RunRngStream::Treasure),
            RunRngStreamState {
                seed: 700,
                counter: 8
            }
        );

        run.set_rng_stream_counter(RunRngStream::CardReward, 11);
        run.set_rng_stream_counter(RunRngStream::CardRandom, 12);
        run.set_rng_stream_counter(RunRngStream::Event, 13);
        run.set_rng_stream_counter(RunRngStream::Merchant, 14);
        run.set_rng_stream_counter(RunRngStream::Misc, 15);
        run.set_rng_stream_counter(RunRngStream::Potion, 16);
        run.set_rng_stream_counter(RunRngStream::Relic, 17);
        run.set_rng_stream_counter(RunRngStream::Treasure, 18);

        assert_eq!(run.card_rng_counter, 11);
        assert_eq!(run.card_random_rng_counter, 12);
        assert_eq!(run.event_rng_counter, 13);
        assert_eq!(run.merchant_rng_counter, 14);
        assert_eq!(run.misc_rng_counter, 15);
        assert_eq!(run.potion_rng_counter, 16);
        assert_eq!(run.relic_rng_counter, 17);
        assert_eq!(run.treasure_rng_counter, 18);
    }

    #[test]
    fn rng_stream_accessors_do_not_change_run_state_serialization_shape() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 100;
        run.card_rng_counter = 3;

        let value = serde_json::to_value(&run).expect("serialize run state");

        assert_eq!(value["reward_rng_seed"], 100);
        assert_eq!(value["card_rng_counter"], 3);
        assert!(value.get("rng_streams").is_none());
    }

    #[test]
    fn neow_lament_reward_applies_to_next_three_combats_only() {
        let mut run = RunState::map_fixture();
        crate::run::neow::apply_neow_lament_reward(&mut run);

        for expected_remaining in [2, 1, 0] {
            let combat = run.init_combat_consuming_relics(CombatState::initial_fixture());

            assert_eq!(combat.monsters[0].hp, 1);
            assert_eq!(run.neow_lament_combats_remaining, expected_remaining);
        }

        let combat = run.init_combat_consuming_relics(CombatState::initial_fixture());

        assert_eq!(combat.monsters[0].hp, 40);
        assert_eq!(run.neow_lament_combats_remaining, 0);
    }

    #[test]
    fn neow_lament_sets_all_current_monster_hp_to_one() {
        let mut run = RunState::map_fixture();
        crate::run::neow::apply_neow_lament_reward(&mut run);

        let combat = run.init_combat_consuming_relics(CombatState::sentry_fixture());

        assert_eq!(combat.monsters.len(), 3);
        assert!(combat.monsters.iter().all(|monster| monster.hp == 1));
        assert_eq!(run.neow_lament_combats_remaining, 2);
    }

    #[test]
    fn neow_lament_counter_round_trips_through_json_and_skips_zero() {
        let mut run = RunState::map_fixture();
        let value = serde_json::to_value(&run).expect("serialize default run state");

        assert!(value.get("neow_lament_combats_remaining").is_none());

        run.neow_lament_combats_remaining = 2;
        let json = serde_json::to_string(&run).expect("serialize run state");
        let restored: RunState = serde_json::from_str(&json).expect("deserialize run state");

        assert_eq!(restored.neow_lament_combats_remaining, 2);
    }

    #[test]
    fn neow_lament_counter_defaults_when_missing_from_json() {
        let value = serde_json::to_value(RunState::map_fixture()).expect("serialize run state");
        let run: RunState = serde_json::from_value(value).expect("deserialize run state");

        assert_eq!(run.neow_lament_combats_remaining, 0);
    }

    #[test]
    fn relic_spawn_context_uses_deck_and_owned_relics() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::CoffeeDripper];
        run.deck.push(CardInstance::new(CardId::new(500), ANGER_ID));
        run.deck
            .push(CardInstance::new(CardId::new(501), FEEL_NO_PAIN_ID));

        let context = run.relic_spawn_context(12, true);

        assert!(context.shop_room);
        assert_eq!(context.floor_num, 12);
        assert!(context.owned_relics.contains(&RelicKey::CoffeeDripper));
        assert!(context.has_non_basic_attack);
        assert!(context.has_power);
        assert!(!context.has_non_basic_skill);
    }

    #[test]
    fn relic_keys_map_for_implemented_relics() {
        assert_eq!(
            Relic::from_key(RelicKey::BurningBlood),
            Some(Relic::BurningBlood)
        );
        assert_eq!(Relic::from_key(Relic::Vajra.key()), Some(Relic::Vajra));
        assert_eq!(Relic::from_key(RelicKey::BloodVial), Some(Relic::BloodVial));
        assert_eq!(Relic::from_key(RelicKey::Pear), Some(Relic::Pear));
        assert_eq!(Relic::from_key(RelicKey::Mango), Some(Relic::Mango));
        assert_eq!(Relic::from_key(RelicKey::OldCoin), Some(Relic::OldCoin));
        assert_eq!(
            Relic::from_key(RelicKey::LeesWaffle),
            Some(Relic::LeesWaffle)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PotionBelt),
            Some(Relic::PotionBelt)
        );
        assert_eq!(Relic::from_key(RelicKey::Lantern), Some(Relic::Lantern));
        assert_eq!(
            Relic::from_key(RelicKey::BagOfPreparation),
            Some(Relic::BagOfPreparation)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BagOfMarbles),
            Some(Relic::BagOfMarbles)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BronzeScales),
            Some(Relic::BronzeScales)
        );
        assert_eq!(
            Relic::from_key(RelicKey::ThreadAndNeedle),
            Some(Relic::ThreadAndNeedle)
        );
        assert_eq!(Relic::from_key(RelicKey::RedSkull), Some(Relic::RedSkull));
        assert_eq!(Relic::from_key(RelicKey::Nunchaku), Some(Relic::Nunchaku));
        assert_eq!(Relic::from_key(RelicKey::ArtOfWar), Some(Relic::ArtOfWar));
        assert_eq!(Relic::from_key(RelicKey::Shuriken), Some(Relic::Shuriken));
        assert_eq!(Relic::from_key(RelicKey::Kunai), Some(Relic::Kunai));
        assert_eq!(
            Relic::from_key(RelicKey::LetterOpener),
            Some(Relic::LetterOpener)
        );
        assert_eq!(
            Relic::from_key(RelicKey::HappyFlower),
            Some(Relic::HappyFlower)
        );
        assert_eq!(
            Relic::from_key(RelicKey::Orichalcum),
            Some(Relic::Orichalcum)
        );
        assert_eq!(Relic::from_key(RelicKey::HornCleat), Some(Relic::HornCleat));
        assert_eq!(
            Relic::from_key(RelicKey::CaptainsWheel),
            Some(Relic::CaptainsWheel)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MercuryHourglass),
            Some(Relic::MercuryHourglass)
        );
        assert_eq!(
            Relic::from_key(RelicKey::StoneCalendar),
            Some(Relic::StoneCalendar)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MeatOnTheBone),
            Some(Relic::MeatOnTheBone)
        );
        assert_eq!(
            Relic::from_key(RelicKey::QuestionCard),
            Some(Relic::QuestionCard)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BlackBlood),
            Some(Relic::BlackBlood)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MealTicket),
            Some(Relic::MealTicket)
        );
        assert_eq!(
            Relic::from_key(RelicKey::RegalPillow),
            Some(Relic::RegalPillow)
        );
        assert_eq!(
            Relic::from_key(RelicKey::DreamCatcher),
            Some(Relic::DreamCatcher)
        );
        assert_eq!(
            Relic::from_key(RelicKey::EternalFeather),
            Some(Relic::EternalFeather)
        );
        assert_eq!(Relic::from_key(RelicKey::Torii), Some(Relic::Torii));
        assert_eq!(
            Relic::from_key(RelicKey::TungstenRod),
            Some(Relic::TungstenRod)
        );
        assert_eq!(
            Relic::from_key(RelicKey::CeramicFish),
            Some(Relic::CeramicFish)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MembershipCard),
            Some(Relic::MembershipCard)
        );
        assert_eq!(
            Relic::from_key(RelicKey::SmilingMask),
            Some(Relic::SmilingMask)
        );
        assert_eq!(
            Relic::from_key(RelicKey::Pantograph),
            Some(Relic::Pantograph)
        );
        assert_eq!(Relic::from_key(RelicKey::Ginger), Some(Relic::Ginger));
        assert_eq!(Relic::from_key(RelicKey::Turnip), Some(Relic::Turnip));
        assert_eq!(
            Relic::from_key(RelicKey::MarkOfPain),
            Some(Relic::MarkOfPain)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MagicFlower),
            Some(Relic::MagicFlower)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PaperPhrog),
            Some(Relic::PaperPhrog)
        );
        assert_eq!(
            Relic::from_key(RelicKey::ChampionBelt),
            Some(Relic::ChampionBelt)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PreservedInsect),
            Some(Relic::PreservedInsect)
        );
        assert_eq!(Relic::from_key(RelicKey::Omamori), Some(Relic::Omamori));
        assert_eq!(
            Relic::from_key(RelicKey::SlingOfCourage),
            Some(Relic::SlingOfCourage)
        );
        assert_eq!(Relic::from_key(RelicKey::MawBank), Some(Relic::MawBank));
        assert_eq!(
            Relic::from_key(RelicKey::AncientTeaSet),
            Some(Relic::AncientTeaSet)
        );
        assert_eq!(Relic::from_key(RelicKey::Calipers), Some(Relic::Calipers));
        assert_eq!(
            Relic::from_key(RelicKey::SingingBowl),
            Some(Relic::SingingBowl)
        );
        assert_eq!(Relic::from_key(RelicKey::ChemicalX), Some(Relic::ChemicalX));
        assert_eq!(
            Relic::from_key(RelicKey::PhilosophersStone),
            Some(Relic::PhilosophersStone)
        );
        assert_eq!(
            Relic::from_key(RelicKey::SlaversCollar),
            Some(Relic::SlaversCollar)
        );
        assert_eq!(Relic::from_key(RelicKey::Ectoplasm), Some(Relic::Ectoplasm));
        assert_eq!(Relic::from_key(RelicKey::RunicDome), Some(Relic::RunicDome));
        assert_eq!(
            Relic::from_key(RelicKey::StrikeDummy),
            Some(Relic::StrikeDummy)
        );
        assert_eq!(Relic::from_key(RelicKey::Brimstone), Some(Relic::Brimstone));
        assert_eq!(
            Relic::from_key(RelicKey::WhiteBeastStatue),
            Some(Relic::WhiteBeastStatue)
        );
        assert_eq!(Relic::from_key(RelicKey::Whetstone), Some(Relic::Whetstone));
        assert_eq!(Relic::from_key(RelicKey::WarPaint), Some(Relic::WarPaint));
        assert_eq!(Relic::from_key(RelicKey::Akabeko), Some(Relic::Akabeko));
        assert_eq!(
            Relic::from_key(RelicKey::CentennialPuzzle),
            Some(Relic::CentennialPuzzle)
        );
        assert_eq!(Relic::from_key(RelicKey::PenNib), Some(Relic::PenNib));
        assert_eq!(
            Relic::from_key(RelicKey::SelfFormingClay),
            Some(Relic::SelfFormingClay)
        );
        assert_eq!(
            Relic::from_key(RelicKey::ClockworkSouvenir),
            Some(Relic::ClockworkSouvenir)
        );
        assert_eq!(Relic::from_key(RelicKey::RunicCube), Some(Relic::RunicCube));
        assert_eq!(Relic::from_key(RelicKey::TheAbacus), Some(Relic::TheAbacus));
        assert_eq!(
            Relic::from_key(RelicKey::GremlinHorn),
            Some(Relic::GremlinHorn)
        );
        assert_eq!(Relic::from_key(RelicKey::Sundial), Some(Relic::Sundial));
        assert_eq!(
            Relic::from_key(RelicKey::CharonsAshes),
            Some(Relic::CharonsAshes)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BlueCandle),
            Some(Relic::BlueCandle)
        );
        assert_eq!(
            Relic::from_key(RelicKey::MedicalKit),
            Some(Relic::MedicalKit)
        );
        assert_eq!(
            Relic::from_key(RelicKey::LizardTail),
            Some(Relic::LizardTail)
        );
        assert_eq!(
            Relic::from_key(RelicKey::Pocketwatch),
            Some(Relic::Pocketwatch)
        );
        assert_eq!(Relic::from_key(RelicKey::HandDrill), Some(Relic::HandDrill));
        assert_eq!(Relic::from_key(RelicKey::Circlet), Some(Relic::Circlet));
        assert_eq!(
            Relic::from_key(RelicKey::RedCirclet),
            Some(Relic::RedCirclet)
        );
        assert_eq!(
            Relic::from_key(RelicKey::SacredBark),
            Some(Relic::SacredBark)
        );
        assert_eq!(
            Relic::from_key(RelicKey::RunicPyramid),
            Some(Relic::RunicPyramid)
        );
        assert_eq!(Relic::from_key(RelicKey::FrozenEye), Some(Relic::FrozenEye));
        assert_eq!(Relic::from_key(RelicKey::PeacePipe), Some(Relic::PeacePipe));
        assert_eq!(
            Relic::from_key(RelicKey::OrangePellets),
            Some(Relic::OrangePellets)
        );
        assert_eq!(Relic::from_key(RelicKey::Girya), Some(Relic::Girya));
        assert_eq!(
            Relic::from_key(RelicKey::UnceasingTop),
            Some(Relic::UnceasingTop)
        );
        assert_eq!(Relic::from_key(RelicKey::Shovel), Some(Relic::Shovel));
        assert_eq!(
            Relic::from_key(RelicKey::FossilizedHelix),
            Some(Relic::FossilizedHelix)
        );
        assert_eq!(Relic::from_key(RelicKey::BlackStar), Some(Relic::BlackStar));
        assert_eq!(
            Relic::from_key(RelicKey::Matryoshka),
            Some(Relic::Matryoshka)
        );
        assert_eq!(Relic::from_key(RelicKey::EmptyCage), Some(Relic::EmptyCage));
        assert_eq!(
            Relic::from_key(RelicKey::BottledFlame),
            Some(Relic::BottledFlame)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BottledLightning),
            Some(Relic::BottledLightning)
        );
        assert_eq!(
            Relic::from_key(RelicKey::BottledTornado),
            Some(Relic::BottledTornado)
        );
        assert_eq!(
            Relic::from_key(RelicKey::DollysMirror),
            Some(Relic::DollysMirror)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PrayerWheel),
            Some(Relic::PrayerWheel)
        );
        assert_eq!(
            Relic::from_key(RelicKey::CrackedCore),
            Some(Relic::CrackedCore)
        );
        assert_eq!(
            Relic::from_key(RelicKey::FrozenCore),
            Some(Relic::FrozenCore)
        );
        assert_eq!(Relic::from_key(RelicKey::PureWater), Some(Relic::PureWater));
        assert_eq!(Relic::from_key(RelicKey::HolyWater), Some(Relic::HolyWater));
        assert_eq!(
            Relic::from_key(RelicKey::RingOfTheSnake),
            Some(Relic::RingOfTheSnake)
        );
        assert_eq!(
            Relic::from_key(RelicKey::RingOfTheSerpent),
            Some(Relic::RingOfTheSerpent)
        );
        assert_eq!(Relic::from_key(RelicKey::Cauldron), Some(Relic::Cauldron));
        assert_eq!(Relic::from_key(RelicKey::TinyHouse), Some(Relic::TinyHouse));
        assert_eq!(
            Relic::from_key(RelicKey::DarkstonePeriapt),
            Some(Relic::DarkstonePeriapt)
        );
        assert_eq!(Relic::from_key(RelicKey::DuVuDoll), Some(Relic::DuVuDoll));
        assert_eq!(
            Relic::from_key(RelicKey::FusionHammer),
            Some(Relic::FusionHammer)
        );
        assert_eq!(Relic::from_key(RelicKey::Sozu), Some(Relic::Sozu));
        assert_eq!(
            Relic::from_key(RelicKey::BustedCrown),
            Some(Relic::BustedCrown)
        );
        assert_eq!(
            Relic::from_key(RelicKey::VelvetChoker),
            Some(Relic::VelvetChoker)
        );
        assert_eq!(
            Relic::from_key(RelicKey::ToyOrnithopter),
            Some(Relic::ToyOrnithopter)
        );
        assert_eq!(Relic::from_key(RelicKey::MoltenEgg), Some(Relic::MoltenEgg));
        assert_eq!(Relic::from_key(RelicKey::ToxicEgg), Some(Relic::ToxicEgg));
        assert_eq!(Relic::from_key(RelicKey::FrozenEgg), Some(Relic::FrozenEgg));
        assert_eq!(Relic::from_key(RelicKey::TheBoot), Some(Relic::TheBoot));
        assert_eq!(
            Relic::from_key(RelicKey::BirdFacedUrn),
            Some(Relic::BirdFacedUrn)
        );
        assert_eq!(
            Relic::from_key(RelicKey::TheCourier),
            Some(Relic::TheCourier)
        );
        assert_eq!(
            Relic::from_key(RelicKey::IncenseBurner),
            Some(Relic::IncenseBurner)
        );
        assert_eq!(Relic::from_key(RelicKey::CursedKey), Some(Relic::CursedKey));
        assert_eq!(Relic::from_key(RelicKey::TinyChest), Some(Relic::TinyChest));
        assert_eq!(Relic::from_key(RelicKey::Orrery), Some(Relic::Orrery));
        assert_eq!(Relic::from_key(RelicKey::SneckoEye), Some(Relic::SneckoEye));
        assert_eq!(
            Relic::from_key(RelicKey::StrangeSpoon),
            Some(Relic::StrangeSpoon)
        );
        assert_eq!(Relic::from_key(RelicKey::WingBoots), Some(Relic::WingBoots));
        assert_eq!(
            Relic::from_key(RelicKey::CallingBell),
            Some(Relic::CallingBell)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PandorasBox),
            Some(Relic::PandorasBox)
        );
        assert_eq!(Relic::from_key(RelicKey::Astrolabe), Some(Relic::Astrolabe));
        assert_eq!(
            Relic::from_key(RelicKey::GamblingChip),
            Some(Relic::GamblingChip)
        );
        assert_eq!(Relic::from_key(RelicKey::Toolbox), Some(Relic::Toolbox));
        assert_eq!(
            Relic::from_key(RelicKey::JuzuBracelet),
            Some(Relic::JuzuBracelet)
        );
        assert_eq!(
            Relic::from_key(RelicKey::PrismaticShard),
            Some(Relic::PrismaticShard)
        );
    }

    #[test]
    fn incense_burner_counter_persists_from_combat_entry() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::IncenseBurner]);
        run.incense_burner_counter = 5;

        let combat = run.init_combat_consuming_relics(CombatState::initial_fixture());

        assert_eq!(combat.relic_counters.incense_burner_counter, 0);
        assert_eq!(combat.player.powers.intangible, 1);
        assert_eq!(run.incense_burner_counter, 0);
    }

    #[test]
    fn pickup_hp_relics_apply_immediately() {
        let mut run = RunState::map_fixture();
        run.player_hp = 40;

        run.gain_relic(Relic::Pear);
        assert_eq!(run.player_max_hp, IRONCLAD_A0_BASE_HP + PEAR_MAX_HP);
        assert_eq!(run.player_hp, 40 + PEAR_MAX_HP);

        run.gain_relic(Relic::Mango);
        assert_eq!(
            run.player_max_hp,
            IRONCLAD_A0_BASE_HP + PEAR_MAX_HP + MANGO_MAX_HP
        );
        assert_eq!(run.player_hp, 40 + PEAR_MAX_HP + MANGO_MAX_HP);

        run.player_hp = 12;
        run.gain_relic(Relic::LeesWaffle);
        assert_eq!(
            run.player_max_hp,
            IRONCLAD_A0_BASE_HP + PEAR_MAX_HP + MANGO_MAX_HP + LEES_WAFFLE_MAX_HP
        );
        assert_eq!(run.player_hp, run.player_max_hp);
    }

    #[test]
    fn old_coin_grants_gold_on_pickup() {
        let mut run = RunState::map_fixture();
        let gold_before = run.gold;

        run.gain_relic(Relic::OldCoin);

        assert_eq!(run.gold, gold_before + OLD_COIN_GOLD);
    }

    #[test]
    fn ectoplasm_grants_energy_and_blocks_gold_gain() {
        let mut run = RunState::map_fixture();
        let gold_before = run.gold;

        run.gain_relic(Relic::Ectoplasm);

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY + ECTOPLASM_ENERGY);
        run.gain_gold(25);
        assert_eq!(run.gold, gold_before);
    }

    #[test]
    fn runic_dome_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::RunicDome);

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY + RUNIC_DOME_ENERGY);
        let combat = run.init_combat(CombatState::initial_fixture());
        assert_eq!(
            combat.player.max_energy,
            BASE_PLAYER_ENERGY + RUNIC_DOME_ENERGY
        );
        assert_eq!(combat.player.energy, BASE_PLAYER_ENERGY + RUNIC_DOME_ENERGY);
    }

    #[test]
    fn ectoplasm_blocks_relic_gold_gain() {
        let mut run = RunState::map_fixture();
        let gold_before = run.gold;
        run.relics = vec![Relic::Ectoplasm];

        run.gain_relic(Relic::OldCoin);
        run.gain_relic(Relic::CeramicFish);
        run.gain_deck_card(ANGER_ID);

        assert_eq!(run.gold, gold_before);
    }

    #[test]
    fn ceramic_fish_grants_gold_when_adding_cards_to_deck() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::CeramicFish];
        let gold_before = run.gold;

        run.gain_deck_card(ANGER_ID);

        assert_eq!(run.gold, gold_before + CERAMIC_FISH_GOLD);
    }

    #[test]
    fn darkstone_periapt_grants_max_hp_when_adding_curse() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::DarkstonePeriapt];
        let hp_before = run.player_hp;
        let max_hp_before = run.player_max_hp;

        run.gain_deck_card(crate::content::cards::REGRET_ID);

        assert_eq!(run.player_max_hp, max_hp_before + DARKSTONE_PERIAPT_MAX_HP);
        assert_eq!(run.player_hp, hp_before + DARKSTONE_PERIAPT_MAX_HP);
    }

    #[test]
    fn darkstone_periapt_ignores_status_cards_that_are_not_curses() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::DarkstonePeriapt];
        let hp_before = run.player_hp;
        let max_hp_before = run.player_max_hp;

        run.gain_deck_card(WOUND_ID);

        assert_eq!(run.player_max_hp, max_hp_before);
        assert_eq!(run.player_hp, hp_before);
    }

    #[test]
    fn omamori_prevents_next_two_curses_from_entering_deck() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Omamori];
        let deck_len_before = run.deck.len();

        run.gain_deck_card(crate::content::cards::REGRET_ID);
        run.gain_deck_card(crate::content::cards::DOUBT_ID);

        assert_eq!(run.deck.len(), deck_len_before);
        assert_eq!(run.omamori_charges_used, OMAMORI_CHARGES);
    }

    #[test]
    fn omamori_allows_third_curse_after_two_preventions() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Omamori];

        run.gain_deck_card(crate::content::cards::REGRET_ID);
        run.gain_deck_card(crate::content::cards::DOUBT_ID);
        run.gain_deck_card(crate::content::cards::REGRET_ID);

        assert_eq!(
            run.count_content_in_deck(crate::content::cards::REGRET_ID),
            1
        );
        assert_eq!(run.omamori_charges_used, OMAMORI_CHARGES);
    }

    #[test]
    fn omamori_does_not_consume_charge_on_non_curse_card() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Omamori];

        run.gain_deck_card(ANGER_ID);

        assert_eq!(run.count_content_in_deck(ANGER_ID), 1);
        assert_eq!(run.omamori_charges_used, 0);
    }

    #[test]
    fn omamori_prevented_curse_skips_card_added_relic_hooks() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Omamori, Relic::DarkstonePeriapt, Relic::CeramicFish];
        let hp_before = run.player_hp;
        let max_hp_before = run.player_max_hp;
        let gold_before = run.gold;

        run.gain_deck_card(crate::content::cards::REGRET_ID);

        assert_eq!(
            run.count_content_in_deck(crate::content::cards::REGRET_ID),
            0
        );
        assert_eq!(run.player_max_hp, max_hp_before);
        assert_eq!(run.player_hp, hp_before);
        assert_eq!(run.gold, gold_before);
    }

    #[test]
    fn egg_relics_upgrade_matching_card_types_when_added_to_deck() {
        let mut run = RunState::map_fixture();
        run.gain_relic(Relic::MoltenEgg);
        run.gain_relic(Relic::ToxicEgg);
        run.gain_relic(Relic::FrozenEgg);

        run.gain_deck_card(POMMEL_STRIKE_ID);
        run.gain_deck_card(SEEING_RED_ID);
        run.gain_deck_card(INFLAME_ID);

        let added = &run.deck[run.deck.len() - 3..];
        assert_eq!(added[0].content_id, POMMEL_STRIKE_PLUS_ID);
        assert_eq!(added[1].content_id, SEEING_RED_PLUS_ID);
        assert_eq!(added[2].content_id, INFLAME_PLUS_ID);
    }

    #[test]
    fn egg_relics_leave_mismatched_card_types_unchanged() {
        let mut run = RunState::map_fixture();
        run.gain_relic(Relic::ToxicEgg);

        run.gain_deck_card(POMMEL_STRIKE_ID);

        assert_eq!(
            run.deck.last().expect("added card").content_id,
            POMMEL_STRIKE_ID
        );
    }

    #[test]
    fn whetstone_pickup_upgrades_two_random_non_starter_attacks() {
        let mut run = RunState::map_fixture();
        run.deck.clear();
        run.deck.push(CardInstance::new(CardId::new(500), ANGER_ID));
        run.deck
            .push(CardInstance::new(CardId::new(501), CLEAVE_ID));
        run.deck
            .push(CardInstance::new(CardId::new(502), POMMEL_STRIKE_ID));
        run.deck
            .push(CardInstance::new(CardId::new(503), SHRUG_IT_OFF_ID));

        run.gain_relic(Relic::Whetstone);

        let upgraded_attacks = run
            .deck
            .iter()
            .filter(|card| {
                matches!(
                    card.content_id,
                    crate::content::cards::ANGER_PLUS_ID
                        | crate::content::cards::CLEAVE_PLUS_ID
                        | crate::content::cards::POMMEL_STRIKE_PLUS_ID
                )
            })
            .count();
        assert_eq!(upgraded_attacks, 2);
        assert_eq!(run.card_random_rng_counter, 0);
        assert_eq!(run.misc_rng_counter, 1);
    }

    #[test]
    fn whetstone_pickup_can_upgrade_starter_strikes() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::Whetstone);

        assert_eq!(
            run.count_content_in_deck(crate::content::cards::STRIKE_R_PLUS_ID),
            2
        );
        assert_eq!(run.count_content_in_deck(STRIKE_R_ID), 3);
        assert_eq!(run.misc_rng_counter, 1);
    }

    #[test]
    fn whetstone_pickup_without_valid_attacks_does_not_consume_rng() {
        let mut run = RunState::map_fixture();
        run.deck.clear();
        run.deck
            .push(CardInstance::new(CardId::new(500), SHRUG_IT_OFF_ID));

        run.gain_relic(Relic::Whetstone);

        assert_eq!(run.count_content_in_deck(STRIKE_R_ID), 0);
        assert_eq!(run.count_content_in_deck(SHRUG_IT_OFF_ID), 1);
        assert_eq!(run.misc_rng_counter, 0);
    }

    #[test]
    fn war_paint_pickup_upgrades_two_random_skills() {
        let mut run = RunState::map_fixture();
        run.deck.clear();
        run.deck
            .push(CardInstance::new(CardId::new(500), BATTLE_TRANCE_ID));
        run.deck
            .push(CardInstance::new(CardId::new(501), SEEING_RED_ID));
        run.deck
            .push(CardInstance::new(CardId::new(502), WARCRY_ID));
        run.deck.push(CardInstance::new(CardId::new(503), ANGER_ID));

        run.gain_relic(Relic::WarPaint);

        let upgraded_skills = run
            .deck
            .iter()
            .filter(|card| {
                matches!(
                    card.content_id,
                    crate::content::cards::BATTLE_TRANCE_PLUS_ID
                        | crate::content::cards::SEEING_RED_PLUS_ID
                        | crate::content::cards::WARCRY_PLUS_ID
                )
            })
            .count();
        assert_eq!(upgraded_skills, 2);
        assert_eq!(run.card_random_rng_counter, 0);
        assert_eq!(run.misc_rng_counter, 1);
    }

    #[test]
    fn war_paint_pickup_without_valid_skills_does_not_consume_rng() {
        let mut run = RunState::map_fixture();
        run.deck.clear();
        run.deck.push(CardInstance::new(CardId::new(500), ANGER_ID));

        run.gain_relic(Relic::WarPaint);

        assert_eq!(run.count_content_in_deck(ANGER_ID), 1);
        assert_eq!(run.misc_rng_counter, 0);
    }

    #[test]
    fn pantograph_heals_at_boss_combat_start() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").current_node = MapNodeId::new(6);
        run.player_hp = 20;
        run.relics = vec![Relic::Pantograph];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.hp, 20 + PANTOGRAPH_HEAL);
    }

    #[test]
    fn magic_flower_increases_pantograph_boss_combat_healing() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").current_node = MapNodeId::new(6);
        run.player_hp = 20;
        run.relics = vec![Relic::Pantograph, Relic::MagicFlower];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.hp, 20 + 38);
    }

    #[test]
    fn pantograph_does_not_heal_non_boss_combat() {
        let mut run = RunState::map_fixture();
        run.player_hp = 20;
        run.relics = vec![Relic::Pantograph];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.hp, 20);
    }

    #[test]
    fn preserved_insect_reduces_elite_monster_hp_on_combat_start() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").map.nodes[0].room_kind = RoomKind::Elite;
        run.relics = vec![Relic::PreservedInsect];
        let base = CombatState::initial_fixture();
        let base_hp = base.monsters[0].hp;

        let combat = run.init_combat(base);
        let expected = base_hp * PRESERVED_INSECT_HP_NUMERATOR / PRESERVED_INSECT_HP_DENOMINATOR;

        assert_eq!(combat.monsters[0].hp, expected);
    }

    #[test]
    fn preserved_insect_does_not_apply_outside_elite_rooms() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::PreservedInsect];
        let base = CombatState::initial_fixture();
        let base_hp = base.monsters[0].hp;

        let combat = run.init_combat(base);

        assert_eq!(combat.monsters[0].hp, base_hp);
    }

    #[test]
    fn preserved_insect_keeps_one_hp_monsters_alive() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").map.nodes[0].room_kind = RoomKind::Elite;
        run.relics = vec![Relic::PreservedInsect];
        let mut base = CombatState::initial_fixture();
        base.monsters[0].hp = 1;

        let combat = run.init_combat(base);

        assert_eq!(combat.monsters[0].hp, 1);
    }

    #[test]
    fn sling_of_courage_grants_strength_in_elite_combat() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").map.nodes[0].room_kind = RoomKind::Elite;
        run.relics = vec![Relic::SlingOfCourage];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.powers.strength, SLING_OF_COURAGE_STRENGTH);
    }

    #[test]
    fn sling_of_courage_does_not_apply_outside_elite_rooms() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::SlingOfCourage];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.powers.strength, 0);
    }

    #[test]
    fn slavers_collar_grants_energy_in_elite_combat() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").map.nodes[0].room_kind = RoomKind::Elite;
        run.relics = vec![Relic::SlaversCollar];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            combat.player.max_energy,
            run.energy_per_turn + SLAVERS_COLLAR_ENERGY
        );
        assert_eq!(
            combat.player.energy,
            run.energy_per_turn + SLAVERS_COLLAR_ENERGY
        );
    }

    #[test]
    fn slavers_collar_grants_energy_in_boss_combat() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map").current_node = MapNodeId::new(6);
        run.relics = vec![Relic::SlaversCollar];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            combat.player.max_energy,
            run.energy_per_turn + SLAVERS_COLLAR_ENERGY
        );
        assert_eq!(
            combat.player.energy,
            run.energy_per_turn + SLAVERS_COLLAR_ENERGY
        );
    }

    #[test]
    fn slavers_collar_does_not_grant_energy_in_normal_combat() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::SlaversCollar];

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn ancient_tea_set_grants_energy_when_armed_for_next_combat() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::AncientTeaSet];
        run.ancient_tea_set_armed = true;

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            combat.player.energy,
            run.energy_per_turn + ANCIENT_TEA_SET_ENERGY
        );
    }

    #[test]
    fn ancient_tea_set_combat_entry_consumes_armed_flag() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::AncientTeaSet];
        run.ancient_tea_set_armed = true;

        let combat = run.init_combat_consuming_relics(CombatState::initial_fixture());

        assert_eq!(
            combat.player.energy,
            run.energy_per_turn + ANCIENT_TEA_SET_ENERGY
        );
        assert!(!run.ancient_tea_set_armed);
    }

    #[test]
    fn du_vu_doll_grants_strength_per_curse_at_combat_start() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::DuVuDoll];
        run.gain_deck_card(crate::content::cards::REGRET_ID);
        run.gain_deck_card(crate::content::cards::DOUBT_ID);
        run.gain_deck_card(WOUND_ID);

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            combat.player.powers.strength,
            2 * DU_VU_DOLL_STRENGTH_PER_CURSE
        );
    }

    #[test]
    fn girya_grants_strength_per_lift_at_combat_start() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Girya];
        run.girya_lifts = 3;

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(combat.player.powers.strength, 3);
    }

    #[test]
    fn potion_belt_increases_potion_capacity() {
        let mut run = RunState::map_fixture();

        assert_eq!(run.potion_capacity(), MAX_POTIONS);
        run.gain_relic(Relic::PotionBelt);

        assert_eq!(run.potion_capacity(), MAX_POTIONS + POTION_BELT_SLOTS);
    }

    #[test]
    fn mark_of_pain_pickup_adds_energy_without_permanent_wounds() {
        let mut run = RunState::map_fixture();
        let deck_len = run.deck.len();

        run.gain_relic(Relic::MarkOfPain);

        assert_eq!(
            run.energy_per_turn,
            BASE_PLAYER_ENERGY + MARK_OF_PAIN_ENERGY
        );
        assert_eq!(run.deck.len(), deck_len);
        assert_eq!(run.count_content_in_deck(WOUND_ID), 0);
    }

    #[test]
    fn fusion_hammer_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::FusionHammer);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            run.energy_per_turn,
            BASE_PLAYER_ENERGY + FUSION_HAMMER_ENERGY
        );
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn cursed_key_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::CursedKey);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY + 1);
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn sozu_pickup_adds_energy_and_blocks_potion_gain() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::Sozu);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY + SOZU_ENERGY);
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
        assert!(!run.can_gain_potions());
    }

    #[test]
    fn cauldron_pickup_fills_open_potion_slots() {
        let mut run = RunState::map_fixture();
        run.potion_rng_seed = 1_218_623;
        run.potion_rng_counter = 0;
        run.potions.push(Potion::Fire);
        let counter_before = run.potion_rng_counter;

        run.gain_relic(Relic::Cauldron);

        assert_eq!(run.potions.len(), run.potion_capacity());
        assert_eq!(run.potions[0], Potion::Fire);
        assert!(run.potion_rng_counter > counter_before);
    }

    #[test]
    fn cauldron_respects_potion_belt_capacity() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::PotionBelt);

        run.gain_relic(Relic::Cauldron);

        assert_eq!(run.potions.len(), CAULDRON_POTIONS);
        assert_eq!(run.potions.len(), run.potion_capacity());
    }

    #[test]
    fn cauldron_does_not_roll_when_slots_are_full() {
        let mut run = RunState::map_fixture();
        run.potions = vec![Potion::Fire, Potion::Block, Potion::Fairy];
        run.potion_rng_counter = 7;

        run.gain_relic(Relic::Cauldron);

        assert_eq!(
            run.potions,
            vec![Potion::Fire, Potion::Block, Potion::Fairy]
        );
        assert_eq!(run.potion_rng_counter, 7);
    }

    #[test]
    fn cauldron_does_not_roll_when_sozu_blocks_potions() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::Sozu);
        run.potion_rng_counter = 7;

        run.gain_relic(Relic::Cauldron);

        assert!(run.potions.is_empty());
        assert_eq!(run.potion_rng_counter, 7);
    }

    #[test]
    fn tiny_house_pickup_applies_bundle_and_pending_card_reward() {
        let mut run = RunState::map_fixture();
        run.player_hp = 60;
        run.gold = 10;
        run.misc_rng_seed = 1_218_623;
        run.reward = Some(RewardScreen {
            choices: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            relic_offer: Some(Relic::TinyHouse),
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });

        run.gain_relic(Relic::TinyHouse);

        assert_eq!(run.player_max_hp, IRONCLAD_A0_BASE_HP + TINY_HOUSE_MAX_HP);
        assert_eq!(run.player_hp, 60 + TINY_HOUSE_MAX_HP + TINY_HOUSE_HEAL);
        assert_eq!(run.gold, 10 + TINY_HOUSE_GOLD);
        assert!(run
            .deck
            .iter()
            .any(|card| card.content_id == crate::content::cards::STRIKE_R_PLUS_ID));
        assert_eq!(
            run.reward
                .as_ref()
                .expect("reward")
                .pending_card_reward_count(),
            1
        );
        assert_eq!(run.misc_rng_counter, 1);
    }

    #[test]
    fn tiny_house_heal_caps_at_new_max_hp() {
        let mut run = RunState::map_fixture();
        run.player_hp = run.player_max_hp;

        run.gain_relic(Relic::TinyHouse);

        assert_eq!(run.player_max_hp, IRONCLAD_A0_BASE_HP + TINY_HOUSE_MAX_HP);
        assert_eq!(run.player_hp, run.player_max_hp);
    }

    #[test]
    fn busted_crown_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::BustedCrown);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            run.energy_per_turn,
            BASE_PLAYER_ENERGY + BUSTED_CROWN_ENERGY
        );
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn snecko_eye_pickup_adds_energy_and_combat_card_random_rng() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 123;
        run.current_floor = 4;
        run.card_random_rng_counter = 2;

        run.gain_relic(Relic::SneckoEye);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY + SNECKO_EYE_ENERGY);
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
        assert_eq!(
            combat.card_random_rng.as_ref().expect("card rng").counter(),
            run.card_random_rng_counter
        );
    }

    #[test]
    fn velvet_choker_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::VelvetChoker);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            run.energy_per_turn,
            BASE_PLAYER_ENERGY + VELVET_CHOKER_ENERGY
        );
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn philosophers_stone_pickup_adds_energy_for_combat() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::PhilosophersStone);
        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            run.energy_per_turn,
            BASE_PLAYER_ENERGY + PHILOSOPHERS_STONE_ENERGY
        );
        assert_eq!(combat.player.max_energy, run.energy_per_turn);
        assert_eq!(combat.player.energy, run.energy_per_turn);
    }

    #[test]
    fn philosophers_stone_grants_strength_to_all_monsters_at_combat_start() {
        let mut run = RunState::map_fixture();
        run.gain_relic(Relic::PhilosophersStone);
        let mut base = CombatState::initial_fixture();
        base.monsters.push(crate::content::monsters::monster_state(
            &crate::content::monsters::FIXED_SIMPLE_MONSTER,
            MonsterId::new(2),
        ));
        base.monsters[1].powers.strength = 2;

        let combat = run.init_combat(base);

        assert_eq!(
            combat.monsters[0].powers.strength,
            PHILOSOPHERS_STONE_MONSTER_STRENGTH
        );
        assert_eq!(
            combat.monsters[1].powers.strength,
            2 + PHILOSOPHERS_STONE_MONSTER_STRENGTH
        );
    }

    #[test]
    fn gain_relic_key_promotes_start_combat_relics_to_modeled_relics() {
        let mut run = RunState::map_fixture();

        run.gain_relic_key(RelicKey::Lantern);

        assert_eq!(run.relics, vec![Relic::Lantern]);
        assert!(run.relic_keys.is_empty());
    }

    #[test]
    fn gambling_chip_opens_start_of_combat_discard_selection() {
        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::GamblingChip);

        let combat = run.init_combat(CombatState::initial_fixture());

        assert_eq!(
            combat
                .exhaust_select
                .as_ref()
                .expect("Gambling Chip selection")
                .purpose,
            crate::combat::state::ExhaustSelectPurpose::GamblingChip
        );
        assert!(combat
            .exhaust_select
            .as_ref()
            .expect("Gambling Chip selection")
            .selected_hand_indices
            .is_empty());
    }

    #[test]
    fn gambling_chip_discards_selected_hand_cards_and_redraws() {
        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::GamblingChip);
        let combat = run.init_combat(CombatState::initial_fixture());
        let discarded = combat.piles.hand[0].id;
        let drawn = combat.piles.draw_pile.last().expect("draw pile card").id;
        run.phase = RunPhase::Combat;
        run.combat = Some(combat);

        let selected =
            crate::run::apply_run_action(&run, RunAction::ChooseExhaustSelect { index: 0 })
                .expect("select discard");
        let confirmed = crate::run::apply_run_action(&selected, RunAction::ConfirmExhaustSelect)
            .expect("confirm discard");
        let combat = confirmed.combat.expect("combat remains active");

        assert!(combat.exhaust_select.is_none());
        assert!(combat.piles.hand.iter().any(|card| card.id == drawn));
        assert!(!combat.piles.hand.iter().any(|card| card.id == discarded));
        assert!(combat
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == discarded));
        assert!(combat.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn gambling_chip_confirmation_does_not_trigger_dead_branch() {
        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::GamblingChip);
        run.gain_relic(Relic::DeadBranch);
        let combat = run.init_combat(CombatState::initial_fixture());
        run.phase = RunPhase::Combat;
        run.combat = Some(combat);

        let selected =
            crate::run::apply_run_action(&run, RunAction::ChooseExhaustSelect { index: 0 })
                .expect("select discard");
        let confirmed = crate::run::apply_run_action(&selected, RunAction::ConfirmExhaustSelect)
            .expect("confirm discard");
        let combat = confirmed.combat.expect("combat remains active");

        assert!(combat.piles.exhaust_pile.is_empty());
        assert!(!combat.piles.hand.iter().any(|card| card.combat_only));
    }

    #[test]
    fn toolbox_opens_start_of_combat_colorless_card_choice() {
        let mut run = RunState::map_fixture();
        run.card_random_rng_counter = 7;
        run.gain_relic_key(RelicKey::Toolbox);

        let combat = run.init_combat(CombatState::initial_fixture());

        let choices = combat.toolbox_card_reward.as_ref().expect("Toolbox reward");
        assert_eq!(choices.len(), 3);
        assert_eq!(
            combat.card_random_rng.as_ref().expect("card rng").counter(),
            10
        );
    }

    #[test]
    fn toolbox_choice_adds_normal_cost_combat_only_card_to_hand() {
        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::Toolbox);
        let combat = run.init_combat(CombatState::initial_fixture());
        let chosen_content =
            combat.toolbox_card_reward.as_ref().expect("Toolbox reward")[0].content_id;
        run.phase = RunPhase::Combat;
        run.combat = Some(combat);

        let after =
            crate::run::apply_run_action(&run, RunAction::ChooseCombatCardReward { index: 0 })
                .expect("choose Toolbox card");
        let combat = after.combat.expect("combat remains active");
        let added = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == chosen_content && card.combat_only)
            .expect("chosen card added");

        assert!(combat.toolbox_card_reward.is_none());
        assert!(added.temp_cost.is_none());
        assert!(added.combat_only);
    }

    #[test]
    fn toolbox_consuming_combat_entry_persists_card_random_counter() {
        let mut run = RunState::map_fixture();
        run.card_random_rng_counter = 7;
        run.gain_relic_key(RelicKey::Toolbox);

        let combat = run.init_combat_consuming_relics(CombatState::initial_fixture());

        assert!(combat.toolbox_card_reward.is_some());
        assert_eq!(run.card_random_rng_counter, 10);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub phase: RunPhase,
    pub deck: Vec<CardInstance>,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub gold: i32,
    #[serde(default = "default_energy_per_turn")]
    pub energy_per_turn: i32,
    pub map: Option<MapRunState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_room_override: Option<RoomKind>,
    pub combat: Option<CombatState>,
    pub reward: Option<RewardScreen>,
    #[serde(default)]
    pub event: Option<super::event::EventScreen>,
    pub shop: Option<super::shop::ShopScreen>,
    #[serde(default)]
    pub card_grid: Option<super::grid::CardGridScreen>,
    #[serde(default)]
    pub relics: Vec<Relic>,
    #[serde(default)]
    pub potions: Vec<Potion>,
    #[serde(default)]
    pub event_rng_seed: u64,
    #[serde(default)]
    pub reward_rng_seed: u64,
    #[serde(default)]
    pub card_rng_counter: u32,
    #[serde(default)]
    pub card_random_rng_counter: u32,
    #[serde(default = "default_card_rarity_factor")]
    pub card_rarity_factor: i32,
    #[serde(default)]
    pub treasure_rng_seed: u64,
    #[serde(default)]
    pub treasure_rng_counter: u32,
    #[serde(default)]
    pub potion_rng_seed: u64,
    #[serde(default)]
    pub potion_rng_counter: u32,
    #[serde(default)]
    pub potion_chance: i32,
    #[serde(default)]
    pub relic_rng_seed: u64,
    #[serde(default)]
    pub relic_rng_counter: u32,
    #[serde(default)]
    pub relic_pools: Option<RelicPoolState>,
    #[serde(default)]
    pub relic_keys: Vec<RelicKey>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub omamori_charges_used: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub maw_bank_broken: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ancient_tea_set_armed: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub lizard_tail_used: bool,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub girya_lifts: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub matryoshka_chests_opened: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub incense_burner_counter: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub tiny_chest_counter: u32,
    #[serde(default = "default_event_room_monster_chance")]
    pub event_room_monster_chance: u32,
    #[serde(default = "default_event_room_shop_chance")]
    pub event_room_shop_chance: u32,
    #[serde(default = "default_event_room_treasure_chance")]
    pub event_room_treasure_chance: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub wing_boots_charges: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub neow_lament_combats_remaining: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub normal_combat_count: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub elite_combat_count: u32,
    #[serde(default)]
    pub merchant_rng_seed: u64,
    #[serde(default)]
    pub merchant_rng_counter: u32,
    #[serde(default)]
    pub event_rng_counter: u32,
    #[serde(default)]
    pub misc_rng_seed: u64,
    #[serde(default)]
    pub misc_rng_counter: u32,
    #[serde(default)]
    pub monster_rng_seed: u64,
    #[serde(default)]
    pub monster_rng_counter: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normal_encounter_list: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elite_encounter_list: Vec<String>,
    #[serde(default)]
    pub current_floor: i32,
    #[serde(default)]
    pub current_act: i32,
    #[serde(default)]
    pub shop_remove_count: u32,
    #[serde(default)]
    pub act1_event_list: Vec<super::event::Event>,
    #[serde(default)]
    pub act1_shrine_list: Vec<super::event::Event>,
    #[serde(default)]
    pub act2_event_list: Vec<super::event::Event>,
    #[serde(default)]
    pub act2_shrine_list: Vec<super::event::Event>,
    #[serde(default)]
    pub ascension: u8,
    #[serde(default)]
    pub treasure_room: Option<super::reward::TreasureRoomState>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest_room_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunRngStream {
    CardReward,
    CardRandom,
    Event,
    Merchant,
    Misc,
    Potion,
    Relic,
    Treasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunRngStreamState {
    pub seed: u64,
    pub counter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Combat,
    Reward,
    Treasure,
    Rest,
    Event,
    Shop,
    Idle,
}

pub const REWARD_GOLD_AMOUNT: i32 = 20;

fn default_card_rarity_factor() -> i32 {
    5
}

pub const DEFAULT_EVENT_ROOM_MONSTER_CHANCE: u32 = 10;
pub const DEFAULT_EVENT_ROOM_SHOP_CHANCE: u32 = 3;
pub const DEFAULT_EVENT_ROOM_TREASURE_CHANCE: u32 = 2;

fn default_event_room_monster_chance() -> u32 {
    DEFAULT_EVENT_ROOM_MONSTER_CHANCE
}

fn default_event_room_shop_chance() -> u32 {
    DEFAULT_EVENT_ROOM_SHOP_CHANCE
}

fn default_event_room_treasure_chance() -> u32 {
    DEFAULT_EVENT_ROOM_TREASURE_CHANCE
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

fn apply_neow_lament_to_combat(combat: &mut CombatState) {
    for monster in &mut combat.monsters {
        monster.hp = 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardScreen {
    pub choices: Vec<CardInstance>,
    pub gold_offer: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub stolen_gold_offer: i32,
    pub potion_offer: Option<Potion>,
    pub relic_offer: Option<Relic>,
    #[serde(default)]
    pub relic_key_offer: Option<RelicKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_relic_offer: Option<Relic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_relic_key_offer: Option<RelicKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_relic_key_offers: Vec<RelicKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boss_relic_choices: Vec<RelicKey>,
    #[serde(default)]
    pub card_reward_active: bool,
    /// Normal combat rewards defer card RNG until the player opens the card screen.
    #[serde(default)]
    pub card_reward_pending: bool,
    /// Number of unopened card reward screens remaining.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub pending_card_reward_count: u8,
}

impl RewardScreen {
    #[must_use]
    pub fn pending_card_reward_count(&self) -> u8 {
        if self.pending_card_reward_count > 0 {
            self.pending_card_reward_count
        } else if self.card_reward_pending {
            1
        } else {
            0
        }
    }

    pub fn set_pending_card_rewards(&mut self, count: u8) {
        self.pending_card_reward_count = count;
        self.card_reward_pending = count > 0;
    }

    pub fn consume_pending_card_reward(&mut self) {
        let count = self.pending_card_reward_count().saturating_sub(1);
        self.set_pending_card_rewards(count);
    }
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunAction {
    SkipReward,
    CloseCardReward,
    TakeCardReward {
        card_id: CardId,
    },
    TakeSingingBowlReward,
    TakeGoldReward,
    TakeStolenGoldReward,
    TakePotionReward,
    TakeRelicReward,
    ChooseBossRelicReward {
        index: usize,
    },
    Proceed,
    OpenChest,
    OpenCardReward,
    SkipPotionReward,
    BuyShopCard {
        slot: usize,
    },
    BuyShopRelic {
        slot: usize,
    },
    BuyShopPotion {
        slot: usize,
    },
    UsePotion {
        slot: usize,
        target: Option<MonsterId>,
    },
    DiscardPotion {
        slot: usize,
    },
    ChooseCombatCardReward {
        index: usize,
    },
    ChooseHandSelect {
        index: usize,
    },
    ConfirmHandSelect,
    ChooseDrawSelect {
        index: usize,
    },
    ConfirmDrawSelect,
    ChooseDiscardSelect {
        index: usize,
    },
    ConfirmDiscardSelect,
    ChooseExhaustSelect {
        index: usize,
    },
    ConfirmExhaustSelect,
    EnterShop,
    LeaveShop,
    OpenShopRemove,
}

impl RunState {
    #[must_use]
    pub fn rng_stream_state(&self, stream: RunRngStream) -> RunRngStreamState {
        match stream {
            RunRngStream::CardReward => RunRngStreamState {
                seed: self.reward_rng_seed,
                counter: self.card_rng_counter,
            },
            RunRngStream::CardRandom => RunRngStreamState {
                seed: (self.reward_rng_seed as i64).wrapping_add(i64::from(self.current_floor))
                    as u64,
                counter: self.card_random_rng_counter,
            },
            RunRngStream::Event => RunRngStreamState {
                seed: self.event_rng_seed,
                counter: self.event_rng_counter,
            },
            RunRngStream::Merchant => RunRngStreamState {
                seed: self.merchant_rng_seed,
                counter: self.merchant_rng_counter,
            },
            RunRngStream::Misc => RunRngStreamState {
                seed: self.misc_rng_seed,
                counter: self.misc_rng_counter,
            },
            RunRngStream::Potion => RunRngStreamState {
                seed: self.potion_rng_seed,
                counter: self.potion_rng_counter,
            },
            RunRngStream::Relic => RunRngStreamState {
                seed: self.relic_rng_seed,
                counter: self.relic_rng_counter,
            },
            RunRngStream::Treasure => RunRngStreamState {
                seed: self.treasure_rng_seed,
                counter: self.treasure_rng_counter,
            },
        }
    }

    pub fn set_rng_stream_counter(&mut self, stream: RunRngStream, counter: u32) {
        match stream {
            RunRngStream::CardReward => self.card_rng_counter = counter,
            RunRngStream::CardRandom => self.card_random_rng_counter = counter,
            RunRngStream::Event => self.event_rng_counter = counter,
            RunRngStream::Merchant => self.merchant_rng_counter = counter,
            RunRngStream::Misc => self.misc_rng_counter = counter,
            RunRngStream::Potion => self.potion_rng_counter = counter,
            RunRngStream::Relic => self.relic_rng_counter = counter,
            RunRngStream::Treasure => self.treasure_rng_counter = counter,
        }
    }

    #[must_use]
    pub fn rng_for_stream(&self, stream: RunRngStream) -> StsRng {
        let state = self.rng_stream_state(stream);
        StsRng::with_counter(state.seed as i64, state.counter)
    }

    pub fn store_rng_counter(&mut self, stream: RunRngStream, rng: &StsRng) {
        self.set_rng_stream_counter(stream, rng.counter());
    }

    #[must_use]
    pub fn init_combat(&self, base: CombatState) -> CombatState {
        let mut combat = base;
        combat.player.hp = self.player_hp;
        combat.player.max_hp = self.player_max_hp;
        combat.player.max_energy = self.energy_per_turn;
        combat.player.energy = self.energy_per_turn;
        combat.relics = self.relics.clone();
        combat.ascension = self.ascension;
        if self.relics.contains(&Relic::SneckoEye) {
            combat.card_random_rng = Some(self.card_random_rng());
        }
        if matches!(
            self.current_room_kind(),
            Some(RoomKind::Elite | RoomKind::Boss)
        ) && self.relics.contains(&Relic::SlaversCollar)
        {
            combat.player.max_energy += SLAVERS_COLLAR_ENERGY;
            combat.player.energy += SLAVERS_COLLAR_ENERGY;
        }
        if self.current_room_kind() == Some(RoomKind::Boss)
            && self.relics.contains(&Relic::Pantograph)
        {
            crate::relic::heal_player_in_combat_with_relics(
                &mut combat.player.hp,
                combat.player.max_hp,
                PANTOGRAPH_HEAL,
                &self.relics,
            );
        }
        if self.current_room_kind() == Some(RoomKind::Elite)
            && self.relics.contains(&Relic::PreservedInsect)
        {
            for monster in &mut combat.monsters {
                monster.hp = (monster.hp * PRESERVED_INSECT_HP_NUMERATOR
                    / PRESERVED_INSECT_HP_DENOMINATOR)
                    .max(1);
            }
        }
        if self.current_room_kind() == Some(RoomKind::Elite)
            && self.relics.contains(&Relic::SlingOfCourage)
        {
            combat.player.powers.strength += SLING_OF_COURAGE_STRENGTH;
        }
        if self.relics.contains(&Relic::DuVuDoll) {
            let curses = self
                .deck
                .iter()
                .filter(|card| is_curse_content_id(card.content_id))
                .count() as i32;
            combat.player.powers.strength += curses * DU_VU_DOLL_STRENGTH_PER_CURSE;
        }
        if self.relics.contains(&Relic::Girya) {
            combat.player.powers.strength += self.girya_lifts as i32;
        }
        if self.relics.contains(&Relic::AncientTeaSet) && self.ancient_tea_set_armed {
            combat.player.energy += ANCIENT_TEA_SET_ENERGY;
        }
        if self.relics.contains(&Relic::PhilosophersStone) {
            for monster in &mut combat.monsters {
                monster.powers.strength += PHILOSOPHERS_STONE_MONSTER_STRENGTH;
            }
        }
        if self.relics.contains(&Relic::IncenseBurner) {
            combat.relic_counters.incense_burner_counter = self.incense_burner_counter;
        }
        apply_start_of_combat_relics(&mut combat, &self.relics);
        if self.relics.contains(&Relic::GamblingChip) {
            crate::combat::open_gambling_chip_select(&mut combat)
                .expect("Gambling Chip selection opens without validation side effects");
        }
        if self.relics.contains(&Relic::Toolbox) {
            let mut rng = combat
                .card_random_rng
                .take()
                .unwrap_or_else(|| self.card_random_rng());
            let next_card_id = combat.piles.max_card_instance_id() + 1;
            let choices = colorless_discovery_card_choices(&mut rng, 3)
                .into_iter()
                .enumerate()
                .map(|(index, content_id)| {
                    CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                })
                .collect();
            combat.card_random_rng = Some(rng);
            combat.toolbox_card_reward = Some(choices);
        }
        combat
    }

    #[must_use]
    pub fn init_combat_consuming_relics(&mut self, base: CombatState) -> CombatState {
        let mut combat = self.init_combat(base);
        if self.neow_lament_combats_remaining > 0 {
            apply_neow_lament_to_combat(&mut combat);
            self.neow_lament_combats_remaining -= 1;
        }
        if self.ancient_tea_set_armed {
            self.ancient_tea_set_armed = false;
        }
        if self.relics.contains(&Relic::IncenseBurner) {
            self.incense_burner_counter = combat.relic_counters.incense_burner_counter;
        }
        if self.relics.contains(&Relic::Toolbox) {
            if let Some(rng) = combat.card_random_rng.as_ref() {
                self.card_random_rng_counter = rng.counter();
            }
        }
        combat
    }

    #[must_use]
    pub fn card_random_rng(&self) -> StsRng {
        self.rng_for_stream(RunRngStream::CardRandom)
    }

    pub fn reset_card_random_rng_for_combat(&mut self) {
        self.card_random_rng_counter = 0;
    }

    #[must_use]
    pub fn current_room_kind(&self) -> Option<RoomKind> {
        if let Some(room_kind) = self.current_room_override {
            return Some(room_kind);
        }
        self.map.as_ref().and_then(|map_state| {
            map_state
                .map
                .node(map_state.current_node)
                .map(|node| node.room_kind)
        })
    }

    #[must_use]
    pub fn ascension_config(&self) -> AscensionConfig {
        AscensionConfig::new(self.ascension)
    }

    #[must_use]
    pub fn combat_fixture() -> Self {
        Self::combat_fixture_with_relics(Vec::new())
    }

    #[must_use]
    pub fn combat_fixture_with_relics(relics: Vec<Relic>) -> Self {
        Self::combat_fixture_with_options(relics, 0)
    }

    #[must_use]
    pub fn combat_fixture_with_ascension(ascension: u8) -> Self {
        Self::combat_fixture_with_options(Vec::new(), ascension)
    }

    #[must_use]
    pub fn combat_fixture_with_options(relics: Vec<Relic>, ascension: u8) -> Self {
        let deck = crate::content::deck::ironclad_starter_deck_for_ascension(ascension);
        let mut run = Self {
            phase: RunPhase::Combat,
            deck,
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            energy_per_turn: BASE_PLAYER_ENERGY,
            map: None,
            current_room_override: None,
            combat: None,
            reward: None,
            event: None,
            shop: None,
            card_grid: None,
            relics,
            potions: Vec::new(),
            event_rng_seed: 0,
            reward_rng_seed: 0,
            card_rng_counter: 0,
            card_random_rng_counter: 0,
            card_rarity_factor: default_card_rarity_factor(),
            treasure_rng_seed: 0,
            treasure_rng_counter: 0,
            potion_rng_seed: 0,
            potion_rng_counter: 0,
            potion_chance: 0,
            relic_rng_seed: 0,
            relic_rng_counter: 0,
            relic_pools: None,
            relic_keys: Vec::new(),
            omamori_charges_used: 0,
            maw_bank_broken: false,
            ancient_tea_set_armed: false,
            lizard_tail_used: false,
            girya_lifts: 0,
            matryoshka_chests_opened: 0,
            incense_burner_counter: 0,
            tiny_chest_counter: 0,
            event_room_monster_chance: DEFAULT_EVENT_ROOM_MONSTER_CHANCE,
            event_room_shop_chance: DEFAULT_EVENT_ROOM_SHOP_CHANCE,
            event_room_treasure_chance: DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
            wing_boots_charges: 0,
            neow_lament_combats_remaining: 0,
            normal_combat_count: 0,
            elite_combat_count: 0,
            merchant_rng_seed: 0,
            merchant_rng_counter: 0,
            event_rng_counter: 0,
            misc_rng_seed: 0,
            misc_rng_counter: 0,
            monster_rng_seed: 0,
            monster_rng_counter: 0,
            normal_encounter_list: Vec::new(),
            elite_encounter_list: Vec::new(),
            current_floor: 0,
            current_act: 1,
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            act2_event_list: Vec::new(),
            act2_shrine_list: Vec::new(),
            ascension,
            treasure_room: None,
            rest_room_complete: false,
        };
        let combat = run.init_combat(CombatState::initial_fixture());
        run.player_hp = combat.player.hp;
        run.player_max_hp = combat.player.max_hp;
        run.combat = Some(combat);
        run
    }

    #[must_use]
    pub fn map_fixture() -> Self {
        Self {
            phase: RunPhase::Idle,
            deck: crate::content::deck::ironclad_starter_deck(),
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            energy_per_turn: BASE_PLAYER_ENERGY,
            map: Some(milestone8_fixture()),
            current_room_override: None,
            combat: None,
            reward: None,
            event: None,
            shop: None,
            card_grid: None,
            relics: Vec::new(),
            potions: Vec::new(),
            event_rng_seed: 0,
            reward_rng_seed: 0,
            card_rng_counter: 0,
            card_random_rng_counter: 0,
            card_rarity_factor: default_card_rarity_factor(),
            treasure_rng_seed: 0,
            treasure_rng_counter: 0,
            potion_rng_seed: 0,
            potion_rng_counter: 0,
            potion_chance: 0,
            relic_rng_seed: 0,
            relic_rng_counter: 0,
            relic_pools: None,
            relic_keys: Vec::new(),
            omamori_charges_used: 0,
            maw_bank_broken: false,
            ancient_tea_set_armed: false,
            lizard_tail_used: false,
            girya_lifts: 0,
            matryoshka_chests_opened: 0,
            incense_burner_counter: 0,
            tiny_chest_counter: 0,
            event_room_monster_chance: DEFAULT_EVENT_ROOM_MONSTER_CHANCE,
            event_room_shop_chance: DEFAULT_EVENT_ROOM_SHOP_CHANCE,
            event_room_treasure_chance: DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
            wing_boots_charges: 0,
            neow_lament_combats_remaining: 0,
            normal_combat_count: 0,
            elite_combat_count: 0,
            merchant_rng_seed: 0,
            merchant_rng_counter: 0,
            event_rng_counter: 0,
            misc_rng_seed: 0,
            misc_rng_counter: 0,
            monster_rng_seed: 0,
            monster_rng_counter: 0,
            normal_encounter_list: Vec::new(),
            elite_encounter_list: Vec::new(),
            current_floor: 0,
            current_act: 1,
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            act2_event_list: Vec::new(),
            act2_shrine_list: Vec::new(),
            ascension: 0,
            treasure_room: None,
            rest_room_complete: false,
        }
    }

    /// Start a simulator-only seeded Ironclad run.
    ///
    /// Fidelity: placeholder. This uses the deterministic placeholder map
    /// generator, not target-game seed-start parity.
    #[must_use]
    pub fn placeholder_seeded_ironclad(seed: u64, ascension: u8) -> Self {
        let mut run = Self::map_fixture();
        run.deck = crate::content::deck::ironclad_starter_deck_for_ascension(ascension);
        run.map = Some(generate_target_fixed_map(
            seed as i64,
            TargetMapAct::Exordium,
        ));
        run.relics = vec![Relic::BurningBlood];
        run.phase = RunPhase::Event;
        run.event = Some(super::event::neow_talk_screen());
        run.ascension = ascension;
        run.event_rng_seed = seed;
        run.reward_rng_seed = seed;
        run.treasure_rng_seed = seed;
        run.potion_rng_seed = seed;
        run.relic_rng_seed = seed;
        run.merchant_rng_seed = seed;
        run.misc_rng_seed = seed;
        run.monster_rng_seed = seed;
        run
    }

    pub fn reinit_misc_rng_for_floor(&mut self) {
        let base = self.reward_rng_seed as i64;
        self.misc_rng_seed = base.wrapping_add(i64::from(self.current_floor)) as u64;
        self.misc_rng_counter = 0;
    }

    pub fn reinit_room_rngs_for_floor(&mut self) {
        self.card_random_rng_counter = 0;
        self.reinit_misc_rng_for_floor();
    }

    pub fn ensure_ironclad_relic_pools(&mut self) {
        if self.relic_pools.is_none() {
            let mut rng = StsRng::with_counter(self.relic_rng_seed as i64, self.relic_rng_counter);
            self.relic_pools = Some(initialize_ironclad_relic_pools(&mut rng));
            self.relic_rng_counter = rng.counter();
            let owned_keys: Vec<_> = self
                .relics
                .iter()
                .map(|relic| relic.key())
                .chain(self.relic_keys.iter().copied())
                .collect();
            if let Some(pools) = self.relic_pools.as_mut() {
                for key in owned_keys {
                    pools.remove_relic(key);
                }
            }
        }
    }

    #[must_use]
    pub fn relic_spawn_context(&self, floor_num: i32, shop_room: bool) -> RelicSpawnContext {
        let mut owned_relics: Vec<_> = self.relics.iter().map(|relic| relic.key()).collect();
        owned_relics.extend(self.relic_keys.iter().copied());
        RelicSpawnContext {
            floor_num,
            shop_room,
            owned_relics,
            has_non_basic_attack: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id).is_some_and(|(card_type, _)| {
                    card_type == CardType::Attack && !is_basic_starter_card(card.content_id)
                })
            }),
            has_non_basic_skill: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id).is_some_and(|(card_type, _)| {
                    card_type == CardType::Skill && !is_basic_starter_card(card.content_id)
                })
            }),
            has_power: self.deck.iter().any(|card| {
                card_type_and_rarity(card.content_id)
                    .is_some_and(|(card_type, _)| card_type == CardType::Power)
            }),
        }
    }

    pub fn next_card_instance_id(&self) -> u64 {
        self.deck
            .iter()
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn gain_deck_card(&mut self, content_id: ContentId) {
        let id = CardId::new(self.next_card_instance_id());
        self.add_deck_card(CardInstance::new(id, content_id));
    }

    pub fn add_deck_card(&mut self, mut card: CardInstance) {
        if self.should_omamori_prevent_card(card.content_id) {
            self.omamori_charges_used += 1;
            return;
        }
        card.content_id = self.content_id_after_card_add_relics(card.content_id);
        let content_id = card.content_id;
        self.deck.push(card);
        self.apply_card_added_relics(content_id);
    }

    fn should_omamori_prevent_card(&self, content_id: ContentId) -> bool {
        self.relics.contains(&Relic::Omamori)
            && is_curse_content_id(content_id)
            && self.omamori_charges_used < OMAMORI_CHARGES
    }

    #[must_use]
    pub(crate) fn content_id_after_card_add_relics(&self, content_id: ContentId) -> ContentId {
        let Some(upgraded) = upgrade_content_id(content_id) else {
            return content_id;
        };
        let Some(definition) = get_card_definition(content_id) else {
            return content_id;
        };
        let has_matching_egg = match definition.card_type {
            CardType::Attack => self.relics.contains(&Relic::MoltenEgg),
            CardType::Skill => self.relics.contains(&Relic::ToxicEgg),
            CardType::Power => self.relics.contains(&Relic::FrozenEgg),
            CardType::Status => false,
        };
        if has_matching_egg {
            upgraded
        } else {
            content_id
        }
    }

    fn apply_card_added_relics(&mut self, content_id: ContentId) {
        if self.relics.contains(&Relic::CeramicFish) {
            self.gain_gold(CERAMIC_FISH_GOLD);
        }
        if self.relics.contains(&Relic::DarkstonePeriapt) && is_curse_content_id(content_id) {
            self.player_max_hp += DARKSTONE_PERIAPT_MAX_HP;
            self.player_hp += DARKSTONE_PERIAPT_MAX_HP;
        }
    }

    pub fn potion_capacity(&self) -> usize {
        MAX_POTIONS
            + self
                .relics
                .iter()
                .filter(|relic| **relic == Relic::PotionBelt)
                .count()
                * POTION_BELT_SLOTS
    }

    pub fn can_gain_potions(&self) -> bool {
        !self.relics.contains(&Relic::Sozu)
    }

    pub fn can_gain_gold(&self) -> bool {
        !self.relics.contains(&Relic::Ectoplasm)
    }

    pub fn gain_gold(&mut self, amount: i32) {
        if amount > 0 && self.can_gain_gold() {
            self.gold += amount;
        }
    }

    pub fn apply_floor_entry_relics(&mut self) {
        if self.relics.contains(&Relic::MawBank) && !self.maw_bank_broken {
            self.gain_gold(MAW_BANK_GOLD);
        }
    }

    pub fn apply_rest_site_entry_relics(&mut self) {
        if self.relics.contains(&Relic::AncientTeaSet) {
            self.ancient_tea_set_armed = true;
        }
        if self.relics.contains(&Relic::EternalFeather) {
            let heal = (self.deck.len() as i32 / 5) * ETERNAL_FEATHER_HEAL_PER_FIVE_CARDS;
            self.player_hp = (self.player_hp + heal).min(self.player_max_hp);
        }
    }

    pub fn break_maw_bank_on_shop_spend(&mut self) {
        if self.relics.contains(&Relic::MawBank) {
            self.maw_bank_broken = true;
        }
    }

    pub fn gain_relic_key(&mut self, key: RelicKey) {
        self.ensure_ironclad_relic_pools();
        if let Some(pools) = self.relic_pools.as_mut() {
            pools.remove_relic(key);
        }
        if let Some(relic) = Relic::from_key(key) {
            self.gain_relic(relic);
        } else {
            self.relic_keys.push(key);
        }
    }

    pub fn gain_relic(&mut self, relic: Relic) {
        if let Some(pools) = self.relic_pools.as_mut() {
            pools.remove_relic(relic.key());
        }
        self.relics.push(relic);
        match relic {
            Relic::Strawberry => {
                self.player_max_hp += STRAWBERRY_MAX_HP;
                self.player_hp += STRAWBERRY_MAX_HP;
            }
            Relic::Pear => {
                self.player_max_hp += PEAR_MAX_HP;
                self.player_hp += PEAR_MAX_HP;
            }
            Relic::Mango => {
                self.player_max_hp += MANGO_MAX_HP;
                self.player_hp += MANGO_MAX_HP;
            }
            Relic::OldCoin => {
                self.gain_gold(OLD_COIN_GOLD);
            }
            Relic::LeesWaffle => {
                self.player_max_hp += LEES_WAFFLE_MAX_HP;
                self.player_hp = self.player_max_hp;
            }
            Relic::CoffeeDripper => {
                self.energy_per_turn += COFFEE_DRIPPER_ENERGY;
            }
            Relic::MarkOfPain => {
                self.energy_per_turn += MARK_OF_PAIN_ENERGY;
            }
            Relic::FusionHammer => {
                self.energy_per_turn += FUSION_HAMMER_ENERGY;
            }
            Relic::Sozu => {
                self.energy_per_turn += SOZU_ENERGY;
            }
            Relic::BustedCrown => {
                self.energy_per_turn += BUSTED_CROWN_ENERGY;
            }
            Relic::SneckoEye => {
                self.energy_per_turn += SNECKO_EYE_ENERGY;
            }
            Relic::WingBoots => {
                self.wing_boots_charges = u32::from(WING_BOOTS_CHARGES);
            }
            Relic::CallingBell => {
                super::grid::open_calling_bell_grid(self);
            }
            Relic::PandorasBox => {
                super::grid::open_pandoras_box_grid(self);
            }
            Relic::Astrolabe => {
                super::grid::open_astrolabe_grid(self);
            }
            Relic::VelvetChoker => {
                self.energy_per_turn += VELVET_CHOKER_ENERGY;
            }
            Relic::PhilosophersStone => {
                self.energy_per_turn += PHILOSOPHERS_STONE_ENERGY;
            }
            Relic::CursedKey => {
                self.energy_per_turn += 1;
            }
            Relic::Ectoplasm => {
                self.energy_per_turn += ECTOPLASM_ENERGY;
            }
            Relic::RunicDome => {
                self.energy_per_turn += RUNIC_DOME_ENERGY;
            }
            Relic::Whetstone => {
                self.upgrade_random_deck_cards(CardType::Attack, 2);
            }
            Relic::WarPaint => {
                self.upgrade_random_deck_cards(CardType::Skill, 2);
            }
            Relic::EmptyCage => {
                super::grid::open_empty_cage_grid(self);
            }
            Relic::BottledFlame => {
                super::grid::open_bottle_grid(self, CardType::Attack);
            }
            Relic::BottledLightning => {
                super::grid::open_bottle_grid(self, CardType::Skill);
            }
            Relic::BottledTornado => {
                super::grid::open_bottle_grid(self, CardType::Power);
            }
            Relic::DollysMirror => {
                super::grid::open_dollys_mirror_grid(self);
            }
            Relic::Cauldron => {
                self.fill_potions_from_cauldron();
            }
            Relic::TinyHouse => {
                self.player_max_hp += TINY_HOUSE_MAX_HP;
                self.player_hp =
                    (self.player_hp + TINY_HOUSE_MAX_HP + TINY_HOUSE_HEAL).min(self.player_max_hp);
                self.gain_gold(TINY_HOUSE_GOLD);
                self.upgrade_random_deck_cards_matching(1, |_| true);
                if let Some(reward) = self.reward.as_mut() {
                    reward.set_pending_card_rewards(reward.pending_card_reward_count() + 1);
                }
            }
            Relic::Orrery => {
                if let Some(reward) = self.reward.as_mut() {
                    reward.set_pending_card_rewards(
                        reward.pending_card_reward_count() + ORRERY_CARD_REWARDS,
                    );
                }
            }
            Relic::BloodVial
            | Relic::ToyOrnithopter
            | Relic::MoltenEgg
            | Relic::ToxicEgg
            | Relic::FrozenEgg
            | Relic::TheBoot
            | Relic::BirdFacedUrn
            | Relic::PrayerWheel
            | Relic::CrackedCore
            | Relic::FrozenCore
            | Relic::PureWater
            | Relic::HolyWater
            | Relic::RingOfTheSnake
            | Relic::RingOfTheSerpent
            | Relic::PotionBelt
            | Relic::Lantern
            | Relic::BagOfPreparation
            | Relic::BagOfMarbles
            | Relic::BronzeScales
            | Relic::ThreadAndNeedle
            | Relic::RedSkull
            | Relic::Nunchaku
            | Relic::ArtOfWar
            | Relic::Shuriken
            | Relic::Kunai
            | Relic::LetterOpener
            | Relic::HappyFlower
            | Relic::Orichalcum
            | Relic::HornCleat
            | Relic::CaptainsWheel
            | Relic::MercuryHourglass
            | Relic::StoneCalendar
            | Relic::MeatOnTheBone
            | Relic::QuestionCard
            | Relic::BlackBlood
            | Relic::MealTicket
            | Relic::RegalPillow
            | Relic::DreamCatcher
            | Relic::EternalFeather
            | Relic::Torii
            | Relic::TungstenRod
            | Relic::CeramicFish
            | Relic::MembershipCard
            | Relic::SmilingMask
            | Relic::MawBank
            | Relic::AncientTeaSet
            | Relic::Calipers
            | Relic::SingingBowl
            | Relic::Pantograph
            | Relic::Ginger
            | Relic::Turnip
            | Relic::MagicFlower
            | Relic::PaperPhrog
            | Relic::ChampionBelt
            | Relic::PreservedInsect
            | Relic::Omamori
            | Relic::SlingOfCourage
            | Relic::DarkstonePeriapt
            | Relic::DuVuDoll
            | Relic::Vajra
            | Relic::OddlySmoothStone
            | Relic::Anchor
            | Relic::InkBottle
            | Relic::OrnamentalFan
            | Relic::IceCream
            | Relic::ChemicalX
            | Relic::SlaversCollar
            | Relic::StrikeDummy
            | Relic::Brimstone
            | Relic::WhiteBeastStatue
            | Relic::Akabeko
            | Relic::CentennialPuzzle
            | Relic::PenNib
            | Relic::SelfFormingClay
            | Relic::ClockworkSouvenir
            | Relic::RunicCube
            | Relic::TheAbacus
            | Relic::GremlinHorn
            | Relic::Sundial
            | Relic::CharonsAshes
            | Relic::BlueCandle
            | Relic::MedicalKit
            | Relic::LizardTail
            | Relic::Pocketwatch
            | Relic::HandDrill
            | Relic::BurningBlood
            | Relic::Circlet
            | Relic::RedCirclet
            | Relic::SacredBark
            | Relic::RunicPyramid
            | Relic::FrozenEye
            | Relic::PeacePipe
            | Relic::OrangePellets
            | Relic::Girya
            | Relic::UnceasingTop
            | Relic::Shovel
            | Relic::FossilizedHelix
            | Relic::BlackStar
            | Relic::Matryoshka
            | Relic::DeadBranch
            | Relic::MummifiedHand
            | Relic::TheCourier
            | Relic::IncenseBurner
            | Relic::TinyChest
            | Relic::StrangeSpoon
            | Relic::GamblingChip
            | Relic::Toolbox
            | Relic::JuzuBracelet
            | Relic::PrismaticShard
            | Relic::GoldenIdol
            | Relic::BloodyIdol
            | Relic::MutagenicStrength => {}
        }
    }

    fn fill_potions_from_cauldron(&mut self) {
        if !self.can_gain_potions() {
            return;
        }

        let open_slots = self.potion_capacity().saturating_sub(self.potions.len());
        let rolls = CAULDRON_POTIONS.min(open_slots);
        if rolls == 0 {
            return;
        }

        let mut potion_rng =
            StsRng::with_counter(self.potion_rng_seed as i64, self.potion_rng_counter);
        for _ in 0..rolls {
            self.potions
                .push(super::reward::target_random_potion(&mut potion_rng));
        }
        self.potion_rng_counter = potion_rng.counter();
    }

    fn upgrade_random_deck_cards(&mut self, card_type: CardType, amount: usize) {
        self.upgrade_random_deck_cards_matching(amount, |card| {
            card_type_and_rarity(card.content_id).is_some_and(|(candidate_type, _)| {
                candidate_type == card_type
                    && crate::content::cards::upgrade_content_id(card.content_id).is_some()
            })
        });
    }

    fn upgrade_random_deck_cards_matching(
        &mut self,
        amount: usize,
        matches_card: impl Fn(&CardInstance) -> bool,
    ) {
        let mut upgradeable: Vec<_> = self
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, card)| {
                (matches_card(card) && upgrade_content_id(card.content_id).is_some())
                    .then_some(index)
            })
            .collect();

        if upgradeable.is_empty() {
            return;
        }

        let mut misc_rng = StsRng::with_counter(self.misc_rng_seed as i64, self.misc_rng_counter);
        let shuffle_seed = misc_rng.random_long();
        self.misc_rng_counter = misc_rng.counter();

        JavaRng::new(shuffle_seed).collections_shuffle(&mut upgradeable);

        for deck_index in upgradeable.into_iter().take(amount) {
            let content_id = self.deck[deck_index].content_id;
            let upgraded =
                upgrade_content_id(content_id).expect("upgradeable card validated before shuffle");
            self.deck[deck_index].content_id = upgraded;
        }
    }

    pub fn validate_reward_action(&self, action: RunAction) -> SimResult<()> {
        if self.phase != RunPhase::Reward {
            return Err(SimError::IllegalAction(
                "reward actions require reward phase",
            ));
        }

        let reward = self
            .reward
            .as_ref()
            .ok_or(SimError::InvalidState("reward screen is missing"))?;

        match action {
            RunAction::SkipReward => Ok(()),
            RunAction::CloseCardReward => {
                if reward.card_reward_active {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("card reward is not open"))
                }
            }
            RunAction::TakeGoldReward => {
                if reward.gold_offer > 0 {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("no gold reward offered"))
                }
            }
            RunAction::TakeStolenGoldReward => {
                if reward.stolen_gold_offer > 0 {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("no stolen gold reward offered"))
                }
            }
            RunAction::TakePotionReward => {
                if reward.potion_offer.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                if !self.can_gain_potions() {
                    return Err(SimError::IllegalAction("potions cannot be obtained"));
                }
                if self.potions.len() >= self.potion_capacity() {
                    return Err(SimError::IllegalAction("potion belt is full"));
                }
                Ok(())
            }
            RunAction::TakeRelicReward => {
                if reward.relic_offer.is_none() && reward.relic_key_offer.is_none() {
                    return Err(SimError::IllegalAction("no relic reward offered"));
                }
                if let Some(relic) = reward.relic_offer {
                    if self.relics.contains(&relic) {
                        return Err(SimError::IllegalAction("relic already owned"));
                    }
                }
                if let Some(key) = reward.relic_key_offer {
                    if self.relics.iter().any(|relic| relic.key() == key)
                        || self.relic_keys.contains(&key)
                    {
                        return Err(SimError::IllegalAction("relic already owned"));
                    }
                }
                Ok(())
            }
            RunAction::ChooseBossRelicReward { index } => {
                if index < reward.boss_relic_choices.len() {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("boss relic choice is not offered"))
                }
            }
            RunAction::Proceed => Err(SimError::IllegalAction("not a reward action")),
            RunAction::OpenCardReward => {
                if reward.pending_card_reward_count() == 0 {
                    return Err(SimError::IllegalAction("no card reward offered"));
                }
                if reward.card_reward_active {
                    return Err(SimError::IllegalAction("card reward already open"));
                }
                Ok(())
            }
            RunAction::OpenChest => Err(SimError::IllegalAction("not a reward action")),
            RunAction::SkipPotionReward => {
                if reward.potion_offer.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                Ok(())
            }
            RunAction::TakeCardReward { card_id } => {
                if reward.choices.iter().any(|choice| choice.id == card_id) {
                    Ok(())
                } else {
                    Err(SimError::UnknownCard(card_id))
                }
            }
            RunAction::TakeSingingBowlReward => {
                if !self.relics.contains(&Relic::SingingBowl) {
                    return Err(SimError::IllegalAction("singing bowl is not owned"));
                }
                if !reward.card_reward_active || reward.choices.is_empty() {
                    return Err(SimError::IllegalAction("no open card reward to bowl"));
                }
                Ok(())
            }
            RunAction::BuyShopCard { .. }
            | RunAction::BuyShopRelic { .. }
            | RunAction::BuyShopPotion { .. }
            | RunAction::EnterShop
            | RunAction::LeaveShop
            | RunAction::OpenShopRemove => Err(SimError::IllegalAction("not a reward action")),
            RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::ChooseCombatCardReward { .. } => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::ChooseHandSelect { .. } | RunAction::ConfirmHandSelect => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::ChooseDrawSelect { .. } | RunAction::ConfirmDrawSelect => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::ChooseDiscardSelect { .. } | RunAction::ConfirmDiscardSelect => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::ChooseExhaustSelect { .. } | RunAction::ConfirmExhaustSelect => {
                Err(SimError::IllegalAction("not a reward action"))
            }
        }
    }

    pub fn count_content_in_deck(&self, content_id: ContentId) -> usize {
        self.deck
            .iter()
            .filter(|card| card.content_id == content_id)
            .count()
    }
}

impl Relic {
    #[must_use]
    pub fn key(self) -> RelicKey {
        match self {
            Relic::BurningBlood => RelicKey::BurningBlood,
            Relic::BloodVial => RelicKey::BloodVial,
            Relic::Vajra => RelicKey::Vajra,
            Relic::OddlySmoothStone => RelicKey::OddlySmoothStone,
            Relic::Strawberry => RelicKey::Strawberry,
            Relic::Pear => RelicKey::Pear,
            Relic::Mango => RelicKey::Mango,
            Relic::OldCoin => RelicKey::OldCoin,
            Relic::LeesWaffle => RelicKey::LeesWaffle,
            Relic::PotionBelt => RelicKey::PotionBelt,
            Relic::Lantern => RelicKey::Lantern,
            Relic::BagOfPreparation => RelicKey::BagOfPreparation,
            Relic::BagOfMarbles => RelicKey::BagOfMarbles,
            Relic::BronzeScales => RelicKey::BronzeScales,
            Relic::ThreadAndNeedle => RelicKey::ThreadAndNeedle,
            Relic::RedSkull => RelicKey::RedSkull,
            Relic::Nunchaku => RelicKey::Nunchaku,
            Relic::ArtOfWar => RelicKey::ArtOfWar,
            Relic::Shuriken => RelicKey::Shuriken,
            Relic::Kunai => RelicKey::Kunai,
            Relic::LetterOpener => RelicKey::LetterOpener,
            Relic::HappyFlower => RelicKey::HappyFlower,
            Relic::Orichalcum => RelicKey::Orichalcum,
            Relic::HornCleat => RelicKey::HornCleat,
            Relic::CaptainsWheel => RelicKey::CaptainsWheel,
            Relic::MercuryHourglass => RelicKey::MercuryHourglass,
            Relic::StoneCalendar => RelicKey::StoneCalendar,
            Relic::MeatOnTheBone => RelicKey::MeatOnTheBone,
            Relic::QuestionCard => RelicKey::QuestionCard,
            Relic::BlackBlood => RelicKey::BlackBlood,
            Relic::MealTicket => RelicKey::MealTicket,
            Relic::RegalPillow => RelicKey::RegalPillow,
            Relic::DreamCatcher => RelicKey::DreamCatcher,
            Relic::EternalFeather => RelicKey::EternalFeather,
            Relic::Torii => RelicKey::Torii,
            Relic::TungstenRod => RelicKey::TungstenRod,
            Relic::CeramicFish => RelicKey::CeramicFish,
            Relic::MembershipCard => RelicKey::MembershipCard,
            Relic::SmilingMask => RelicKey::SmilingMask,
            Relic::Pantograph => RelicKey::Pantograph,
            Relic::Ginger => RelicKey::Ginger,
            Relic::Turnip => RelicKey::Turnip,
            Relic::MarkOfPain => RelicKey::MarkOfPain,
            Relic::MagicFlower => RelicKey::MagicFlower,
            Relic::PaperPhrog => RelicKey::PaperPhrog,
            Relic::ChampionBelt => RelicKey::ChampionBelt,
            Relic::PreservedInsect => RelicKey::PreservedInsect,
            Relic::Omamori => RelicKey::Omamori,
            Relic::SlingOfCourage => RelicKey::SlingOfCourage,
            Relic::MawBank => RelicKey::MawBank,
            Relic::AncientTeaSet => RelicKey::AncientTeaSet,
            Relic::Calipers => RelicKey::Calipers,
            Relic::SingingBowl => RelicKey::SingingBowl,
            Relic::DarkstonePeriapt => RelicKey::DarkstonePeriapt,
            Relic::DuVuDoll => RelicKey::DuVuDoll,
            Relic::FusionHammer => RelicKey::FusionHammer,
            Relic::Sozu => RelicKey::Sozu,
            Relic::BustedCrown => RelicKey::BustedCrown,
            Relic::VelvetChoker => RelicKey::VelvetChoker,
            Relic::ToyOrnithopter => RelicKey::ToyOrnithopter,
            Relic::MoltenEgg => RelicKey::MoltenEgg,
            Relic::ToxicEgg => RelicKey::ToxicEgg,
            Relic::FrozenEgg => RelicKey::FrozenEgg,
            Relic::TheBoot => RelicKey::TheBoot,
            Relic::BirdFacedUrn => RelicKey::BirdFacedUrn,
            Relic::CoffeeDripper => RelicKey::CoffeeDripper,
            Relic::Anchor => RelicKey::Anchor,
            Relic::InkBottle => RelicKey::InkBottle,
            Relic::OrnamentalFan => RelicKey::OrnamentalFan,
            Relic::IceCream => RelicKey::IceCream,
            Relic::ChemicalX => RelicKey::ChemicalX,
            Relic::PhilosophersStone => RelicKey::PhilosophersStone,
            Relic::SlaversCollar => RelicKey::SlaversCollar,
            Relic::Ectoplasm => RelicKey::Ectoplasm,
            Relic::RunicDome => RelicKey::RunicDome,
            Relic::StrikeDummy => RelicKey::StrikeDummy,
            Relic::Brimstone => RelicKey::Brimstone,
            Relic::WhiteBeastStatue => RelicKey::WhiteBeastStatue,
            Relic::Whetstone => RelicKey::Whetstone,
            Relic::WarPaint => RelicKey::WarPaint,
            Relic::Akabeko => RelicKey::Akabeko,
            Relic::CentennialPuzzle => RelicKey::CentennialPuzzle,
            Relic::PenNib => RelicKey::PenNib,
            Relic::SelfFormingClay => RelicKey::SelfFormingClay,
            Relic::ClockworkSouvenir => RelicKey::ClockworkSouvenir,
            Relic::RunicCube => RelicKey::RunicCube,
            Relic::TheAbacus => RelicKey::TheAbacus,
            Relic::GremlinHorn => RelicKey::GremlinHorn,
            Relic::Sundial => RelicKey::Sundial,
            Relic::CharonsAshes => RelicKey::CharonsAshes,
            Relic::BlueCandle => RelicKey::BlueCandle,
            Relic::MedicalKit => RelicKey::MedicalKit,
            Relic::LizardTail => RelicKey::LizardTail,
            Relic::Pocketwatch => RelicKey::Pocketwatch,
            Relic::HandDrill => RelicKey::HandDrill,
            Relic::Circlet => RelicKey::Circlet,
            Relic::RedCirclet => RelicKey::RedCirclet,
            Relic::SacredBark => RelicKey::SacredBark,
            Relic::RunicPyramid => RelicKey::RunicPyramid,
            Relic::FrozenEye => RelicKey::FrozenEye,
            Relic::PeacePipe => RelicKey::PeacePipe,
            Relic::OrangePellets => RelicKey::OrangePellets,
            Relic::Girya => RelicKey::Girya,
            Relic::UnceasingTop => RelicKey::UnceasingTop,
            Relic::Shovel => RelicKey::Shovel,
            Relic::FossilizedHelix => RelicKey::FossilizedHelix,
            Relic::BlackStar => RelicKey::BlackStar,
            Relic::Matryoshka => RelicKey::Matryoshka,
            Relic::EmptyCage => RelicKey::EmptyCage,
            Relic::BottledFlame => RelicKey::BottledFlame,
            Relic::BottledLightning => RelicKey::BottledLightning,
            Relic::BottledTornado => RelicKey::BottledTornado,
            Relic::DollysMirror => RelicKey::DollysMirror,
            Relic::PrayerWheel => RelicKey::PrayerWheel,
            Relic::CrackedCore => RelicKey::CrackedCore,
            Relic::FrozenCore => RelicKey::FrozenCore,
            Relic::PureWater => RelicKey::PureWater,
            Relic::HolyWater => RelicKey::HolyWater,
            Relic::RingOfTheSnake => RelicKey::RingOfTheSnake,
            Relic::RingOfTheSerpent => RelicKey::RingOfTheSerpent,
            Relic::Cauldron => RelicKey::Cauldron,
            Relic::TinyHouse => RelicKey::TinyHouse,
            Relic::DeadBranch => RelicKey::DeadBranch,
            Relic::MummifiedHand => RelicKey::MummifiedHand,
            Relic::TheCourier => RelicKey::TheCourier,
            Relic::IncenseBurner => RelicKey::IncenseBurner,
            Relic::CursedKey => RelicKey::CursedKey,
            Relic::TinyChest => RelicKey::TinyChest,
            Relic::Orrery => RelicKey::Orrery,
            Relic::SneckoEye => RelicKey::SneckoEye,
            Relic::StrangeSpoon => RelicKey::StrangeSpoon,
            Relic::WingBoots => RelicKey::WingBoots,
            Relic::CallingBell => RelicKey::CallingBell,
            Relic::PandorasBox => RelicKey::PandorasBox,
            Relic::Astrolabe => RelicKey::Astrolabe,
            Relic::GamblingChip => RelicKey::GamblingChip,
            Relic::Toolbox => RelicKey::Toolbox,
            Relic::JuzuBracelet => RelicKey::JuzuBracelet,
            Relic::PrismaticShard => RelicKey::PrismaticShard,
            Relic::MutagenicStrength => RelicKey::MutagenicStrength,
            Relic::GoldenIdol => RelicKey::GoldenIdol,
            Relic::BloodyIdol => RelicKey::BloodyIdol,
        }
    }

    #[must_use]
    pub fn from_key(key: RelicKey) -> Option<Self> {
        match key {
            RelicKey::BurningBlood => Some(Relic::BurningBlood),
            RelicKey::BloodVial => Some(Relic::BloodVial),
            RelicKey::Vajra => Some(Relic::Vajra),
            RelicKey::OddlySmoothStone => Some(Relic::OddlySmoothStone),
            RelicKey::Strawberry => Some(Relic::Strawberry),
            RelicKey::Pear => Some(Relic::Pear),
            RelicKey::Mango => Some(Relic::Mango),
            RelicKey::OldCoin => Some(Relic::OldCoin),
            RelicKey::LeesWaffle => Some(Relic::LeesWaffle),
            RelicKey::PotionBelt => Some(Relic::PotionBelt),
            RelicKey::Lantern => Some(Relic::Lantern),
            RelicKey::BagOfPreparation => Some(Relic::BagOfPreparation),
            RelicKey::BagOfMarbles => Some(Relic::BagOfMarbles),
            RelicKey::BronzeScales => Some(Relic::BronzeScales),
            RelicKey::ThreadAndNeedle => Some(Relic::ThreadAndNeedle),
            RelicKey::RedSkull => Some(Relic::RedSkull),
            RelicKey::Nunchaku => Some(Relic::Nunchaku),
            RelicKey::ArtOfWar => Some(Relic::ArtOfWar),
            RelicKey::Shuriken => Some(Relic::Shuriken),
            RelicKey::Kunai => Some(Relic::Kunai),
            RelicKey::LetterOpener => Some(Relic::LetterOpener),
            RelicKey::HappyFlower => Some(Relic::HappyFlower),
            RelicKey::Orichalcum => Some(Relic::Orichalcum),
            RelicKey::HornCleat => Some(Relic::HornCleat),
            RelicKey::CaptainsWheel => Some(Relic::CaptainsWheel),
            RelicKey::MercuryHourglass => Some(Relic::MercuryHourglass),
            RelicKey::StoneCalendar => Some(Relic::StoneCalendar),
            RelicKey::MeatOnTheBone => Some(Relic::MeatOnTheBone),
            RelicKey::QuestionCard => Some(Relic::QuestionCard),
            RelicKey::BlackBlood => Some(Relic::BlackBlood),
            RelicKey::MealTicket => Some(Relic::MealTicket),
            RelicKey::RegalPillow => Some(Relic::RegalPillow),
            RelicKey::DreamCatcher => Some(Relic::DreamCatcher),
            RelicKey::EternalFeather => Some(Relic::EternalFeather),
            RelicKey::Torii => Some(Relic::Torii),
            RelicKey::TungstenRod => Some(Relic::TungstenRod),
            RelicKey::CeramicFish => Some(Relic::CeramicFish),
            RelicKey::MembershipCard => Some(Relic::MembershipCard),
            RelicKey::SmilingMask => Some(Relic::SmilingMask),
            RelicKey::Pantograph => Some(Relic::Pantograph),
            RelicKey::Ginger => Some(Relic::Ginger),
            RelicKey::Turnip => Some(Relic::Turnip),
            RelicKey::MarkOfPain => Some(Relic::MarkOfPain),
            RelicKey::MagicFlower => Some(Relic::MagicFlower),
            RelicKey::PaperPhrog => Some(Relic::PaperPhrog),
            RelicKey::ChampionBelt => Some(Relic::ChampionBelt),
            RelicKey::PreservedInsect => Some(Relic::PreservedInsect),
            RelicKey::Omamori => Some(Relic::Omamori),
            RelicKey::SlingOfCourage => Some(Relic::SlingOfCourage),
            RelicKey::MawBank => Some(Relic::MawBank),
            RelicKey::AncientTeaSet => Some(Relic::AncientTeaSet),
            RelicKey::Calipers => Some(Relic::Calipers),
            RelicKey::SingingBowl => Some(Relic::SingingBowl),
            RelicKey::DarkstonePeriapt => Some(Relic::DarkstonePeriapt),
            RelicKey::DuVuDoll => Some(Relic::DuVuDoll),
            RelicKey::FusionHammer => Some(Relic::FusionHammer),
            RelicKey::Sozu => Some(Relic::Sozu),
            RelicKey::BustedCrown => Some(Relic::BustedCrown),
            RelicKey::VelvetChoker => Some(Relic::VelvetChoker),
            RelicKey::ToyOrnithopter => Some(Relic::ToyOrnithopter),
            RelicKey::MoltenEgg => Some(Relic::MoltenEgg),
            RelicKey::ToxicEgg => Some(Relic::ToxicEgg),
            RelicKey::FrozenEgg => Some(Relic::FrozenEgg),
            RelicKey::TheBoot => Some(Relic::TheBoot),
            RelicKey::BirdFacedUrn => Some(Relic::BirdFacedUrn),
            RelicKey::CoffeeDripper => Some(Relic::CoffeeDripper),
            RelicKey::Anchor => Some(Relic::Anchor),
            RelicKey::InkBottle => Some(Relic::InkBottle),
            RelicKey::OrnamentalFan => Some(Relic::OrnamentalFan),
            RelicKey::IceCream => Some(Relic::IceCream),
            RelicKey::ChemicalX => Some(Relic::ChemicalX),
            RelicKey::PhilosophersStone => Some(Relic::PhilosophersStone),
            RelicKey::SlaversCollar => Some(Relic::SlaversCollar),
            RelicKey::Ectoplasm => Some(Relic::Ectoplasm),
            RelicKey::RunicDome => Some(Relic::RunicDome),
            RelicKey::StrikeDummy => Some(Relic::StrikeDummy),
            RelicKey::Brimstone => Some(Relic::Brimstone),
            RelicKey::WhiteBeastStatue => Some(Relic::WhiteBeastStatue),
            RelicKey::Whetstone => Some(Relic::Whetstone),
            RelicKey::WarPaint => Some(Relic::WarPaint),
            RelicKey::Akabeko => Some(Relic::Akabeko),
            RelicKey::CentennialPuzzle => Some(Relic::CentennialPuzzle),
            RelicKey::PenNib => Some(Relic::PenNib),
            RelicKey::SelfFormingClay => Some(Relic::SelfFormingClay),
            RelicKey::ClockworkSouvenir => Some(Relic::ClockworkSouvenir),
            RelicKey::RunicCube => Some(Relic::RunicCube),
            RelicKey::TheAbacus => Some(Relic::TheAbacus),
            RelicKey::GremlinHorn => Some(Relic::GremlinHorn),
            RelicKey::Sundial => Some(Relic::Sundial),
            RelicKey::CharonsAshes => Some(Relic::CharonsAshes),
            RelicKey::BlueCandle => Some(Relic::BlueCandle),
            RelicKey::MedicalKit => Some(Relic::MedicalKit),
            RelicKey::LizardTail => Some(Relic::LizardTail),
            RelicKey::Pocketwatch => Some(Relic::Pocketwatch),
            RelicKey::HandDrill => Some(Relic::HandDrill),
            RelicKey::Necronomicon | RelicKey::Enchiridion | RelicKey::NilrysCodex => None,
            RelicKey::GoldenIdol => Some(Relic::GoldenIdol),
            RelicKey::BloodyIdol => Some(Relic::BloodyIdol),
            RelicKey::Circlet => Some(Relic::Circlet),
            RelicKey::RedCirclet => Some(Relic::RedCirclet),
            RelicKey::SacredBark => Some(Relic::SacredBark),
            RelicKey::RunicPyramid => Some(Relic::RunicPyramid),
            RelicKey::FrozenEye => Some(Relic::FrozenEye),
            RelicKey::PeacePipe => Some(Relic::PeacePipe),
            RelicKey::OrangePellets => Some(Relic::OrangePellets),
            RelicKey::Girya => Some(Relic::Girya),
            RelicKey::UnceasingTop => Some(Relic::UnceasingTop),
            RelicKey::Shovel => Some(Relic::Shovel),
            RelicKey::FossilizedHelix => Some(Relic::FossilizedHelix),
            RelicKey::BlackStar => Some(Relic::BlackStar),
            RelicKey::Matryoshka => Some(Relic::Matryoshka),
            RelicKey::EmptyCage => Some(Relic::EmptyCage),
            RelicKey::BottledFlame => Some(Relic::BottledFlame),
            RelicKey::BottledLightning => Some(Relic::BottledLightning),
            RelicKey::BottledTornado => Some(Relic::BottledTornado),
            RelicKey::DollysMirror => Some(Relic::DollysMirror),
            RelicKey::PrayerWheel => Some(Relic::PrayerWheel),
            RelicKey::CrackedCore => Some(Relic::CrackedCore),
            RelicKey::FrozenCore => Some(Relic::FrozenCore),
            RelicKey::PureWater => Some(Relic::PureWater),
            RelicKey::HolyWater => Some(Relic::HolyWater),
            RelicKey::RingOfTheSnake => Some(Relic::RingOfTheSnake),
            RelicKey::RingOfTheSerpent => Some(Relic::RingOfTheSerpent),
            RelicKey::Cauldron => Some(Relic::Cauldron),
            RelicKey::TinyHouse => Some(Relic::TinyHouse),
            RelicKey::DeadBranch => Some(Relic::DeadBranch),
            RelicKey::MummifiedHand => Some(Relic::MummifiedHand),
            RelicKey::TheCourier => Some(Relic::TheCourier),
            RelicKey::IncenseBurner => Some(Relic::IncenseBurner),
            RelicKey::CursedKey => Some(Relic::CursedKey),
            RelicKey::TinyChest => Some(Relic::TinyChest),
            RelicKey::Orrery => Some(Relic::Orrery),
            RelicKey::SneckoEye => Some(Relic::SneckoEye),
            RelicKey::StrangeSpoon => Some(Relic::StrangeSpoon),
            RelicKey::WingBoots => Some(Relic::WingBoots),
            RelicKey::CallingBell => Some(Relic::CallingBell),
            RelicKey::PandorasBox => Some(Relic::PandorasBox),
            RelicKey::Astrolabe => Some(Relic::Astrolabe),
            RelicKey::GamblingChip => Some(Relic::GamblingChip),
            RelicKey::Toolbox => Some(Relic::Toolbox),
            RelicKey::JuzuBracelet => Some(Relic::JuzuBracelet),
            RelicKey::PrismaticShard => Some(Relic::PrismaticShard),
            RelicKey::MutagenicStrength => Some(Relic::MutagenicStrength),
        }
    }
}
