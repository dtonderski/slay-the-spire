pub mod burning_blood;
mod card_effects;
pub mod damage;
pub mod draw;
pub mod hand;
pub mod legal;
pub mod piles;
pub mod setup;
pub mod state;
pub mod transition;
pub mod turn;
pub mod turn_powers;

pub use burning_blood::apply_burning_blood;
pub use damage::{DamageInfo, DamageSource};
pub use draw::draw_cards;
pub use legal::{legal_combat_actions, validate_combat_action};
pub use setup::{
    card_has_innate, initialize_combat_piles, initialize_combat_piles_with_relics,
    order_deck_for_combat_shuffle, starter_only_deck,
};
pub use state::{
    CardPiles, CombatPhase, CombatState, DiscardSelectState, ExhaustSelectPurpose,
    ExhaustSelectState, HandSelectPurpose, HandSelectState, MonsterIntent, MonsterState,
    PlayerState, BASE_PLAYER_ENERGY,
};
pub use transition::{
    apply_combat_action, apply_combat_action_with_events, choose_hand_select, confirm_hand_select,
    hand_select_ui_to_hand_index, open_gambling_chip_select, CombatTransition,
};
pub use turn::end_player_turn;
