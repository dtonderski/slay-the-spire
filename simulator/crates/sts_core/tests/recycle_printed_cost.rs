use sts_core::adapter_internals::{
    apply_combat_action_on_run,
    content::cards::{get_card_definition, RECYCLE_ANY_COLOR_ID},
    effective_card_cost_with_corruption, CardId, CardInstance, CombatAction, RunState,
};

fn play(upgrades: u8, energy: i32) -> sts_core::adapter_internals::SimResult<RunState> {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    let mut card = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    card.upgrades = upgrades;
    combat.piles.hand = vec![card];
    combat.piles.draw_pile.clear();
    combat.player.energy = energy;
    run.validate().expect("initial");
    apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: card.id,
            target: None,
        },
    )
}

#[test]
fn recycle_definition_and_instance_cost_match_constructor() {
    assert_eq!(
        get_card_definition(RECYCLE_ANY_COLOR_ID)
            .expect("definition")
            .cost,
        1
    );
    let card = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    assert_eq!(
        effective_card_cost_with_corruption(&card, false).expect("cost"),
        1
    );
}

#[test]
fn unupgraded_recycle_spends_one_energy_and_is_not_free_at_zero_energy() {
    let next = play(0, 3).expect("play");
    next.validate().expect("successor");
    assert_eq!(next.combat.expect("combat").player.energy, 2);
    assert!(play(0, 0).is_err());
}

#[test]
fn upgraded_recycle_is_free() {
    let next = play(1, 0).expect("play upgraded Recycle at zero energy");
    next.validate().expect("successor");
    assert_eq!(next.combat.expect("combat").player.energy, 0);
}

#[test]
fn temporary_recycle_cost_overrides_upgrade_discount() {
    let mut card = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    card.upgrades = 1;
    card.temp_cost = Some(3);
    assert_eq!(
        effective_card_cost_with_corruption(&card, false).expect("cost"),
        3
    );
}
