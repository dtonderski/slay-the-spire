use sts_core::adapter_internals::{
    apply_combat_action, content::cards::LIMIT_BREAK_PLUS_ID, CardId, CardInstance, CombatAction,
    CombatState,
};

fn doubled(permanent: i32, temporary: i32) -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.player.powers.strength = permanent;
    state.player.temp_strength = temporary;
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), LIMIT_BREAK_PLUS_ID)];
    apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .expect("Limit Break should apply bounded Strength normally")
}

#[test]
fn limit_break_positive_strength_stops_at_999() {
    let next = doubled(700, 0);
    assert_eq!(next.player.powers.strength, 999);
    next.validate().unwrap();
}

#[test]
fn limit_break_negative_strength_stops_at_minus_999() {
    let next = doubled(-700, 0);
    assert_eq!(next.player.powers.strength, -999);
    next.validate().unwrap();
}

#[test]
fn limit_break_caps_visible_strength_including_temporary_strength() {
    let next = doubled(700, 2);
    assert_eq!(next.player.powers.strength + next.player.temp_strength, 999);
    assert_eq!(next.player.temp_strength, 2);
    assert_eq!(next.player.powers.strength, 997);
}

#[test]
fn temporary_loss_after_negative_cap_remains_bounded() {
    let state = doubled(-700, 2);
    assert_eq!(
        state.player.powers.strength + state.player.temp_strength,
        -999
    );
    let next = apply_combat_action(&state, CombatAction::EndTurn).unwrap();
    assert_eq!(next.player.powers.strength, -999);
    assert_eq!(next.player.temp_strength, 0);
}

#[test]
fn temporary_loss_after_positive_cap_is_not_restored() {
    let state = doubled(700, 2);
    let next = apply_combat_action(&state, CombatAction::EndTurn).unwrap();
    assert_eq!(next.player.powers.strength, 997);
    assert_eq!(next.player.temp_strength, 0);
}

#[test]
fn limit_break_at_strength_cap_does_not_grow_further() {
    let next = doubled(999, 0);
    assert_eq!(next.player.powers.strength, 999);
    assert_eq!(next.player.energy, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.discard_pile.len(), 1);
}
