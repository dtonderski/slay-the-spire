//! Source-backed initialized death lifecycle, not dedicated real-game parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    Potion, Relic, RunDecisionAction, RunState,
};
fn setup(content: sts_core::adapter_internals::ContentId, relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    let strike = c.piles.hand[0].content_id;
    c.piles.hand = (100..105)
        .map(|id| CardInstance::new(CardId::new(id), strike))
        .collect();
    c.piles.hand[0].content_id = content;
    c.piles.draw_pile = (105..125)
        .map(|id| CardInstance::new(CardId::new(id), strike))
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
fn play(r: &RunState, target: Option<sts_core::adapter_internals::MonsterId>) -> RunState {
    let a = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target,
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
fn assert_frozen(n: &RunState, energy: i32) {
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.player.energy, energy);
    assert_eq!(c.piles.hand.len(), 4);
    assert_eq!(c.piles.draw_pile.len(), 20);
    assert!(c.piles.exhaust_pile.is_empty());
    assert!(c.piles.discard_pile.is_empty());
    assert_eq!(c.piles.limbo.len(), 1);
    assert_eq!(c.piles.limbo[0].id, CardId::new(100));
    assert_eq!(c.card_in_use, Some(CardId::new(100)));
}
#[test]
fn offering_true_death_freezes_energy_draw_and_use_card() {
    assert_frozen(
        &play(&setup(sts_core::content::cards::OFFERING_ID, vec![]), None),
        3,
    );
}
#[test]
fn bloodletting_true_death_freezes_energy_and_use_card() {
    assert_frozen(
        &play(
            &setup(sts_core::content::cards::BLOODLETTING_ID, vec![]),
            None,
        ),
        3,
    );
}
#[test]
fn death_never_runs_exhaust_callbacks() {
    let mut r = setup(
        sts_core::content::cards::OFFERING_ID,
        vec![Relic::DeadBranch],
    );
    let c = r.combat.as_mut().unwrap();
    c.player.powers.feel_no_pain = 6;
    c.player.powers.dark_embrace = 1;
    let rng = c.rng.clone();
    let n = play(&r, None);
    assert_frozen(&n, 3);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.block, 0);
    assert_eq!(c.rng, rng);
}
#[test]
fn bloom_rejects_revival_then_freezes_queue() {
    let mut r = setup(
        sts_core::content::cards::OFFERING_ID,
        vec![Relic::MarkOfBloom, Relic::LizardTail],
    );
    r.potions = vec![Potion::Fairy];
    let n = play(&r, None);
    assert_frozen(&n, 3);
    assert!(!n.lizard_tail_used);
    assert_eq!(n.potions, vec![Potion::Fairy]);
}
#[test]
fn fairy_revives_and_continues_queue() {
    let mut r = setup(sts_core::content::cards::OFFERING_ID, vec![]);
    r.potions = vec![Potion::Fairy];
    let n = play(&r, None);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.player.energy, 5);
    assert_eq!(c.piles.hand.len(), 7);
    assert_eq!(c.piles.exhaust_pile.len(), 1);
    assert!(c.piles.limbo.is_empty());
}
#[test]
fn tail_revives_and_continues_queue() {
    let n = play(
        &setup(
            sts_core::content::cards::OFFERING_ID,
            vec![Relic::LizardTail],
        ),
        None,
    );
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.player.energy, 5);
    assert_eq!(c.piles.hand.len(), 7);
    assert_eq!(c.piles.exhaust_pile.len(), 1);
}
#[test]
fn buffer_prevents_death_and_keeps_following_effects() {
    let mut r = setup(sts_core::content::cards::OFFERING_ID, vec![]);
    r.combat.as_mut().unwrap().player.powers.buffer = 1;
    let n = play(&r, None);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 1);
    assert_eq!(c.player.powers.buffer, 0);
    assert_eq!(c.player.energy, 5);
    assert_eq!(c.piles.hand.len(), 7);
}
#[test]
fn spike_death_freezes_strike_settlement() {
    let mut r = setup(sts_core::content::cards::STRIKE_R_ID, vec![]);
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].powers.spikes = 1;
    let target = c.monsters[0].id;
    let n = play(&r, Some(target));
    assert_frozen(&n, 2);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].hp, 994);
}
#[test]
fn spike_tail_revival_still_settles_strike() {
    let mut r = setup(
        sts_core::content::cards::STRIKE_R_ID,
        vec![Relic::LizardTail],
    );
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].powers.spikes = 1;
    let target = c.monsters[0].id;
    let n = play(&r, Some(target));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.piles.discard_pile.len(), 1);
    assert!(c.piles.limbo.is_empty());
}
