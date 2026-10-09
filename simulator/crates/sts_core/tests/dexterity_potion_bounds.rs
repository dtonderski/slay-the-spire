//! Synthetic source-backed application regressions, not new real-game parity.
//! DexterityPower is the actual amount; temp_dexterity is DexLoss debt, not
//! a component to add to that amount (unlike simulator temporary Strength).
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, Potion, Relic, RunAction,
    RunDecisionAction, RunState,
};
fn setup(dexterity: i32, debt: i32, bark: bool) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(if bark {
        vec![Relic::SacredBark]
    } else {
        vec![]
    });
    run.potions = vec![Potion::Dexterity];
    run.empty_potion_slots = vec![1, 2];
    let c = run.combat.as_mut().unwrap();
    c.player.powers.dexterity = dexterity;
    c.player.temp_dexterity = debt;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    run.validate().unwrap();
    run
}
fn step(run: &RunState, action: RunDecisionAction) -> RunState {
    assert!(legal_run_decision_actions(run).unwrap().contains(&action));
    let before = serde_json::to_value(run).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_run_decision_action(run, action).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(run).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, action).unwrap()).unwrap()
    );
    println!(
        "dexterity_potion_transition={}",
        serde_json::json!({"initial":before,"action":action,"result":next})
    );
    next
}
fn drink(run: &RunState) -> RunState {
    let next = step(
        run,
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        }),
    );
    assert_eq!(next.potion_at_slot(0), None);
    assert_eq!(
        next.combat.as_ref().unwrap().rng,
        run.combat.as_ref().unwrap().rng
    );
    next
}
#[test]
fn potion_caps_near_full_dexterity() {
    assert_eq!(
        drink(&setup(998, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn potion_caps_full_dexterity() {
    assert_eq!(
        drink(&setup(999, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn bark_potion_caps_dexterity() {
    assert_eq!(
        drink(&setup(996, 0, true))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn ordinary_positive_control() {
    assert_eq!(
        drink(&setup(3, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        5
    );
}
#[test]
fn signed_control_does_not_treat_debt_as_extra_dexterity() {
    let next = drink(&setup(-2, 5, false));
    assert_eq!(next.combat.as_ref().unwrap().player.powers.dexterity, 0);
    assert_eq!(next.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let end = step(&next, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(end.combat.unwrap().player.powers.dexterity, -5);
}

#[test]
fn cap_keeps_nominal_existing_loss_until_expiry() {
    for bark in [false, true] {
        let next = drink(&setup(998, 5, bark));
        let c = next.combat.as_ref().unwrap();
        assert_eq!(c.player.powers.dexterity, 999);
        assert_eq!(c.player.temp_dexterity, 5);
        let end = step(&next, RunDecisionAction::Combat(CombatAction::EndTurn));
        assert_eq!(end.combat.as_ref().unwrap().player.powers.dexterity, 994);
        assert_eq!(end.combat.as_ref().unwrap().player.temp_dexterity, 0);
    }
}

#[test]
fn dex_loss_debt_is_not_clipped_or_added_to_the_power() {
    let next = drink(&setup(999, 1000, false));
    assert_eq!(next.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(next.combat.as_ref().unwrap().player.temp_dexterity, 1000);
    let end = step(&next, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(end.combat.as_ref().unwrap().player.powers.dexterity, -1);
    assert_eq!(end.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn positive_application_retains_artifact() {
    let mut initial = setup(998, 5, false);
    initial.combat.as_mut().unwrap().player.powers.artifact = 1;
    let next = drink(&initial);
    assert_eq!(next.combat.as_ref().unwrap().player.powers.dexterity, 999);
    assert_eq!(next.combat.as_ref().unwrap().player.powers.artifact, 1);
    // Artifact handling at DexLoss expiry is a separate source-confirmed
    // candidate, preserved in after-1.log/after-1-source.rs and not fixed here.
}

#[test]
fn ordinary_signed_and_bark_gains_preserve_amount_and_rng() {
    for bark in [false, true] {
        for dex in [-999, -5, 0, 5, 990] {
            let next = drink(&setup(dex, 0, bark));
            assert_eq!(
                next.combat.as_ref().unwrap().player.powers.dexterity,
                dex + if bark { 4 } else { 2 }
            );
        }
    }
}

#[test]
fn following_defend_uses_actual_dexterity_not_loss_debt() {
    let next = drink(&setup(-4, 50, false));
    let card = next
        .combat
        .as_ref()
        .unwrap()
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == sts_core::content::cards::DEFEND_R_ID)
        .unwrap()
        .id;
    let blocked = step(
        &next,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: card,
            target: None,
        }),
    );
    assert_eq!(blocked.combat.as_ref().unwrap().player.block, 3);
}

#[test]
fn malformed_overflow_preserves_error_and_original_input() {
    let run = setup(i32::MAX, 0, false);
    let before = serde_json::to_value(&run).unwrap();
    assert_eq!(
        apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: 0,
                target: None
            })
        ),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "combat potion stat gain overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&run).unwrap(), before);
}
