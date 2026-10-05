//! Synthetic source-backed END prefixes, not dedicated boundary trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
fn setup(permanent: i32, loss: i32, ritual: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.strength = permanent;
    c.player.temp_strength = loss;
    c.player.powers.ritual = ritual;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    r.validate().unwrap();
    r
}
fn end(r: &RunState) -> RunState {
    step(r, RunDecisionAction::Combat(CombatAction::EndTurn))
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
    println!(
        "player_ritual_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
#[test]
fn ritual_caps_full_strength() {
    assert_eq!(
        end(&setup(999, 0, 1))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        999
    );
}
#[test]
fn ritual_caps_near_full_strength() {
    assert_eq!(
        end(&setup(998, 0, 2))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        999
    );
}
#[test]
fn ritual_caps_combined_strength_before_existing_loss_expires() {
    let n = end(&setup(997, 2, 1));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 997);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 0);
}
#[test]
fn large_nominal_loss_is_not_erased_by_the_ritual_cap() {
    let n = end(&setup(-1, 1000, 1));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, -1);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 0);
}
#[test]
fn ordinary_ritual_repeats_each_end_control() {
    let n = end(&setup(0, 0, 2));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 2);
    assert_eq!(end(&n).combat.unwrap().player.powers.strength, 4);
}
#[test]
fn signed_strength_control() {
    assert_eq!(
        end(&setup(-999, 0, 2))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -997
    );
}

#[test]
fn positive_ritual_does_not_consume_artifact() {
    let mut r = setup(999, 0, 1);
    r.combat.as_mut().unwrap().player.powers.artifact = 3;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 3);
}

#[test]
fn later_nominal_loss_can_consume_artifact_after_ritual() {
    let mut r = setup(997, 2, 1);
    r.combat.as_mut().unwrap().player.powers.artifact = 1;
    let n = end(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 0);
}

#[test]
fn nilry_pause_resumes_ritual_cap_once() {
    let mut r =
        RunState::combat_fixture_with_relics(vec![sts_core::adapter_internals::Relic::NilrysCodex]);
    r.combat.as_mut().unwrap().player.powers.strength = 998;
    r.combat.as_mut().unwrap().player.powers.ritual = 2;
    let paused = end(&r);
    assert_eq!(paused.combat.as_ref().unwrap().player.powers.strength, 998);
    let n = step(
        &paused,
        RunDecisionAction::Run(sts_core::adapter_internals::RunAction::SkipCombatCardReward),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 999);
}

#[test]
fn malformed_permanent_or_combined_overflow_rejects_atomically() {
    for (perm, temp) in [(i32::MAX, 0), (i32::MAX - 1, 2)] {
        let r = setup(perm, temp, 1);
        let before = serde_json::to_value(&r).unwrap();
        assert_eq!(
            apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn)),
            Err(sts_core::adapter_internals::SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(serde_json::to_value(&r).unwrap(), before);
    }
}

#[test]
fn initialized_zero_ritual_does_not_repair_unrelated_strength() {
    // Out-of-source-range initializer diagnostic, not a natural legal prefix.
    let n = end(&setup(1005, 0, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 1005);
}

#[test]
fn capped_ritual_controls_following_strike_damage() {
    let n = end(&setup(999, 0, 1));
    let c = n.combat.as_ref().unwrap();
    let card = c
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == sts_core::content::cards::STRIKE_R_ID)
        .unwrap()
        .id;
    let target = c.monsters[0].id;
    let hp = c.monsters[0].hp;
    let hit = step(
        &n,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: card,
            target: Some(target),
        }),
    );
    assert_eq!(hp - hit.combat.as_ref().unwrap().monsters[0].hp, 1005);
}
