use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{LIMIT_BREAK_ID, LIMIT_BREAK_PLUS_ID},
    CardId, CardInstance, CombatAction, CombatState, ContentId,
};

fn scenario(content: ContentId, temporary: i32) -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.player.powers.strength = 5;
    state.player.temp_strength = temporary;
    state.player.energy = 1;
    state.piles.hand = vec![CardInstance::new(CardId::new(1), content)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.duplication_potion_pending = true;
    state.duplication_potion_stacks = 1;
    state.validate().unwrap();
    state
}

fn play(state: &CombatState) -> CombatState {
    apply_combat_action(
        state,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: None,
        },
    )
    .unwrap()
}

#[test]
fn copied_base_limit_break_doubles_live_strength() {
    // LimitBreakAction.update reads the current Strength power for each use.
    let next = play(&scenario(LIMIT_BREAK_ID, 0));
    assert_eq!(next.player.powers.strength, 20);
    assert_eq!(next.player.energy, 0);
    assert!(next.piles.hand.is_empty());
    assert_eq!(next.piles.exhaust_pile.len(), 1);
    next.validate().unwrap();
}

#[test]
fn copied_upgraded_limit_break_settles_original_once() {
    let next = play(&scenario(LIMIT_BREAK_PLUS_ID, 0));
    assert_eq!(next.player.powers.strength, 20);
    assert_eq!(next.player.energy, 0);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert!(next.piles.exhaust_pile.is_empty());
}

#[test]
fn copied_limit_break_includes_live_temporary_strength() {
    let next = play(&scenario(LIMIT_BREAK_PLUS_ID, 2));
    assert_eq!(next.player.powers.strength + next.player.temp_strength, 28);
    assert_eq!(next.player.temp_strength, 2);
}

#[test]
fn copied_limit_break_is_snapshot_deterministic() {
    let state = scenario(LIMIT_BREAK_PLUS_ID, 0);
    let restored = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    let next = play(&state);
    assert_eq!(next, play(&restored));
    assert_eq!(next.player.powers.strength, 20);
}
