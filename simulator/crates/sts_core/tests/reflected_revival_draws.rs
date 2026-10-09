//! Declared HP1/25carddeck/enemyHP1000/spikes1 before actions. Synthetic not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    Potion, Relic, RunDecisionAction, RunState,
};
fn setup(hp: i32, relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = hp;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters[0].powers.spikes = 1;
    let content = sts_core::content::cards::STRIKE_R_ID;
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), content))
        .collect();
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
    let c = r.combat.as_ref().unwrap();
    let a = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: Some(c.monsters[0].id),
    };
    r.validate().unwrap();
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
#[test]
fn spikes_tail_cube_draw_survives() {
    let n = play(&setup(1, vec![Relic::LizardTail, Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 5);
}
#[test]
fn spikes_tail_puzzle_draw_survives() {
    let n = play(&setup(1, vec![Relic::LizardTail, Relic::CentennialPuzzle]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn nonlethal_spikes_cube_control() {
    let n = play(&setup(20, vec![Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 19);
    assert_eq!(c.piles.hand.len(), 5);
}
#[test]
fn fairy_cube_draw_survives() {
    let mut r = setup(1, vec![Relic::RunicCube]);
    r.potions = vec![Potion::Fairy];
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 5);
}
#[test]
fn fairy_puzzle_draw_survives() {
    let mut r = setup(1, vec![Relic::CentennialPuzzle]);
    r.potions = vec![Potion::Fairy];
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn true_death_drops_pending_cube_draw() {
    let n = play(&setup(1, vec![Relic::RunicCube]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.piles.hand.len(), 4);
    assert_eq!(c.piles.limbo.len(), 1);
}
#[test]
fn bloom_drops_pending_puzzle_draw() {
    let n = play(&setup(
        1,
        vec![
            Relic::MarkOfBloom,
            Relic::LizardTail,
            Relic::CentennialPuzzle,
        ],
    ));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.piles.hand.len(), 4);
    assert_eq!(c.piles.limbo.len(), 1);
}
#[test]
fn buffer_blocks_spikes_without_relic_draw() {
    let mut r = setup(1, vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().player.powers.buffer = 1;
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 1);
    assert_eq!(c.piles.hand.len(), 4);
    assert!(!n.lizard_tail_used);
}
#[test]
fn revived_cube_draw_respects_no_draw() {
    let mut r = setup(1, vec![Relic::LizardTail, Relic::RunicCube]);
    r.combat.as_mut().unwrap().player.cannot_draw = true;
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 4);
}
#[test]
fn revived_cube_keeps_evolve_and_fire_breathing_followups() {
    let mut r = setup(1, vec![Relic::LizardTail, Relic::RunicCube]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.evolve = 1;
    c.player.powers.fire_breathing = 6;
    c.draw_trigger_power_order = vec![
        sts_core::power::DrawTriggerPower::Evolve,
        sts_core::power::DrawTriggerPower::FireBreathing,
    ];
    c.piles.draw_pile.last_mut().unwrap().content_id = sts_core::content::cards::BURN_ID;
    r.deck.last_mut().unwrap().content_id = sts_core::content::cards::BURN_ID;
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.hand.len(), 6);
    assert_eq!(c.monsters[0].hp, 988);
    assert!(c.pending_hp_loss_draw_follow_ups.is_empty());
}
