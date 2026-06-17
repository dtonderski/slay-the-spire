#![forbid(unsafe_code)]
#![doc = "Core library for the Slay the Spire simulator."]

pub mod card;
pub mod combat;
pub mod content;
pub mod error;
pub mod ids;
pub mod snapshot;

pub mod action;

pub use action::CombatAction;
pub use card::{CardDefinition, CardInstance, CardType, CardValues, TargetRequirement};
pub use combat::{
    legal_combat_actions, validate_combat_action, CardPiles, CombatPhase, CombatState,
    MonsterState, PlayerState,
};
pub use error::{SimError, SimResult};
pub use ids::{ActionId, CardId, ContentId, MonsterId};
pub use snapshot::{PlaceholderState, Snapshot, SnapshotHash, SNAPSHOT_SCHEMA_VERSION};
