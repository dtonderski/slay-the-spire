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
    end_player_turn, legal_combat_actions, validate_combat_action, CardPiles, CombatPhase,
    CombatState, CombatTransition, DamageInfo, DamageSource, MonsterIntent, MonsterState,
    PlayerState,
};
pub use content::character::{BURNING_BLOOD_HEAL_AMOUNT, IRONCLAD_A0_BASE_HP};
pub use content::deck::ironclad_starter_deck;
pub use error::{SimError, SimResult};
pub use ids::{ActionId, CardId, ContentId, MapNodeId, MonsterId};
pub use map::{
    apply_map_action, generate_map_placeholder, generated_map_fixture, legal_map_actions,
    milestone8_fixture, milestone8_map, reachable_nodes, validate_map_action, FixedMap, MapAction,
    MapNode, MapRunState, RoomKind,
};
pub use potion::{Potion, FIRE_POTION_DAMAGE, FIRE_POTION_ID, MAX_POTIONS};
pub use power::{MonsterPowers, PlayerPowers};
pub use relic::{
    apply_start_of_combat_relics, Relic, ODDLY_SMOOTH_STONE_DEXTERITY, ODDLY_SMOOTH_STONE_ID,
    VAJRA_ID, VAJRA_STRENGTH,
};
pub use rng::{RngDraw, RngStream, SimulatorRng};
pub use run::{
    apply_combat_action_on_run, apply_event_action, apply_map_action_on_run, apply_potion_action,
    apply_rest_action, apply_run_action, apply_shop_action, card_reward_choices,
    enter_fixed_event_screen, enter_reward_screen, enter_shop_screen, fixed_card_reward_choices,
    fixed_event_screen, fixed_shop_screen, legal_event_actions, legal_map_actions_on_run,
    legal_rest_actions, legal_shop_actions, rest_heal_amount, validate_event_action,
    validate_potion_action, validate_rest_action, validate_shop_action, Event, EventChoice,
    EventScreen, RewardScreen, RunAction, RunPhase, RunState, ShopCardSlot, ShopPotionSlot,
    ShopRelicSlot, ShopScreen, GOLDEN_SHRINE_GOLD, REST_HEAL_PERCENT, REWARD_GOLD_AMOUNT,
    SHOP_ANGER_PRICE, SHOP_FIRE_POTION_PRICE, SHOP_VAJRA_PRICE, STARTING_GOLD,
};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
