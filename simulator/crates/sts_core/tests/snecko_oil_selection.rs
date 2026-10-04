use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{STRIKE_R_ID, THIRD_EYE_ANY_COLOR_ID, WHIRLWIND_ID},
    legal_run_decision_actions, CardId, CardInstance, CombatAction, Potion, Relic, RunAction,
    RunState,
};

fn scry(oils: usize, bark: bool) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(if bark {
        vec![Relic::SacredBark]
    } else {
        vec![]
    });
    run.potions = vec![Potion::SneckoOil; oils];
    run.empty_potion_slots = (oils..3).collect();
    let combat = run.combat.as_mut().unwrap();
    combat.piles.hand = vec![
        CardInstance::new(CardId::new(14), THIRD_EYE_ANY_COLOR_ID),
        CardInstance::new(CardId::new(40), WHIRLWIND_ID),
        CardInstance::new(CardId::new(41), STRIKE_R_ID),
    ];
    combat.piles.draw_pile = (20..30)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    combat.piles.discard_pile.clear();
    run.validate().unwrap();
    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(14),
            target: None,
        },
    )
    .unwrap();
    apply_run_action(&opened, RunAction::ChooseDrawSelect { index: 0 }).unwrap()
}

fn use_oil(run: &RunState, slot: usize) -> RunState {
    apply_run_action(run, RunAction::UsePotion { slot, target: None }).unwrap()
}

// Reduced from seed 96235: SneckoOil.use queues DrawCardAction then
// RandomizeHandCostAction behind ScryAction's still-open selection.
#[test]
fn snecko_oil_waits_for_scry_without_mutating_hand_draw_or_rng() {
    let selected = scry(1, false);
    let before = selected.combat.as_ref().unwrap();
    let queued = use_oil(&selected, 0);
    let combat = queued.combat.as_ref().unwrap();
    assert_eq!(combat.piles.hand, before.piles.hand);
    assert_eq!(combat.piles.draw_pile, before.piles.draw_pile);
    assert_eq!(
        combat.draw_select().unwrap().selected_draw_indices,
        before.draw_select().unwrap().selected_draw_indices
    );
    assert_eq!(combat.rng.card_random_rng, before.rng.card_random_rng);
    assert_eq!(
        queued.card_random_rng_counter,
        selected.card_random_rng_counter
    );
    queued.validate().unwrap();
}

#[test]
fn scry_confirm_discards_selected_card_before_snecko_draw_and_cost_rolls() {
    for bark in [false, true] {
        let selected = scry(1, bark);
        let combat = selected.combat.as_ref().unwrap();
        let selected_index = combat.draw_select().unwrap().selected_draw_indices[0];
        let discarded_id = combat.piles.draw_pile[selected_index].id;
        let counter = combat.rng.card_random_rng.counter();
        let queued = use_oil(&selected, 0);
        let next = apply_run_action(&queued, RunAction::ConfirmDrawSelect)
            .expect("enumerated Scry confirmation remains valid after Snecko Oil");
        next.validate().unwrap();
        let combat = next.combat.as_ref().unwrap();
        assert!(combat.decision.is_none());
        assert!(combat
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == discarded_id));
        assert!(!combat.piles.hand.iter().any(|card| card.id == discarded_id));
        assert_eq!(combat.piles.hand.len(), if bark { 10 } else { 7 });
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .find(|c| c.id == CardId::new(40))
                .unwrap()
                .temp_cost,
            None
        );
        let rolls = combat.piles.hand.len() as u32 - 1;
        assert_eq!(combat.rng.card_random_rng.counter(), counter + rolls);
        assert_eq!(
            next.card_random_rng_counter,
            combat.rng.card_random_rng.counter()
        );
        assert!(combat
            .piles
            .hand
            .iter()
            .filter(|c| c.id != CardId::new(40))
            .all(|c| c.temp_cost.is_some_and(|cost| cost <= 3)));
    }
}

// Synthetic durable-profile seed 34136 chose the third Scry card before
// Snecko Oil drew through it. Confirmation must still discard that card.
#[test]
fn snecko_oil_preserves_a_non_top_scry_selection() {
    let selected = apply_run_action(&scry(1, false), RunAction::ChooseDrawSelect { index: 0 })
        .expect("deselect top card");
    let selected = apply_run_action(&selected, RunAction::ChooseDrawSelect { index: 2 })
        .expect("select third card");
    let before = selected.combat.as_ref().unwrap();
    let selected_index = before.draw_select().unwrap().selected_draw_indices[0];
    let selected_id = before.piles.draw_pile[selected_index].id;
    let queued = use_oil(&selected, 0);
    assert_eq!(
        queued.combat.as_ref().unwrap().piles.draw_pile,
        before.piles.draw_pile
    );
    let next = apply_run_action(&queued, RunAction::ConfirmDrawSelect)
        .expect("confirm remains legal after potion use");
    next.validate().unwrap();
    let combat = next.combat.as_ref().unwrap();
    assert!(combat
        .piles
        .discard_pile
        .iter()
        .any(|card| card.id == selected_id));
    assert!(!combat.piles.hand.iter().any(|card| card.id == selected_id));
}

#[test]
fn two_queued_oils_keep_draw_randomize_draw_randomize_order() {
    let selected = scry(2, false);
    let counter = selected
        .combat
        .as_ref()
        .unwrap()
        .rng
        .card_random_rng
        .counter();
    let queued = use_oil(&use_oil(&selected, 0), 1);
    let next = apply_run_action(&queued, RunAction::ConfirmDrawSelect).unwrap();
    next.validate().unwrap();
    let combat = next.combat.as_ref().unwrap();
    assert_eq!(combat.piles.hand.len(), 10);
    // First draw: seven cards, one X-cost. Second draw: ten, one X-cost.
    assert_eq!(combat.rng.card_random_rng.counter(), counter + 6 + 9);
    assert_eq!(
        next.card_random_rng_counter,
        combat.rng.card_random_rng.counter()
    );
}

#[test]
fn queued_oil_snapshot_round_trip_and_legal_queries_preserve_rng() {
    let queued = use_oil(&scry(1, false), 0);
    let bytes = serde_json::to_vec(&queued).unwrap();
    let restored: RunState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        legal_run_decision_actions(&queued).unwrap(),
        legal_run_decision_actions(&restored).unwrap()
    );
    assert_eq!(serde_json::to_vec(&queued).unwrap(), bytes);
    let next = apply_run_action(&queued, RunAction::ConfirmDrawSelect).unwrap();
    let replayed = apply_run_action(&restored, RunAction::ConfirmDrawSelect).unwrap();
    assert_eq!(next, replayed);
}
