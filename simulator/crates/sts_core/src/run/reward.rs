use crate::{
    card::CardInstance,
    combat::{apply_combat_action, CombatPhase},
    content::cards::{ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID},
    ids::CardId,
    CombatAction, RewardScreen, RunAction, RunPhase, RunState, SimError, SimResult,
};

const REWARD_CHOICE_CONTENT_IDS: [crate::ContentId; 3] = [ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID];

#[must_use]
pub fn fixed_card_reward_choices(next_card_id: u64) -> Vec<CardInstance> {
    REWARD_CHOICE_CONTENT_IDS
        .iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(CardId::new(next_card_id + index as u64), *content_id)
        })
        .collect()
}

pub fn enter_reward_screen(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: fixed_card_reward_choices(next_card_id),
    });
}

pub fn apply_combat_action_on_run(run: &RunState, action: CombatAction) -> SimResult<RunState> {
    if run.phase != RunPhase::Combat {
        return Err(SimError::IllegalAction(
            "combat actions require combat phase",
        ));
    }

    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::InvalidState("combat state is missing"))?;

    let next_combat = apply_combat_action(combat, action)?;
    let mut next = run.clone();
    next.combat = Some(next_combat.clone());
    next.player_hp = next_combat.player.hp;
    next.player_max_hp = next_combat.player.max_hp;

    if next_combat.phase == CombatPhase::Won {
        enter_reward_screen(&mut next);
    }

    Ok(next)
}

pub fn apply_run_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate_reward_action(action)?;

    let mut next = run.clone();
    match action {
        RunAction::SkipReward => {
            next.phase = RunPhase::Idle;
            next.reward = None;
        }
        RunAction::TakeCardReward { card_id } => {
            let choice = next
                .reward
                .as_ref()
                .expect("validated reward screen")
                .choices
                .iter()
                .find(|choice| choice.id == card_id)
                .copied()
                .expect("validated reward card");
            next.deck.push(choice);
            next.phase = RunPhase::Idle;
            next.reward = None;
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{BASH_ID, STRIKE_R_ID};

    fn winning_combat_run() -> RunState {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.monsters[0].hp = 14;

        let bash_id = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == BASH_ID)
            .expect("bash in hand")
            .id;
        let strike_id = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == STRIKE_R_ID)
            .expect("strike in hand")
            .id;
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

    #[test]
    fn combat_win_enters_reward_with_three_fixed_choices() {
        let run = winning_combat_run();

        assert_eq!(run.phase, RunPhase::Reward);
        assert!(run.combat.is_none());
        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.choices.len(), 3);
        assert_eq!(
            reward
                .choices
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID]
        );
    }

    #[test]
    fn skip_reward_leaves_deck_unchanged() {
        let run = winning_combat_run();
        let deck_before = run.deck.clone();

        let next = apply_run_action(&run, RunAction::SkipReward).expect("skip reward");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
        assert_eq!(next.deck, deck_before);
    }

    #[test]
    fn take_card_reward_adds_choice_to_master_deck() {
        let run = winning_combat_run();
        let deck_len_before = run.deck.len();
        let chosen = run.reward.as_ref().expect("reward screen").choices[1].id;

        let next = apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen })
            .expect("take reward");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
        assert_eq!(next.deck.len(), deck_len_before + 1);
        assert!(next.deck.iter().any(|card| card.id == chosen));
        assert_eq!(next.count_content_in_deck(CLEAVE_ID), 1);
    }

    #[test]
    fn take_card_reward_rejects_unknown_card_id() {
        let run = winning_combat_run();

        let err = apply_run_action(
            &run,
            RunAction::TakeCardReward {
                card_id: CardId::new(999),
            },
        )
        .expect_err("unknown reward card");

        assert_eq!(err, SimError::UnknownCard(CardId::new(999)));
    }
}
