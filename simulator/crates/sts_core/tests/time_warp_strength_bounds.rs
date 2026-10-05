//! Source-backed synthetic legal prefixes, not dedicated interaction-trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    MonsterId, MonsterIntent, RunDecisionAction, RunState,
};
use sts_core::content::cards::{BURNING_PACT_ID, DEFEND_R_ID, DISARM_ID, STRIKE_R_ID};
use sts_core::content::monsters::{SPHERIC_GUARDIAN_ID, TIME_EATER_ID};
fn setup(strength: i32, counter: i32, ally: bool) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 5000;
    c.player.max_hp = 5000;
    c.piles.hand = vec![CardInstance::new(CardId::new(100), DEFEND_R_ID)];
    c.piles.draw_pile.clear();
    c.piles.discard_pile.clear();
    c.monsters.truncate(1);
    let m = &mut c.monsters[0];
    m.content_id = TIME_EATER_ID;
    m.hp = 1000;
    m.max_hp = 1000;
    m.powers.strength = strength;
    m.powers.time_warp = counter;
    m.intent = MonsterIntent::Attack { damage: 26 };
    if ally {
        let mut a = m.clone();
        a.id = MonsterId::new(2);
        a.content_id = SPHERIC_GUARDIAN_ID;
        a.powers.time_warp = 0;
        a.intent = MonsterIntent::Block { block: 0 };
        c.monsters.push(a);
    }
    r.validate().unwrap();
    r
}
fn play(r: &RunState) -> RunState {
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: (r.combat.as_ref().unwrap().piles.hand[0].content_id == DISARM_ID)
            .then_some(MonsterId::new(1)),
    });
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
        "time_warp_strength_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
#[test]
fn twelfth_card_caps_near_full_strength() {
    let n = play(&setup(998, 11, false));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn twelfth_card_caps_full_strength() {
    let n = play(&setup(999, 11, false));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 999);
}
#[test]
fn twelfth_card_caps_each_living_monster() {
    let n = play(&setup(998, 11, true));
    assert!(n
        .combat
        .unwrap()
        .monsters
        .iter()
        .all(|m| m.powers.strength == 999));
}
#[test]
fn ordinary_trigger_control() {
    let n = play(&setup(3, 11, false));
    assert_eq!(n.combat.unwrap().monsters[0].powers.strength, 5);
}
#[test]
fn clipped_gain_retains_actual_same_frame_damage_strength() {
    let mut r = setup(998, 11, false);
    r.combat.as_mut().unwrap().time_warp_duplicate_monster_queue = true;
    let n = play(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].powers.strength, 999);
    assert_eq!(c.player.hp, 2957);
    assert!(c.time_warp_pre_gain_strength.is_empty());
}
#[test]
fn already_capped_gain_retains_actual_same_frame_damage_strength() {
    let mut r = setup(999, 11, false);
    r.combat.as_mut().unwrap().time_warp_duplicate_monster_queue = true;
    let n = play(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].powers.strength, 999);
    assert_eq!(c.player.hp, 2955);
}
#[test]
fn card_reduction_precedes_capped_time_warp_gain() {
    let mut r = setup(999, 11, false);
    r.combat.as_mut().unwrap().piles.hand[0].content_id = DISARM_ID;
    let n = play(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.monsters[0].powers.strength, 999);
    assert_eq!(c.player.hp, 3975);
}
#[test]
fn ordinary_same_frame_queue_context_control() {
    let mut r = setup(3, 11, false);
    r.combat.as_mut().unwrap().time_warp_duplicate_monster_queue = true;
    let n = play(&r);
    let c = n.combat.unwrap();
    assert_eq!(c.player.hp, 4947);
    assert_eq!(c.monsters[0].powers.strength, 5);
}
#[test]
fn positive_time_warp_gain_preserves_artifact() {
    let mut r = setup(998, 11, false);
    r.combat.as_mut().unwrap().monsters[0].powers.artifact = 1;
    let n = play(&r);
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.powers.strength, 999);
    assert_eq!(m.powers.artifact, 1);
}
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
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
    n
}
#[test]
fn deferred_selection_restores_and_caps_gain_once() {
    let mut r = setup(998, 11, false);
    let c = r.combat.as_mut().unwrap();
    c.piles.hand[0].content_id = BURNING_PACT_ID;
    c.piles
        .hand
        .push(CardInstance::new(CardId::new(101), DEFEND_R_ID));
    c.piles
        .hand
        .push(CardInstance::new(CardId::new(102), STRIKE_R_ID));
    let opened = play(&r);
    let chosen = step(
        &opened,
        RunDecisionAction::Run(
            sts_core::adapter_internals::RunAction::ChooseExhaustSelect { index: 0 },
        ),
    );
    let settled = step(
        &chosen,
        RunDecisionAction::Run(sts_core::adapter_internals::RunAction::ConfirmExhaustSelect),
    );
    let c = settled.combat.unwrap();
    assert_eq!(c.monsters[0].powers.strength, 999);
    assert_eq!(c.monsters[0].powers.time_warp, 0);
    assert!(c.time_warp_pre_gain_strength.is_empty());
}
#[test]
fn malformed_time_warp_gain_overflow_is_atomic() {
    let r = setup(i32::MAX, 11, false);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    });
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: combat integer addition overflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_pending_damage_without_context_is_rejected_not_guessed() {
    let mut r = setup(999, 0, false);
    r.combat.as_mut().unwrap().time_warp_duplicate_monster_queue = true;
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: queued Time Warp damage has no Strength snapshot"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn before_threshold_control() {
    let n = play(&setup(998, 10, false));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.powers.strength, 998);
    assert_eq!(m.powers.time_warp, 11);
}
