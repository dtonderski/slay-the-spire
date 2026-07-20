use sts_core::{
    apply_combat_action,
    card::CardInstance,
    combat::{DrawSelectPurpose, DrawSelectState, HandSelectState},
    content::cards,
    content::shop_pool::shop_card_content_id,
    enter_reward_screen, CardId, CombatAction, CombatState, ContentId, RunPhase, RunState,
    SimError,
};

#[test]
fn explicit_combat_and_run_fixtures_are_valid() {
    CombatState::initial_fixture()
        .validate()
        .expect("combat fixture has explicit valid state");
    RunState::combat_fixture()
        .validate()
        .expect("combat run fixture is valid");
    RunState::map_fixture()
        .validate()
        .expect("map run fixture is valid");
}

#[test]
fn missing_combat_rng_fails_before_action_execution() {
    let mut state = CombatState::initial_fixture();
    state.card_random_rng = None;

    assert_eq!(
        apply_combat_action(&state, CombatAction::EndTurn),
        Err(SimError::InvalidState("combat RNG state is incomplete"))
    );
}

#[test]
fn duplicate_card_ids_fail_validation() {
    let mut state = CombatState::initial_fixture();
    state.piles.draw_pile[0].id = state.piles.hand[0].id;

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "card instance appears in more than one pile"
        ))
    );
}

#[test]
fn unknown_content_fails_validation() {
    let mut state = CombatState::initial_fixture();
    state.piles.hand[0].content_id = ContentId::new(u64::MAX);

    assert_eq!(
        state.validate(),
        Err(SimError::UnknownContent(ContentId::new(u64::MAX)))
    );
}

#[test]
fn invalid_player_bounds_fail_validation() {
    let mut state = CombatState::initial_fixture();
    state.player.hp = state.player.max_hp + 1;

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState("combat player HP is out of bounds"))
    );
}

#[test]
fn multiple_active_combat_decisions_fail_validation() {
    let mut state = CombatState::initial_fixture();
    state.hand_select = Some(HandSelectState {
        purpose: Default::default(),
        source_card_id: state.piles.hand[0].id,
        selected_hand_index: None,
        selected_hand_indices: Vec::new(),
    });
    state.draw_select = Some(DrawSelectState {
        purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
        source_card_id: state.piles.hand[0].id,
        selected_draw_index: None,
    });

    assert_eq!(
        state.validate(),
        Err(SimError::InvalidState(
            "multiple combat decisions are active"
        ))
    );
}

#[test]
fn run_phase_ownership_is_validated() {
    let mut run = RunState::combat_fixture();
    run.phase = RunPhase::Idle;

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "combat state exists outside combat phase"
        ))
    );
}

#[test]
fn duplicate_run_deck_ids_fail_validation() {
    let mut run = RunState::map_fixture();
    run.deck[1].id = run.deck[0].id;

    assert_eq!(
        run.validate(),
        Err(SimError::InvalidState(
            "duplicate run deck card instance ID"
        ))
    );
}

#[test]
fn known_generated_card_content_remains_valid() {
    let mut run = RunState::map_fixture();
    run.pending_obtain_cards.push(cards::ANGER_ID);

    run.validate().expect("known pending card is valid");
}

#[test]
fn known_unmodeled_reward_identity_is_valid_until_selected() {
    let mut run = RunState::map_fixture();
    enter_reward_screen(&mut run);
    let content_id = shop_card_content_id("REACH_HEAVEN");
    let reward_id = CardId::new(run.next_card_instance_id());
    run.reward
        .as_mut()
        .expect("reward screen")
        .choices
        .push(CardInstance::new(reward_id, content_id));

    run.validate()
        .expect("known unmodeled card may remain a visible reward choice");

    let deck_id = CardId::new(run.next_card_instance_id());
    run.deck.push(CardInstance::new(deck_id, content_id));
    assert_eq!(run.validate(), Err(SimError::UnknownContent(content_id)));
}
