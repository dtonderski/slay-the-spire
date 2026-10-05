//! Synthetic legal prefixes; source-backed, not dedicated real-game parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, Potion, RunAction,
    RunDecisionAction, RunState,
};
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let restore: RunState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restore, a).unwrap()).unwrap()
    );
    println!(
        "dexterity_expiry_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn setup() -> RunState {
    let mut r = RunState::combat_fixture();
    r.potions = vec![Potion::Speed, Potion::Ancient];
    r.empty_potion_slots = vec![2];
    r.validate().unwrap();
    r
}
#[test]
fn speed_then_ancient_blocks_expiry_once() {
    let r = step(
        &setup(),
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        }),
    );
    assert_eq!(r.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(r.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let r = step(
        &r,
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 1,
            target: None,
        }),
    );
    assert_eq!(r.combat.as_ref().unwrap().player.powers.artifact, 1);
    let r = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(r.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(r.combat.as_ref().unwrap().player.temp_dexterity, 0);
    let again = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(again.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(again.combat.as_ref().unwrap().player.powers.artifact, 0);
}
#[test]
fn speed_without_ancient_control() {
    let r = step(
        &setup(),
        RunDecisionAction::Run(RunAction::UsePotion {
            slot: 0,
            target: None,
        }),
    );
    let r = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(r.combat.as_ref().unwrap().player.powers.dexterity, 0);
    assert_eq!(r.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

fn power_setup(dex: i32, debt: i32, artifact: i32, nilry: bool) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(if nilry {
        vec![sts_core::adapter_internals::Relic::NilrysCodex]
    } else {
        vec![]
    });
    let c = r.combat.as_mut().unwrap();
    c.player.powers.dexterity = dex;
    c.player.temp_dexterity = debt;
    c.player.powers.artifact = artifact;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    r.validate().unwrap();
    r
}

#[test]
fn expiry_consumes_artifact_before_monster_debuff() {
    let mut r = power_setup(5, 5, 1, false);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.content_id = sts_core::content::monsters::CHOSEN_ID;
    m.intent = sts_core::adapter_internals::MonsterIntent::ApplyPlayerWeak { amount: 3 };
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.powers.artifact, 0);
    assert!(
        p.powers.weak > 0,
        "Artifact must not remain to absorb the later monster debuff"
    );
}

#[test]
fn two_artifacts_block_expiry_then_monster_debuff() {
    let mut r = power_setup(5, 5, 2, false);
    let m = &mut r.combat.as_mut().unwrap().monsters[0];
    m.content_id = sts_core::content::monsters::CHOSEN_ID;
    m.intent = sts_core::adapter_internals::MonsterIntent::ApplyPlayerWeak { amount: 3 };
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.weak, 0);
}

#[test]
fn nominal_expiry_caps_the_negative_dexterity_power() {
    let r = power_setup(-998, 5, 0, false);
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -999);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn nilry_pauses_before_expiry_and_resume_applies_it_once() {
    let r = power_setup(5, 5, 2, true);
    let paused = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(paused.combat.as_ref().unwrap().player.powers.artifact, 2);
    assert_eq!(paused.combat.as_ref().unwrap().player.temp_dexterity, 5);
    let n = step(
        &paused,
        RunDecisionAction::Run(RunAction::SkipCombatCardReward),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 1);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
    let paused = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    let n = step(
        &paused,
        RunDecisionAction::Run(RunAction::SkipCombatCardReward),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 1);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
}

#[test]
fn no_debt_does_not_consume_artifact_or_repair_dexterity() {
    let r = power_setup(-1005, 0, 1, false);
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 1);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, -1005);
}

#[test]
fn malformed_subtraction_rejects_atomically() {
    let r = power_setup(i32::MIN, 5, 0, false);
    let before = serde_json::to_value(&r).unwrap();
    assert_eq!(
        apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn)),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "combat integer subtraction overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}

#[test]
fn forced_time_warp_end_resolves_loss_before_next_turn() {
    let mut r = power_setup(5, 5, 1, false);
    let c = r.combat.as_mut().unwrap();
    let m = &mut c.monsters[0];
    m.content_id = sts_core::content::monsters::TIME_EATER_ID;
    m.powers.time_warp = 11;
    m.intent = sts_core::adapter_internals::MonsterIntent::Block { block: 0 };
    let card = c
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == sts_core::content::cards::DEFEND_R_ID)
        .unwrap()
        .id;
    let n = step(
        &r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: card,
            target: None,
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn stacked_speed_loss_consumes_only_one_artifact() {
    let mut r = setup();
    r.potions = vec![Potion::Speed, Potion::Speed, Potion::Ancient];
    r.empty_potion_slots.clear();
    for slot in [0, 1, 2] {
        r = step(
            &r,
            RunDecisionAction::Run(RunAction::UsePotion { slot, target: None }),
        );
    }
    assert_eq!(r.combat.as_ref().unwrap().player.powers.dexterity, 10);
    assert_eq!(r.combat.as_ref().unwrap().player.temp_dexterity, 10);
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 10);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn initialized_pre_discard_queue_context_resolves_loss() {
    // Diagnostic initialized publication boundary, not a natural legal prefix.
    let mut r = power_setup(5, 5, 1, false);
    r.combat
        .as_mut()
        .unwrap()
        .time_warp_end_turn_pre_discard_settled = true;
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}

#[test]
fn artifact_rejection_precedes_malformed_subtraction() {
    let r = power_setup(i32::MIN, 5, 1, false);
    let n = step(&r, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.artifact, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, i32::MIN);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}
