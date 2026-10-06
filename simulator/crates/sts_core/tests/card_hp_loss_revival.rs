//! Declared25carddeck/HP1/durableenemy1000 beforeactions. No observed-state repair/dedicatedparity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    Relic, RunDecisionAction, RunState,
};
fn setup(hp: i32, relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = hp;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    let content = c.piles.hand[0].content_id;
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), content))
        .collect();
    c.piles.hand[0].content_id = sts_core::content::cards::OFFERING_ID;
    c.piles.draw_pile = (105..125)
        .map(|id| CardInstance::new(CardId::new(id), content))
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
fn play(r: &RunState) -> RunState {
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
    eprintln!("offering_revival_transition={:?}", a);
    n
}
#[test]
fn offering_tail_continues_energy_and_draw_queue() {
    let n = play(&setup(1, vec![Relic::LizardTail]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.player.energy, 5);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn revived_offering_cube_draws_before_offering_draw() {
    let n = play(&setup(1, vec![Relic::LizardTail, Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 8);
}
#[test]
fn revived_offering_puzzle_draws_before_offering_draw() {
    let n = play(&setup(1, vec![Relic::LizardTail, Relic::CentennialPuzzle]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 10);
}
#[test]
fn nonlethal_offering_control() {
    let n = play(&setup(20, vec![Relic::LizardTail]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 14);
    assert_eq!(c.player.energy, 5);
    assert_eq!(c.piles.hand.len(), 7);
}
// Queued energy/draw after true death is a separately frozen unresolved candidate.
#[test]
fn true_death_hp_only_control() {
    let n = play(&setup(1, vec![]));
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 0);
}
#[test]
fn fairy_cube_draw_survives_hp_loss() {
    let mut r = setup(1, vec![Relic::RunicCube]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 8);
    assert!(n.potions.is_empty());
}
#[test]
fn fairy_puzzle_draw_survives_hp_loss() {
    let mut r = setup(1, vec![Relic::CentennialPuzzle]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 10);
}
#[test]
fn bloodletting_revives_then_draws_cube() {
    let mut r = setup(1, vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().piles.hand[0].content_id = sts_core::content::cards::BLOODLETTING_ID;
    r.deck[0].content_id = sts_core::content::cards::BLOODLETTING_ID;
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 5);
    assert_eq!(c.player.energy, 5);
}
