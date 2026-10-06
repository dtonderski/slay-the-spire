//! Source-backed initialized Buffer/stat fixtures, not dedicatedinteractiontrace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    MonsterId, MonsterIntent, Relic, RunDecisionAction, RunState,
};
use sts_core::content::{cards::COMBUST_ID, monsters::EXPLODER_ID};
fn setup() -> RunState {
    let mut r = RunState::combat_fixture_with_relics(vec![Relic::TungstenRod]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.buffer = 1;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: CombatAction) -> RunState {
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
    eprintln!("buffer_rod_transition={:?}", a);
    n
}
#[test]
fn normal_raw_one_consumes_buffer_before_rod() {
    let mut r = setup();
    r.combat.as_mut().unwrap().player.powers.plated_armor = 5;
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn thorns_raw_one_consumes_buffer_before_rod() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    let mut explosion = c.monsters[0].clone();
    explosion.id = MonsterId::new(2);
    explosion.content_id = EXPLODER_ID;
    explosion.intent = MonsterIntent::Stun;
    explosion.powers.explosive = 1;
    c.monsters[0].intent = MonsterIntent::Block { block: 0 };
    c.monsters.insert(0, explosion);
    c.player.powers.plated_armor = 29;
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn combust_hp_loss_consumes_buffer_before_rod() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].intent = MonsterIntent::Block { block: 0 };
    c.piles.hand = vec![CardInstance::new(CardId::new(100), COMBUST_ID)];
    let n = step(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        },
    );
    let n = step(&n, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn reflected_thorns_raw_one_consumes_buffer_before_rod() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].powers.spikes = 1;
    c.piles.hand = vec![CardInstance::new(
        CardId::new(100),
        sts_core::content::cards::STRIKE_R_ID,
    )];
    let n = step(
        &r,
        CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: Some(MonsterId::new(1)),
        },
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn multi_hit_raw_one_consumes_only_first_buffer() {
    let mut r = setup();
    r.combat.as_mut().unwrap().monsters[0].intent =
        MonsterIntent::AttackMultiple { damage: 1, hits: 2 };
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
#[test]
fn zero_damage_does_not_consume_buffer() {
    let mut r = setup();
    r.combat.as_mut().unwrap().monsters[0].intent = MonsterIntent::Attack { damage: 0 };
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 1);
}
#[test]
fn fully_blocked_damage_does_not_consume_buffer() {
    let mut r = setup();
    r.combat.as_mut().unwrap().player.powers.plated_armor = 6;
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 1);
}
#[test]
fn absent_buffer_still_allows_rod_to_zero_one() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.buffer = 0;
    c.player.powers.plated_armor = 5;
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
}
#[test]
fn ordinary_large_damage_buffer_control() {
    let r = setup();
    let n = step(&r, CombatAction::EndTurn);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
}
