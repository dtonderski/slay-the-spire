use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{
        BURNING_PACT_ID, BURNING_PACT_PLUS_ID, DAZED_ID, DEFEND_R_ID, RECKLESS_CHARGE_ID,
        STRIKE_R_ID, WILD_STRIKE_ID, WOUND_ID,
    },
    CardId, CardInstance, CombatAction, ContentId, Potion, RunAction, RunState,
};

fn selected_pact(source: ContentId, generator: ContentId, corruption: bool) -> RunState {
    let mut run = RunState::combat_fixture();
    run.potions = vec![Potion::DistilledChaos];
    run.empty_potion_slots = vec![1, 2];
    let combat = run.combat.as_mut().unwrap();
    combat.player.powers.corruption = i32::from(corruption);
    combat.monsters[0].hp = 500;
    combat.monsters[0].max_hp = 500;
    combat.piles.hand = vec![
        CardInstance::new(CardId::new(99), source),
        CardInstance::new(CardId::new(1), STRIKE_R_ID),
        CardInstance::new(CardId::new(2), DEFEND_R_ID),
    ];
    combat.piles.draw_pile = vec![
        CardInstance::new(CardId::new(98), STRIKE_R_ID),
        CardInstance::new(CardId::new(4), generator),
        CardInstance::new(CardId::new(5), DEFEND_R_ID),
        CardInstance::new(CardId::new(6), DEFEND_R_ID),
        CardInstance::new(CardId::new(7), STRIKE_R_ID),
        CardInstance::new(CardId::new(8), STRIKE_R_ID),
    ];
    if source == BURNING_PACT_PLUS_ID {
        combat
            .piles
            .draw_pile
            .push(CardInstance::new(CardId::new(9), STRIKE_R_ID));
    }
    combat.piles.discard_pile.clear();
    combat.piles.exhaust_pile.clear();
    run.validate().unwrap();
    let opened = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .unwrap();
    let selected = apply_run_action(&opened, RunAction::ChooseExhaustSelect { index: 0 }).unwrap();
    let queued = apply_run_action(
        &selected,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .unwrap();
    queued.validate().unwrap();
    queued
}

fn check_source_ownership(source: ContentId, generator: ContentId, status: ContentId) {
    // Reduced synthetic campaign failure 646663: Distilled Chaos was used
    // while Burning Pact's selection was open. Nested autoplay temporarily
    // replaces card_in_use, but must not release the held physical source ID.
    // This tests ownership/determinism, not newly established game parity.
    let selected = selected_pact(source, generator, false);
    let restored: RunState =
        serde_json::from_slice(&serde_json::to_vec(&selected).unwrap()).unwrap();
    let next = apply_run_action(&selected, RunAction::ConfirmExhaustSelect).unwrap();
    next.validate()
        .expect("held source and generated status remain distinct");
    assert_eq!(
        next,
        apply_run_action(&restored, RunAction::ConfirmExhaustSelect).unwrap()
    );
    let combat = next.combat.as_ref().unwrap();
    assert!(combat.decision.is_none());
    assert!(combat.piles.limbo.is_empty());
    assert_eq!(
        combat
            .piles
            .discard_pile
            .iter()
            .filter(|c| c.id == CardId::new(99))
            .count(),
        1
    );
    assert_eq!(
        combat.piles.discard_pile.last().unwrap().id,
        CardId::new(99),
        "do not move the source ahead of the deferred autoplay settlements"
    );
    let generated = combat
        .piles
        .draw_pile
        .iter()
        .find(|c| c.content_id == status)
        .unwrap();
    assert!(generated.id.get() > 99);
    assert!(generated.combat_only);
}

#[test]
fn burning_pact_reserves_held_source_during_deferred_wild_strike() {
    check_source_ownership(BURNING_PACT_ID, WILD_STRIKE_ID, WOUND_ID);
}

#[test]
fn upgraded_burning_pact_reserves_held_source_during_deferred_wild_strike() {
    check_source_ownership(BURNING_PACT_PLUS_ID, WILD_STRIKE_ID, WOUND_ID);
}

#[test]
fn burning_pact_reserves_held_source_during_deferred_reckless_charge() {
    check_source_ownership(BURNING_PACT_ID, RECKLESS_CHARGE_ID, DAZED_ID);
}

#[test]
fn upgraded_burning_pact_reserves_held_source_during_deferred_reckless_charge() {
    check_source_ownership(BURNING_PACT_PLUS_ID, RECKLESS_CHARGE_ID, DAZED_ID);
}

#[test]
fn corruption_exhausts_held_source_once_after_deferred_generation() {
    for source in [BURNING_PACT_ID, BURNING_PACT_PLUS_ID] {
        let selected = selected_pact(source, WILD_STRIKE_ID, true);
        let next = apply_run_action(&selected, RunAction::ConfirmExhaustSelect).unwrap();
        next.validate().unwrap();
        let combat = next.combat.as_ref().unwrap();
        assert!(combat.piles.limbo.is_empty());
        assert_eq!(
            combat.piles.exhaust_pile.last().unwrap().id,
            CardId::new(99)
        );
        assert_eq!(
            combat
                .piles
                .exhaust_pile
                .iter()
                .filter(|card| card.id == CardId::new(99))
                .count(),
            1
        );
        let generated = combat
            .piles
            .draw_pile
            .iter()
            .find(|card| card.content_id == WOUND_ID)
            .unwrap();
        assert_eq!(generated.id, CardId::new(100));
    }
}
