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
pub mod run;

pub use action::{CardPile, CombatAction, InternalAction, RestAction};
pub use card::{
    CardDefinition, CardInstance, CardKeywords, CardType, CardValues, TargetRequirement,
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
    apply_map_action, legal_map_actions, milestone8_fixture, milestone8_map, reachable_nodes,
    validate_map_action, FixedMap, MapAction, MapNode, MapRunState, RoomKind,
};
pub use power::{MonsterPowers, PlayerPowers};
pub use rng::{RngDraw, RngStream, SimulatorRng};
pub use run::{
    apply_combat_action_on_run, apply_map_action_on_run, apply_rest_action, apply_run_action,
    enter_reward_screen, fixed_card_reward_choices, legal_map_actions_on_run, legal_rest_actions,
    rest_heal_amount, validate_rest_action, RewardScreen, RunAction, RunPhase, RunState,
    REST_HEAL_PERCENT, REWARD_GOLD_AMOUNT, STARTING_GOLD,
};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
