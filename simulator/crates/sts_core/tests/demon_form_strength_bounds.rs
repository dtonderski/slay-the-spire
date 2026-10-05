//! Source-backed synthetic END prefixes, not dedicated interaction trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
fn setup(permanent: i32, loss: i32, demon: i32, artifact: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.strength = permanent;
    c.player.temp_strength = loss;
    c.player.powers.demon_form = demon;
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
        "demon_form_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn end(r: &RunState) -> RunState {
    step(r, RunDecisionAction::Combat(CombatAction::EndTurn))
}
#[test]
fn full_power_caps_demon_form_gain() {
    assert_eq!(
        end(&setup(999, 0, 2, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        999
    );
}
#[test]
fn near_full_power_caps_demon_form_gain() {
    assert_eq!(
        end(&setup(998, 0, 2, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        999
    );
}
#[test]
fn stacked_demon_form_caps_its_full_application() {
    assert_eq!(
        end(&setup(997, 0, 4, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        999
    );
}
#[test]
fn artifact_blocked_prior_loss_does_not_bypass_later_gain_cap() {
    let n = end(&setup(997, 2, 2, 1));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 999);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn ordinary_demon_form_repeats_after_draw_control() {
    let n = end(&setup(0, 0, 2, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 2);
    assert_eq!(end(&n).combat.unwrap().player.powers.strength, 4);
}
#[test]
fn positive_demon_gain_retains_artifact() {
    let n = end(&setup(999, 0, 2, 3));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 3);
}

#[test]
fn previous_nominal_loss_expires_before_post_draw_gain() {
    let n = end(&setup(997, 2, 2, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.strength, 999);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 0);
}

#[test]
fn malformed_gain_overflow_rejects_the_whole_end_atomically() {
    let r = setup(i32::MAX, 0, 2, 1);
    let before = serde_json::to_value(&r).unwrap();
    assert_eq!(
        apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn)),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}

#[test]
fn initialized_zero_demon_form_does_not_repair_other_strength() {
    assert_eq!(
        end(&setup(1005, 0, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        1005
    );
}

#[test]
fn capped_demon_form_controls_following_strike_damage() {
    let n = end(&setup(999, 0, 2, 0));
    let c = n.combat.as_ref().unwrap();
    let card = c
        .piles
        .hand
        .iter()
        .find(|c| c.content_id == sts_core::content::cards::STRIKE_R_ID)
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

#[test]
fn application_preserves_draw_order_costs_and_rng_ownership() {
    let mut gain = setup(999, 0, 2, 0);
    gain.combat.as_mut().unwrap().player.powers.confusion = 1;
    let mut control = gain.clone();
    control.combat.as_mut().unwrap().player.powers.demon_form = 0;
    let n = end(&gain);
    let m = end(&control);
    assert_eq!(
        n.combat.as_ref().unwrap().piles.hand,
        m.combat.as_ref().unwrap().piles.hand
    );
    assert_eq!(
        serde_json::to_value(&n.combat.as_ref().unwrap().rng).unwrap(),
        serde_json::to_value(&m.combat.as_ref().unwrap().rng).unwrap()
    );
    assert_eq!(n.card_random_rng_counter, m.card_random_rng_counter);
    assert_eq!(n.card_rng_counter, m.card_rng_counter);
}

#[test]
fn large_action_amount_stacks_raw_on_existing_negative_power() {
    // ApplyPowerAction stacks its raw action amount, not the capped amount
    // of the newly constructed StrengthPower, when that ID already exists.
    assert_eq!(
        end(&setup(-999, 0, 1000, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        1
    );
}

#[test]
fn signed_power_control() {
    assert_eq!(
        end(&setup(-999, 0, 3, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -996
    );
}
