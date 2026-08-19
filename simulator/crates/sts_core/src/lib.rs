#![forbid(unsafe_code)]
#![doc = "Core library for the Slay the Spire simulator."]

pub mod card;
pub mod combat;
pub mod content;
pub mod error;
pub mod ids;
pub mod power;
pub mod rng;
pub mod snapshot;

pub mod action;
pub mod map;
pub mod potion;
pub mod relic;
pub mod run;

pub use action::{CardPile, CombatAction, EventAction, InternalAction, RestAction};
pub use card::{
    CardDefinition, CardInstance, CardKeywords, CardRarity, CardType, CardValues, TargetRequirement,
};
pub use combat::{
    apply_burning_blood, apply_combat_action, apply_combat_action_with_events, end_player_turn,
    fair_combat_observation, initialize_combat_piles_with_relics, legal_combat_actions,
    starter_only_deck, validate_combat_action, CardPiles, CombatDecisionState, CombatPhase,
    CombatState, CombatTransition, DamageInfo, DamageSource, FairCard, FairCardDynamicValues,
    FairCombatObservation, FairCombatPhase, FairCounter, FairHandCard, FairIntentCategory,
    FairMonster, FairMonsterIntent, FairObservationError, FairPile, FairPlayer, FairPotionSlot,
    FairPower, FairRelic, FairRunContext, FairSelection, FairSelectionKind, FairSelectionOption,
    MonsterIntent, MonsterState, PlayerState, SlimeSize, BASE_PLAYER_ENERGY,
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
};
pub use content::ascension::AscensionConfig;
pub use content::character::{BURNING_BLOOD_HEAL_AMOUNT, IRONCLAD_A0_BASE_HP};
pub use content::deck::{ironclad_starter_deck, ironclad_starter_deck_for_ascension};
pub use error::{SimError, SimResult};
pub use ids::{
    headbutt_alias_sibling_id, ActionId, CardId, ContentId, MapNodeId, MonsterId,
    HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET,
};
pub use map::{
    apply_map_action, city_room_kinds_on_path, exordium_room_kinds_on_path,
    generate_city_fixed_map, generate_city_map_choices_after_path, generate_city_map_topology,
    generate_exordium_fixed_map, generate_exordium_map_choices_after_path,
    generate_exordium_map_topology, generate_target_fixed_map,
    generate_target_map_choices_after_path, generate_target_map_topology, legal_map_actions,
    reachable_nodes, target_room_kinds_on_path, validate_map_action, CityMapChoiceStep,
    CityMapTopology, ExordiumFixedRoomRow, ExordiumMapChoiceStep, ExordiumMapTopology, FixedMap,
    MapAction, MapNode, MapRunState, RoomKind, TargetAssignedRoom, TargetFixedRoomRow,
    TargetMapAct, TargetMapChild, TargetMapChoiceStep, TargetMapTopology, TargetRoomTypeCounts,
};
pub use potion::{
    Potion, BLOCK_POTION_BLOCK, BLOCK_POTION_ID, FEAR_POTION_ID, FEAR_POTION_VULNERABLE,
    FIRE_POTION_DAMAGE, FIRE_POTION_ID, GAMBLERS_BREW_POTION_ID, GAMBLE_POTION_ID, MAX_POTIONS,
};
pub use power::{MonsterPowers, PlayerPowers};
pub use relic::{
    apply_on_card_play_relics, apply_start_of_combat_relics, initialize_ironclad_relic_pools,
    preserves_energy_between_turns, relic_can_spawn, reset_turn_relic_counters, Relic,
    RelicCounters, RelicDefinition, RelicEffectStatus, RelicKey, RelicPoolState, RelicSpawnContext,
    RelicTier, ALL_RELICS, ANCHOR_BLOCK, ANCHOR_ID, COFFEE_DRIPPER_ENERGY, COFFEE_DRIPPER_ID,
    ICE_CREAM_ID, INK_BOTTLE_ID, INK_BOTTLE_THRESHOLD, ODDLY_SMOOTH_STONE_DEXTERITY,
    ODDLY_SMOOTH_STONE_ID, ORNAMENTAL_FAN_BLOCK, ORNAMENTAL_FAN_ID, ORNAMENTAL_FAN_THRESHOLD,
    STRAWBERRY_ID, STRAWBERRY_MAX_HP, VAJRA_ID, VAJRA_STRENGTH,
};
pub use rng::{
    capture_rng_trace, set_rng_trace_context, ExternalRngInput, ExternalRngKind, JavaRng,
    MathUtilsRngState, RngTraceContext, RngTraceEvent, RngTraceOperation, RngTraceStream, StsRng,
};
pub use run::{
    advance_card_rng_for_combat_entry, affordable_shop_picks, apply_combat_action_on_run,
    apply_event_action, apply_initial_monster_ai_rolls, apply_map_action_on_run,
    apply_neow_boss_swap, apply_neow_relic_reward, apply_neow_simple_drawback,
    apply_neow_simple_reward, apply_potion_action, apply_rest_action, apply_run_action,
    apply_run_decision_action, apply_shop_action, cancel_grid, confirm_grid,
    consume_neow_three_potions_hidden_card_reward, enter_boss_relic_reward_screen,
    enter_chest_relic_reward_screen, enter_elite_combat_reward_screen,
    enter_elite_relic_reward_screen, enter_event_screen, enter_normal_combat_reward_screen,
    enter_reward_screen, enter_shop_room, enter_shop_screen, event_screen, fair_run_observation,
    generate_neow_card_reward, generate_neow_card_reward_with_rng, generate_neow_colorless_reward,
    generate_neow_colorless_reward_with_rng, generate_neow_options,
    generate_neow_options_rng_counter, generate_neow_rare_card_reward,
    generate_neow_rare_card_reward_with_rng, generate_neow_three_potions,
    generate_neow_three_potions_with_rng, generate_neow_transform_reward,
    generate_neow_transform_reward_with_rng, generate_shop_screen, leave_shop_merchant,
    leave_shop_room, legal_event_actions, legal_map_actions_on_run, legal_rest_actions,
    legal_run_decision_actions, legal_shop_actions, match_and_keep_group_index_for_label,
    match_and_keep_label_index_for_group, open_neow_remove_grid, open_neow_reward_grid,
    open_neow_upgrade_grid, open_shop_merchant, player_choices, resolve_player_choice,
    rest_heal_amount, select_grid_card, shop_action_for_choice_index, shop_card_rarity_roll,
    shop_relic_tier_roll, target_card_reward_choices, target_elite_relic_tier,
    target_normal_combat_gold, target_potion_reward_offer, target_random_combat_potion,
    target_random_potion, target_relic_tier, validate_event_action, validate_potion_action,
    validate_rest_action, validate_run_decision_action, validate_shop_action, Act1Boss, Act3Boss,
    CardGridScreen, CardRewardFlow, CombatRewardKind, DecisionRevision, Event, EventChoice,
    EventScreen, GeneratedNeowOption, GridPurpose, NeowBossSwapReward, NeowCardReward,
    NeowColorlessReward, NeowDrawback, NeowPotionReward, NeowRelicReward, NeowRewardType,
    NeowTransformReward, PlayerChoice, PlayerChoiceError, PlayerChoiceRequest, PlayerChoiceSet,
    RewardContinuation, RewardScreen, RunAction, RunDecisionAction, RunPhase, RunState,
    ShopCardSlot, ShopPick, ShopPotionSlot, ShopRelicSlot, ShopScreen,
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION, GOLDEN_SHRINE_GOLD, PLAYER_CHOICE_SCHEMA_VERSION,
    REST_HEAL_PERCENT, REWARD_GOLD_AMOUNT, STARTING_GOLD,
};
pub use snapshot::{
    restore_combat_snapshot_json, restore_run_snapshot_json, Snapshot, SnapshotHash,
    SnapshotRestoreError, LEGACY_COMBAT_DECISION_SNAPSHOT_SCHEMA_VERSION,
    LEGACY_COMBUST_SNAPSHOT_SCHEMA_VERSION, LEGACY_NEOWS_LAMENT_RELIC_SNAPSHOT_SCHEMA_VERSION,
    LEGACY_RELIC_STORAGE_SNAPSHOT_SCHEMA_VERSION, LEGACY_REWARD_FLOW_SNAPSHOT_SCHEMA_VERSION,
    LEGACY_VALIDATED_SNAPSHOT_SCHEMA_VERSION, PREVIOUS_SNAPSHOT_SCHEMA_VERSION,
    SNAPSHOT_SCHEMA_VERSION,
};
