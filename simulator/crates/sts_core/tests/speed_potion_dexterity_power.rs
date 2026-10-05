//! Source-backed synthetic application prefixes, not dedicated trace parity.
//! SpeedPotion applies positive actual Dexterity, then separate nominal DexLoss.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, Potion, Relic, RunAction,
    RunDecisionAction, RunState,
};
fn setup(dex: i32, debt: i32, artifact: i32, bark: bool) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(if bark {
        vec![Relic::SacredBark]
    } else {
        vec![]
    });
    r.potions = vec![Potion::Speed];
    r.empty_potion_slots = vec![1, 2];
    let c = r.combat.as_mut().unwrap();
    c.player.powers.dexterity = dex;
    c.player.temp_dexterity = debt;
    c.player.powers.artifact = artifact;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    r.validate().unwrap();
    r
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
        "speed_potion_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn drink(r: &RunState) -> RunState {
    let n = step(
        r,
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        }),
    );
    assert_eq!(n.potion_at_slot(0), None);
    assert_eq!(
        n.combat.as_ref().unwrap().rng,
        r.combat.as_ref().unwrap().rng
    );
    n
}
#[test]
fn near_cap_gain_retains_nominal_loss() {
    let n = drink(&setup(998, 0, 0, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
}
#[test]
fn full_cap_gain_retains_nominal_loss() {
    let n = drink(&setup(999, 0, 0, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
}
#[test]
fn bark_gain_retains_doubled_nominal_loss() {
    let n = drink(&setup(996, 0, 0, true));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 10);
}
#[test]
fn immediate_artifact_blocks_only_new_loss() {
    let n = drink(&setup(0, 0, 1, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
}
#[test]
fn artifact_does_not_remove_existing_loss() {
    let n = drink(&setup(5, 5, 1, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 10);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
}
#[test]
fn ordinary_gain_and_expiry_control() {
    let n = drink(&setup(0, 0, 0, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 0);
}
#[test]
fn signed_gain_and_expiry_control() {
    let n = drink(&setup(-999, 0, 0, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -994);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -999);
}

#[test]
fn capped_gain_expiry_uses_full_nominal_debt() {
    for bark in [false, true] {
        for old_debt in [0, 5, 1000] {
            let n = drink(&setup(998, old_debt, 0, bark));
            let debt = old_debt + if bark { 10 } else { 5 };
            assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
            assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, debt);
            let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
            assert_eq!(
                n.combat.as_ref().unwrap().player.powers.dexterity,
                (999 - debt).clamp(-999, 999)
            );
            assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
        }
    }
}

#[test]
fn artifact_keeps_large_existing_debt_for_later_expiry() {
    let n = drink(&setup(5, 1000, 1, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 10);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 1000);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -990);
}

#[test]
fn following_defend_uses_actual_power_not_loss_debt() {
    let n = drink(&setup(0, 0, 1, false));
    let card = n
        .combat
        .as_ref()
        .unwrap()
        .piles
        .hand
        .iter()
        .find(|c| c.content_id == sts_core::content::cards::DEFEND_R_ID)
        .unwrap()
        .id;
    let n = step(
        &n,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: card,
            target: None,
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 10);
}

#[test]
fn each_speed_use_consumes_at_most_one_artifact() {
    let n = drink(&setup(998, 0, 2, true));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 1);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 1);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 999);
}

#[test]
fn repeated_speed_after_blocked_loss_creates_only_second_debt() {
    let mut r = setup(0, 0, 1, false);
    r.potions = vec![Potion::Speed, Potion::Speed];
    r.empty_potion_slots = vec![2];
    let n = drink(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
    let n = step(
        &n,
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 1,
            target: None,
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 10);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn malformed_gain_or_unblocked_debt_overflow_is_atomic() {
    for (dex, debt, artifact) in [(i32::MAX, 0, 0), (i32::MAX, 0, 1), (0, i32::MAX, 0)] {
        let r = setup(dex, debt, artifact, false);
        let before = serde_json::to_value(&r).unwrap();
        assert_eq!(
            apply_run_decision_action(
                &r,
                RunDecisionAction::Run(RunAction::UsePotion {
                    slot: 0,
                    target: None
                })
            ),
            Err(sts_core::adapter_internals::SimError::InvalidState(
                "combat potion stat gain overflows i32"
            ))
        );
        assert_eq!(serde_json::to_value(&r).unwrap(), before);
    }
}

#[test]
fn artifact_rejection_precedes_debt_overflow_arithmetic() {
    // Labeled malformed-counter diagnostic: blocked new debt must not be added.
    let n = drink(&setup(0, i32::MAX, 1, false));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, i32::MAX);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
}
