//! Declared custom two-enemy THORNS1/50, HP1, 25cards before actions; run resource publication, not dedicated parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    MonsterId, Potion, Relic, RunDecisionAction, RunState,
};
fn play(first: i32, last: i32, relics: Vec<Relic>, buffer: i32, fairy: bool) -> RunState {
    play_with_tail_state(first, last, relics, buffer, fairy, false)
}
fn play_with_tail_state(
    first: i32,
    last: i32,
    relics: Vec<Relic>,
    buffer: i32,
    fairy: bool,
    used_tail: bool,
) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.lizard_tail_used = used_tail;
    if fairy {
        r.potions = vec![Potion::Fairy];
    }
    let c = r.combat.as_mut().unwrap();
    c.lizard_tail_used = used_tail;
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
fn reverse_initial_target_powers_changes_survival() {
    let n = play(50, 1, vec![Relic::LizardTail], 0, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.piles.limbo.len(), 1);
    assert!(c.piles.discard_pile.is_empty());
    assert!(c.lizard_tail_used);
    assert!(n.lizard_tail_used);
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
fn used_tail_is_not_rearmed_by_run_fallback() {
    let n = play_with_tail_state(1, 50, vec![Relic::LizardTail], 0, false, true);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 0);
    assert!(n.lizard_tail_used);
}
#[test]
fn absent_tail_never_creates_a_revival() {
    let n = play(1, 50, vec![], 0, false);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 0);
    assert!(!n.lizard_tail_used);
}
#[test]
fn buffer_is_consumed_by_the_last_target_first() {
    let n = play(1, 50, vec![Relic::LizardTail], 1, false);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.player.powers.buffer, 0);
}
