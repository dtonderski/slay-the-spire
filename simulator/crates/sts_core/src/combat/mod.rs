pub mod legal;
pub mod state;

pub use legal::{legal_combat_actions, validate_combat_action};
pub use state::{CardPiles, CombatPhase, CombatState, MonsterState, PlayerState};
