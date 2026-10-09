use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{HAVOC_ID, RECYCLE_ANY_COLOR_ID, STRIKE_R_ID},
    legal_run_decision_actions, CardId, CardInstance, CombatAction, RunAction, RunState,
};

fn scenario(hand: &[u64]) -> RunState {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    combat.piles.hand = hand
        .iter()
        .map(|id| {
            CardInstance::new(
                CardId::new(*id),
                if *id == 12 {
                    RECYCLE_ANY_COLOR_ID
                } else {
                    STRIKE_R_ID
                },
            )
        })
        .collect();
    combat.piles.draw_pile.clear();
    run.validate().expect("valid scenario");
    run
}

// Source: PC RecycleAction.update's first-duration empty/singleton branches.
// Synthetic fuzz seeds 3050, 7334, 7677 found an empty, unfinishable screen.
#[test]
fn recycle_empty_hand_finishes_without_a_selection() {
    let run = scenario(&[12]);
    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(12),
            target: None,
        },
    )
    .expect("play");
    next.validate().expect("valid successor");
    let combat = next.combat.as_ref().expect("combat");
    assert!(combat.decision.is_none());
    assert!(combat
        .piles
        .discard_pile
        .iter()
        .any(|c| c.id == CardId::new(12)));
    assert!(!legal_run_decision_actions(&next).expect("legal").is_empty());
}

#[test]
fn recycle_singleton_auto_exhausts_without_a_selection() {
    let run = scenario(&[12, 20]);
    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(12),
            target: None,
        },
    )
    .expect("play");
    next.validate().expect("valid successor");
    let combat = next.combat.as_ref().expect("combat");
    assert!(combat.decision.is_none());
    assert!(combat
        .piles
        .exhaust_pile
        .iter()
        .any(|c| c.id == CardId::new(20)));
    assert!(combat
        .piles
        .discard_pile
        .iter()
        .any(|c| c.id == CardId::new(12)));
}

#[test]
fn havoc_recycle_selection_does_not_resettle_an_already_exhausted_source() {
    let mut run = scenario(&[20, 21, 22]);
    let combat = run.combat.as_mut().expect("combat");
    combat.piles.hand[0].content_id = HAVOC_ID;
    combat
        .piles
        .draw_pile
        .push(CardInstance::new(CardId::new(12), RECYCLE_ANY_COLOR_ID));
    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(20),
            target: None,
        },
    )
    .expect("Havoc auto-plays Recycle");
    let selected =
        apply_run_action(&opened, RunAction::ChooseExhaustSelect { index: 0 }).expect("choose");
    let next = apply_run_action(&selected, RunAction::ConfirmExhaustSelect).expect("confirm");
    next.validate().expect("valid successor");
    let combat = next.combat.as_ref().expect("combat");
    assert_eq!(
        combat
            .piles
            .exhaust_pile
            .iter()
            .filter(|c| c.id == CardId::new(12))
            .count(),
        1
    );
}
