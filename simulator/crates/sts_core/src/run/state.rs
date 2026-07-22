use crate::{
    card::{CardInstance, CardType},
    combat::state::BASE_PLAYER_ENERGY,
    combat::{CombatDecisionState, CombatState},
    content::cards::{
        card_instance_after_upgrades, card_instance_is_upgradeable, card_type_and_rarity,
        get_card_definition, is_basic_starter_card, is_curse_content_id, upgrade_card_instance,
        validate_searing_blow_metadata,
    },
    content::character::IRONCLAD_A0_BASE_HP,
    content::reward_pool::ironclad_reward_card_rarity,
    content::shop_pool::{
        colorless_discovery_card_choices, discovery_card_choices, shop_card_is_colorless,
        shop_card_type,
    },
    ids::{
        card_instance_id_is_supported, reserve_card_instance_id_range, CardId, ContentId, MonsterId,
    },
    map::{generate_target_fixed_map, milestone8_fixture, MapRunState, RoomKind, TargetMapAct},
    potion::{Potion, MAX_POTIONS},
    relic::{
        apply_start_of_combat_relics, initialize_ironclad_relic_pools, Relic, RelicKey,
        RelicPoolState, RelicSpawnContext, ANCIENT_TEA_SET_ENERGY, BLOODY_IDOL_HEAL,
        BUSTED_CROWN_ENERGY, CAULDRON_POTIONS, CERAMIC_FISH_GOLD, COFFEE_DRIPPER_ENERGY,
        DARKSTONE_PERIAPT_MAX_HP, DU_VU_DOLL_STRENGTH_PER_CURSE, ECTOPLASM_ENERGY,
        ETERNAL_FEATHER_HEAL_PER_FIVE_CARDS, FUSION_HAMMER_ENERGY, GIRYA_MAX_LIFTS,
        HAPPY_FLOWER_THRESHOLD, INCENSE_BURNER_THRESHOLD, INK_BOTTLE_THRESHOLD, LEES_WAFFLE_MAX_HP,
        MANGO_MAX_HP, MARK_OF_PAIN_ENERGY, MATRYOSHKA_MAX_CHESTS, MAW_BANK_GOLD,
        NUNCHAKU_THRESHOLD, OLD_COIN_GOLD, OMAMORI_CHARGES, ORRERY_CARD_REWARDS, PANTOGRAPH_HEAL,
        PEAR_MAX_HP, PEN_NIB_THRESHOLD, PHILOSOPHERS_STONE_ENERGY,
        PHILOSOPHERS_STONE_MONSTER_STRENGTH, POTION_BELT_SLOTS, PRESERVED_INSECT_HP_DENOMINATOR,
        PRESERVED_INSECT_HP_NUMERATOR, RUNIC_DOME_ENERGY, SLAVERS_COLLAR_ENERGY,
        SLING_OF_COURAGE_STRENGTH, SOZU_ENERGY, SSSERPENT_HEAD_GOLD, STRAWBERRY_MAX_HP,
        TINY_CHEST_THRESHOLD, TINY_HOUSE_GOLD, TINY_HOUSE_HEAL, TINY_HOUSE_MAX_HP,
        VELVET_CHOKER_ENERGY, WING_BOOTS_CHARGES,
    },
    rng::{rng_counter_is_supported, JavaRng, StsRng},
    SimError, SimResult,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, num::NonZeroU8};

pub use crate::content::encounters::{Act1Boss, Act3Boss};

pub const STARTING_GOLD: i32 = 99;
const ENCHIRIDION_HAND_LIMIT: usize = 10;
pub(crate) const NEOW_LAMENT_COMBATS: u32 = 3;

fn default_energy_per_turn() -> i32 {
    BASE_PLAYER_ENERGY
}

fn checked_combat_initialization_add(value: i32, amount: i32) -> SimResult<i32> {
    value.checked_add(amount).ok_or(SimError::InvalidState(
        "combat integer addition overflows i32",
    ))
}

fn checked_run_add(value: i32, amount: i32) -> SimResult<i32> {
    value
        .checked_add(amount)
        .ok_or(SimError::InvalidState("run integer addition overflows i32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_house_upgrades_the_target_starter_instance_on_session32_seed() {
        let mut run = RunState::seeded_ironclad(5_556_398_760_754_084_786_u64, 0);
        // Neow consumes one hidden miscRng draw before equipping its relic.
        run.misc_rng_counter = 1;
        run.relics = vec![Relic::BurningBlood];

        let reward = crate::run::neow::apply_neow_boss_swap(&mut run)
            .expect("seed-start fixture can allocate its boss relic reward");
        assert_eq!(reward.relic, RelicKey::TinyHouse);

        let upgraded = run
            .deck
            .iter()
            .filter(|card| {
                matches!(
                    card.content_id,
                    id if id == crate::content::cards::STRIKE_R_PLUS_ID
                        || id == crate::content::cards::DEFEND_R_PLUS_ID
                        || id == crate::content::cards::BASH_PLUS_ID
                )
            })
            .map(|card| (card.id, card.content_id))
            .collect::<Vec<_>>();
        assert_eq!(
            upgraded,
            vec![(CardId::new(2), crate::content::cards::STRIKE_R_PLUS_ID)]
        );
        assert_eq!(
            run.reward.as_ref().and_then(|reward| reward.potion_offer),
            Some(Potion::Colorless)
        );
        assert_eq!(
            run.reward.as_ref().map(|reward| reward.continuation),
            Some(RewardContinuation::Neow)
        );
    }

    #[test]
    fn seeded_ironclad_uses_canonical_state_without_the_map_fixture() {
        let run = RunState::seeded_ironclad(22_079_335_079, 10);
        let fixture = RunState::map_fixture();

        run.validate().expect("seeded production run is valid");
        assert_eq!(run.phase, RunPhase::Event);
        assert_eq!(run.ascension, 10);
        assert_eq!(
            run.deck,
            crate::content::deck::ironclad_starter_deck_for_ascension(10)
        );
        assert_eq!(run.relics, vec![Relic::BurningBlood]);
        assert_ne!(run.map, fixture.map);
    }

    #[test]
    fn card_random_floor_seed_wraps_in_every_build_profile() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.reward_rng_seed = u64::MAX;
        run.current_floor = 1;

        assert_eq!(run.rng_stream_state(RunRngStream::CardRandom).seed, 0);
    }

    #[test]
    fn run_validation_rejects_card_ids_outside_the_allocation_domain() {
        let mut run = RunState::map_fixture();
        run.deck[0].id = CardId::new(0);
        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "card instance ID is outside the supported allocation range"
            ))
        );

        run.deck[0].id = CardId::new(i64::MAX as u64 + 1);
        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "card instance ID is outside the supported allocation range"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_combat_local_rampage_metadata() {
        let mut run = RunState::map_fixture();
        run.deck[0].rampage_damage_bonus = 5;

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "run card retains a combat-local Rampage damage bonus"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_combat_local_card_cost_metadata() {
        let mut temp_cost = RunState::map_fixture();
        temp_cost.deck[0].temp_cost = Some(0);
        assert_eq!(
            temp_cost.validate(),
            Err(SimError::InvalidState(
                "run card retains combat-local temporary cost metadata"
            ))
        );

        let mut combat_only = RunState::map_fixture();
        combat_only.deck[0].combat_only = true;
        assert_eq!(
            combat_only.validate(),
            Err(SimError::InvalidState("run card is marked as combat-only"))
        );

        let mut reduction = RunState::map_fixture();
        reduction.deck[0].blood_for_blood_cost_reduction = 1;
        assert_eq!(
            reduction.validate(),
            Err(SimError::InvalidState(
                "run card retains combat-local Blood for Blood cost reduction"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_unrepresentable_note_card_upgrades() {
        let mut run = RunState::map_fixture();
        run.note_card_content_id = crate::content::cards::BASH_ID;
        run.note_card_upgrades = 2;

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "card upgrade count exceeds its content upgrade path"
            ))
        );

        run.note_card_content_id = crate::content::cards::SEARING_BLOW_PLUS_ID;
        run.note_card_upgrades = 0;
        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "Searing Blow+ is missing its upgrade count"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_rng_counters_outside_the_target_domain() {
        let mut run = RunState::map_fixture();
        run.monster_rng_counter = i32::MAX as u32 + 1;

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "run RNG counter exceeds the target signed range"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_consumed_or_impossible_relic_counters() {
        let mut run = RunState::map_fixture();
        run.tiny_chest_counter = TINY_CHEST_THRESHOLD - 1;
        run.wing_boots_charges = u32::from(WING_BOOTS_CHARGES);
        run.validate().expect("stable counter maxima are valid");

        run.tiny_chest_counter = TINY_CHEST_THRESHOLD;
        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "run relic counter is outside its stable range"
            ))
        );

        run.tiny_chest_counter = 0;
        run.wing_boots_charges = u32::from(WING_BOOTS_CHARGES) + 1;
        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "run relic counter is outside its stable range"
            ))
        );
    }

    #[test]
    fn run_validation_rejects_active_neows_lament_without_owned_identity() {
        let mut run = RunState::map_fixture();
        run.neow_lament_combats_remaining = 1;

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "active Neow's Lament counter has no owned relic"
            ))
        );
    }

    #[test]
    fn healing_at_the_target_integer_limit_clamps_without_overflow() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = i32::MAX;
        run.player_hp = i32::MAX - 2;

        run.heal_player(10).expect("healing remains representable");

        assert_eq!(run.player_hp, i32::MAX);
        run.validate().expect("clamped healing remains valid");
    }

    #[test]
    fn gold_gain_and_floor_relic_overflow_roll_back_exactly() {
        let mut direct = RunState::map_fixture();
        direct.gold = i32::MAX;
        let direct_before = direct.clone();
        assert_eq!(
            direct.gain_gold(1),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(direct, direct_before);

        let mut late_failure = RunState::map_fixture();
        late_failure.gold = 100;
        late_failure.player_max_hp = i32::MAX;
        late_failure.player_hp = i32::MIN;
        late_failure.relics.push(Relic::BloodyIdol);
        let late_before = late_failure.clone();
        assert_eq!(
            late_failure.gain_gold(1),
            Err(SimError::InvalidState("run HP difference overflows i32"))
        );
        assert_eq!(late_failure, late_before);

        let mut floor = RunState::map_fixture();
        floor.gold = i32::MAX - MAW_BANK_GOLD;
        floor.current_room_override = Some(RoomKind::Event);
        floor.relics.extend([Relic::MawBank, Relic::SsserpentHead]);
        let floor_before = floor.clone();
        assert_eq!(
            floor.apply_floor_entry_relics(),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(floor, floor_before);
    }

    #[test]
    fn immediate_neow_reward_rejects_overflow_and_wrong_reward_kind() {
        let mut run = RunState::map_fixture();
        run.player_max_hp = i32::MAX;
        let before = run.clone();
        assert_eq!(
            crate::run::neow::apply_neow_simple_reward(
                &mut run,
                crate::run::neow::NeowRewardType::TenPercentHpBonus,
            ),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(run, before);

        assert_eq!(
            crate::run::neow::apply_neow_simple_reward(
                &mut run,
                crate::run::neow::NeowRewardType::BossRelic,
            ),
            Err(SimError::IllegalAction(
                "Neow reward is not a simple immediate reward"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn max_hp_gain_and_floor_advance_reject_overflow_atomically() {
        let mut max_hp = RunState::map_fixture();
        max_hp.player_max_hp = i32::MAX;
        let max_hp_before = max_hp.clone();
        assert_eq!(
            max_hp.gain_max_hp(1),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(max_hp, max_hp_before);

        let mut current_hp = RunState::map_fixture();
        current_hp.player_hp = i32::MAX;
        let current_hp_before = current_hp.clone();
        assert_eq!(
            current_hp.gain_max_hp(1),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(current_hp, current_hp_before);

        assert_eq!(
            current_hp.gain_max_hp(-1),
            Err(SimError::IllegalAction("max HP gain cannot be negative"))
        );
        assert_eq!(current_hp, current_hp_before);

        let mut floor = RunState::map_fixture();
        floor.current_floor = i32::MAX;
        let floor_before = floor.clone();
        assert_eq!(
            floor.advance_floor(),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(floor, floor_before);
    }

    #[test]
    fn relic_integer_overflow_is_rejected_without_mutating_run() {
        let mut hp_run = RunState::map_fixture();
        hp_run.player_max_hp = i32::MAX;
        let hp_before = hp_run.clone();
        assert_eq!(
            hp_run.gain_relic(Relic::Strawberry),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(hp_run, hp_before);

        let mut energy_run = RunState::map_fixture();
        energy_run.energy_per_turn = i32::MAX;
        let energy_before = energy_run.clone();
        assert_eq!(
            energy_run.gain_relic(Relic::CoffeeDripper),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(energy_run, energy_before);

        let mut gold_run = RunState::map_fixture();
        gold_run.gold = i32::MAX;
        let gold_before = gold_run.clone();
        assert_eq!(
            gold_run.gain_relic(Relic::OldCoin),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(gold_run, gold_before);

        let mut reward_run = RunState::map_fixture();
        reward_run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: CardRewardFlow::pending(u8::MAX),
        });
        let reward_before = reward_run.clone();
        assert_eq!(
            reward_run.gain_relic(Relic::TinyHouse),
            Err(SimError::InvalidState("card reward count overflows u8"))
        );
        assert_eq!(reward_run, reward_before);
    }

    #[test]
    fn snecko_eye_does_not_grant_energy() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::SneckoEye)
            .expect("Snecko Eye pickup succeeds");
        let combat = run
            .init_combat(CombatState::cultist_fixture())
            .expect("combat initializes");

        assert_eq!(run.energy_per_turn, BASE_PLAYER_ENERGY);
        assert_eq!(combat.player.max_energy, BASE_PLAYER_ENERGY);
        assert_eq!(combat.player.energy, BASE_PLAYER_ENERGY);
    }

    #[test]
    fn deck_card_gain_rejects_exhausted_instance_ids_without_mutating_run() {
        let mut run = RunState::map_fixture();
        run.deck[0].id = CardId::new(crate::ids::MAX_SUPPORTED_CARD_INSTANCE_ID);
        let before = run.clone();

        assert_eq!(
            run.gain_deck_card(crate::content::cards::ANGER_ID),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn deck_card_gain_rejects_unknown_content_without_relic_side_effects() {
        let mut run = RunState::map_fixture();
        run.relics.extend([Relic::CeramicFish, Relic::MoltenEgg]);
        let before = run.clone();
        let unknown = ContentId::new(999_999);

        assert_eq!(
            run.gain_deck_card(unknown),
            Err(SimError::UnknownContent(unknown))
        );
        assert_eq!(run, before);
        assert_eq!(
            run.content_id_after_card_add_relics(crate::content::cards::STRIKE_R_ID),
            Ok(crate::content::cards::STRIKE_R_PLUS_ID)
        );
        assert_eq!(
            run.content_id_after_card_add_relics(crate::content::cards::STRIKE_R_PLUS_ID),
            Ok(crate::content::cards::STRIKE_R_PLUS_ID)
        );
    }

    #[test]
    fn deck_card_add_rejects_duplicate_ids_and_combat_metadata_atomically() {
        let mut duplicate = RunState::map_fixture();
        let duplicate_before = duplicate.clone();
        assert_eq!(
            duplicate.add_deck_card(duplicate.deck[0]),
            Err(SimError::InvalidState(
                "duplicate run deck card instance ID"
            ))
        );
        assert_eq!(duplicate, duplicate_before);

        let mut combat_metadata = RunState::map_fixture();
        let mut card = CardInstance::new(CardId::new(100), crate::content::cards::ANGER_ID);
        card.temp_cost = Some(0);
        let metadata_before = combat_metadata.clone();
        assert_eq!(
            combat_metadata.add_deck_card(card),
            Err(SimError::InvalidState(
                "run card retains combat-local temporary cost metadata"
            ))
        );
        assert_eq!(combat_metadata, metadata_before);
    }

    #[test]
    fn card_added_relic_overflow_is_rejected_without_mutating_run() {
        let mut run = RunState::map_fixture();
        run.relics.push(Relic::CeramicFish);
        run.gold = i32::MAX;
        let before = run.clone();

        assert_eq!(
            run.add_deck_card(CardInstance::new(
                CardId::new(100),
                crate::content::cards::ANGER_ID,
            )),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(run, before);

        let mut hp_run = RunState::map_fixture();
        hp_run.relics.push(Relic::DarkstonePeriapt);
        hp_run.player_max_hp = i32::MAX;
        let hp_before = hp_run.clone();

        assert_eq!(
            hp_run.add_deck_card(CardInstance::new(
                CardId::new(100),
                crate::content::cards::PAIN_ID,
            )),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(hp_run, hp_before);
    }

    #[test]
    fn combat_initialization_rejects_starting_relic_arithmetic_overflow() {
        let mut energy_run = RunState::map_fixture();
        energy_run.energy_per_turn = i32::MAX;
        energy_run.relics = vec![Relic::Lantern];
        assert_eq!(
            energy_run.init_combat(CombatState::initial_fixture()),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );

        let mut strength_run = RunState::map_fixture();
        strength_run.relics = vec![Relic::Vajra];
        let mut base = CombatState::initial_fixture();
        base.player.powers.strength = i32::MAX;
        base.validate().expect("input combat is valid");
        assert_eq!(
            strength_run.init_combat(base),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );

        let mut red_skull_run = RunState::map_fixture();
        red_skull_run.relics = vec![Relic::RedSkull];
        red_skull_run.player_hp = red_skull_run.player_max_hp / 2;
        let mut base = CombatState::initial_fixture();
        base.player.powers.strength = i32::MAX;
        base.validate().expect("input combat is valid");
        assert_eq!(
            red_skull_run.init_combat(base),
            Err(SimError::InvalidState(
                "Red Skull Strength activation overflows i32"
            ))
        );
    }

    #[test]
    fn combat_initialization_rejects_first_turn_counter_overflow() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Brimstone];
        let mut base = CombatState::initial_fixture();
        base.relic_counters.player_turns_started = i32::MAX as u32;
        base.validate().expect("input combat is valid");

        assert_eq!(
            run.init_combat(base),
            Err(SimError::InvalidState(
                "combat relic counter exceeds the target signed range"
            ))
        );
    }

    #[test]
    fn blood_vial_at_the_target_hp_limit_clamps_without_overflow() {
        let mut run = RunState::map_fixture();
        run.player_hp = i32::MAX;
        run.player_max_hp = i32::MAX;
        run.relics = vec![Relic::BloodVial];

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");

        assert_eq!(combat.player.hp, i32::MAX);
        assert_eq!(combat.player.max_hp, i32::MAX);
    }

    #[test]
    fn failed_consuming_combat_initialization_does_not_mutate_run_state() {
        let mut run = RunState::map_fixture();
        run.energy_per_turn = i32::MAX;
        run.relics = vec![Relic::Lantern];
        run.neow_lament_combats_remaining = 1;
        run.ancient_tea_set_armed = true;
        let before = run.clone();

        assert_eq!(
            run.init_combat_consuming_relics(CombatState::initial_fixture()),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn card_reward_flow_opens_closes_and_consumes_exactly_once() {
        let reward = CardRewardFlow::pending(2);

        let mut screen = RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: reward,
        };

        screen.open_card_reward().expect("first reward opens");
        assert_eq!(screen.card_reward_flow, CardRewardFlow::active(2));
        screen.close_card_reward().expect("first reward closes");
        assert_eq!(screen.card_reward_flow, CardRewardFlow::pending(2));
        screen.open_card_reward().expect("first reward reopens");
        screen
            .consume_active_card_reward()
            .expect("first reward consumes");
        assert_eq!(screen.card_reward_flow, CardRewardFlow::pending(1));
        screen.open_card_reward().expect("second reward opens");
        screen
            .consume_active_card_reward()
            .expect("second reward consumes");
        assert_eq!(screen.card_reward_flow, CardRewardFlow::None);
    }

    #[test]
    fn zero_count_reward_flows_are_invalid() {
        assert!(
            serde_json::from_str::<CardRewardFlow>(r#"{"state":"pending","remaining":0}"#).is_err()
        );
        assert!(
            serde_json::from_str::<CardRewardFlow>(r#"{"state":"active","remaining":0}"#).is_err()
        );
    }
}

fn add_enchiridion_power_to_hand(combat: &mut CombatState) -> SimResult<()> {
    if combat.piles.hand.len() >= ENCHIRIDION_HAND_LIMIT {
        return Ok(());
    }

    let next_id = CardId::new(combat.next_card_instance_id()?);
    let content_id = discovery_card_choices(&mut combat.rng.card_random_rng, CardType::Power, 1)[0];
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    card.temp_cost = Some(0);
    card.temp_cost_turn_only = true;
    combat.piles.hand.push(card);
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_and_keep: Option<super::event::MatchAndKeepState>,
    pub shop: Option<super::shop::ShopScreen>,
    #[serde(default)]
    pub shop_merchant_open: bool,
    #[serde(default)]
    pub card_grid: Option<super::grid::CardGridScreen>,
    #[serde(default)]
    pub relics: Vec<Relic>,
    #[serde(default)]
    pub potions: Vec<Potion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub empty_potion_slots: Vec<usize>,
    /// Cards queued by visual obtain effects that have not committed to the
    /// master deck in the canonical simulator state yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_obtain_cards: Vec<ContentId>,
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
    pub shuffle_rng_seed: u64,
    #[serde(default)]
    pub shuffle_rng_counter: u32,
    #[serde(default)]
    pub relic_pools: Option<RelicPoolState>,
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
    pub pen_nib_attacks_played: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub ink_bottle_cards_played: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub happy_flower_turns: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub sundial_shuffles: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub nunchaku_attacks_played: u32,
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
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_event_combat_gold_offer: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_event_combat_relic_offer: Option<Relic>,
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
    /// Source `CardCrawlGame.playtime`, used by Secret Portal eligibility.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub playtime_seconds: u32,
    /// Profile-backed card used by Note For Yourself.
    #[serde(default = "default_note_card_content_id")]
    pub note_card_content_id: ContentId,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub note_card_upgrades: u8,
    #[serde(default, skip_serializing_if = "Act1Boss::is_default")]
    pub act1_boss: Act1Boss,
    #[serde(default, skip_serializing_if = "Act3Boss::is_default")]
    pub act3_boss: Act3Boss,
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
    pub act3_event_list: Vec<super::event::Event>,
    #[serde(default)]
    pub act3_shrine_list: Vec<super::event::Event>,
    #[serde(default)]
    pub special_one_time_event_list: Vec<super::event::Event>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub special_one_time_events_initialized: bool,
    #[serde(default)]
    pub ascension: u8,
    #[serde(default)]
    pub treasure_room: Option<super::reward::TreasureRoomState>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub boss_chest_opened: bool,
    /// Boss relic choices retained while the selection screen is closed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_boss_relic_choices: Vec<RelicKey>,
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
    Shuffle,
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
    Complete,
}

pub const REWARD_GOLD_AMOUNT: i32 = 20;

fn default_card_rarity_factor() -> i32 {
    5
}

fn default_note_card_content_id() -> ContentId {
    crate::content::cards::IRON_WAVE_ID
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
#[serde(deny_unknown_fields)]
pub struct RewardScreen {
    #[serde(default)]
    pub continuation: RewardContinuation,
    pub choices: Vec<CardInstance>,
    /// Card reward choices generated eagerly by effects such as Orrery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_card_rewards: Vec<Vec<CardInstance>>,
    pub gold_offer: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub stolen_gold_offer: i32,
    pub potion_offer: Option<Potion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub potion_offers: Vec<Potion>,
    pub relic_offer: Option<Relic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_relic_offer: Option<Relic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_relic_offers: Vec<Relic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boss_relic_choices: Vec<RelicKey>,
    /// State of the card-reward subflow. `remaining` includes an active screen.
    #[serde(default)]
    pub card_reward_flow: CardRewardFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RewardContinuation {
    #[default]
    None,
    Rest,
    Event,
    Shop,
    Map,
    Treasure,
    Neow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CardRewardFlow {
    #[default]
    None,
    Pending {
        remaining: NonZeroU8,
    },
    Active {
        remaining: NonZeroU8,
    },
}

impl CardRewardFlow {
    #[must_use]
    pub const fn pending(remaining: u8) -> Self {
        match NonZeroU8::new(remaining) {
            Some(remaining) => Self::Pending { remaining },
            None => panic!("pending card reward flow requires a positive count"),
        }
    }

    #[must_use]
    pub const fn active(remaining: u8) -> Self {
        match NonZeroU8::new(remaining) {
            Some(remaining) => Self::Active { remaining },
            None => panic!("active card reward flow requires a positive count"),
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    #[must_use]
    pub const fn remaining(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Pending { remaining } | Self::Active { remaining } => remaining.get(),
        }
    }
}

impl RewardScreen {
    #[must_use]
    pub fn remaining_card_reward_count(&self) -> u8 {
        self.card_reward_flow.remaining()
    }

    #[must_use]
    pub fn card_reward_is_active(&self) -> bool {
        self.card_reward_flow.is_active()
    }

    #[must_use]
    pub fn card_reward_is_pending(&self) -> bool {
        self.card_reward_flow.is_pending()
    }

    pub(crate) fn set_card_reward_remaining(&mut self, count: u8) {
        self.card_reward_flow = match (self.card_reward_flow, count) {
            (_, 0) => CardRewardFlow::None,
            (CardRewardFlow::Active { .. }, remaining) => CardRewardFlow::active(remaining),
            (_, remaining) => CardRewardFlow::pending(remaining),
        };
    }

    pub fn open_card_reward(&mut self) -> SimResult<()> {
        self.card_reward_flow = match self.card_reward_flow {
            CardRewardFlow::Pending { remaining } => CardRewardFlow::active(remaining.get()),
            CardRewardFlow::None => {
                return Err(SimError::InvalidState(
                    "cannot open absent card reward flow",
                ));
            }
            CardRewardFlow::Active { .. } => {
                return Err(SimError::InvalidState("card reward flow is already active"));
            }
        };
        Ok(())
    }

    pub fn close_card_reward(&mut self) -> SimResult<()> {
        self.card_reward_flow = match self.card_reward_flow {
            CardRewardFlow::Active { remaining } => CardRewardFlow::pending(remaining.get()),
            CardRewardFlow::None | CardRewardFlow::Pending { .. } => {
                return Err(SimError::InvalidState("card reward flow is not active"));
            }
        };
        Ok(())
    }

    pub fn consume_active_card_reward(&mut self) -> SimResult<()> {
        self.card_reward_flow = match self.card_reward_flow {
            CardRewardFlow::Active { remaining } if remaining.get() == 1 => CardRewardFlow::None,
            CardRewardFlow::Active { remaining } => CardRewardFlow::pending(remaining.get() - 1),
            CardRewardFlow::None | CardRewardFlow::Pending { .. } => {
                return Err(SimError::InvalidState("card reward flow is not active"));
            }
        };
        Ok(())
    }
}

fn validate_run_card_content(card: &CardInstance) -> SimResult<()> {
    validate_run_card_instance_id(card)?;
    validate_run_card_metadata(card)?;
    get_card_definition(card.content_id)
        .map(|_| ())
        .ok_or(SimError::UnknownContent(card.content_id))
}

fn validate_run_choice_card_content(card: &CardInstance) -> SimResult<()> {
    validate_run_card_instance_id(card)?;
    validate_run_card_metadata(card)?;
    if get_card_definition(card.content_id).is_some()
        || ironclad_reward_card_rarity(card.content_id).is_some()
        || super::reward::any_color_reward_card_key(card.content_id).is_some()
    {
        Ok(())
    } else {
        Err(SimError::UnknownContent(card.content_id))
    }
}

fn validate_run_card_metadata(card: &CardInstance) -> SimResult<()> {
    validate_searing_blow_metadata(card)?;
    if card.temp_cost.is_some() || card.temp_cost_turn_only {
        return Err(SimError::InvalidState(
            "run card retains combat-local temporary cost metadata",
        ));
    }
    if card.combat_only {
        return Err(SimError::InvalidState("run card is marked as combat-only"));
    }
    if card.blood_for_blood_cost_reduction != 0 {
        return Err(SimError::InvalidState(
            "run card retains combat-local Blood for Blood cost reduction",
        ));
    }
    if card.rampage_damage_bonus != 0 {
        return Err(SimError::InvalidState(
            "run card retains a combat-local Rampage damage bonus",
        ));
    }
    Ok(())
}

fn validate_run_card_instance_id(card: &CardInstance) -> SimResult<()> {
    if !card_instance_id_is_supported(card.id) {
        return Err(SimError::InvalidState(
            "card instance ID is outside the supported allocation range",
        ));
    }
    Ok(())
}

fn validate_run_choice_cards(cards: &[CardInstance]) -> SimResult<()> {
    for card in cards {
        validate_run_choice_card_content(card)?;
    }
    Ok(())
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
    TakePotionReward {
        index: usize,
    },
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
    SkipCombatCardReward,
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
    /// Validates invariants required by authoritative run transitions.
    ///
    /// Overlay screens may legitimately coexist with their owning phase, so
    /// this rejects contradictory ownership without normalizing valid
    /// event/reward/grid subflows.
    pub fn validate(&self) -> SimResult<()> {
        if [
            self.card_rng_counter,
            self.card_random_rng_counter,
            self.treasure_rng_counter,
            self.potion_rng_counter,
            self.relic_rng_counter,
            self.shuffle_rng_counter,
            self.merchant_rng_counter,
            self.event_rng_counter,
            self.misc_rng_counter,
            self.monster_rng_counter,
        ]
        .into_iter()
        .any(|counter| !rng_counter_is_supported(counter))
        {
            return Err(SimError::InvalidState(
                "run RNG counter exceeds the target signed range",
            ));
        }
        if self.omamori_charges_used > OMAMORI_CHARGES
            || self.girya_lifts > GIRYA_MAX_LIFTS
            || self.matryoshka_chests_opened > MATRYOSHKA_MAX_CHESTS
            || self.incense_burner_counter >= INCENSE_BURNER_THRESHOLD
            || self.pen_nib_attacks_played >= PEN_NIB_THRESHOLD
            || self.ink_bottle_cards_played >= INK_BOTTLE_THRESHOLD
            || self.happy_flower_turns >= HAPPY_FLOWER_THRESHOLD
            || self.nunchaku_attacks_played >= NUNCHAKU_THRESHOLD
            || self.tiny_chest_counter >= TINY_CHEST_THRESHOLD
            || self.wing_boots_charges > u32::from(WING_BOOTS_CHARGES)
            || self.neow_lament_combats_remaining > NEOW_LAMENT_COMBATS
        {
            return Err(SimError::InvalidState(
                "run relic counter is outside its stable range",
            ));
        }
        if self.neow_lament_combats_remaining > 0 && !self.relics.contains(&Relic::NeowsLament) {
            return Err(SimError::InvalidState(
                "active Neow's Lament counter has no owned relic",
            ));
        }
        if self.ascension > 20 {
            return Err(SimError::InvalidState("run ascension exceeds 20"));
        }
        if self.shop_remove_count > super::shop::MAX_SHOP_REMOVE_COUNT {
            return Err(SimError::InvalidState(
                "shop remove count exceeds the supported price range",
            ));
        }
        if self.player_max_hp <= 0 || self.player_hp < 0 || self.player_hp > self.player_max_hp {
            return Err(SimError::InvalidState("run player HP is out of bounds"));
        }
        if self.gold < 0 || self.energy_per_turn < 0 {
            return Err(SimError::InvalidState("run gold or energy is negative"));
        }
        if self.current_floor < 0 || !(1..=4).contains(&self.current_act) {
            return Err(SimError::InvalidState("run floor or act is out of bounds"));
        }
        if let Some(map) = self.map.as_ref() {
            map.validate()?;
            if i32::try_from(map.floor).is_err() {
                return Err(SimError::InvalidState(
                    "map floor exceeds supported run range",
                ));
            }
        }

        let mut deck_ids = BTreeSet::new();
        for card in &self.deck {
            if !deck_ids.insert(card.id) {
                return Err(SimError::InvalidState(
                    "duplicate run deck card instance ID",
                ));
            }
            validate_run_card_content(card)?;
        }
        for content_id in &self.pending_obtain_cards {
            if get_card_definition(*content_id).is_none() {
                return Err(SimError::UnknownContent(*content_id));
            }
        }
        if get_card_definition(self.note_card_content_id).is_none() {
            return Err(SimError::UnknownContent(self.note_card_content_id));
        }
        card_instance_after_upgrades(
            CardInstance::new(CardId::new(1), self.note_card_content_id),
            self.note_card_upgrades,
        )?;
        let mut owned_relics = Vec::with_capacity(self.relics.len());
        for relic in &self.relics {
            if *relic != Relic::Circlet && owned_relics.contains(relic) {
                return Err(SimError::InvalidState("duplicate owned relic"));
            }
            owned_relics.push(*relic);
        }

        match (&self.phase, &self.combat) {
            (RunPhase::Combat, Some(combat)) => {
                combat.validate()?;
                if combat.ascension != self.ascension {
                    return Err(SimError::InvalidState(
                        "run and combat ascension do not match",
                    ));
                }
            }
            (RunPhase::Combat, None) => {
                return Err(SimError::InvalidState("combat phase has no combat state"));
            }
            (_, Some(_)) => {
                return Err(SimError::InvalidState(
                    "combat state exists outside combat phase",
                ));
            }
            (_, None) => {}
        }

        if self.shop_merchant_open && self.shop.is_none() {
            return Err(SimError::InvalidState("open merchant has no shop screen"));
        }
        match self.phase {
            RunPhase::Reward if self.reward.is_none() => {
                return Err(SimError::InvalidState("reward phase has no reward screen"));
            }
            RunPhase::Event if self.event.is_none() => {
                return Err(SimError::InvalidState("event phase has no event screen"));
            }
            RunPhase::Shop if self.shop.is_none() => {
                return Err(SimError::InvalidState("shop phase has no shop screen"));
            }
            RunPhase::Rest if self.current_room_kind() != Some(RoomKind::Rest) => {
                return Err(SimError::InvalidState("rest phase is not in a rest room"));
            }
            RunPhase::Treasure
                if self.treasure_room.is_none()
                    && self.current_room_kind() != Some(RoomKind::Boss) =>
            {
                return Err(SimError::InvalidState(
                    "treasure phase has no treasure room",
                ));
            }
            _ => {}
        }

        if self.reward.is_some() && self.phase != RunPhase::Reward {
            return Err(SimError::InvalidState(
                "reward screen exists outside reward phase",
            ));
        }
        let has_terminal_event = self.phase == RunPhase::Complete
            && self.event.as_ref().is_some_and(|event| {
                event.event == super::event::Event::SpireHeart
                    && event.stage == 4
                    && event.choices.is_empty()
            });
        if self.event.is_some()
            && !matches!(self.phase, RunPhase::Event | RunPhase::Reward)
            && !has_terminal_event
        {
            return Err(SimError::InvalidState(
                "event screen exists outside event, reward, or terminal complete phase",
            ));
        }
        if self.shop.is_some() && !matches!(self.phase, RunPhase::Shop | RunPhase::Reward) {
            return Err(SimError::InvalidState(
                "shop screen exists outside shop or reward phase",
            ));
        }
        if self.shop_merchant_open && !matches!(self.phase, RunPhase::Shop | RunPhase::Reward) {
            return Err(SimError::InvalidState(
                "open merchant exists outside shop or reward phase",
            ));
        }
        if self.phase == RunPhase::Reward && self.shop.is_some() && !self.shop_merchant_open {
            return Err(SimError::InvalidState(
                "reward-retained shop has no open merchant",
            ));
        }
        if self.treasure_room.is_some()
            && !matches!(self.phase, RunPhase::Treasure | RunPhase::Reward)
        {
            return Err(SimError::InvalidState(
                "treasure room exists outside treasure or reward phase",
            ));
        }
        if self.rest_room_complete {
            if !matches!(self.phase, RunPhase::Rest | RunPhase::Reward) {
                return Err(SimError::InvalidState(
                    "completed rest room exists outside rest or reward phase",
                ));
            }
            if self.current_room_kind() != Some(RoomKind::Rest) {
                return Err(SimError::InvalidState(
                    "completed rest state is not in a rest room",
                ));
            }
        }
        if self.boss_chest_opened || !self.pending_boss_relic_choices.is_empty() {
            if !matches!(self.phase, RunPhase::Reward | RunPhase::Treasure) {
                return Err(SimError::InvalidState(
                    "boss chest state exists outside reward or treasure phase",
                ));
            }
            if self.current_room_kind() != Some(RoomKind::Boss) {
                return Err(SimError::InvalidState(
                    "boss chest state exists outside a boss room",
                ));
            }
        }

        if let Some(screen) = &self.event {
            super::event::validate_event_screen_authority(self, screen)?;
        }

        match (&self.event, &self.match_and_keep) {
            (Some(screen), Some(state)) if screen.event == super::event::Event::MatchAndKeep => {
                for card in &state.cards {
                    if get_card_definition(card.content_id).is_none() {
                        return Err(SimError::UnknownContent(card.content_id));
                    }
                }
                for content_id in &state.matched_cards {
                    if get_card_definition(*content_id).is_none() {
                        return Err(SimError::UnknownContent(*content_id));
                    }
                }
                if state
                    .first_flipped_index
                    .is_some_and(|index| index >= state.cards.len())
                    || state
                        .second_flipped_index
                        .is_some_and(|index| index >= state.cards.len())
                {
                    return Err(SimError::InvalidState(
                        "Match and Keep flipped card index is out of bounds",
                    ));
                }
                if state.second_flipped_index.is_some() && state.first_flipped_index.is_none() {
                    return Err(SimError::InvalidState(
                        "Match and Keep second flip has no first flip",
                    ));
                }
                if state.first_flipped_index.is_some()
                    && state.first_flipped_index == state.second_flipped_index
                {
                    return Err(SimError::InvalidState(
                        "Match and Keep flipped card indices are not unique",
                    ));
                }
                if state.cards.is_empty() {
                    return Err(SimError::InvalidState("Match and Keep card state is empty"));
                }
                if state.attempts_remaining > 5 {
                    return Err(SimError::InvalidState(
                        "Match and Keep attempts exceed the starting count",
                    ));
                }
                if state.second_flipped_index.is_some() {
                    return Err(SimError::InvalidState(
                        "Match and Keep retains an unresolved second flip",
                    ));
                }
                if let Some(index) = state.first_flipped_index {
                    let card = &state.cards[index];
                    if !card.revealed || card.matched {
                        return Err(SimError::InvalidState(
                            "Match and Keep first flip flags are inconsistent",
                        ));
                    }
                }
                if state
                    .cards
                    .iter()
                    .any(|card| card.matched && !card.revealed)
                {
                    return Err(SimError::InvalidState(
                        "Match and Keep matched card is not revealed",
                    ));
                }
                for content_id in state
                    .cards
                    .iter()
                    .map(|card| card.content_id)
                    .chain(state.matched_cards.iter().copied())
                {
                    let marked_count = state
                        .cards
                        .iter()
                        .filter(|card| card.content_id == content_id && card.matched)
                        .count();
                    let recorded_count = state
                        .matched_cards
                        .iter()
                        .filter(|recorded| **recorded == content_id)
                        .count();
                    if marked_count % 2 != 0 || marked_count / 2 != recorded_count {
                        return Err(SimError::InvalidState(
                            "Match and Keep matched-card accounting is inconsistent",
                        ));
                    }
                }
                match screen.stage {
                    0 | 1
                        if state.attempts_remaining != 5
                            || state.first_flipped_index.is_some()
                            || !state.matched_cards.is_empty()
                            || state.cards.iter().any(|card| card.revealed || card.matched) =>
                    {
                        return Err(SimError::InvalidState(
                            "Match and Keep intro retains modified game state",
                        ));
                    }
                    2 if state.attempts_remaining == 0 => {
                        return Err(SimError::InvalidState(
                            "Match and Keep play stage has no attempts remaining",
                        ));
                    }
                    3 if state.attempts_remaining != 0 || state.first_flipped_index.is_some() => {
                        return Err(SimError::InvalidState(
                            "Match and Keep completion state is inconsistent",
                        ));
                    }
                    _ => {}
                }
            }
            (Some(screen), None) if screen.event == super::event::Event::MatchAndKeep => {
                return Err(SimError::InvalidState("Match and Keep state is missing"));
            }
            (_, Some(_)) => {
                return Err(SimError::InvalidState(
                    "Match and Keep state exists outside its event",
                ));
            }
            _ => {}
        }

        if self.potions.len() + self.empty_potion_slots.len() > self.potion_capacity() {
            return Err(SimError::InvalidState("potion slots exceed capacity"));
        }
        let mut empty_slots = BTreeSet::new();
        for slot in &self.empty_potion_slots {
            if *slot >= self.potion_capacity() || !empty_slots.insert(*slot) {
                return Err(SimError::InvalidState("empty potion slot is invalid"));
            }
        }

        if let Some(reward) = &self.reward {
            let retained_event = self.event.is_some();
            let retained_shop = self.shop.is_some() && self.shop_merchant_open;
            let retained_rest =
                self.rest_room_complete && self.current_room_kind() == Some(RoomKind::Rest);
            let retained_map_treasure = self.treasure_room.is_some()
                && self.current_room_kind() == Some(RoomKind::Treasure);
            let retained_boss_treasure =
                self.current_room_kind() == Some(RoomKind::Boss) && self.boss_chest_opened;
            let retained_treasure = retained_map_treasure || retained_boss_treasure;
            match reward.continuation {
                RewardContinuation::None => {
                    if self.phase == RunPhase::Reward
                        && (retained_event || retained_shop || retained_rest || retained_treasure)
                    {
                        return Err(SimError::InvalidState(
                            "reward retained owner has no typed continuation",
                        ));
                    }
                }
                RewardContinuation::Rest => {
                    if !retained_rest || retained_event || retained_shop || retained_treasure {
                        return Err(SimError::InvalidState(
                            "rest reward continuation has no completed rest room",
                        ));
                    }
                }
                RewardContinuation::Event
                    if !retained_event || retained_shop || retained_rest || retained_treasure =>
                {
                    return Err(SimError::InvalidState(
                        "event reward continuation has no event screen",
                    ));
                }
                RewardContinuation::Shop
                    if !retained_shop || retained_event || retained_rest || retained_treasure =>
                {
                    return Err(SimError::InvalidState(
                        "shop reward continuation has no open merchant",
                    ));
                }
                RewardContinuation::Map
                    if !retained_map_treasure
                        || retained_event
                        || retained_shop
                        || retained_rest
                        || retained_boss_treasure =>
                {
                    return Err(SimError::InvalidState(
                        "map reward continuation has no opened treasure-room chest",
                    ));
                }
                RewardContinuation::Treasure
                    if !retained_boss_treasure
                        || retained_event
                        || retained_shop
                        || retained_rest
                        || retained_map_treasure =>
                {
                    return Err(SimError::InvalidState(
                        "treasure reward continuation has no opened chest",
                    ));
                }
                RewardContinuation::Neow
                    if !self.event.as_ref().is_some_and(|event| {
                        event.event == super::event::Event::Neow && event.stage == 2
                    }) || retained_shop
                        || retained_rest
                        || retained_treasure =>
                {
                    return Err(SimError::InvalidState(
                        "Neow reward continuation has no Neow leave screen",
                    ));
                }
                RewardContinuation::Event
                | RewardContinuation::Shop
                | RewardContinuation::Map
                | RewardContinuation::Treasure
                | RewardContinuation::Neow => {}
            }
            validate_run_choice_cards(&reward.choices)?;
            for choices in &reward.queued_card_rewards {
                validate_run_choice_cards(choices)?;
            }
            match reward.card_reward_flow {
                CardRewardFlow::None
                    if !reward.choices.is_empty() || !reward.queued_card_rewards.is_empty() =>
                {
                    return Err(SimError::InvalidState(
                        "card reward choices exist without a card reward flow",
                    ));
                }
                CardRewardFlow::Active { .. } if reward.choices.is_empty() => {
                    return Err(SimError::InvalidState("active card reward has no choices"));
                }
                CardRewardFlow::None
                | CardRewardFlow::Pending { .. }
                | CardRewardFlow::Active { .. } => {}
            }
            if reward.queued_card_rewards.len() > usize::from(reward.remaining_card_reward_count())
            {
                return Err(SimError::InvalidState(
                    "queued card rewards exceed remaining card reward screens",
                ));
            }
            if reward.gold_offer < 0 || reward.stolen_gold_offer < 0 {
                return Err(SimError::InvalidState("reward gold is negative"));
            }
        }
        if let Some(grid) = &self.card_grid {
            use super::grid::GridPurpose;

            let has_phase_owner = match grid.purpose {
                GridPurpose::RestSmith | GridPurpose::RestRemove => {
                    self.phase == RunPhase::Rest && !self.rest_room_complete
                }
                GridPurpose::ShopRemove | GridPurpose::DollysMirror => {
                    self.phase == RunPhase::Shop && self.shop.is_some() && self.shop_merchant_open
                }
                GridPurpose::EventRemove
                | GridPurpose::EventObtainCard
                | GridPurpose::EventUpgrade
                | GridPurpose::EventTransform { .. } => {
                    self.phase == RunPhase::Event && self.event.is_some()
                }
                GridPurpose::EventRemoveReturnToEvent { event }
                | GridPurpose::EventObtainCardReturnToEvent { event }
                | GridPurpose::EventUpgradeReturnToEvent { event }
                | GridPurpose::EventTransformReturnToEvent { event, .. } => {
                    self.phase == RunPhase::Event
                        && self
                            .event
                            .as_ref()
                            .is_some_and(|screen| screen.event == event)
                }
                GridPurpose::NeowRemove { .. }
                | GridPurpose::NeowUpgrade
                | GridPurpose::NeowTransform { .. } => {
                    self.phase == RunPhase::Event
                        && self
                            .event
                            .as_ref()
                            .is_some_and(|screen| screen.event == super::event::Event::Neow)
                }
                GridPurpose::BonfireElementals => {
                    self.phase == RunPhase::Event
                        && self.event.as_ref().is_some_and(|screen| {
                            screen.event == super::event::Event::BonfireElementals
                        })
                }
                GridPurpose::DesignerRemoveAndUpgrade => {
                    self.phase == RunPhase::Event
                        && self
                            .event
                            .as_ref()
                            .is_some_and(|screen| screen.event == super::event::Event::Designer)
                }
                GridPurpose::EmptyCage { .. }
                | GridPurpose::CallingBellCurse
                | GridPurpose::PandorasBox
                | GridPurpose::Astrolabe => {
                    (self.phase == RunPhase::Event
                        && self
                            .event
                            .as_ref()
                            .is_some_and(|screen| screen.event == super::event::Event::Neow))
                        || (self.phase == RunPhase::Treasure
                            && self.current_room_kind() == Some(RoomKind::Boss)
                            && self.boss_chest_opened)
                }
                GridPurpose::Bottle { .. } => match self.phase {
                    RunPhase::Event => self.event.is_some(),
                    RunPhase::Reward => self.reward.is_some(),
                    RunPhase::Shop => self.shop.is_some() && self.shop_merchant_open,
                    _ => false,
                },
            };
            if !has_phase_owner {
                return Err(SimError::InvalidState(
                    "card grid purpose has no authoritative phase owner",
                ));
            }
            validate_run_choice_cards(&grid.cards)?;
            let mut selected_indices = BTreeSet::new();
            if grid.selected.is_some_and(|index| index >= grid.cards.len())
                || grid
                    .selected_indices
                    .iter()
                    .any(|index| *index >= grid.cards.len())
            {
                return Err(SimError::InvalidState(
                    "card grid selection index is out of bounds",
                ));
            }
            if grid
                .selected_indices
                .iter()
                .any(|index| !selected_indices.insert(*index))
            {
                return Err(SimError::InvalidState(
                    "card grid selection indices contain duplicates",
                ));
            }
        }
        if let Some(shop) = &self.shop {
            for slot in &shop.cards {
                validate_run_choice_card_content(&slot.card)?;
                if !shop_card_is_colorless(slot.card.content_id)
                    && shop_card_type(slot.card.content_id).is_none()
                {
                    return Err(SimError::UnsupportedMechanic(slot.card.content_id));
                }
                if slot.price < 0 {
                    return Err(SimError::InvalidState("shop card price is negative"));
                }
            }
            if shop.relics.iter().any(|slot| slot.price < 0)
                || shop.potions.iter().any(|slot| slot.price < 0)
                || shop.remove_cost < 0
            {
                return Err(SimError::InvalidState("shop price is negative"));
            }
            if shop
                .sale_slot
                .is_some_and(|index| index >= shop.cards.len())
            {
                return Err(SimError::InvalidState("shop sale slot is out of bounds"));
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn rng_stream_state(&self, stream: RunRngStream) -> RunRngStreamState {
        match stream {
            RunRngStream::CardReward => RunRngStreamState {
                seed: self.reward_rng_seed,
                counter: self.card_rng_counter,
            },
            RunRngStream::CardRandom => RunRngStreamState {
                seed: self.reward_rng_seed.wrapping_add(self.current_floor as u64),
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
            RunRngStream::Shuffle => RunRngStreamState {
                seed: self.shuffle_rng_seed,
                counter: self.shuffle_rng_counter,
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
            RunRngStream::Shuffle => self.shuffle_rng_counter = counter,
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

    pub fn init_combat(&self, base: CombatState) -> SimResult<CombatState> {
        base.validate()?;
        let mut combat = base;
        combat.mark_of_bloom = self.has_mark_of_bloom();
        combat.player.hp = self.player_hp;
        combat.player.max_hp = self.player_max_hp;
        combat.player.max_energy = self.energy_per_turn;
        combat.player.energy = self.energy_per_turn;
        combat.relics = self.relics.clone();
        combat.relic_counters.lizard_tail_available =
            self.relics.contains(&Relic::LizardTail) && !self.lizard_tail_used;
        combat.ascension = self.ascension;
        if matches!(
            self.current_room_kind(),
            Some(RoomKind::Elite | RoomKind::Boss)
        ) && self.relics.contains(&Relic::SlaversCollar)
        {
            combat.player.max_energy =
                checked_combat_initialization_add(combat.player.max_energy, SLAVERS_COLLAR_ENERGY)?;
            combat.player.energy =
                checked_combat_initialization_add(combat.player.energy, SLAVERS_COLLAR_ENERGY)?;
        }
        if self.current_room_kind() == Some(RoomKind::Boss)
            && self.relics.contains(&Relic::Pantograph)
        {
            crate::relic::heal_combat_player_with_relics(&mut combat, PANTOGRAPH_HEAL)?;
        }
        if self.current_room_kind() == Some(RoomKind::Elite)
            && self.relics.contains(&Relic::PreservedInsect)
        {
            for monster in &mut combat.monsters {
                monster.hp = i32::try_from(
                    (i64::from(monster.hp) * i64::from(PRESERVED_INSECT_HP_NUMERATOR)
                        / i64::from(PRESERVED_INSECT_HP_DENOMINATOR))
                    .max(1),
                )
                .map_err(|_| SimError::InvalidState("combat HP bounds overflow i32"))?;
            }
        }
        if self.current_room_kind() == Some(RoomKind::Elite)
            && self.relics.contains(&Relic::SlingOfCourage)
        {
            combat.player.powers.strength = checked_combat_initialization_add(
                combat.player.powers.strength,
                SLING_OF_COURAGE_STRENGTH,
            )?;
        }
        if self.relics.contains(&Relic::DuVuDoll) {
            let curses = i32::try_from(
                self.deck
                    .iter()
                    .filter(|card| is_curse_content_id(card.content_id))
                    .count(),
            )
            .map_err(|_| SimError::InvalidState("combat curse count exceeds i32"))?;
            let strength =
                curses
                    .checked_mul(DU_VU_DOLL_STRENGTH_PER_CURSE)
                    .ok_or(SimError::InvalidState(
                        "combat integer multiplication overflows i32",
                    ))?;
            combat.player.powers.strength =
                checked_combat_initialization_add(combat.player.powers.strength, strength)?;
        }
        if self.relics.contains(&Relic::Girya) {
            let strength = i32::try_from(self.girya_lifts)
                .map_err(|_| SimError::InvalidState("Girya lifts exceed i32"))?;
            combat.player.powers.strength =
                checked_combat_initialization_add(combat.player.powers.strength, strength)?;
        }
        if self.relics.contains(&Relic::AncientTeaSet) && self.ancient_tea_set_armed {
            combat.player.energy =
                checked_combat_initialization_add(combat.player.energy, ANCIENT_TEA_SET_ENERGY)?;
        }
        if self.relics.contains(&Relic::PhilosophersStone) {
            for monster in &mut combat.monsters {
                monster.powers.strength = checked_combat_initialization_add(
                    monster.powers.strength,
                    PHILOSOPHERS_STONE_MONSTER_STRENGTH,
                )?;
            }
        }
        if self.relics.contains(&Relic::IncenseBurner) {
            combat.relic_counters.incense_burner_counter = self.incense_burner_counter;
        }
        if self.relics.contains(&Relic::PenNib) {
            combat.relic_counters.pen_nib_attacks_played = self.pen_nib_attacks_played;
        }
        if self.relics.contains(&Relic::InkBottle) {
            combat.relic_counters.ink_bottle_cards_played = self.ink_bottle_cards_played;
        }
        if self.relics.contains(&Relic::HappyFlower) {
            combat.relic_counters.happy_flower_turns = self.happy_flower_turns;
        }
        if self.relics.contains(&Relic::Sundial) {
            combat.relic_counters.sundial_shuffles = self.sundial_shuffles;
        }
        if self.relics.contains(&Relic::Nunchaku) {
            combat.relic_counters.nunchaku_attacks_played = self.nunchaku_attacks_played;
        }
        apply_start_of_combat_relics(&mut combat, &self.relics)?;
        if self.relics.contains(&Relic::Enchiridion) {
            add_enchiridion_power_to_hand(&mut combat)?;
        }
        if self.relics.contains(&Relic::GamblingChip) {
            crate::combat::open_gambling_chip_select(&mut combat)
                .expect("Gambling Chip selection opens without validation side effects");
        }
        if self.relics.contains(&Relic::Toolbox) {
            let next_card_id = combat.reserve_card_instance_ids(3)?;
            let choices = colorless_discovery_card_choices(&mut combat.rng.card_random_rng, 3)
                .into_iter()
                .enumerate()
                .map(|(index, content_id)| {
                    CardInstance::new(CardId::new(next_card_id + index as u64), content_id)
                })
                .collect();
            if let Some(existing) = combat
                .decision
                .replace(CombatDecisionState::ToolboxCardReward { choices })
            {
                combat.queued_decisions.push_back(existing);
            }
        }
        crate::relic::apply_start_of_player_turn_post_draw_relics(&mut combat)?;
        combat.validate()?;
        Ok(combat)
    }

    pub fn init_combat_consuming_relics(&mut self, base: CombatState) -> SimResult<CombatState> {
        let mut combat = self.init_combat(base)?;
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
        if self.relics.contains(&Relic::PenNib) {
            self.pen_nib_attacks_played = combat.relic_counters.pen_nib_attacks_played;
        }
        if self.relics.contains(&Relic::InkBottle) {
            self.ink_bottle_cards_played = combat.relic_counters.ink_bottle_cards_played;
        }
        if self.relics.contains(&Relic::HappyFlower) {
            self.happy_flower_turns = combat.relic_counters.happy_flower_turns;
        }
        if self.relics.contains(&Relic::Sundial) {
            self.sundial_shuffles = combat.relic_counters.sundial_shuffles;
        }
        if self.relics.contains(&Relic::Nunchaku) {
            self.nunchaku_attacks_played = combat.relic_counters.nunchaku_attacks_played;
        }
        if self.relics.contains(&Relic::Toolbox) || self.relics.contains(&Relic::Enchiridion) {
            self.card_random_rng_counter = combat.rng.card_random_rng.counter();
        }
        combat.validate()?;
        Ok(combat)
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
            match_and_keep: None,
            shop: None,
            shop_merchant_open: false,
            card_grid: None,
            relics,
            potions: Vec::new(),
            empty_potion_slots: Vec::new(),
            pending_obtain_cards: Vec::new(),
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
            shuffle_rng_seed: 0,
            shuffle_rng_counter: 0,
            relic_pools: None,
            omamori_charges_used: 0,
            maw_bank_broken: false,
            ancient_tea_set_armed: false,
            lizard_tail_used: false,
            girya_lifts: 0,
            matryoshka_chests_opened: 0,
            incense_burner_counter: 0,
            pen_nib_attacks_played: 0,
            ink_bottle_cards_played: 0,
            happy_flower_turns: 0,
            sundial_shuffles: 0,
            nunchaku_attacks_played: 0,
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
            pending_event_combat_gold_offer: 0,
            pending_event_combat_relic_offer: None,
            monster_rng_seed: 0,
            monster_rng_counter: 0,
            normal_encounter_list: Vec::new(),
            elite_encounter_list: Vec::new(),
            current_floor: 0,
            current_act: 1,
            playtime_seconds: 0,
            note_card_content_id: default_note_card_content_id(),
            note_card_upgrades: 0,
            act1_boss: Act1Boss::default(),
            act3_boss: Act3Boss::default(),
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            act2_event_list: Vec::new(),
            act2_shrine_list: Vec::new(),
            act3_event_list: Vec::new(),
            act3_shrine_list: Vec::new(),
            special_one_time_event_list: Vec::new(),
            special_one_time_events_initialized: false,
            ascension,
            treasure_room: None,
            boss_chest_opened: false,
            pending_boss_relic_choices: Vec::new(),
            rest_room_complete: false,
        };
        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
        run.player_hp = combat.player.hp;
        run.player_max_hp = combat.player.max_hp;
        run.combat = Some(combat);
        run
    }

    fn ironclad_run_base(ascension: u8) -> Self {
        Self {
            phase: RunPhase::Idle,
            deck: crate::content::deck::ironclad_starter_deck_for_ascension(ascension),
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            energy_per_turn: BASE_PLAYER_ENERGY,
            map: None,
            current_room_override: None,
            combat: None,
            reward: None,
            event: None,
            match_and_keep: None,
            shop: None,
            shop_merchant_open: false,
            card_grid: None,
            relics: Vec::new(),
            potions: Vec::new(),
            empty_potion_slots: Vec::new(),
            pending_obtain_cards: Vec::new(),
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
            shuffle_rng_seed: 0,
            shuffle_rng_counter: 0,
            relic_pools: None,
            omamori_charges_used: 0,
            maw_bank_broken: false,
            ancient_tea_set_armed: false,
            lizard_tail_used: false,
            girya_lifts: 0,
            matryoshka_chests_opened: 0,
            incense_burner_counter: 0,
            pen_nib_attacks_played: 0,
            ink_bottle_cards_played: 0,
            happy_flower_turns: 0,
            sundial_shuffles: 0,
            nunchaku_attacks_played: 0,
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
            pending_event_combat_gold_offer: 0,
            pending_event_combat_relic_offer: None,
            monster_rng_seed: 0,
            monster_rng_counter: 0,
            normal_encounter_list: Vec::new(),
            elite_encounter_list: Vec::new(),
            current_floor: 0,
            current_act: 1,
            playtime_seconds: 0,
            note_card_content_id: default_note_card_content_id(),
            note_card_upgrades: 0,
            act1_boss: Act1Boss::default(),
            act3_boss: Act3Boss::default(),
            shop_remove_count: 0,
            act1_event_list: Vec::new(),
            act1_shrine_list: Vec::new(),
            act2_event_list: Vec::new(),
            act2_shrine_list: Vec::new(),
            act3_event_list: Vec::new(),
            act3_shrine_list: Vec::new(),
            special_one_time_event_list: Vec::new(),
            special_one_time_events_initialized: false,
            ascension,
            treasure_room: None,
            boss_chest_opened: false,
            pending_boss_relic_choices: Vec::new(),
            rest_room_complete: false,
        }
    }

    /// Explicit deterministic map fixture for tests and examples.
    /// Production runs must use [`Self::seeded_ironclad`].
    #[must_use]
    pub fn map_fixture() -> Self {
        let mut run = Self::ironclad_run_base(0);
        run.map = Some(milestone8_fixture());
        run
    }

    /// Start a deterministic seeded Ironclad run from production state.
    #[must_use]
    pub fn seeded_ironclad(seed: u64, ascension: u8) -> Self {
        Self::try_seeded_ironclad(seed, ascension).expect("static target encounter pools are valid")
    }

    pub fn try_seeded_ironclad(seed: u64, ascension: u8) -> SimResult<Self> {
        Self::try_seeded_ironclad_with_boss_unlocks(
            seed,
            ascension,
            crate::content::encounters::BossUnlockState::default(),
        )
    }

    /// Start a deterministic seeded Ironclad run with explicit profile boss history.
    #[must_use]
    pub fn seeded_ironclad_with_boss_unlocks(
        seed: u64,
        ascension: u8,
        boss_unlocks: crate::content::encounters::BossUnlockState,
    ) -> Self {
        Self::try_seeded_ironclad_with_boss_unlocks(seed, ascension, boss_unlocks)
            .expect("static target encounter pools are valid")
    }

    /// Fallible production constructor for a deterministic seeded Ironclad run.
    pub fn try_seeded_ironclad_with_boss_unlocks(
        seed: u64,
        ascension: u8,
        boss_unlocks: crate::content::encounters::BossUnlockState,
    ) -> SimResult<Self> {
        let mut run = Self::ironclad_run_base(ascension);
        run.map = Some(generate_target_fixed_map(
            seed as i64,
            TargetMapAct::Exordium,
        ));
        run.act1_boss = crate::content::encounters::target_exordium_act_one_boss_kind_with_unlocks(
            seed as i64,
            boss_unlocks,
        )?;
        run.act3_boss = crate::content::encounters::target_beyond_act_three_boss_kind_with_unlocks(
            seed as i64,
            boss_unlocks,
        )?;
        run.relics = vec![Relic::BurningBlood];
        run.phase = RunPhase::Event;
        run.event = Some(super::event::neow_talk_screen());
        run.ascension = ascension;
        run.event_rng_seed = seed;
        run.reward_rng_seed = seed;
        run.treasure_rng_seed = seed;
        run.potion_rng_seed = seed;
        run.relic_rng_seed = seed;
        run.shuffle_rng_seed = seed;
        run.merchant_rng_seed = seed;
        run.misc_rng_seed = seed;
        run.monster_rng_seed = seed;
        Ok(run)
    }

    pub fn reinit_misc_rng_for_floor(&mut self) {
        let base = self.reward_rng_seed as i64;
        self.misc_rng_seed = base.wrapping_add(i64::from(self.current_floor)) as u64;
        self.misc_rng_counter = 0;
        self.shuffle_rng_seed = self.reward_rng_seed.wrapping_add(self.current_floor as u64);
        self.shuffle_rng_counter = 0;
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
            let owned_keys: Vec<_> = self.relics.iter().map(|relic| relic.key()).collect();
            if let Some(pools) = self.relic_pools.as_mut() {
                for key in owned_keys {
                    pools.remove_relic(key);
                }
            }
        }
    }

    #[must_use]
    pub fn relic_spawn_context(&self, floor_num: i32, shop_room: bool) -> RelicSpawnContext {
        RelicSpawnContext {
            floor_num,
            shop_room,
            owned_relics: self.relics.iter().map(|relic| relic.key()).collect(),
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

    pub(crate) fn reserve_card_instance_ids(&self, count: usize) -> SimResult<u64> {
        let max_id = self
            .deck
            .iter()
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0);
        reserve_card_instance_id_range(max_id, count)
    }

    pub fn next_card_instance_id(&self) -> SimResult<u64> {
        self.reserve_card_instance_ids(1)
    }

    pub fn gain_deck_card(&mut self, content_id: ContentId) -> SimResult<()> {
        if self.should_omamori_prevent_card(content_id) {
            self.omamori_charges_used = self
                .omamori_charges_used
                .checked_add(1)
                .ok_or(SimError::InvalidState("Omamori charge usage overflows u32"))?;
            return Ok(());
        }
        let id = CardId::new(self.next_card_instance_id()?);
        self.add_deck_card(CardInstance::new(id, content_id))
    }

    pub fn queue_pending_obtain_card(&mut self, content_id: ContentId) {
        self.pending_obtain_cards.push(content_id);
    }

    pub fn flush_pending_obtain_cards(&mut self) -> SimResult<()> {
        let mut next = self.clone();
        let pending = std::mem::take(&mut next.pending_obtain_cards);
        for content_id in pending {
            next.gain_deck_card(content_id)?;
        }
        *self = next;
        Ok(())
    }

    pub fn add_deck_card(&mut self, card: CardInstance) -> SimResult<()> {
        let mut next = self.clone();
        next.add_deck_card_inner(card)?;
        *self = next;
        Ok(())
    }

    fn add_deck_card_inner(&mut self, mut card: CardInstance) -> SimResult<()> {
        validate_run_card_content(&card)?;
        if self.deck.iter().any(|existing| existing.id == card.id) {
            return Err(SimError::InvalidState(
                "duplicate run deck card instance ID",
            ));
        }
        if self.should_omamori_prevent_card(card.content_id) {
            self.omamori_charges_used = self
                .omamori_charges_used
                .checked_add(1)
                .ok_or(SimError::InvalidState("Omamori charge usage overflows u32"))?;
            return Ok(());
        }
        card.content_id = self.content_id_after_card_add_relics(card.content_id)?;
        let content_id = card.content_id;
        self.deck.push(card);
        self.apply_card_added_relics(content_id)
    }

    pub fn remove_deck_card(&mut self, card_id: CardId) -> Option<CardInstance> {
        let index = self.deck.iter().position(|card| card.id == card_id)?;
        let card = self.deck.remove(index);
        self.apply_card_removed_effects(card.content_id);
        Some(card)
    }

    fn should_omamori_prevent_card(&self, content_id: ContentId) -> bool {
        self.relics.contains(&Relic::Omamori)
            && is_curse_content_id(content_id)
            && self.omamori_charges_used < OMAMORI_CHARGES
    }

    pub(crate) fn content_id_after_card_add_relics(
        &self,
        content_id: ContentId,
    ) -> SimResult<ContentId> {
        if !self
            .relics
            .iter()
            .any(|relic| matches!(relic, Relic::MoltenEgg | Relic::ToxicEgg | Relic::FrozenEgg))
        {
            return Ok(content_id);
        }
        let definition = get_card_definition(content_id).ok_or_else(|| {
            if super::reward::any_color_reward_card_key(content_id).is_some() {
                SimError::UnsupportedMechanic(content_id)
            } else {
                SimError::UnknownContent(content_id)
            }
        })?;
        let has_matching_egg = match definition.card_type {
            CardType::Attack => self.relics.contains(&Relic::MoltenEgg),
            CardType::Skill => self.relics.contains(&Relic::ToxicEgg),
            CardType::Power => self.relics.contains(&Relic::FrozenEgg),
            CardType::Status => false,
        };
        Ok(if has_matching_egg {
            definition.upgrade.unwrap_or(content_id)
        } else {
            content_id
        })
    }

    fn apply_card_added_relics(&mut self, content_id: ContentId) -> SimResult<()> {
        if self.relics.contains(&Relic::CeramicFish) {
            self.gain_gold(CERAMIC_FISH_GOLD)?;
        }
        if self.relics.contains(&Relic::DarkstonePeriapt) && is_curse_content_id(content_id) {
            self.player_max_hp = checked_run_add(self.player_max_hp, DARKSTONE_PERIAPT_MAX_HP)?;
            self.player_hp = checked_run_add(self.player_hp, DARKSTONE_PERIAPT_MAX_HP)?;
        }
        Ok(())
    }

    fn apply_card_removed_effects(&mut self, content_id: ContentId) {
        if content_id == crate::content::cards::PARASITE_ID {
            self.player_max_hp = (self.player_max_hp - 3).max(1);
            self.player_hp = self.player_hp.min(self.player_max_hp);
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

    pub fn open_potion_slots(&self) -> usize {
        self.potion_capacity().saturating_sub(self.potions.len())
    }

    pub fn potion_index_for_slot(&self, slot: usize) -> Option<usize> {
        if slot >= self.potion_capacity() || self.empty_potion_slots.contains(&slot) {
            return None;
        }

        let earlier_empty_slots = self
            .empty_potion_slots
            .iter()
            .filter(|empty_slot| **empty_slot < slot)
            .count();
        let potion_index = slot.checked_sub(earlier_empty_slots)?;
        (potion_index < self.potions.len()).then_some(potion_index)
    }

    pub fn potion_at_slot(&self, slot: usize) -> Option<Potion> {
        self.potion_index_for_slot(slot)
            .and_then(|index| self.potions.get(index).copied())
    }

    pub fn occupied_potion_slots(&self) -> Vec<(usize, Potion)> {
        (0..self.potion_capacity())
            .filter_map(|slot| self.potion_at_slot(slot).map(|potion| (slot, potion)))
            .collect()
    }

    pub fn gain_potion(&mut self, potion: Potion) -> SimResult<()> {
        if !self.can_gain_potions() {
            return Err(SimError::IllegalAction("potions cannot be obtained"));
        }
        if self.potions.len() >= self.potion_capacity() {
            return Err(SimError::IllegalAction("potion belt is full"));
        }

        if let Some((empty_index, slot)) = self
            .empty_potion_slots
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|(_, slot)| *slot)
        {
            self.empty_potion_slots.remove(empty_index);
            let earlier_empty_slots = self
                .empty_potion_slots
                .iter()
                .filter(|empty_slot| **empty_slot < slot)
                .count();
            let potion_index = slot.saturating_sub(earlier_empty_slots);
            self.potions
                .insert(potion_index.min(self.potions.len()), potion);
        } else {
            self.potions.push(potion);
        }

        Ok(())
    }

    pub fn take_potion_slot(&mut self, slot: usize) -> SimResult<Potion> {
        let Some(potion_index) = self.potion_index_for_slot(slot) else {
            return Err(SimError::IllegalAction("potion slot is not available"));
        };

        let potion = self.potions.remove(potion_index);
        if slot < self.potion_capacity() && !self.empty_potion_slots.contains(&slot) {
            self.empty_potion_slots.push(slot);
            self.empty_potion_slots.sort_unstable();
        }

        Ok(potion)
    }

    pub fn can_gain_gold(&self) -> bool {
        !self.relics.contains(&Relic::Ectoplasm)
    }

    #[must_use]
    pub fn has_mark_of_bloom(&self) -> bool {
        self.relics.contains(&Relic::MarkOfBloom)
    }

    pub fn heal_player(&mut self, amount: i32) -> SimResult<()> {
        if amount > 0 && !self.has_mark_of_bloom() {
            let missing_hp = self
                .player_max_hp
                .checked_sub(self.player_hp)
                .ok_or(SimError::InvalidState("run HP difference overflows i32"))?;
            if missing_hp > 0 {
                self.player_hp = checked_run_add(self.player_hp, amount.min(missing_hp))?;
            }
        }
        Ok(())
    }

    pub fn gain_max_hp(&mut self, amount: i32) -> SimResult<()> {
        if amount < 0 {
            return Err(SimError::IllegalAction("max HP gain cannot be negative"));
        }
        let player_max_hp = checked_run_add(self.player_max_hp, amount)?;
        let player_hp = checked_run_add(self.player_hp, amount)?;
        self.player_max_hp = player_max_hp;
        self.player_hp = player_hp;
        Ok(())
    }

    pub(crate) fn advance_floor(&mut self) -> SimResult<()> {
        self.current_floor = checked_run_add(self.current_floor, 1)?;
        Ok(())
    }

    pub fn gain_gold(&mut self, amount: i32) -> SimResult<()> {
        let mut next = self.clone();
        next.gain_gold_inner(amount)?;
        *self = next;
        Ok(())
    }

    fn gain_gold_inner(&mut self, amount: i32) -> SimResult<()> {
        if amount <= 0 || !self.can_gain_gold() {
            return Ok(());
        }
        self.gold = checked_run_add(self.gold, amount)?;
        if self.relics.contains(&Relic::BloodyIdol) {
            self.heal_player(BLOODY_IDOL_HEAL)?;
        }
        Ok(())
    }

    pub fn apply_floor_entry_relics(&mut self) -> SimResult<()> {
        let mut next = self.clone();
        if next.relics.contains(&Relic::MawBank) && !next.maw_bank_broken {
            next.gain_gold(MAW_BANK_GOLD)?;
        }
        if next.current_room_kind() == Some(RoomKind::Event)
            && next.relics.contains(&Relic::SsserpentHead)
        {
            next.gain_gold(SSSERPENT_HEAD_GOLD)?;
        }
        *self = next;
        Ok(())
    }

    pub fn apply_rest_site_entry_relics(&mut self) -> SimResult<()> {
        let mut next = self.clone();
        if next.relics.contains(&Relic::AncientTeaSet) {
            next.ancient_tea_set_armed = true;
        }
        if next.relics.contains(&Relic::EternalFeather) {
            let deck_len = i32::try_from(next.deck.len())
                .map_err(|_| SimError::InvalidState("run deck size exceeds i32"))?;
            let heal = (deck_len / 5)
                .checked_mul(ETERNAL_FEATHER_HEAL_PER_FIVE_CARDS)
                .ok_or(SimError::InvalidState(
                    "run integer multiplication overflows i32",
                ))?;
            next.heal_player(heal)?;
        }
        *self = next;
        Ok(())
    }

    pub fn break_maw_bank_on_shop_spend(&mut self) {
        if self.relics.contains(&Relic::MawBank) {
            self.maw_bank_broken = true;
        }
    }

    pub fn gain_relic_key(&mut self, key: RelicKey) -> SimResult<()> {
        let mut next = self.clone();
        next.ensure_ironclad_relic_pools();
        if let Some(pools) = next.relic_pools.as_mut() {
            pools.remove_relic(key);
        }
        next.gain_relic(key)?;
        *self = next;
        Ok(())
    }

    pub fn gain_relic(&mut self, relic: Relic) -> SimResult<()> {
        let mut next = self.clone();
        next.gain_relic_inner(relic)?;
        *self = next;
        Ok(())
    }

    fn gain_relic_inner(&mut self, relic: Relic) -> SimResult<()> {
        if let Some(pools) = self.relic_pools.as_mut() {
            pools.remove_relic(relic.key());
        }
        let replaced_starter = self.replace_starter_relic_slot(relic);
        if !replaced_starter {
            self.relics.push(relic);
        }
        match relic {
            Relic::Strawberry => {
                self.player_max_hp = checked_run_add(self.player_max_hp, STRAWBERRY_MAX_HP)?;
                self.heal_player(STRAWBERRY_MAX_HP)?;
            }
            Relic::Pear => {
                self.player_max_hp = checked_run_add(self.player_max_hp, PEAR_MAX_HP)?;
                self.heal_player(PEAR_MAX_HP)?;
            }
            Relic::Mango => {
                self.player_max_hp = checked_run_add(self.player_max_hp, MANGO_MAX_HP)?;
                self.heal_player(MANGO_MAX_HP)?;
            }
            Relic::OldCoin => {
                self.gain_gold(OLD_COIN_GOLD)?;
            }
            Relic::LeesWaffle => {
                self.player_max_hp = checked_run_add(self.player_max_hp, LEES_WAFFLE_MAX_HP)?;
                self.heal_player(self.player_max_hp)?;
            }
            Relic::CoffeeDripper => {
                self.energy_per_turn =
                    checked_run_add(self.energy_per_turn, COFFEE_DRIPPER_ENERGY)?;
            }
            Relic::MarkOfPain => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, MARK_OF_PAIN_ENERGY)?;
            }
            Relic::FusionHammer => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, FUSION_HAMMER_ENERGY)?;
            }
            Relic::Sozu => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, SOZU_ENERGY)?;
            }
            Relic::BustedCrown => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, BUSTED_CROWN_ENERGY)?;
            }
            Relic::SneckoEye => {}
            Relic::WingBoots => {
                self.wing_boots_charges = u32::from(WING_BOOTS_CHARGES);
            }
            Relic::CallingBell => {
                super::grid::open_calling_bell_grid(self)?;
            }
            Relic::PandorasBox => {
                super::grid::open_pandoras_box_grid(self)?;
            }
            Relic::Astrolabe => {
                super::grid::open_astrolabe_grid(self)?;
            }
            Relic::VelvetChoker => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, VELVET_CHOKER_ENERGY)?;
            }
            Relic::PhilosophersStone => {
                self.energy_per_turn =
                    checked_run_add(self.energy_per_turn, PHILOSOPHERS_STONE_ENERGY)?;
            }
            Relic::CursedKey => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, 1)?;
            }
            Relic::Ectoplasm => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, ECTOPLASM_ENERGY)?;
            }
            Relic::RunicDome => {
                self.energy_per_turn = checked_run_add(self.energy_per_turn, RUNIC_DOME_ENERGY)?;
            }
            Relic::Whetstone => {
                self.upgrade_random_deck_cards(CardType::Attack, 2)?;
            }
            Relic::WarPaint => {
                self.upgrade_random_deck_cards(CardType::Skill, 2)?;
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
                self.player_max_hp = checked_run_add(self.player_max_hp, TINY_HOUSE_MAX_HP)?;
                self.heal_player(TINY_HOUSE_MAX_HP + TINY_HOUSE_HEAL)?;
                self.upgrade_random_deck_cards_matching(1, |_| true)?;
                if let Some(reward) = self.reward.as_mut() {
                    reward.gold_offer = checked_run_add(reward.gold_offer, TINY_HOUSE_GOLD)?;
                    let mut misc_rng =
                        StsRng::with_counter(self.misc_rng_seed as i64, self.misc_rng_counter);
                    reward.potion_offer = Some(crate::run::reward::target_uniform_random_potion(
                        &mut misc_rng,
                    ));
                    self.misc_rng_counter = misc_rng.counter();
                    let remaining = reward
                        .remaining_card_reward_count()
                        .checked_add(1)
                        .ok_or(SimError::InvalidState("card reward count overflows u8"))?;
                    reward.set_card_reward_remaining(remaining);
                }
            }
            Relic::Orrery => {
                if let Some(reward) = self.reward.as_mut() {
                    let remaining = reward
                        .remaining_card_reward_count()
                        .checked_add(ORRERY_CARD_REWARDS)
                        .ok_or(SimError::InvalidState("card reward count overflows u8"))?;
                    reward.set_card_reward_remaining(remaining);
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
            | Relic::CultistMask
            | Relic::FaceOfCleric
            | Relic::GremlinMask
            | Relic::NlothsMask
            | Relic::SsserpentHead
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
            | Relic::RedMask
            | Relic::Necronomicon
            | Relic::Enchiridion
            | Relic::NilrysCodex
            | Relic::MutagenicStrength
            | Relic::WarpedTongs
            | Relic::MarkOfBloom
            | Relic::SpiritPoop
            | Relic::OddMushroom
            | Relic::NlothsGift
            | Relic::NeowsLament => {}
        }
        Ok(())
    }

    fn replace_starter_relic_slot(&mut self, relic: Relic) -> bool {
        let replaced = match relic {
            Relic::BlackBlood => Some(Relic::BurningBlood),
            Relic::FrozenCore => Some(Relic::CrackedCore),
            Relic::HolyWater => Some(Relic::PureWater),
            Relic::RingOfTheSerpent => Some(Relic::RingOfTheSnake),
            _ => None,
        };
        let Some(starter_relic) = replaced else {
            return false;
        };
        let mut replaced_relic = false;
        for owned in &mut self.relics {
            if *owned == starter_relic {
                *owned = relic;
                replaced_relic = true;
            }
        }
        replaced_relic
    }

    fn fill_potions_from_cauldron(&mut self) {
        if !self.can_gain_potions() {
            return;
        }

        let open_slots = self.open_potion_slots();
        let rolls = CAULDRON_POTIONS.min(open_slots);
        if rolls == 0 {
            return;
        }

        let mut potion_rng =
            StsRng::with_counter(self.potion_rng_seed as i64, self.potion_rng_counter);
        for _ in 0..rolls {
            self.gain_potion(super::reward::target_random_potion(&mut potion_rng))
                .expect("open potion slot validated");
        }
        self.potion_rng_counter = potion_rng.counter();
    }

    fn upgrade_random_deck_cards(&mut self, card_type: CardType, amount: usize) -> SimResult<()> {
        self.upgrade_random_deck_cards_matching(amount, |card| {
            card_type_and_rarity(card.content_id).is_some_and(|(candidate_type, _)| {
                candidate_type == card_type && card_instance_is_upgradeable(card)
            })
        })
    }

    fn upgrade_random_deck_cards_matching(
        &mut self,
        amount: usize,
        matches_card: impl Fn(&CardInstance) -> bool,
    ) -> SimResult<()> {
        let mut upgradeable: Vec<_> = self
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, card)| {
                (matches_card(card) && card_instance_is_upgradeable(card)).then_some(index)
            })
            .collect();

        if upgradeable.is_empty() {
            return Ok(());
        }

        let mut misc_rng = StsRng::with_counter(self.misc_rng_seed as i64, self.misc_rng_counter);
        let shuffle_seed = misc_rng.random_long();
        JavaRng::new(shuffle_seed).collections_shuffle(&mut upgradeable);

        let upgrades = upgradeable
            .into_iter()
            .take(amount)
            .map(|deck_index| {
                let upgraded = upgrade_card_instance(self.deck[deck_index])?.ok_or(
                    SimError::InvalidState("random upgrade selected a non-upgradeable card"),
                )?;
                Ok((deck_index, upgraded))
            })
            .collect::<SimResult<Vec<_>>>()?;
        self.misc_rng_counter = misc_rng.counter();
        for (deck_index, upgraded) in upgrades {
            self.deck[deck_index] = upgraded;
        }
        Ok(())
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
                if reward.card_reward_is_active() {
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
            RunAction::TakePotionReward { index } => {
                let offered = reward
                    .potion_offers
                    .get(index)
                    .copied()
                    .or_else(|| (index == 0).then_some(reward.potion_offer).flatten());
                if offered.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                if !self.can_gain_potions() {
                    return Err(SimError::IllegalAction("potions cannot be obtained"));
                }
                if self.open_potion_slots() == 0 {
                    return Err(SimError::IllegalAction("potion belt is full"));
                }
                Ok(())
            }
            RunAction::TakeRelicReward => {
                if reward.relic_offer.is_none() {
                    return Err(SimError::IllegalAction("no relic reward offered"));
                }
                if let Some(relic) = reward.relic_offer {
                    if self.relics.contains(&relic) {
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
            RunAction::Proceed => {
                let final_boss_victory =
                    self.current_act == 3 && self.current_room_kind() == Some(RoomKind::Boss);
                if final_boss_victory
                    || (reward.continuation != RewardContinuation::None
                        && super::reward::reward_is_empty(reward))
                {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("cannot proceed from reward"))
                }
            }
            RunAction::OpenCardReward => {
                if reward.remaining_card_reward_count() == 0 {
                    return Err(SimError::IllegalAction("no card reward offered"));
                }
                if reward.card_reward_is_active() {
                    return Err(SimError::IllegalAction("card reward already open"));
                }
                Ok(())
            }
            RunAction::OpenChest => Err(SimError::IllegalAction("not a reward action")),
            RunAction::SkipPotionReward => {
                if reward.potion_offer.is_none() && reward.potion_offers.is_empty() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                Ok(())
            }
            RunAction::TakeCardReward { card_id } => {
                if !reward.card_reward_is_active() {
                    return Err(SimError::IllegalAction("card reward is not open"));
                }
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
                if !reward.card_reward_is_active() || reward.choices.is_empty() {
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
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward => {
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
    pub const fn key(self) -> RelicKey {
        self
    }

    #[must_use]
    pub const fn from_key(key: RelicKey) -> Option<Self> {
        Some(key)
    }
}
