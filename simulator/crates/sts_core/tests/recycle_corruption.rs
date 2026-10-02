use sts_core::adapter_internals::{
    apply_combat_action_on_run,
    content::cards::{
        DEFEND_R_ID, MALAISE_ANY_COLOR_ID, RECYCLE_ANY_COLOR_ID, STRIKE_R_ID, WOUND_ID,
    },
    CardId, CardInstance, CombatAction, ContentId, RunState,
};

fn energy_after_recycle(target: ContentId, cost: Option<u8>, corruption: bool) -> i32 {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    let mut source = CardInstance::new(CardId::new(99), RECYCLE_ANY_COLOR_ID);
    source.upgrades = 1;
    let mut selected = CardInstance::new(CardId::new(20), target);
    selected.temp_cost = cost;
    combat.piles.hand = vec![selected, source];
    combat.piles.draw_pile.clear();
    combat.player.energy = 5;
    combat.player.powers.corruption = i32::from(corruption);
    run.validate().expect("initial");
    let next = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: source.id,
            target: None,
        },
    )
    .expect("auto exhaust");
    next.validate().expect("successor");
    next.combat.expect("combat").player.energy
}

#[test]
fn corruption_makes_selected_skill_cost_zero_for_recycle() {
    assert_eq!(energy_after_recycle(DEFEND_R_ID, None, true), 5);
}

#[test]
fn corruption_overrides_selected_skill_temporary_positive_cost() {
    assert_eq!(energy_after_recycle(DEFEND_R_ID, Some(3), true), 5);
    assert_eq!(energy_after_recycle(DEFEND_R_ID, Some(3), false), 8);
}

#[test]
fn corruption_does_not_change_attack_recycle_energy() {
    assert_eq!(energy_after_recycle(STRIKE_R_ID, Some(2), true), 7);
}

#[test]
fn corruption_does_not_replace_x_cost_sentinel_for_recycle() {
    // CorruptionPower.setCostForTurn(-9) does not modify negative costForTurn;
    // RecycleAction still takes its X-cost/current-energy branch.
    assert_eq!(energy_after_recycle(MALAISE_ANY_COLOR_ID, None, true), 10);
}

#[test]
fn unplayable_cards_still_grant_no_recycle_energy() {
    assert_eq!(energy_after_recycle(WOUND_ID, None, true), 5);
}
