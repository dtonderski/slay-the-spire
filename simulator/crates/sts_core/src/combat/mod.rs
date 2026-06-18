pub mod burning_blood;
pub mod damage;
pub mod draw;
pub mod hand;
pub mod legal;
pub mod piles;
pub mod state;
pub mod transition;
pub mod turn;
pub mod turn_powers;

pub use burning_blood::apply_burning_blood;
pub use damage::{DamageInfo, DamageSource};
pub use draw::draw_cards;
pub use legal::{legal_combat_actions, validate_combat_action};
pub use state::{
    CardPiles, CombatPhase, CombatState, MonsterIntent, MonsterState, PlayerState,
    BASE_PLAYER_ENERGY,
};
pub use transition::{apply_combat_action, apply_combat_action_with_events, CombatTransition};
pub use turn::end_player_turn;
