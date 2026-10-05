//! Source-backed synthetic card prefixes, not dedicated cross-color trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
use sts_core::content::cards::FASTING_ANY_COLOR_ID;
fn setup(dex: i32, debt: i32, upgrade: bool) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.dexterity = dex;
    c.player.temp_dexterity = debt;
    c.piles.hand[0].content_id = FASTING_ANY_COLOR_ID;
    c.piles.hand[0].upgrades = u8::from(upgrade);
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
        "card_dex_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn play(r: &RunState) -> RunState {
    let id = r.combat.as_ref().unwrap().piles.hand[0].id;
    step(
        r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: id,
            target: None,
        }),
    )
}
#[test]
fn full_power_caps_gain() {
    assert_eq!(
        play(&setup(999, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn near_full_power_caps_gain() {
    assert_eq!(
        play(&setup(998, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn upgraded_power_caps_gain() {
    assert_eq!(
        play(&setup(997, 0, true))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn existing_loss_is_not_an_extra_actual_component() {
    let n = play(&setup(998, 5, false));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 999);
    assert_eq!(p.temp_dexterity, 5);
}
#[test]
fn ordinary_gain_control() {
    assert_eq!(
        play(&setup(2, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        5
    );
}
#[test]
fn accepted_loss_debt_expires_after_clipped_gain() {
    let n = play(&setup(998, 5, false));
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 994);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn large_nominal_loss_is_not_erased_by_gain_cap() {
    let n = play(&setup(998, 1002, false));
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 1002);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -3);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn following_frail_defend_uses_capped_actual_dexterity() {
    let mut r = setup(998, 5, false);
    r.combat.as_mut().unwrap().player.powers.frail = 1;
    let n = play(&r);
    let c = n.combat.as_ref().unwrap();
    let id = c
        .piles
        .hand
        .iter()
        .find(|c| c.content_id == sts_core::content::cards::DEFEND_R_ID)
        .unwrap()
        .id;
    let n = step(
        &n,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: id,
            target: None,
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 753);
}

#[test]
fn gain_and_consumption_do_not_draw_rng() {
    let r = setup(998, 5, false);
    let n = play(&r);
    assert_eq!(
        serde_json::to_value(&n.combat.as_ref().unwrap().rng).unwrap(),
        serde_json::to_value(&r.combat.as_ref().unwrap().rng).unwrap()
    );
    assert_eq!(n.card_rng_counter, r.card_rng_counter);
    assert_eq!(n.card_random_rng_counter, r.card_random_rng_counter);
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 1);
    assert!(!n
        .combat
        .as_ref()
        .unwrap()
        .piles
        .hand
        .iter()
        .any(|c| c.content_id == FASTING_ANY_COLOR_ID));
}

#[test]
fn malformed_overflow_rejects_all_earlier_card_effects_atomically() {
    let r = setup(i32::MAX, 0, false);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: r.combat.as_ref().unwrap().piles.hand[0].id,
        target: None,
    });
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    assert_eq!(
        apply_run_decision_action(&r, a),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}

#[test]
fn signed_gain_control() {
    assert_eq!(
        play(&setup(-999, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        -996
    );
}
