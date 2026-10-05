//! Initialized callback diagnostics; not natural accepted-prefix evidence.
use super::*;
#[test]
fn pending_fnp_gain_caps_live_block_and_keeps_nominal_callback() {
    let mut state = CombatState::initial_fixture();
    state.player.block = 998;
    state.player.powers.juggernaut = 5;
    let rng_before = state.rng.card_random_rng.clone();
    assert_eq!(
        apply_deferred_end_turn_exhaust_block(&mut state, 6).unwrap(),
        Some(5)
    );
    assert_eq!(state.player.block, 999);
    assert_eq!(state.rng.card_random_rng, rng_before);
    assert_eq!(
        apply_deferred_end_turn_exhaust_block(&mut state, 6).unwrap(),
        Some(5)
    );
    assert_eq!(state.player.block, 999);
}
#[test]
fn malformed_pending_gain_overflow_retains_checked_error() {
    let mut state = CombatState::initial_fixture();
    state.player.block = i32::MAX;
    let before = serde_json::to_value(&state).unwrap();
    assert_eq!(
        apply_deferred_end_turn_exhaust_block(&mut state, 6),
        Err(SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}
