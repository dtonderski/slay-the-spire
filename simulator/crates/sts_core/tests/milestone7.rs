use sts_core::{
    apply_combat_action_on_run, apply_run_action, card_reward_choices,
    content::cards::{BASH_ID, STRIKE_R_ID},
    rng::SimulatorRng,
    CombatAction, Potion, Relic, RunAction, RunPhase, RunState, SimError, STARTING_GOLD,
};

#[test]
fn combat_fixture_starts_with_starting_gold() {
    let run = RunState::combat_fixture();

    assert_eq!(run.gold, STARTING_GOLD);
}

#[test]
fn combat_win_transitions_to_reward_phase() {
    let run = win_fixture_combat();

    assert_eq!(run.phase, RunPhase::Reward);
    let reward = run.reward.as_ref().expect("reward screen");
    assert_eq!(reward.choices.len(), 3);
    assert_eq!(reward.gold_offer, 11);
    assert_eq!(reward.potion_offer, Some(Potion::Fire));
    assert_eq!(reward.relic_offer, Some(Relic::OddlySmoothStone));
}

#[test]
fn skip_reward_preserves_master_deck_and_gold() {
    let run = win_fixture_combat();
    let deck_before = run.deck.clone();
    let gold_before = run.gold;

    let after = apply_run_action(&run, RunAction::SkipReward).expect("skip reward");

    assert_eq!(after.phase, RunPhase::Idle);
    assert_eq!(after.deck, deck_before);
    assert_eq!(after.gold, gold_before);
}

#[test]
fn reward_card_choices_are_deterministic_for_seed() {
    let mut first = SimulatorRng::new(99);
    let mut second = SimulatorRng::new(99);

    assert_eq!(
        card_reward_choices(&mut first, 50),
        card_reward_choices(&mut second, 50)
    );
}

#[test]
fn take_card_reward_appends_selected_card_to_master_deck() {
    let run = win_fixture_combat();
    let deck_len = run.deck.len();
    let chosen = run.reward.as_ref().expect("reward screen").choices[0].id;
    let chosen_content = run.reward.as_ref().expect("reward screen").choices[0].content_id;

    let after =
        apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen }).expect("take reward");

    assert_eq!(after.phase, RunPhase::Reward);
    assert!(after.reward.as_ref().expect("reward").choices.is_empty());
    assert_eq!(after.deck.len(), deck_len + 1);
    assert!(after.deck.iter().any(|card| card.id == chosen));
    assert_eq!(after.count_content_in_deck(chosen_content), 1);
}

#[test]
fn take_gold_reward_adds_fixed_amount_without_changing_deck() {
    let run = win_fixture_combat();
    let deck_before = run.deck.clone();
    let gold_before = run.gold;
    let gold_offer = run.reward.as_ref().expect("reward").gold_offer;

    let after = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");

    assert_eq!(after.phase, RunPhase::Reward);
    assert_eq!(after.reward.as_ref().expect("reward").gold_offer, 0);
    assert_eq!(after.deck, deck_before);
    assert_eq!(after.gold, gold_before + gold_offer);
}

#[test]
fn take_potion_reward_adds_to_belt_and_consumes_potion_offer() {
    let run = win_fixture_combat();

    let after = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");

    assert_eq!(after.phase, RunPhase::Reward);
    assert_eq!(after.potions, vec![Potion::Fire]);
    assert_eq!(after.reward.as_ref().expect("reward").potion_offer, None);
}

#[test]
fn take_relic_reward_adds_oddly_smooth_stone_and_consumes_relic_offer() {
    let run = win_fixture_combat();

    let after = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");

    assert_eq!(after.phase, RunPhase::Reward);
    assert_eq!(after.relics, vec![Relic::OddlySmoothStone]);
    assert_eq!(after.reward.as_ref().expect("reward").relic_offer, None);
}

#[test]
fn multiple_reward_offers_can_be_collected_before_skip() {
    let run = win_fixture_combat();
    let gold_offer = run.reward.as_ref().expect("reward").gold_offer;

    let run = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");
    let run = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");
    let run = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");
    let run = apply_run_action(&run, RunAction::SkipReward).expect("skip reward");

    assert_eq!(run.phase, RunPhase::Idle);
    assert!(run.reward.is_none());
    assert_eq!(run.gold, STARTING_GOLD + gold_offer);
    assert_eq!(run.potions, vec![Potion::Fire]);
    assert_eq!(run.relics, vec![Relic::OddlySmoothStone]);
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
