//! Declared synthetic StaticDischarge/orbcapacity/durableHP fixtures; no interactiontrace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CombatAction, MonsterIntent, Relic,
    RunDecisionAction, RunState,
};
fn setup() -> RunState {
    setup_with_relics(vec![Relic::TungstenRod])
}
fn setup_with_relics(relics: Vec<Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.static_discharge = 1;
    c.max_orbs = 3;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
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
    eprintln!("static_late_transition={:?}", a);
    n
}
#[test]
fn positive_attack_erased_by_rod_still_channels() {
    let mut r = setup();
    r.combat.as_mut().unwrap().player.powers.plated_armor = 5;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 1);
}
#[test]
fn each_positive_multi_hit_erased_by_rod_still_channels() {
    let mut r = setup();
    r.combat.as_mut().unwrap().monsters[0].intent =
        MonsterIntent::AttackMultiple { damage: 1, hits: 2 };
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 2);
}
#[test]
fn torii_and_rod_do_not_erase_earlier_power_callback() {
    let mut r = setup_with_relics(vec![Relic::Torii, Relic::TungstenRod]);
    r.combat.as_mut().unwrap().monsters[0].intent = MonsterIntent::Attack { damage: 2 };
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 1);
}
#[test]
fn full_block_prevents_channel() {
    let mut r = setup();
    r.combat.as_mut().unwrap().player.powers.plated_armor = 6;
    let n = end(&r);
    assert!(n.combat.as_ref().unwrap().orbs.is_empty());
}
#[test]
fn ordinary_no_rod_control() {
    let r = setup_with_relics(vec![]);
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 74);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 1);
}
#[test]
fn intangible_caps_before_callback_rod_is_later() {
    let mut r = setup();
    r.combat.as_mut().unwrap().player.powers.intangible = 1;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 1);
}
#[test]
fn nominal_static_potency_is_not_hp_loss_amount() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.plated_armor = 5;
    c.player.powers.static_discharge = 2;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().orbs.len(), 2);
}
#[test]
fn early_buffer_control_blocks_static() {
    let mut r = setup();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.plated_armor = 5;
    c.player.powers.buffer = 1;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert!(n.combat.as_ref().unwrap().orbs.is_empty());
}
