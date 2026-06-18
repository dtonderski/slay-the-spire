use sts_core::{
    apply_combat_action_on_run, apply_run_action,
    content::cards::{ANGER_ID, BASH_ID, STRIKE_R_ID},
    CombatAction, RunAction, RunPhase, RunState, SimError,
};

#[test]
fn combat_win_transitions_to_reward_phase() {
    let run = win_fixture_combat();

    assert_eq!(run.phase, RunPhase::Reward);
    assert_eq!(run.reward.as_ref().expect("reward screen").choices.len(), 3);
}

#[test]
fn skip_reward_preserves_master_deck() {
    let run = win_fixture_combat();
    let deck_before = run.deck.clone();

    let after = apply_run_action(&run, RunAction::SkipReward).expect("skip reward");

    assert_eq!(after.phase, RunPhase::Idle);
    assert_eq!(after.deck, deck_before);
}

#[test]
fn take_card_reward_appends_selected_card_to_master_deck() {
    let run = win_fixture_combat();
    let deck_len = run.deck.len();
    let chosen = run.reward.as_ref().expect("reward screen").choices[0].id;

    let after =
        apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen }).expect("take reward");

    assert_eq!(after.deck.len(), deck_len + 1);
    assert!(after.deck.iter().any(|card| card.id == chosen));
    assert_eq!(after.count_content_in_deck(ANGER_ID), 1);
}

#[test]
fn reward_actions_are_illegal_outside_reward_phase() {
    let run = RunState::combat_fixture();

    let err = apply_run_action(&run, RunAction::SkipReward).expect_err("not in reward");

    assert_eq!(
        err,
        SimError::IllegalAction("reward actions require reward phase")
    );
}

fn win_fixture_combat() -> RunState {
    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat fixture");
    combat.monsters[0].hp = 14;

    let bash_id = hand_card_id(combat, BASH_ID);
    let strike_id = hand_card_id(combat, STRIKE_R_ID);
    let monster_id = combat.monsters[0].id;

    run = apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: bash_id,
            target: Some(monster_id),
        },
    )
    .expect("bash applies");
    apply_combat_action_on_run(
        &run,
        CombatAction::PlayCard {
            card_id: strike_id,
            target: Some(monster_id),
        },
    )
    .expect("strike wins combat")
}

fn hand_card_id(
    run_combat: &sts_core::CombatState,
    content_id: sts_core::ContentId,
) -> sts_core::CardId {
    run_combat
        .piles
        .hand
        .iter()
        .find(|card| card.content_id == content_id)
        .expect("card in hand")
        .id
}
