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

pub use action::{CardPile, CombatAction, InternalAction};
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
pub use ids::{ActionId, CardId, ContentId, MonsterId};
pub use power::{MonsterPowers, PlayerPowers};
pub use rng::{RngDraw, RngStream, SimulatorRng};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
