//! Source-backed Reaper HealAction must follow reflected THORNS, not undo an early heal.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    Relic, RunDecisionAction, RunState,
};
fn play(hp: i32, relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = hp;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters[0].powers.spikes = 3;
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), sts_core::content::cards::STRIKE_R_ID))
        .collect();
    c.piles.hand[0].content_id = sts_core::content::cards::REAPER_ID;
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
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].hp, 996);
    n
}
#[test]
fn capped_reaper_heal_follows_reflection() {
    let n = play(80, vec![]);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn tail_revival_precedes_reaper_heal() {
    let n = play(1, vec![Relic::LizardTail]);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 44);
}
#[test]
fn true_death_freezes_reaper_heal() {
    let n = play(1, vec![]);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.piles.limbo.len(), 1);
    assert!(c.piles.exhaust_pile.is_empty());
}
#[test]
fn rod_mitigation_precedes_capped_heal() {
    let n = play(80, vec![Relic::TungstenRod]);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
