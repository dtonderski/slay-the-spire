//! GameActionManager leftover duration under SuperFastMode.
//!
//! Collection-fork SuperFastMode multiplies `Gdx.graphics.getDeltaTime()`
//! (`deltaMultiplier=100` at a 600 FPS cap). Vanilla `tickDuration` subtracts
//! that multiplied delta; `GameActionManager.update` starts the next action in
//! the same call when `currentAction.isDone`, so unused occupancy carries into
//! the next action's first tick.
//!
//! Screen-open actions (`PutOnDeckAction`, `DiscoveryAction`, `DiscardAction`
//! for Gambling Chip, `ExhaustAction`, …) call `tickDuration` once when they
//! open. If leftover occupancy plus that tick drives duration below 0, the
//! action is already `isDone` while the screen is up, and CONFIRM/CHOOSE never
//! runs retrieval.

use super::state::CombatState;
use crate::action::InternalAction;

/// `Settings.ACTION_DUR_FAST`.
pub const ACTION_DUR_FAST_MILLIS: i32 = 250;
/// `DamageAction.DURATION`.
pub const DAMAGE_ACTION_MILLIS: i32 = 100;
/// SuperFastMode multiplied delta: `100 / 600s` in milliticks, rounded.
pub const MULTIPLIED_DELTA_MILLIS: i32 = 167;
/// Raw `getDelta()` milliticks used by collection-fork `SuperFastMode.tickDuration`
/// (DiscardAction) and leftover occupancy windows (`0.1 / 0.016 ≈ 7`).
#[allow(dead_code)]
pub const RAW_DELTA_MILLIS: i32 = 16;

/// Tick a duration-bearing action to completion and store leftover occupancy.
///
/// `GameActionManager.update` only starts the next action in the same call when
/// `currentAction` is already `isDone` at entry. An action that finishes on its
/// first `tickDuration` therefore leaves unused occupancy for the next action.
/// An action that needs later frames (FAST 0.25 vs multiplied ~0.167) completes
/// on a subsequent update; the next action then starts on a fresh frame with no
/// carried overshoot — otherwise Warcry's DrawCardAction would always skip
/// PutOnDeck retrieval.
pub fn complete_action_duration(state: &mut CombatState, duration_millis: i32) {
    if duration_millis <= 0 {
        return;
    }
    let first = MULTIPLIED_DELTA_MILLIS.saturating_add(state.action_leftover_millis);
    state.action_leftover_millis = 0;
    if first >= duration_millis {
        state.action_leftover_millis = first - duration_millis;
    }
}

/// Opening-screen `tickDuration`. Returns true when the action is `isDone`.
pub fn tick_screen_open(state: &mut CombatState) -> bool {
    let tick = MULTIPLIED_DELTA_MILLIS.saturating_add(state.action_leftover_millis);
    state.action_leftover_millis = 0;
    if tick >= ACTION_DUR_FAST_MILLIS {
        state.action_leftover_millis = tick - ACTION_DUR_FAST_MILLIS;
        state.skip_screen_retrieval = true;
        state.screen_remaining_millis = 0;
        true
    } else {
        state.skip_screen_retrieval = false;
        state.screen_remaining_millis = ACTION_DUR_FAST_MILLIS - tick;
        false
    }
}

/// Post-CONFIRM retrieve `tickDuration` on an action that was not `isDone` at open.
pub fn tick_screen_retrieve(state: &mut CombatState) {
    let remaining = state.screen_remaining_millis;
    state.screen_remaining_millis = 0;
    state.skip_screen_retrieval = false;
    if remaining > 0 {
        complete_action_duration(state, remaining);
    }
}

pub fn consume_skip_screen_retrieval(state: &mut CombatState) -> bool {
    let skip = state.skip_screen_retrieval;
    state.skip_screen_retrieval = false;
    state.screen_remaining_millis = 0;
    skip
}

/// Same-bound `addToRandomSpot` rolls while leftover occupancy still covers
/// raw `tickDuration` frames (FIDL01680 Reckless Charge Dazed).
#[allow(dead_code)]
pub fn leftover_same_bound_add_to_random_spot_rolls(state: &CombatState) -> usize {
    let extra = (state.action_leftover_millis / RAW_DELTA_MILLIS).max(0) as usize;
    extra.saturating_add(1)
}

pub fn tick_internal_action_duration(state: &mut CombatState, action: &InternalAction) {
    match action {
        InternalAction::AwaitHandSelect { .. }
        | InternalAction::AwaitDrawSelect { .. }
        | InternalAction::AwaitDiscardSelect { .. }
        | InternalAction::AwaitCopiedDiscardSelect { .. }
        | InternalAction::AwaitExhaustSelect { .. }
        | InternalAction::OpenDiscoveryCardReward { .. } => {
            tick_screen_open(state);
        }
        InternalAction::DealDamage { .. }
        | InternalAction::DealBodySlamDamage { .. }
        | InternalAction::DealHandOfGreedDamage { .. }
        | InternalAction::DealRitualDaggerDamage { .. }
        | InternalAction::DealDamageAndHealUnblocked { .. }
        | InternalAction::DealDamageRandomEnemy { .. }
        | InternalAction::DealFeedDamage { .. }
        | InternalAction::DealDamageAll { .. }
        | InternalAction::FireBreathingDamage { .. }
        | InternalAction::DealDamageAllAndHealUnblocked { .. }
        | InternalAction::DealThornsDamageToPlayer { .. }
        | InternalAction::DealUnmodifiedDamage { .. }
        | InternalAction::DealUnmodifiedDamageRandom { .. }
        | InternalAction::LoseHp { .. } => complete_action_duration(state, DAMAGE_ACTION_MILLIS),
        InternalAction::DealDamageAllRepeated { times, .. } => {
            for _ in 0..(*times).max(0) {
                complete_action_duration(state, DAMAGE_ACTION_MILLIS);
            }
        }
        InternalAction::GainBlock { .. }
        | InternalAction::GainBlockDirect { .. }
        | InternalAction::GainBlockFromExhaust { .. }
        | InternalAction::GainMonsterBlock { .. }
        | InternalAction::DoublePlayerBlock
        | InternalAction::DrawCards { .. }
        | InternalAction::DrawCardsWithoutEvolve { .. }
        | InternalAction::DrawCardsWhilePlayedCardIsInLimbo { .. }
        | InternalAction::DrawCardsWhilePlayedCardIsInLimboWithoutEvolve { .. }
        | InternalAction::DrawCardsFromInkBottle { .. }
        | InternalAction::DrawCardsIfNoAttacksInHand { .. }
        | InternalAction::DrawRandomAttacksFromDrawPile { .. }
        | InternalAction::UnceasingTopDraw
        | InternalAction::ShuffleDiscardIntoDraw
        | InternalAction::DeepBreathShuffleDiscardIntoDraw
        | InternalAction::AddGeneratedCardToPile { .. }
        | InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
        | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
        | InternalAction::AddRandomColorlessCardToHand { .. }
        | InternalAction::AddCardToPile { .. }
        | InternalAction::AddStatEquivalentCopyToPile { .. }
        | InternalAction::AddCardInstanceToHandOrDiscard { .. } => {
            complete_action_duration(state, ACTION_DUR_FAST_MILLIS);
        }
        InternalAction::ApplyVulnerable { .. }
        | InternalAction::ApplyPlayerVulnerable { .. }
        | InternalAction::ApplyWeak { .. }
        | InternalAction::ApplyMark { .. } => complete_action_duration(state, DAMAGE_ACTION_MILLIS),
        InternalAction::GainStrength { .. }
        | InternalAction::GainDexterity { .. }
        | InternalAction::GainTempStrength { .. }
        | InternalAction::GainEnergy { .. } => {
            complete_action_duration(state, ACTION_DUR_FAST_MILLIS);
        }
        InternalAction::GainFeelNoPain { .. }
        | InternalAction::GainDarkEmbrace { .. }
        | InternalAction::GainBarricade { .. }
        | InternalAction::GainEvolve { .. }
        | InternalAction::GainBerserk { .. }
        | InternalAction::GainFasting { .. }
        | InternalAction::GainRupture { .. }
        | InternalAction::GainJuggernaut { .. }
        | InternalAction::GainBrutality { .. }
        | InternalAction::GainMayhem { .. }
        | InternalAction::GainPanache { .. }
        | InternalAction::GainCombust { .. }
        | InternalAction::GainDoubleTap { .. }
        | InternalAction::GainFireBreathing { .. }
        | InternalAction::GainCorruption { .. }
        | InternalAction::GainSadisticNature { .. }
        | InternalAction::GainMagnetism { .. }
        | InternalAction::GainMetallicize { .. }
        | InternalAction::GainRage { .. }
        | InternalAction::GainIntangible { .. }
        | InternalAction::GainRitual { .. }
        | InternalAction::GainArtifact { .. } => {
            complete_action_duration(state, ACTION_DUR_FAST_MILLIS);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn stable_multiplied_delta_does_not_finish_fast_open() {
        let mut state = CombatState::initial_fixture();
        assert!(!tick_screen_open(&mut state));
        assert!(!state.skip_screen_retrieval);
        assert_eq!(
            state.screen_remaining_millis,
            ACTION_DUR_FAST_MILLIS - MULTIPLIED_DELTA_MILLIS
        );
    }

    #[test]
    fn two_tick_fast_action_does_not_skip_following_open() {
        let mut state = CombatState::initial_fixture();
        complete_action_duration(&mut state, ACTION_DUR_FAST_MILLIS);
        assert_eq!(state.action_leftover_millis, 0);
        assert!(!tick_screen_open(&mut state));
    }

    #[test]
    fn chained_one_tick_damage_builds_skip_occupancy() {
        let mut state = CombatState::initial_fixture();
        complete_action_duration(&mut state, DAMAGE_ACTION_MILLIS);
        complete_action_duration(&mut state, DAMAGE_ACTION_MILLIS);
        assert!(tick_screen_open(&mut state));
        assert!(state.skip_screen_retrieval);
    }

    #[test]
    fn damage_then_fast_open_still_retrieves() {
        let mut state = CombatState::initial_fixture();
        complete_action_duration(&mut state, DAMAGE_ACTION_MILLIS);
        assert!(!tick_screen_open(&mut state));
    }

    #[test]
    fn leftover_same_bound_rolls_scale_with_occupancy() {
        let mut state = CombatState::initial_fixture();
        assert_eq!(leftover_same_bound_add_to_random_spot_rolls(&state), 1);
        complete_action_duration(&mut state, DAMAGE_ACTION_MILLIS);
        assert!(leftover_same_bound_add_to_random_spot_rolls(&state) >= 4);
    }
}
