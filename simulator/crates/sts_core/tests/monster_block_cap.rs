//! Source-backed synthetic legal END prefixes; mixed formations are explicit
//! test setups, not claims about vanilla encounter generation or trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterId, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::content::monsters::{
    CENTURION_ID, DECA_ID, GREMLIN_TSUNDERE_ID, SPHERIC_GUARDIAN_ID,
};
fn setup(
    block: i32,
    intent: MonsterIntent,
    ally: Option<(sts_core::adapter_internals::ContentId, i32)>,
) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    let mut sphere = c.monsters[0].clone();
    sphere.content_id = SPHERIC_GUARDIAN_ID;
    sphere.id = MonsterId::new(1);
    sphere.hp = 20;
    sphere.max_hp = 20;
    sphere.block = block;
    sphere.intent = intent;
    sphere.moves_executed = 0;
    sphere.move_history.clear();
    c.monsters = vec![sphere];
    if let Some((content, amount)) = ally {
        let mut m = c.monsters[0].clone();
        m.id = MonsterId::new(2);
        m.content_id = content;
        m.block = 0;
        m.intent = MonsterIntent::Block { block: amount };
        m.hp = 100;
        m.max_hp = 100;
        c.monsters.push(m);
    }
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
        "monster_block_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn block(r: &RunState) -> i32 {
    r.combat.as_ref().unwrap().monsters[0].block
}
#[test]
fn sphere_activate_caps_retained_block() {
    let r = setup(998, MonsterIntent::Block { block: 25 }, None);
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn sphere_activate_at_cap_stays_capped() {
    let r = setup(999, MonsterIntent::Block { block: 25 }, None);
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn shield_gremlin_caps_ally_block() {
    let r = setup(
        998,
        MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        },
        Some((GREMLIN_TSUNDERE_ID, 7)),
    );
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn centurion_caps_ally_block() {
    let r = setup(
        998,
        MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        },
        Some((CENTURION_ID, 15)),
    );
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn deca_caps_each_living_ally() {
    let r = setup(
        998,
        MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        },
        Some((DECA_ID, 16)),
    );
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn ordinary_sphere_block_control() {
    let r = setup(40, MonsterIntent::Block { block: 25 }, None);
    assert_eq!(block(&end(&r)), 65);
}
#[test]
fn existing_harden_cap_control() {
    let r = setup(
        998,
        MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        },
        None,
    );
    assert_eq!(block(&end(&r)), 999);
}
#[test]
fn source_clipped_block_changes_following_attack_damage() {
    let mut r = setup(998, MonsterIntent::Block { block: 25 }, None);
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
    assert_eq!(after.combat.as_ref().unwrap().monsters[0].hp, 19);
}
#[test]
fn malformed_intent_overflow_is_atomic() {
    let r = setup(i32::MAX, MonsterIntent::Block { block: 25 }, None);
    let before = serde_json::to_value(&r).unwrap();
    let error = apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid state: monster intent arithmetic overflow"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_end_power_overflow_is_atomic() {
    let mut r = setup(
        998,
        MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        },
        None,
    );
    r.combat.as_mut().unwrap().monsters[0].powers.plated_armor = i32::MAX;
    let before = serde_json::to_value(&r).unwrap();
    let error = apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid state: monster end-turn arithmetic overflow"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
