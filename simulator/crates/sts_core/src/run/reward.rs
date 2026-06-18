use super::state::REWARD_GOLD_AMOUNT;
use crate::{
    card::CardInstance,
    combat::{apply_combat_action, CombatPhase},
    content::cards::{
        ANGER_ID, BATTLE_TRANCE_ID, CLEAVE_ID, HAVOC_ID, POMMEL_STRIKE_ID, SEARING_BLOW_ID,
        SHRUG_IT_OFF_ID, TWIN_STRIKE_ID, WARCRY_ID,
    },
    ids::CardId,
    rng::{RngStream, SimulatorRng},
    run::shop::apply_shop_action,
    CombatAction, ContentId, RewardScreen, RunAction, RunPhase, RunState, SimError, SimResult,
};

const IRONCLAD_REWARD_POOL: [ContentId; 9] = [
    ANGER_ID,
    CLEAVE_ID,
    SHRUG_IT_OFF_ID,
    TWIN_STRIKE_ID,
    POMMEL_STRIKE_ID,
    BATTLE_TRANCE_ID,
    HAVOC_ID,
    WARCRY_ID,
    SEARING_BLOW_ID,
];

const REWARD_CARD_COUNT: usize = 3;

/// Deterministic fixed pool used in early milestones before RNG wiring.
#[must_use]
pub fn fixed_card_reward_choices(next_card_id: u64) -> Vec<CardInstance> {
    [ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID]
        .iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(CardId::new(next_card_id + index as u64), *content_id)
        })
        .collect()
}

#[must_use]
pub fn card_reward_choices(rng: &mut SimulatorRng, next_card_id: u64) -> Vec<CardInstance> {
    let mut pool: Vec<ContentId> = IRONCLAD_REWARD_POOL.to_vec();
    let mut choices = Vec::with_capacity(REWARD_CARD_COUNT);

    for index in 0..REWARD_CARD_COUNT {
        let pick = rng.next_usize(RngStream::RewardCard, "reward_card", pool.len());
        let content_id = pool.remove(pick);
        choices.push(CardInstance::new(
            CardId::new(next_card_id + index as u64),
            content_id,
        ));
    }

    choices
}

pub fn enter_reward_screen(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let mut rng = SimulatorRng::new(run.reward_rng_seed);
    let choices = card_reward_choices(&mut rng, next_card_id);
    run.reward_rng_seed = rng.seed_state();
    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices,
        gold_offer: REWARD_GOLD_AMOUNT,
        potion_offer: None,
        relic_offer: None,
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
    match action {
        RunAction::BuyShopCard { .. } => apply_shop_action(run, action),
        _ => apply_reward_action(run, action),
    }
}

fn apply_reward_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
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
        RunAction::TakeGoldReward => {
            let gold_offer = next
                .reward
                .as_ref()
                .expect("validated reward screen")
                .gold_offer;
            next.gold += gold_offer;
            next.phase = RunPhase::Idle;
            next.reward = None;
        }
        RunAction::BuyShopCard { .. } => unreachable!("validated reward action"),
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
    fn card_reward_choices_are_deterministic_for_seed() {
        let mut first = SimulatorRng::new(7);
        let mut second = SimulatorRng::new(7);

        assert_eq!(
            card_reward_choices(&mut first, 100),
            card_reward_choices(&mut second, 100)
        );
    }

    #[test]
    fn card_reward_choices_pick_three_unique_cards_from_pool() {
        let mut rng = SimulatorRng::new(42);
        let choices = card_reward_choices(&mut rng, 1);

        assert_eq!(choices.len(), 3);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();
        assert_eq!(content_ids.len(), {
            let unique: std::collections::BTreeSet<_> = content_ids.iter().copied().collect();
            unique.len()
        });
        assert!(content_ids
            .iter()
            .all(|id| IRONCLAD_REWARD_POOL.contains(id)));
    }

    #[test]
    fn combat_win_enters_reward_with_three_rng_choices() {
        let run = winning_combat_run();

        assert_eq!(run.phase, RunPhase::Reward);
        assert!(run.combat.is_none());
        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.choices.len(), 3);
        assert_eq!(reward.gold_offer, REWARD_GOLD_AMOUNT);
        assert!(reward.potion_offer.is_none());
        assert!(reward.relic_offer.is_none());
        assert!(reward
            .choices
            .iter()
            .all(|card| IRONCLAD_REWARD_POOL.contains(&card.content_id)));
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
        let chosen = run.reward.as_ref().expect("reward screen").choices[0].id;
        let chosen_content = run.reward.as_ref().expect("reward screen").choices[0].content_id;

        let next = apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen })
            .expect("take reward");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
        assert_eq!(next.deck.len(), deck_len_before + 1);
        assert!(next.deck.iter().any(|card| card.id == chosen));
        assert_eq!(next.count_content_in_deck(chosen_content), 1);
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

    #[test]
    fn take_gold_reward_adds_fixed_amount_and_leaves_deck_unchanged() {
        let run = winning_combat_run();
        let deck_before = run.deck.clone();
        let gold_before = run.gold;

        let next = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
        assert_eq!(next.deck, deck_before);
        assert_eq!(next.gold, gold_before + REWARD_GOLD_AMOUNT);
    }

    #[test]
    fn combat_fixture_starts_with_starting_gold() {
        let run = RunState::combat_fixture();

        assert_eq!(run.gold, crate::run::state::STARTING_GOLD);
    }
}
