//! Declared custom two-enemy THORNS1/50, HP1, 25cards before actions; not dedicated interaction parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    MonsterId, Potion, Relic, RunDecisionAction, RunState,
};
fn play(first: i32, last: i32, relics: Vec<Relic>, buffer: i32, fairy: bool) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    if fairy {
        r.potions = vec![Potion::Fairy];
    }
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.player.powers.buffer = buffer;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters[0].powers.spikes = first;
    let mut other = c.monsters[0].clone();
    other.id = MonsterId::new(64);
    other.powers.spikes = last;
    c.monsters.push(other);
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
        .collect();
    c.piles.hand[0].content_id = sts_core::content::cards::CLEAVE_ID;
    c.piles.draw_pile = (105..125)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
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
    let a = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    };
    assert!(legal_run_decision_actions(&r)
        .unwrap()
        .contains(&RunDecisionAction::Combat(a)));
    if first == 50 && last == 1 {
        // Core must die after consuming Tail once. The separate run wrapper
        // can currently revive it twice; that defect is frozen independently.
        let core = sts_core::adapter_internals::apply_combat_action(r.combat.as_ref().unwrap(), a)
            .unwrap();
        assert_eq!(core.player.hp, 0);
        assert!(core.lizard_tail_used);
        assert_eq!(core.piles.limbo.len(), 1);
    }
    let before = serde_json::to_value(&r).unwrap();
    let n = apply_combat_action_on_run(&r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_combat_action_on_run(&restored, a).unwrap()).unwrap()
    );
    assert!(n
        .combat
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .all(|m| m.hp == 992));
    n
}
#[test]
fn area_thorns_add_to_top_reverses_target_order() {
    let n = play(1, 50, vec![Relic::LizardTail], 0, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 39);
    assert!(n.lizard_tail_used);
    assert_eq!(c.piles.discard_pile.len(), 1);
}
#[test]
fn equal_thorns_control() {
    let n = play(1, 1, vec![Relic::LizardTail], 0, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 39);
    assert!(n.lizard_tail_used);
    assert_eq!(c.piles.discard_pile.len(), 1);
}
#[test]
fn reverse_initial_target_powers_kills_core_after_one_tail() {
    // Core assertions are in play; no claim that the run wrapper is fixed.
    let _ = play(50, 1, vec![Relic::LizardTail], 0, false);
}
#[test]
fn fairy_revives_before_the_older_thorns_hit() {
    let n = play(1, 50, vec![], 0, true);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 23);
    assert!(n.potions.is_empty());
}
#[test]
fn cube_draws_once_per_settled_hp_loss() {
    let n = play(1, 50, vec![Relic::LizardTail, Relic::RunicCube], 0, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 39);
    assert_eq!(c.piles.hand.len(), 6);
}
#[test]
fn puzzle_draws_only_once_across_both_hits() {
    let n = play(
        1,
        50,
        vec![Relic::LizardTail, Relic::CentennialPuzzle],
        0,
        false,
    );
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 39);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn buffer_is_consumed_by_the_last_target_first() {
    let n = play(1, 50, vec![Relic::LizardTail], 1, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.player.powers.buffer, 0);
}
