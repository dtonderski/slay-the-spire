use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{RECYCLE_ANY_COLOR_ID, STRIKE_R_ID, WHIRLWIND_ID, WOUND_ID},
    CardId, CardInstance, CombatAction, ContentId, RunAction, RunState,
};

fn recycle_target(content: ContentId, temp_cost: Option<u8>, expected_energy: i32) {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    let mut target = CardInstance::new(CardId::new(20), content);
    target.temp_cost = temp_cost;
    let mut recycle = CardInstance::new(CardId::new(12), RECYCLE_ANY_COLOR_ID);
    recycle.upgrades = 1; // Recycle+ costs zero in the target.
    combat.piles.hand = vec![
        recycle,
        target,
        CardInstance::new(CardId::new(21), STRIKE_R_ID),
    ];
    combat.piles.draw_pile.clear();
    run.validate().expect("valid initial scenario");
    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(12),
            target: None,
        },
    )
    .expect("Recycle opens selection");
    let chosen = apply_run_action(&opened, RunAction::ChooseExhaustSelect { index: 0 })
        .expect("choose target");
    let next = apply_run_action(&chosen, RunAction::ConfirmExhaustSelect).expect("confirm");
    next.validate().expect("valid post-selection state");
    let combat = next.combat.as_ref().expect("combat");
    assert_eq!(combat.player.energy, expected_energy);
    assert!(combat
        .piles
        .exhaust_pile
        .iter()
        .any(|card| card.id == CardId::new(20)));
}

// Source: desktop-1.0.jar RecycleAction.update. X-cost gains the energy
// available when selected; positive costForTurn gains that amount; other costs
// gain nothing. Synthetic fuzz seed 6933 found the negative-cost failure.
#[test]
fn recycle_unplayable_card_never_spends_energy() {
    recycle_target(WOUND_ID, None, 3);
}

#[test]
fn recycle_x_cost_card_gains_current_energy() {
    recycle_target(WHIRLWIND_ID, None, 6);
}

#[test]
fn recycle_uses_positive_cost_for_turn() {
    recycle_target(STRIKE_R_ID, Some(3), 6);
}

#[test]
fn recycle_zero_cost_card_gains_nothing() {
    recycle_target(STRIKE_R_ID, Some(0), 3);
}
