pub mod burning_blood;
mod card_effects;
pub(crate) mod cost;
pub mod damage;
pub mod draw;
pub mod hand;
pub mod hp_loss;
pub mod legal;
pub(crate) mod piles;
pub mod setup;
pub mod state;
pub mod transition;
pub mod turn;
pub mod turn_powers;

pub use burning_blood::apply_burning_blood;
pub use damage::{DamageInfo, DamageSource};
pub use legal::{legal_combat_actions, validate_combat_action};
pub use setup::{card_has_innate, initialize_combat_piles_with_relics};
pub use state::{
    BombTimer, CardPiles, CombatDecisionState, CombatOrb, CombatPhase, CombatRngState, CombatState,
    DiscardSelectPurpose, DiscardSelectState, DrawSelectPurpose, DrawSelectState,
    ExhaustSelectPurpose, ExhaustSelectState, HandSelectPurpose, HandSelectState, MonsterIntent,
    MonsterState, PlayerState, PotionCardRewardKind, SlimeSize, BASE_PLAYER_ENERGY,
};
pub use transition::{
    add_generated_card_to_draw_pile_random_spot_public, apply_combat_action,
    apply_combat_action_with_events, choose_draw_select, choose_exhaust_select, choose_hand_select,
    close_discovery_source_card, close_discovery_source_card_with_force_exhaust,
    confirm_draw_select, confirm_hand_select, confirm_hand_select_without_retrieval,
    draw_select_ui_to_draw_index, hand_select_ui_to_hand_index, open_gambling_chip_select,
    settle_queued_end_turn_discard_after_rejected_command,
    settle_time_warp_end_turn_if_ready_public, settle_time_warp_pre_discard_if_ready_public,
    CombatTransition,
};
pub use turn::{end_player_turn, finish_monster_turn_after_player_revival, start_player_turn};
