use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{EXHUME_ID, EXHUME_PLUS_ID, STRIKE_R_ID},
    CardId, CardInstance, CombatAction, RunAction, RunState,
};

fn scenario(content: sts_core::adapter_internals::ContentId, exhausted: &[u64]) -> RunState {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().unwrap();
    combat.piles.hand = vec![CardInstance::new(CardId::new(17), content)];
    combat.piles.draw_pile.clear();
    combat.piles.exhaust_pile = exhausted
        .iter()
        .map(|id| CardInstance::new(CardId::new(*id), STRIKE_R_ID))
        .collect();
    combat.duplication_potion_pending = true;
    combat.duplication_potion_stacks = 1;
    run.validate().unwrap();
    run
}

fn play(run: &RunState) -> RunState {
    apply_combat_action_on_run(
        run,
        CombatAction::PlayCard {
            card_id: CardId::new(17),
            target: None,
        },
    )
    .expect("both Exhume actions use the live exhaust pile")
}

fn choose(run: &RunState) -> RunState {
    apply_run_action(run, RunAction::ChooseExhaustSelect { index: 0 }).unwrap()
}

// Reduced from fuzz seed 94637. ExhumeAction.update checks the live pile;
// the copied action must not return the original singleton's ID a second time.
#[test]
fn duplicated_exhume_singleton_is_not_returned_twice() {
    for content in [EXHUME_ID, EXHUME_PLUS_ID] {
        let next = play(&scenario(content, &[33]));
        next.validate().unwrap();
        let combat = next.combat.as_ref().unwrap();
        assert!(combat.decision.is_none());
        assert_eq!(
            combat.piles.hand.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![CardId::new(33)]
        );
        assert_eq!(
            combat
                .piles
                .exhaust_pile
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(17)]
        );
    }
}

#[test]
fn duplicated_exhume_opens_a_second_live_grid_after_original_selection() {
    let opened = play(&scenario(EXHUME_PLUS_ID, &[33, 34]));
    let copied = choose(&opened);
    copied.validate().unwrap();
    assert!(copied.combat.as_ref().unwrap().exhaust_select().is_some());
    let next = choose(&copied);
    next.validate().unwrap();
    let combat = next.combat.as_ref().unwrap();
    assert!(combat.decision.is_none());
    assert_eq!(combat.piles.hand.len(), 2);
    assert_eq!(combat.piles.exhaust_pile.len(), 1);
    assert_eq!(combat.piles.exhaust_pile[0].id, CardId::new(17));
}

#[test]
fn duplicated_exhume_with_empty_exhaust_finishes_without_a_grid() {
    let next = play(&scenario(EXHUME_PLUS_ID, &[]));
    next.validate().unwrap();
    let combat = next.combat.as_ref().unwrap();
    assert!(combat.decision.is_none());
    assert!(combat.piles.hand.is_empty());
    assert_eq!(combat.piles.exhaust_pile.len(), 1);
}

#[test]
fn copied_exhume_skips_when_original_return_fills_the_hand() {
    let mut run = scenario(EXHUME_PLUS_ID, &[33]);
    let combat = run.combat.as_mut().unwrap();
    for id in 40..49 {
        combat
            .piles
            .hand
            .push(CardInstance::new(CardId::new(id), STRIKE_R_ID));
    }
    run.validate().unwrap();
    let next = play(&run);
    next.validate().unwrap();
    let combat = next.combat.as_ref().unwrap();
    assert!(combat.decision.is_none());
    assert_eq!(combat.piles.hand.len(), 10);
    assert_eq!(combat.piles.exhaust_pile.len(), 1);
}
