//! Declared HP1 and durable enemy1000 before actions; no observed-state repair or dedicatedparity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CombatAction, Relic, RunDecisionAction,
    RunState,
};
fn setup(relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    let content = c.piles.hand[0].content_id;
    c.piles.hand = (100..105)
        .map(|id| {
            sts_core::adapter_internals::CardInstance::new(
                sts_core::adapter_internals::CardId::new(id),
                content,
            )
        })
        .collect();
    c.piles.draw_pile = (105..125)
        .map(|id| {
            sts_core::adapter_internals::CardInstance::new(
                sts_core::adapter_internals::CardId::new(id),
                content,
            )
        })
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
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
fn end(r: &RunState) -> RunState {
    let a = CombatAction::EndTurn;
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
    eprintln!("puzzle_revival_transition={:?}", a);
    n
}
#[test]
fn puzzle_draw_survives_revived_first_hp_loss() {
    let n = end(&setup(vec![Relic::LizardTail, Relic::CentennialPuzzle]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.relic_counters.centennial_puzzle_triggers, 1);
    assert_eq!(c.piles.hand.len(), 8);
}
#[test]
fn cube_revive_control() {
    let n = end(&setup(vec![Relic::LizardTail, Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 6);
}
#[test]
fn deferred_cube_multi_hit_control() {
    let mut r = setup(vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().monsters[0].intent =
        sts_core::adapter_internals::MonsterIntent::AttackMultiple { damage: 6, hits: 2 };
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 34);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn fairy_puzzle_draws_after_revive() {
    let mut r = setup(vec![Relic::CentennialPuzzle]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 8);
    assert!(n.potions.is_empty());
}
#[test]
fn fairy_cube_draws_after_revive() {
    let mut r = setup(vec![Relic::RunicCube]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 6);
}
#[test]
fn puzzle_multi_hit_is_queued_once_across_revival() {
    let mut r = setup(vec![Relic::LizardTail, Relic::CentennialPuzzle]);
    r.combat.as_mut().unwrap().monsters[0].intent =
        sts_core::adapter_internals::MonsterIntent::AttackMultiple { damage: 6, hits: 2 };
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 34);
    assert_eq!(c.piles.hand.len(), 8);
    assert_eq!(c.relic_counters.centennial_puzzle_triggers, 1);
}
#[test]
fn cube_true_death_does_not_draw() {
    let n = end(&setup(vec![Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.piles.hand.is_empty());
}
#[test]
fn bloom_cancels_draws_and_revival() {
    let n = end(&setup(vec![
        Relic::LizardTail,
        Relic::CentennialPuzzle,
        Relic::RunicCube,
        Relic::MarkOfBloom,
    ]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.piles.hand.is_empty());
}
#[test]
fn revived_cube_keeps_status_draw_followups() {
    let mut r = setup(vec![Relic::LizardTail, Relic::RunicCube]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.evolve = 1;
    c.player.powers.fire_breathing = 6;
    c.draw_trigger_power_order = vec![
        sts_core::power::DrawTriggerPower::Evolve,
        sts_core::power::DrawTriggerPower::FireBreathing,
    ];
    c.piles.draw_pile.last_mut().unwrap().content_id = sts_core::content::cards::BURN_ID;
    r.deck.last_mut().unwrap().content_id = sts_core::content::cards::BURN_ID;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 7);
    assert_eq!(c.monsters[0].hp, 994);
    assert!(c.pending_hp_loss_draw_follow_ups.is_empty());
}
#[test]
fn true_death_cancels_puzzle_control() {
    let n = end(&setup(vec![Relic::CentennialPuzzle]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.piles.hand.is_empty());
}
