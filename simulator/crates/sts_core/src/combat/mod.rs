pub mod damage;
pub mod legal;
pub mod state;
pub mod transition;

pub use legal::{legal_combat_actions, validate_combat_action};
pub use state::{CardPiles, CombatPhase, CombatState, MonsterState, PlayerState};
pub use transition::apply_combat_action;
