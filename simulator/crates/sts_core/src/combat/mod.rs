pub mod burning_blood;
mod card_effects;
pub(crate) mod cost;
pub mod damage;
pub mod draw;
pub mod fair_observation;
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
pub use fair_observation::{
    fair_combat_observation, FairCard, FairCardDynamicValues, FairCombatObservation,
    FairCombatPhase, FairCounter, FairHandCard, FairIntentCategory, FairMonster, FairMonsterIntent,
    FairObservationError, FairPile, FairPlayer, FairPotionSlot, FairPower, FairRelic,
    FairRunContext, FairSelection, FairSelectionKind, FairSelectionOption,
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
};
pub use legal::{legal_combat_actions, validate_combat_action};
pub use setup::{
    card_has_innate, initialize_combat_piles_with_relics, order_deck_for_combat_shuffle,
    starter_only_deck,
};
pub use state::{
    BombTimer, CardPiles, CombatDecisionState, CombatPhase, CombatRngState, CombatState,
    DiscardSelectPurpose, DiscardSelectState, DrawSelectPurpose, DrawSelectState,
    ExhaustSelectPurpose, ExhaustSelectState, HandSelectPurpose, HandSelectState, MonsterIntent,
    MonsterState, PendingPotionCardRewardSettlement, PlayerState, PotionCardRewardKind, SlimeSize,
    BASE_PLAYER_ENERGY,
};
pub use transition::{
    add_generated_card_to_draw_pile_random_spot_public, apply_combat_action,
    apply_combat_action_with_events, choose_draw_select, choose_exhaust_select, choose_hand_select,
    confirm_draw_select, confirm_exhume_select_skipped_return, confirm_hand_select,
    confirm_hand_select_skipped_put_on_deck_retrieval, draw_select_ui_to_draw_index,
    hand_select_ui_to_hand_index, open_gambling_chip_select, CombatTransition,
};
pub use turn::{end_player_turn, finish_monster_turn_after_player_revival, start_player_turn};
