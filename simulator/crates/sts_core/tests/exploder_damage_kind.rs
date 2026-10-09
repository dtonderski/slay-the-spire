//! Initialized explosion intent/cross-color power/orb capacity diagnostic; no natural-prefix parity claim.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CombatAction, MonsterId,
    RunDecisionAction, RunState,
};
use sts_core::content::monsters::EXPLODER_ID;
fn setup(static_discharge: i32) -> RunState {
    setup_with_relics(static_discharge, Vec::new())
}
fn setup_with_relics(
    static_discharge: i32,
    relics: Vec<sts_core::adapter_internals::Relic>,
) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    let mut explosion = c.monsters[0].clone();
    explosion.id = MonsterId::new(2);
    explosion.content_id = EXPLODER_ID;
    explosion.intent = sts_core::adapter_internals::MonsterIntent::Stun;
    explosion.powers.explosive = 1;
    c.monsters[0].intent = sts_core::adapter_internals::MonsterIntent::Block { block: 0 };
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters.insert(0, explosion);
    c.player.powers.static_discharge = static_discharge;
    c.max_orbs = 3;
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
    eprintln!("explosion_candidate_transition={:?}", a);
    n
}
#[test]
fn thorns_explosion_must_not_channel_static_discharge() {
    let r = setup(1);
    let n = end(&r);
    assert_eq!(
        n.combat.as_ref().unwrap().orbs,
        r.combat.as_ref().unwrap().orbs
    );
}
#[test]
fn thorns_explosion_does_not_apply_torii() {
    let mut r = setup_with_relics(0, vec![sts_core::adapter_internals::Relic::Torii]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.plated_armor = 25;
    let n = end(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.hp, 75);
    assert_eq!(p.powers.plated_armor, 25);
}
#[test]
fn thorns_explosion_keeps_tungsten_but_not_torii() {
    let mut r = setup_with_relics(
        0,
        vec![
            sts_core::adapter_internals::Relic::Torii,
            sts_core::adapter_internals::Relic::TungstenRod,
        ],
    );
    let c = r.combat.as_mut().unwrap();
    c.player.powers.plated_armor = 25;
    let n = end(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.hp, 76);
    assert_eq!(p.powers.plated_armor, 25);
}
#[test]
fn buffer_blocks_thorns_without_static_or_plate_change() {
    let mut r = setup(1);
    r.combat.as_mut().unwrap().player.powers.buffer = 1;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 80);
    assert_eq!(c.player.powers.buffer, 0);
    assert!(c.orbs.is_empty());
}
#[test]
fn intangible_caps_thorns_without_static() {
    let mut r = setup(1);
    r.combat.as_mut().unwrap().player.powers.intangible = 1;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 79);
    assert!(c.orbs.is_empty());
}
#[test]
fn full_block_absorbs_thorns() {
    let mut r = setup(1);
    r.combat.as_mut().unwrap().player.powers.plated_armor = 30;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 80);
    assert_eq!(c.player.powers.plated_armor, 30);
    assert!(c.orbs.is_empty());
}
#[test]
fn normal_attack_still_channels_and_reduces_plate() {
    let mut r = setup(1);
    let c = r.combat.as_mut().unwrap();
    c.monsters.remove(0);
    c.monsters[0].intent = sts_core::adapter_internals::MonsterIntent::Attack { damage: 6 };
    c.player.powers.plated_armor = 4;
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 78);
    assert_eq!(c.player.powers.plated_armor, 3);
    assert_eq!(c.orbs.len(), 1);
}
#[test]
fn no_static_discharge_control() {
    let r = setup(0);
    let n = end(&r);
    assert_eq!(
        n.combat.as_ref().unwrap().orbs,
        r.combat.as_ref().unwrap().orbs
    );
}
