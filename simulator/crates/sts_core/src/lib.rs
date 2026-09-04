#![forbid(unsafe_code)]
#![doc = "Authoritative deterministic Slay the Spire simulator."]

#[doc(hidden)]
pub mod action;
#[doc(hidden)]
pub mod card;
#[doc(hidden)]
pub mod combat;
#[doc(hidden)]
pub mod content;
#[doc(hidden)]
pub mod error;
#[doc(hidden)]
pub mod ids;
#[doc(hidden)]
pub mod map;
#[doc(hidden)]
pub mod potion;
#[doc(hidden)]
pub mod power;
#[doc(hidden)]
pub mod relic;
#[doc(hidden)]
pub mod rng;
#[doc(hidden)]
pub mod run;
#[doc(hidden)]
pub mod seed;
#[doc(hidden)]
pub mod snapshot;

// Stable authoritative engine boundary.
pub use action::CombatAction;
pub use combat::{apply_combat_action, legal_combat_actions, validate_combat_action, CombatState};
pub use error::{SimError, SimResult};
pub use run::{
    apply_run_decision_action, legal_run_decision_actions, validate_run_decision_action,
    RunDecisionAction, RunState,
};
pub use seed::{
    sts_seed_long_to_string, sts_seed_string_to_long, try_sts_seed_string_to_long, SeedParseError,
    STS_SEED_ALPHABET,
};

/// Privileged implementation surface for verifier, environment, and integration
/// adapters. Policy code should depend on `sts_env`, not this module.
#[doc(hidden)]
pub mod adapter_internals {
    pub use crate::action::{CardPile, CombatAction, EventAction, InternalAction, RestAction};
    pub use crate::card::{
        CardDefinition, CardInstance, CardKeywords, CardRarity, CardType, CardValues,
        TargetRequirement,
    };
    pub use crate::combat::cost::effective_card_cost_with_corruption;
    pub use crate::combat::{
        apply_burning_blood, apply_combat_action, apply_combat_action_with_events, end_player_turn,
        initialize_combat_piles_with_relics, legal_combat_actions, validate_combat_action,
        CardPiles, CombatDecisionState, CombatPhase, CombatState, CombatTransition, DamageInfo,
        DamageSource, MonsterIntent, MonsterState, PlayerState, SlimeSize, BASE_PLAYER_ENERGY,
    };
    pub use crate::content::ascension::AscensionConfig;
    pub use crate::content::character::{BURNING_BLOOD_HEAL_AMOUNT, IRONCLAD_A0_BASE_HP};
    pub use crate::content::deck::{ironclad_starter_deck, ironclad_starter_deck_for_ascension};
    pub use crate::error::{SimError, SimResult};
    pub use crate::ids::{ActionId, CardId, ContentId, MapNodeId, MonsterId};
    pub use crate::map::{
        apply_map_action, city_room_kinds_on_path, exordium_room_kinds_on_path,
        generate_city_fixed_map, generate_exordium_fixed_map, generate_target_fixed_map,
        legal_map_actions, reachable_nodes, target_room_kinds_on_path, validate_map_action,
        FixedMap, MapAction, MapNode, MapRunState, RoomKind, TargetMapAct,
    };
    pub use crate::potion::{
        Potion, BLOCK_POTION_BLOCK, BLOCK_POTION_ID, FEAR_POTION_ID, FEAR_POTION_VULNERABLE,
        FIRE_POTION_DAMAGE, FIRE_POTION_ID, GAMBLERS_BREW_POTION_ID, MAX_POTIONS,
    };
    pub use crate::power::{MonsterPowers, PlayerPowers};
    pub use crate::relic::{
        apply_on_card_play_relics, apply_start_of_combat_relics, initialize_ironclad_relic_pools,
        preserves_energy_between_turns, relic_can_spawn, reset_turn_relic_counters, Relic,
        RelicCounters, RelicDefinition, RelicEffectStatus, RelicPoolState, RelicSpawnContext,
        RelicTier, ALL_RELICS, ANCHOR_BLOCK, ANCHOR_ID, COFFEE_DRIPPER_ENERGY, COFFEE_DRIPPER_ID,
        ICE_CREAM_ID, INK_BOTTLE_ID, INK_BOTTLE_THRESHOLD, ODDLY_SMOOTH_STONE_DEXTERITY,
        ODDLY_SMOOTH_STONE_ID, ORNAMENTAL_FAN_BLOCK, ORNAMENTAL_FAN_ID, ORNAMENTAL_FAN_THRESHOLD,
        STRAWBERRY_ID, STRAWBERRY_MAX_HP, VAJRA_ID, VAJRA_STRENGTH,
    };
    pub use crate::rng::{ExternalRngInput, ExternalRngKind, JavaRng, MathUtilsRngState, StsRng};
    pub use crate::run::{
        advance_card_rng_for_combat_entry, affordable_shop_picks, apply_combat_action_on_run,
        apply_event_action, apply_initial_monster_ai_rolls, apply_map_action_on_run,
        apply_neow_boss_swap, apply_neow_relic_reward, apply_neow_simple_drawback,
        apply_neow_simple_reward, apply_potion_action, apply_rest_action, apply_run_action,
        apply_run_decision_action, apply_shop_action, cancel_grid, confirm_grid,
        consume_neow_three_potions_hidden_card_reward, enter_boss_relic_reward_screen,
        enter_chest_relic_reward_screen, enter_elite_combat_reward_screen,
        enter_elite_relic_reward_screen, enter_event_screen, enter_normal_combat_reward_screen,
        enter_reward_screen, enter_shop_room, enter_shop_screen, event_screen,
        generate_neow_card_reward, generate_neow_card_reward_with_rng,
        generate_neow_colorless_reward, generate_neow_colorless_reward_with_rng,
        generate_neow_options, generate_neow_options_rng_counter, generate_neow_rare_card_reward,
        generate_neow_rare_card_reward_with_rng, generate_neow_three_potions,
        generate_neow_three_potions_with_rng, generate_neow_transform_reward,
        generate_neow_transform_reward_with_rng, generate_shop_screen, leave_shop_merchant,
        leave_shop_room, legal_event_actions, legal_map_actions_on_run, legal_rest_actions,
        legal_run_decision_actions, legal_shop_actions, match_and_keep_group_index_for_label,
        match_and_keep_label_index_for_group, open_neow_remove_grid, open_neow_reward_grid,
        open_neow_upgrade_grid, open_shop_merchant, rest_heal_amount, select_grid_card,
        shop_action_for_choice_index, shop_card_rarity_roll, shop_relic_tier_roll,
        target_card_reward_choices, target_elite_relic_tier, target_normal_combat_gold,
        target_potion_reward_offer, target_random_combat_potion, target_random_potion,
        target_relic_tier, validate_event_action, validate_potion_action, validate_rest_action,
        validate_run_decision_action, validate_shop_action, Act1Boss, Act3Boss, CardGridScreen,
        CardRewardFlow, CombatRewardKind, Event, EventChoice, EventScreen, GeneratedNeowOption,
        GridPurpose, NeowBossSwapReward, NeowCardReward, NeowColorlessReward, NeowDrawback,
        NeowPotionReward, NeowRelicReward, NeowRewardType, NeowTransformReward, RewardContinuation,
        RewardScreen, RunAction, RunDecisionAction, RunPhase, RunState, ShopCardSlot, ShopPick,
        ShopPotionSlot, ShopRelicSlot, ShopScreen, GOLDEN_SHRINE_GOLD, REST_HEAL_PERCENT,
        REWARD_GOLD_AMOUNT, STARTING_GOLD,
    };
    pub use crate::seed::{
        sts_seed_long_to_string, sts_seed_string_to_long, try_sts_seed_string_to_long,
        SeedParseError, STS_SEED_ALPHABET,
    };
    pub use crate::{
        action, card, combat, content, ids, map, potion, power, relic, rng, run, seed,
    };
}

// Existing core modules use concise crate-root names internally. Keep those
// aliases crate-private while exposing them outward only through the facade.
#[allow(unused_imports)]
pub(crate) use adapter_internals::*;
