//! Declared HP1/current RedSkull active3/privateflag before legalEND; not naturalstart/dedicatedparity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CombatAction, Relic, RunDecisionAction,
    RunState,
};
fn setup(flower: bool, artifact: i32) -> RunState {
    setup_with_extra(flower, artifact, vec![])
}
fn setup_with_extra(flower: bool, artifact: i32, extra: Vec<Relic>) -> RunState {
    let mut relics = vec![Relic::RedSkull, Relic::LizardTail];
    relics.extend(extra);
    if flower {
        relics.push(Relic::MagicFlower);
    }
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.player.powers.strength = 3;
    c.player.powers.artifact = artifact;
    c.relic_counters.red_skull_active = true;
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
    eprintln!("red_skull_revival_transition={:?}", a);
    n
}
#[test]
fn tail_flower_revival_crosses_not_bloodied_threshold() {
    let n = end(&setup(true, 0));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 60);
    assert_eq!(c.player.powers.strength, 0);
    assert!(!c.relic_counters.red_skull_active);
}
#[test]
fn revived_not_bloodied_loss_is_artifact_blockable() {
    let n = end(&setup(true, 1));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 60);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 0);
    assert!(!c.relic_counters.red_skull_active);
}
#[test]
fn bark_fairy_crosses_not_bloodied_threshold() {
    let mut r = setup_with_extra(false, 0, vec![Relic::SacredBark]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 48);
    assert_eq!(c.player.powers.strength, 0);
    assert!(!c.relic_counters.red_skull_active);
    assert!(!c.lizard_tail_used);
}
#[test]
fn bark_fairy_removal_consumes_artifact_once() {
    let mut r = setup_with_extra(false, 1, vec![Relic::SacredBark]);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 48);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 0);
    assert!(!c.relic_counters.red_skull_active);
}
#[test]
fn blocked_removal_does_not_retry_on_next_hit() {
    let n = end(&setup(true, 1));
    let next = end(&n);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 54);
    assert_eq!(c.player.powers.strength, 3);
    assert!(!c.relic_counters.red_skull_active);
}
#[test]
fn bloom_has_no_heal_threshold_callback() {
    let n = end(&setup_with_extra(true, 1, vec![Relic::MarkOfBloom]));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
    assert!(c.relic_counters.red_skull_active);
}
#[test]
fn no_skull_revival_keeps_unrelated_strength() {
    let mut r = RunState::combat_fixture_with_relics(vec![Relic::LizardTail, Relic::MagicFlower]);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 1;
    c.player.powers.strength = 7;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 60);
    assert_eq!(c.player.powers.strength, 7);
}
#[test]
fn half_health_revival_remains_bloodied_control() {
    let n = end(&setup(false, 1));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
    assert!(c.relic_counters.red_skull_active);
}
