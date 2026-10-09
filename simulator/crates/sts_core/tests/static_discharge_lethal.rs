//! Synthetic declared lowHP/Static1/maxorbs3; not naturalinitialpower or dedicatedtraceparity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
fn setup(hp: i32) -> RunState {
    setup_with_relics(hp, vec![])
}
fn setup_with_relics(hp: i32, relics: Vec<sts_core::adapter_internals::Relic>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.player.hp = hp;
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
    eprintln!("static_lethal_transition={:?}", a);
    n
}
#[test]
fn lethal_hit_freezes_queued_channel() {
    let n = end(&setup(1));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.orbs.is_empty());
}
#[test]
fn lizard_tail_revives_before_channel_settlement() {
    let n = end(&setup_with_relics(
        1,
        vec![sts_core::adapter_internals::Relic::LizardTail],
    ));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert!(c.lizard_tail_used);
    assert_eq!(c.orbs.len(), 1);
}
#[test]
fn bloom_blocks_revival_and_channel() {
    let n = end(&setup_with_relics(
        1,
        vec![
            sts_core::adapter_internals::Relic::LizardTail,
            sts_core::adapter_internals::Relic::MarkOfBloom,
        ],
    ));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.orbs.is_empty());
}
#[test]
fn unused_second_hit_channel_stops_on_lethal_first_hit() {
    let mut r = setup(1);
    r.combat.as_mut().unwrap().monsters[0].intent =
        sts_core::adapter_internals::MonsterIntent::AttackMultiple { damage: 6, hits: 2 };
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert!(c.orbs.is_empty());
}
#[test]
fn lizard_tail_second_hit_is_ordinary_live_callback() {
    let mut r = setup_with_relics(1, vec![sts_core::adapter_internals::Relic::LizardTail]);
    r.combat.as_mut().unwrap().monsters[0].intent =
        sts_core::adapter_internals::MonsterIntent::AttackMultiple { damage: 6, hits: 2 };
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 34);
    assert_eq!(c.orbs.len(), 2);
}
#[test]
fn fairy_revives_then_channels() {
    let mut r = setup(1);
    r.potions = vec![sts_core::adapter_internals::Potion::Fairy];
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 24);
    assert_eq!(c.orbs.len(), 1);
    assert!(n.potions.is_empty());
}
#[test]
fn lethal_full_orb_queue_does_not_evoke_after_death() {
    let mut r = setup(1);
    r.combat.as_mut().unwrap().orbs = vec![sts_core::combat::CombatOrb::Frost; 3];
    r.combat.as_mut().unwrap().monsters[0].intent =
        sts_core::adapter_internals::MonsterIntent::Attack { damage: 20 };
    let n = end(&r);
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 0);
    assert_eq!(c.orbs, vec![sts_core::combat::CombatOrb::Frost; 3]);
}
#[test]
fn nonlethal_channel_control() {
    let n = end(&setup(7));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 1);
    assert_eq!(c.orbs.len(), 1);
}
