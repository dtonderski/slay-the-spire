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
    apply_combat_action_with_events, apply_time_warp_lag_metallicize_keep_hand, choose_draw_select,
    choose_exhaust_select, choose_hand_select, close_discovery_source_card,
    close_discovery_source_card_with_force_exhaust, confirm_draw_select,
    confirm_draw_select_skipped_retrieval, confirm_dual_wield_select_skipped_retrieval,
    confirm_dual_wield_select_skipped_retrieval_without_restore, confirm_exhaust_select,
    confirm_exhaust_select_with_time_warp_policy, confirm_exhume_select_skipped_return,
    confirm_forethought_multi_select_skipped_retrieval,
    confirm_gambling_chip_select_skipped_retrieval, confirm_hand_select,
    confirm_hand_select_skipped_armaments_retrieval,
    confirm_hand_select_skipped_put_on_deck_retrieval,
    confirm_hand_select_time_warp_remaining_status_lag, confirm_hand_select_time_warp_status_lag,
    confirm_hand_select_with_time_warp_policy, confirm_headbutt_select_skipped_retrieval,
    confirm_headbutt_select_skipped_retrieval_with_time_warp_policy,
    confirm_purity_select_skipped_retrieval, confirm_recycle_select_skipped_retrieval,
    confirm_true_grit_select_skipped_retrieval, draw_select_ui_to_draw_index,
    hand_select_ui_to_hand_index, open_gambling_chip_select, process_internal_queue_public,
    settle_leftover_end_turn_hand_discard, settle_queued_end_turn_discard_after_rejected_command,
    settle_time_warp_end_turn_if_ready_public, settle_time_warp_pre_discard_if_ready_public,
    CombatTransition,
};
pub use turn::{
    end_player_turn, finish_monster_turn_after_player_revival,
    settle_leftover_end_turn_monster_and_draw,
    settle_leftover_end_turn_monster_and_draw_skipping_post_draw_relics,
    settle_leftover_end_turn_monster_lose_block,
    settle_leftover_end_turn_monster_then_start_without_draw,
    settle_leftover_end_turn_player_powers_and_discard, start_player_turn,
};
