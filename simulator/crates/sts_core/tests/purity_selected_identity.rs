use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{PURITY_ID, STRIKE_R_ID},
    CardId, CardInstance, CombatAction, Relic, RunAction, RunState,
};

#[test]
fn purity_keeps_not_yet_exhausted_selections_owned_during_dead_branch_callbacks() {
    // Reduced from synthetic fuzz seed 1022. Target ExhaustAction.update keeps
    // every selected card in HandCardSelectScreen.selectedCards while processing
    // moveToExhaustPile one at a time; callbacks cannot reuse a later card's UUID.
    // This is an ownership invariant, not a claim of new real-game parity.
    let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
    let combat = run.combat.as_mut().expect("combat");
    combat.piles.hand = vec![
        CardInstance::new(CardId::new(12), STRIKE_R_ID),
        CardInstance::new(CardId::new(22), STRIKE_R_ID),
        CardInstance::new(CardId::new(14), PURITY_ID),
    ];
    combat.piles.draw_pile = vec![CardInstance::new(CardId::new(21), STRIKE_R_ID)];
    combat.piles.discard_pile.clear();
    combat.piles.exhaust_pile.clear();
    run.validate().expect("valid initial scenario");
    let mut run = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(14),
            target: None,
        },
    )
    .expect("Purity opens selection");
    for _ in 0..2 {
        run = apply_run_action(&run, RunAction::ChooseExhaustSelect { index: 0 }).expect("choose");
    }
    let next = apply_run_action(&run, RunAction::ConfirmExhaustSelect).expect("confirm");
    next.validate().expect("all card identities remain unique");
    let combat = next.combat.as_ref().expect("combat");
    assert_eq!(
        combat
            .piles
            .exhaust_pile
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![CardId::new(12), CardId::new(22), CardId::new(14)]
    );
    assert_eq!(combat.piles.hand.len(), 3);
    assert!(combat.piles.hand.iter().all(|c| c.id.get() > 22));
}
