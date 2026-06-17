pub mod damage;
pub mod legal;
pub mod state;
pub mod transition;
pub mod turn;

pub use damage::{DamageInfo, DamageSource};
pub use legal::{legal_combat_actions, validate_combat_action};
pub use state::{CardPiles, CombatPhase, CombatState, MonsterState, PlayerState};
pub use transition::{apply_combat_action, apply_combat_action_with_events, CombatTransition};
pub use turn::end_player_turn;
