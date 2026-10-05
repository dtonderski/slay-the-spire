//! Source-backed synthetic Lagavulin END prefixes, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::combat::CombatState;
fn setup(dex: i32, artifact: i32, ascension: u8) -> RunState {
    let mut r = RunState::combat_fixture_with_ascension(ascension);
    let c = r.combat.as_mut().unwrap();
    c.monsters = CombatState::lagavulin_fixture().monsters;
    c.player.powers.dexterity = dex;
    c.player.powers.artifact = artifact;
    r.validate().unwrap();
    r
}
fn end(r: &RunState) -> RunState {
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let initial = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(initial.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), initial);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "dex_reduction_transition={}",
        serde_json::json!({"initial":initial,"action":a,"result":n})
    );
    n
}
fn first_siphon(mut r: RunState) -> RunState {
    for _ in 0..12 {
        let siphon = matches!(
            r.combat.as_ref().unwrap().monsters[0].intent,
            MonsterIntent::SiphonPlayer { .. }
        );
        r = end(&r);
        if siphon {
            return r;
        }
    }
    panic!("fixture did not reach Siphon within12 ENDs");
}
#[test]
fn negative_cap_still_caps_incoming_loss() {
    assert_eq!(
        first_siphon(setup(-999, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        -999
    );
}
#[test]
fn repeated_siphon_caps_after_first_loss_reaches_limit() {
    let n = first_siphon(first_siphon(setup(-998, 0, 0)));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -999);
}
#[test]
fn two_artifacts_block_both_losses_control() {
    let n = first_siphon(setup(-999, 2, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, -999);
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn positive_dexterity_control() {
    assert_eq!(
        first_siphon(setup(10, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        9
    );
}
#[test]
fn nominal_temporary_debt_expires_separately_before_siphon() {
    let mut r = setup(10, 0, 0);
    r.combat.as_mut().unwrap().player.temp_dexterity = 5;
    let n = first_siphon(r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 4);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}
#[test]
fn cap_preserves_cycle_resources_draw_and_rng() {
    let n = first_siphon(setup(-999, 0, 0));
    let m = first_siphon(setup(-996, 0, 0));
    let n = n.combat.as_ref().unwrap();
    let m = m.combat.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&n.rng).unwrap(),
        serde_json::to_value(&m.rng).unwrap()
    );
    assert_eq!(n.piles, m.piles);
    assert_eq!(n.player.hp, m.player.hp);
    assert_eq!(n.player.energy, m.player.energy);
    assert_eq!(n.monsters, m.monsters);
}
#[test]
fn malformed_subtraction_preserves_error_and_whole_input() {
    let mut r = setup(i32::MIN, 0, 0);
    for _ in 0..12 {
        if matches!(
            r.combat.as_ref().unwrap().monsters[0].intent,
            MonsterIntent::SiphonPlayer { .. }
        ) {
            let before = serde_json::to_value(&r).unwrap();
            assert_eq!(
                apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn)),
                Err(sts_core::adapter_internals::SimError::InvalidState(
                    "player Dexterity reduction underflows i32"
                ))
            );
            assert_eq!(serde_json::to_value(&r).unwrap(), before);
            return;
        }
        r = end(&r);
    }
    panic!("fixture did not reach Siphon within12 ENDs");
}
#[test]
fn artifact_rejection_does_not_clamp_initialized_out_of_range_dexterity() {
    let n = first_siphon(setup(-1005, 2, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -1005);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
}
#[test]
fn zero_or_negative_reduction_does_not_repair_unrelated_amount() {
    let r = setup(-1005, 1, 0);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    let before = p;
    assert!(!sts_core::power::reduce_player_dexterity(&mut p, 0).unwrap());
    assert!(!sts_core::power::reduce_player_dexterity(&mut p, -2).unwrap());
    assert_eq!(p, before);
}
#[test]
fn ordinary_negative_dexterity_control() {
    assert_eq!(
        first_siphon(setup(-996, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        -997
    );
}
