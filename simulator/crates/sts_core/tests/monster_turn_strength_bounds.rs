//! Source-backed synthetic legal END prefixes, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, ContentId, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::content::monsters::{
    CHAMP_ID, CORRUPT_HEART_ID, CULTIST_ID, JAW_WORM_ID, ORB_WALKER_ID, SPHERIC_GUARDIAN_ID,
    THE_COLLECTOR_ID,
};
fn setup(content: ContentId, intent: MonsterIntent, strength: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.intangible = 2;
    let m = &mut c.monsters[0];
    m.content_id = content;
    m.hp = 100;
    m.max_hp = 100;
    m.moves_executed = 2;
    m.move_history.clear();
    m.intent = intent;
    m.powers.strength = strength;
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
        "monster_strength_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn strength(r: &RunState) -> i32 {
    r.combat.as_ref().unwrap().monsters[0].powers.strength
}
#[test]
fn jaw_worm_bellow_caps_strength() {
    let r = setup(
        JAW_WORM_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        998,
    );
    assert_eq!(strength(&end(&r)), 999);
}
#[test]
fn champ_enrage_caps_after_cleansing() {
    let mut r = setup(CHAMP_ID, MonsterIntent::StrengthSelf { amount: 9 }, 998);
    r.combat.as_mut().unwrap().monsters[0].temp_strength_down = 9;
    let n = end(&r);
    assert_eq!(strength(&n), 999);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn collector_all_monster_gain_caps() {
    let r = setup(
        THE_COLLECTOR_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 15,
        },
        998,
    );
    assert_eq!(strength(&end(&r)), 999);
}
#[test]
fn heart_later_buff_caps_each_gain() {
    let mut r = setup(
        CORRUPT_HEART_ID,
        MonsterIntent::StrengthSelf { amount: 2 },
        990,
    );
    r.combat.as_mut().unwrap().monsters[0]
        .powers
        .heart_buff_count = 3;
    assert_eq!(strength(&end(&r)), 999);
}
#[test]
fn cultist_ritual_caps_end_turn_strength() {
    let mut r = setup(CULTIST_ID, MonsterIntent::Attack { damage: 6 }, 998);
    r.combat.as_mut().unwrap().monsters[0].powers.ritual = 3;
    assert_eq!(strength(&end(&r)), 999);
}
#[test]
fn orb_walker_strength_up_caps_end_turn_strength() {
    let mut r = setup(
        ORB_WALKER_ID,
        MonsterIntent::AddBurnToDiscardAndDraw {
            damage: 10,
            count: 2,
        },
        998,
    );
    r.combat.as_mut().unwrap().monsters[0].powers.strength_up = 3;
    assert_eq!(strength(&end(&r)), 999);
}
#[test]
fn shackled_restoration_caps_without_preserving_debt() {
    let mut r = setup(SPHERIC_GUARDIAN_ID, MonsterIntent::Block { block: 25 }, 998);
    r.combat.as_mut().unwrap().monsters[0].temp_strength_down = 9;
    let n = end(&r);
    assert_eq!(strength(&n), 999);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn later_artifact_does_not_block_positive_restoration() {
    let mut r = setup(SPHERIC_GUARDIAN_ID, MonsterIntent::Block { block: 25 }, 998);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.temp_strength_down = 9;
    m.powers.artifact = 1;
    let n = end(&r);
    assert_eq!(strength(&n), 999);
    let m = &n.combat.as_ref().unwrap().monsters[0];
    assert_eq!(m.temp_strength_down, 0);
    assert_eq!(m.powers.artifact, 1);
}
#[test]
fn ordinary_strength_gain_control() {
    let r = setup(
        JAW_WORM_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        2,
    );
    assert_eq!(strength(&end(&r)), 5);
}
#[test]
fn ordinary_negative_strength_gain_control() {
    let r = setup(
        JAW_WORM_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        -2,
    );
    assert_eq!(strength(&end(&r)), 1);
}
#[test]
fn sequential_end_gains_cap_before_restoration() {
    let mut r = setup(SPHERIC_GUARDIAN_ID, MonsterIntent::Block { block: 25 }, 998);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.powers.ritual = 3;
    m.powers.strength_up = 3;
    m.temp_strength_down = 9;
    let n = end(&r);
    assert_eq!(strength(&n), 999);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn heart_cleansing_preserves_existing_shackled_restoration() {
    let mut r = setup(
        CORRUPT_HEART_ID,
        MonsterIntent::StrengthSelf { amount: 2 },
        -10,
    );
    r.combat.as_mut().unwrap().monsters[0].temp_strength_down = 9;
    let n = end(&r);
    assert_eq!(strength(&n), 11);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn malformed_overflow_preserves_existing_intent_error_and_input() {
    let r = setup(
        JAW_WORM_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        i32::MAX,
    );
    let before = serde_json::to_value(&r).unwrap();
    assert_eq!(
        apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn))
            .unwrap_err()
            .to_string(),
        "invalid state: monster intent arithmetic overflow"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_group_overflow_preserves_existing_error_and_input() {
    let r = setup(
        THE_COLLECTOR_ID,
        MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 15,
        },
        i32::MAX,
    );
    let before = serde_json::to_value(&r).unwrap();
    assert_eq!(
        apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn))
            .unwrap_err()
            .to_string(),
        "invalid state: monster group arithmetic overflow"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_restoration_overflow_preserves_existing_error_and_input() {
    let mut r = setup(
        SPHERIC_GUARDIAN_ID,
        MonsterIntent::Block { block: 25 },
        i32::MAX,
    );
    r.combat.as_mut().unwrap().monsters[0].temp_strength_down = 9;
    let before = serde_json::to_value(&r).unwrap();
    assert_eq!(
        apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn))
            .unwrap_err()
            .to_string(),
        "invalid state: combat integer addition overflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
