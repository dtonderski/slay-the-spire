//! Synthetic legal END prefixes with explicit mixed formations, not vanilla
//! encounter-generation or dedicated interaction-trace parity evidence.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterId, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::content::monsters::{GREMLIN_LEADER_ID, SPHERIC_GUARDIAN_ID};
fn setup(block: i32, count: usize) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    let template = c.monsters[0].clone();
    c.monsters.clear();
    for index in 0..count {
        let mut m = template.clone();
        m.id = MonsterId::new(index as u64 + 1);
        m.content_id = SPHERIC_GUARDIAN_ID;
        m.hp = 20;
        m.max_hp = 20;
        m.block = block;
        m.intent = MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        };
        c.monsters.push(m);
    }
    let mut leader = template;
    leader.id = MonsterId::new(count as u64 + 1);
    leader.content_id = GREMLIN_LEADER_ID;
    leader.hp = 100;
    leader.max_hp = 100;
    leader.block = 998;
    leader.intent = MonsterIntent::EncourageGremlins {
        strength: 3,
        block: 6,
    };
    c.monsters.push(leader);
    r.validate().unwrap();
    r
}
fn end(r: &RunState) -> RunState {
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "encourage_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
#[test]
fn encourage_caps_near_full_ally_block() {
    let n = end(&setup(998, 1));
    assert_eq!(n.combat.unwrap().monsters[0].block, 999);
}
#[test]
fn encourage_caps_already_full_ally_block() {
    let n = end(&setup(999, 1));
    assert_eq!(n.combat.unwrap().monsters[0].block, 999);
}
#[test]
fn encourage_caps_each_living_ally() {
    let n = end(&setup(998, 2));
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].block, 999);
    assert_eq!(c.monsters[1].block, 999);
    assert_eq!(c.monsters[2].block, 0);
    assert!(c.monsters.iter().all(|m| m.powers.strength == 3));
}
#[test]
fn ordinary_gain_and_no_leader_block_control() {
    let n = end(&setup(40, 1));
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].block, 61);
    assert_eq!(c.monsters[1].block, 0);
    assert!(c.monsters.iter().all(|m| m.powers.strength == 3));
}
#[test]
fn dead_ally_is_not_gained_or_repaired_control() {
    let mut r = setup(998, 1);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.hp = 0;
    m.alive = false;
    r.validate().unwrap();
    let n = end(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].block, 998);
    assert_eq!(c.monsters[0].powers.strength, 0);
    assert_eq!(c.monsters[1].block, 0);
}
#[test]
fn capped_ally_block_affects_following_strike_damage() {
    let mut r = setup(998, 1);
    r.combat.as_mut().unwrap().player.powers.strength = 994;
    let n = end(&r);
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: sts_core::adapter_internals::CardId::new(1),
        target: Some(MonsterId::new(1)),
    });
    assert!(legal_run_decision_actions(&n).unwrap().contains(&a));
    let before = serde_json::to_value(&n).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let after = apply_run_decision_action(&n, a).unwrap();
    after.validate().unwrap();
    assert_eq!(serde_json::to_value(&n).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    assert_eq!(after.combat.unwrap().monsters[0].hp, 19);
}
#[test]
fn direct_hook_malformed_block_overflow_keeps_group_unchanged() {
    let r = setup(0, 2);
    let mut group = r.combat.unwrap().monsters;
    group[1].block = i32::MAX;
    let before = serde_json::to_value(&group).unwrap();
    let error = sts_core::content::monsters::apply_gremlin_leader_encourage(
        &mut group,
        MonsterId::new(3),
        3,
        6,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid state: monster group arithmetic overflow"
    );
    assert_eq!(serde_json::to_value(&group).unwrap(), before);
}
