use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{RECYCLE_ANY_COLOR_ID, STRIKE_R_ID, WHIRLWIND_ID},
    CardId, CardInstance, CombatAction, CombatState, Relic, RunAction, RunPhase, RunState,
};

fn exhaust_with_recycle(singleton: bool, corruption: bool) -> CombatState {
    let mut run = RunState::combat_fixture_with_relics(vec![Relic::DeadBranch]);
    let combat = run.combat.as_mut().expect("combat");
    let mut source = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    source.upgrades = 1;
    combat.piles.hand = vec![CardInstance::new(CardId::new(20), STRIKE_R_ID), source];
    if !singleton {
        combat
            .piles
            .hand
            .insert(1, CardInstance::new(CardId::new(21), STRIKE_R_ID));
    }
    combat.piles.draw_pile.clear();
    combat.player.powers.corruption = i32::from(corruption);
    run.validate().expect("valid initial state");
    let mut next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .expect("play Recycle");
    if !singleton {
        next =
            apply_run_action(&next, RunAction::ChooseExhaustSelect { index: 0 }).expect("choose");
        next = apply_run_action(&next, RunAction::ConfirmExhaustSelect).expect("confirm");
    }
    next.validate().expect("unique identities after callbacks");
    next.combat.expect("combat")
}

#[test]
fn singleton_recycle_runs_dead_branch_and_reserves_held_source_identity() {
    // Review reproduction: the automatic case must produce the same one Dead
    // Branch card as normal confirmation, above held source ID 99.
    let automatic = exhaust_with_recycle(true, false);
    let selected = exhaust_with_recycle(false, false);
    let generated = |combat: &CombatState| {
        combat
            .piles
            .hand
            .iter()
            .copied()
            .filter(|card| card.combat_only)
            .collect::<Vec<_>>()
    };
    assert_eq!(generated(&automatic).len(), 1);
    assert_eq!(generated(&automatic), generated(&selected));
    assert_eq!(generated(&automatic)[0].id, CardId::new(100));
    assert!(automatic.decision.is_none());
}

#[test]
fn recycle_under_corruption_generates_once_for_target_and_once_for_source() {
    for singleton in [true, false] {
        let combat = exhaust_with_recycle(singleton, true);
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .filter(|card| card.combat_only)
                .count(),
            2
        );
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(99)));
    }
}

#[test]
fn singleton_exhaust_callbacks_can_finish_combat() {
    let mut run =
        RunState::combat_fixture_with_relics(vec![Relic::DeadBranch, Relic::CharonsAshes]);
    let combat = run.combat.as_mut().expect("combat");
    let mut source = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    source.upgrades = 1;
    combat.piles.hand = vec![CardInstance::new(CardId::new(20), STRIKE_R_ID), source];
    combat.piles.draw_pile.clear();
    for monster in &mut combat.monsters {
        monster.hp = 3;
        monster.block = 0;
    }
    run.validate().expect("valid initial state");
    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .expect("auto exhaust and settle victory");
    next.validate().expect("valid settled victory");
    assert_eq!(next.phase, RunPhase::Reward);
}

#[test]
fn singleton_recycle_of_x_cost_card_checks_energy() {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    let mut source = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    source.upgrades = 1;
    combat.piles.hand = vec![CardInstance::new(CardId::new(20), WHIRLWIND_ID), source];
    combat.piles.draw_pile.clear();
    combat.player.energy = 3;
    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .expect("auto exhaust");
    next.validate().expect("valid successor");
    let combat = next.combat.expect("combat");
    assert_eq!(combat.player.energy, 6);
    assert!(combat.decision.is_none());
}
