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
    apply_burning_blood, apply_combat_action, apply_combat_action_with_events, draw_cards,
    end_player_turn, initialize_combat_piles, legal_combat_actions, starter_only_deck,
    validate_combat_action, CardPiles, CombatPhase, CombatState, CombatTransition, DamageInfo,
    DamageSource, MonsterIntent, MonsterState, PlayerState, BASE_PLAYER_ENERGY,
};
pub use content::ascension::AscensionConfig;
pub use content::character::{BURNING_BLOOD_HEAL_AMOUNT, IRONCLAD_A0_BASE_HP};
pub use content::deck::{ironclad_starter_deck, ironclad_starter_deck_for_ascension};
pub use error::{SimError, SimResult};
pub use ids::{ActionId, CardId, ContentId, MapNodeId, MonsterId};
pub use map::{
    apply_map_action, exordium_room_kinds_on_path, generate_exordium_fixed_map,
    generate_exordium_map_choices_after_path, generate_exordium_map_topology,
    generate_map_placeholder, generated_map_fixture, legal_map_actions, milestone8_fixture,
    milestone8_map, reachable_nodes, validate_map_action, ExordiumFixedRoomRow,
    ExordiumMapChoiceStep, ExordiumMapTopology, FixedMap, MapAction, MapNode, MapRunState,
    RoomKind,
};
pub use potion::{
    Potion, BLOCK_POTION_BLOCK, BLOCK_POTION_ID, FEAR_POTION_ID, FEAR_POTION_WEAK,
    FIRE_POTION_DAMAGE, FIRE_POTION_ID, GAMBLE_POTION_ID, GAMBLE_POTION_LOSS_GOLD,
    GAMBLE_POTION_WIN_GOLD, MAX_POTIONS,
};
pub use power::{MonsterPowers, PlayerPowers};
pub use relic::{
    apply_on_card_play_relics, apply_start_of_combat_relics, initialize_ironclad_relic_pools,
    preserves_energy_between_turns, relic_can_spawn, reset_turn_relic_counters, Relic,
    RelicCounters, RelicKey, RelicPoolState, RelicSpawnContext, ANCHOR_BLOCK, ANCHOR_ID,
    COFFEE_DRIPPER_ENERGY, COFFEE_DRIPPER_ID, ICE_CREAM_ID, INK_BOTTLE_ID, INK_BOTTLE_THRESHOLD,
    ODDLY_SMOOTH_STONE_DEXTERITY, ODDLY_SMOOTH_STONE_ID, ORNAMENTAL_FAN_BLOCK, ORNAMENTAL_FAN_ID,
    ORNAMENTAL_FAN_THRESHOLD, STRAWBERRY_ID, STRAWBERRY_MAX_HP, VAJRA_ID, VAJRA_STRENGTH,
};
pub use rng::{JavaRng, RngDraw, RngStream, SimulatorRng, StsRng};
pub use run::{
    advance_card_rng_for_combat_entry, affordable_shop_picks, apply_combat_action_on_run,
    apply_event_action, apply_map_action_on_run, apply_potion_action, apply_rest_action,
    apply_run_action, apply_shop_action, cancel_grid, card_reward_choices, confirm_grid,
    enter_boss_relic_reward_screen, enter_chest_relic_reward_screen,
    enter_elite_combat_reward_screen, enter_elite_relic_reward_screen, enter_event_screen,
    enter_fixed_event_screen, enter_normal_combat_reward_screen, enter_reward_screen,
    enter_shop_room, enter_shop_screen, event_screen, fixed_card_reward_choices,
    fixed_event_screen, fixed_shop_screen, generate_shop_screen, leave_shop_merchant,
    leave_shop_room, legal_event_actions, legal_map_actions_on_run, legal_rest_actions,
    legal_shop_actions, open_shop_merchant, rest_heal_amount, select_grid_card,
    shop_action_for_choice_index, shop_card_rarity_roll, shop_relic_tier_roll,
    target_card_reward_choices, target_elite_relic_tier, target_normal_combat_gold,
    target_potion_reward_offer, target_random_potion, target_relic_tier, validate_event_action,
    validate_potion_action, validate_rest_action, validate_shop_action, CardGridScreen,
    CombatRewardKind, Event, EventChoice, EventScreen, GridPurpose, RewardScreen, RunAction,
    RunPhase, RunState, ShopCardSlot, ShopPick, ShopPotionSlot, ShopRelicSlot, ShopScreen,
    GOLDEN_SHRINE_GOLD, REST_HEAL_PERCENT, REWARD_GOLD_AMOUNT, SHOP_ANGER_PRICE,
    SHOP_FIRE_POTION_PRICE, SHOP_VAJRA_PRICE, STARTING_GOLD,
};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
