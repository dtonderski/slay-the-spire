//! Declared HP1/25cards/enemy1000/spikes1 before actions; synthetic cross-color Evolve/FireBreathing coverage, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    Potion, Relic, RunDecisionAction, RunState,
};
fn setup(status: bool, relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters[0].powers.spikes = 1;
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
        .collect();
    c.piles.hand[0].content_id = sts_core::content::cards::CLEAVE_ID;
    c.piles.draw_pile = (105..125)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    if status {
        c.player.powers.evolve = 1;
        c.player.powers.fire_breathing = 6;
        c.draw_trigger_power_order = vec![
            sts_core::power::DrawTriggerPower::Evolve,
            sts_core::power::DrawTriggerPower::FireBreathing,
        ];
        c.piles.draw_pile.last_mut().unwrap().content_id = sts_core::content::cards::BURN_ID;
    }
    r.deck = c
        .piles
        .hand
        .iter()
        .chain(c.piles.draw_pile.iter())
        .cloned()
        .collect();
    r.validate().unwrap();
    r
}
fn play(r: &RunState) -> RunState {
    r.validate().unwrap();
    let a = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    };
    assert!(legal_run_decision_actions(r)
        .unwrap()
        .contains(&RunDecisionAction::Combat(a)));
    let before = serde_json::to_value(r).unwrap();
    let n = apply_combat_action_on_run(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_combat_action_on_run(&restored, a).unwrap()).unwrap()
    );
    n
}
fn check(n: &RunState, hp: i32, hand: usize, enemy_hp: i32) {
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, hp);
    assert_eq!(c.piles.hand.len(), hand);
    assert_eq!(c.monsters[0].hp, enemy_hp);
    assert!(c.pending_hp_loss_draw_follow_ups.is_empty());
}
#[test]
fn area_reflection_keeps_revived_status_draw_followups() {
    check(
        &play(&setup(true, vec![Relic::LizardTail, Relic::RunicCube])),
        40,
        6,
        986,
    );
}
#[test]
fn area_reflection_no_status_control() {
    check(
        &play(&setup(false, vec![Relic::LizardTail, Relic::RunicCube])),
        40,
        5,
        992,
    );
}
#[test]
fn fairy_area_reflection_keeps_status_followups() {
    let mut r = setup(true, vec![Relic::RunicCube]);
    r.potions = vec![Potion::Fairy];
    check(&play(&r), 24, 6, 986);
}
#[test]
fn true_death_does_not_draw_or_fire() {
    let n = play(&setup(true, vec![Relic::RunicCube]));
    check(&n, 0, 4, 992);
    assert_eq!(n.combat.as_ref().unwrap().piles.limbo.len(), 1);
}
#[test]
fn bloom_does_not_draw_or_fire() {
    check(
        &play(&setup(
            true,
            vec![Relic::LizardTail, Relic::RunicCube, Relic::MarkOfBloom],
        )),
        0,
        4,
        992,
    );
}
#[test]
fn buffer_does_not_trigger_draw_followups() {
    let mut r = setup(true, vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().player.powers.buffer = 1;
    check(&play(&r), 1, 4, 992);
}
#[test]
fn no_draw_prevents_status_followups() {
    let mut r = setup(true, vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().player.cannot_draw = true;
    check(&play(&r), 40, 4, 992);
}
#[test]
fn puzzle_area_reflection_keeps_status_followups() {
    check(
        &play(&setup(
            true,
            vec![Relic::LizardTail, Relic::CentennialPuzzle],
        )),
        40,
        8,
        986,
    );
}
#[test]
fn fire_breathing_without_evolve_still_returns_damage() {
    let mut r = setup(true, vec![Relic::LizardTail, Relic::RunicCube]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.evolve = 0;
    c.draw_trigger_power_order = vec![sts_core::power::DrawTriggerPower::FireBreathing];
    check(&play(&r), 40, 5, 986);
}
