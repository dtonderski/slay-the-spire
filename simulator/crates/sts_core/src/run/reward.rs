use super::state::REWARD_GOLD_AMOUNT;
use crate::{
    card::{CardInstance, CardRarity},
    combat::{apply_combat_action, CombatPhase},
    content::cards::{ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID},
    content::reward_pool::{RewardCardEntry, IRONCLAD_REWARD_ENTRIES},
    ids::CardId,
    potion::{Potion, MAX_POTIONS},
    relic::Relic,
    rng::{RngStream, SimulatorRng},
    run::potion::apply_potion_action,
    run::shop::apply_shop_action,
    CombatAction, RewardScreen, RunAction, RunPhase, RunState, SimError, SimResult,
};

const REWARD_CARD_COUNT: usize = 3;

/// Placeholder act-1 combat reward rarity weights (not claimed game-accurate).
const COMMON_RARITY_WEIGHT: usize = 100;
const UNCOMMON_RARITY_WEIGHT: usize = 37;
const RARE_RARITY_WEIGHT: usize = 3;

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

fn roll_reward_rarity(rng: &mut SimulatorRng) -> CardRarity {
    let total = COMMON_RARITY_WEIGHT + UNCOMMON_RARITY_WEIGHT + RARE_RARITY_WEIGHT;
    let roll = rng.next_usize(RngStream::RewardRarity, "reward_rarity", total);
    if roll < COMMON_RARITY_WEIGHT {
        CardRarity::Common
    } else if roll < COMMON_RARITY_WEIGHT + UNCOMMON_RARITY_WEIGHT {
        CardRarity::Uncommon
    } else {
        CardRarity::Rare
    }
}

fn resolve_rarity(requested: CardRarity, pool: &[RewardCardEntry]) -> CardRarity {
    for rarity in rarity_search_order(requested) {
        if pool.iter().any(|entry| entry.rarity == rarity) {
            return rarity;
        }
    }

    pool.first()
        .map(|entry| entry.rarity)
        .unwrap_or(CardRarity::Common)
}

fn rarity_search_order(requested: CardRarity) -> [CardRarity; 3] {
    match requested {
        CardRarity::Rare => [CardRarity::Rare, CardRarity::Uncommon, CardRarity::Common],
        CardRarity::Uncommon => [CardRarity::Uncommon, CardRarity::Common, CardRarity::Rare],
        CardRarity::Common => [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare],
    }
}

#[must_use]
pub fn card_reward_choices(rng: &mut SimulatorRng, next_card_id: u64) -> Vec<CardInstance> {
    let mut pool: Vec<RewardCardEntry> = IRONCLAD_REWARD_ENTRIES.to_vec();
    let mut choices = Vec::with_capacity(REWARD_CARD_COUNT);

    for index in 0..REWARD_CARD_COUNT {
        let requested = roll_reward_rarity(rng);
        let rarity = resolve_rarity(requested, &pool);
        let candidate_indices: Vec<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.rarity == rarity)
            .map(|(index, _)| index)
            .collect();
        let pick = rng.next_usize(
            RngStream::RewardCard,
            "reward_card",
            candidate_indices.len(),
        );
        let entry = pool.remove(candidate_indices[pick]);
        choices.push(CardInstance::new(
            CardId::new(next_card_id + index as u64),
            entry.content_id,
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
        potion_offer: if run.potions.len() < MAX_POTIONS {
            Some(Potion::Fire)
        } else {
            None
        },
        relic_offer: if run.relics.contains(&Relic::OddlySmoothStone) {
            None
        } else {
            Some(Relic::OddlySmoothStone)
        },
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
        RunAction::BuyShopCard { .. } | RunAction::BuyShopRelic | RunAction::BuyShopPotion => {
            apply_shop_action(run, action)
        }
        RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
            apply_potion_action(run, action)
        }
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
            let reward = next.reward.as_mut().expect("validated reward screen");
            let choice = reward
                .choices
                .iter()
                .find(|choice| choice.id == card_id)
                .copied()
                .expect("validated reward card");
            reward.choices.clear();
            next.deck.push(choice);
        }
        RunAction::TakeGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let gold_offer = reward.gold_offer;
            reward.gold_offer = 0;
            next.gold += gold_offer;
        }
        RunAction::TakePotionReward => {
            let potion = next
                .reward
                .as_mut()
                .expect("validated reward screen")
                .potion_offer
                .take()
                .expect("validated potion offer");
            next.potions.push(potion);
        }
        RunAction::TakeRelicReward => {
            let relic = next
                .reward
                .as_mut()
                .expect("validated reward screen")
                .relic_offer
                .take()
                .expect("validated relic offer");
            next.gain_relic(relic);
        }
        RunAction::BuyShopCard { .. } | RunAction::BuyShopRelic | RunAction::BuyShopPotion => {
            unreachable!("validated reward action")
        }
        RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
            unreachable!("validated reward action")
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        BASH_ID, HAVOC_ID, SEARING_BLOW_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID, TWIN_STRIKE_ID,
    };

    fn reward_pool_content_ids() -> Vec<crate::ContentId> {
        IRONCLAD_REWARD_ENTRIES
            .iter()
            .map(|entry| entry.content_id)
            .collect()
    }

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
        let pool = reward_pool_content_ids();

        assert_eq!(choices.len(), 3);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();
        assert_eq!(content_ids.len(), {
            let unique: std::collections::BTreeSet<_> = content_ids.iter().copied().collect();
            unique.len()
        });
        assert!(content_ids.iter().all(|id| pool.contains(id)));
    }

    #[test]
    fn card_reward_choices_use_separate_rarity_and_card_rng_streams() {
        let mut rng = SimulatorRng::new(11);
        let _ = card_reward_choices(&mut rng, 1);

        let streams: Vec<_> = rng.log().iter().map(|draw| draw.stream).collect();
        assert!(streams.contains(&RngStream::RewardRarity));
        assert!(streams.contains(&RngStream::RewardCard));
    }

    #[test]
    fn some_seed_rolls_havoc_from_rare_rarity_weight() {
        let havoc_found = (0_u64..10_000).any(|seed| {
            let mut rng = SimulatorRng::new(seed);
            card_reward_choices(&mut rng, 1)
                .iter()
                .any(|card| card.content_id == HAVOC_ID)
        });

        assert!(havoc_found);
    }

    #[test]
    fn seed_7_reward_cards_match_golden_snapshot() {
        let mut rng = SimulatorRng::new(7);
        let choices = card_reward_choices(&mut rng, 100);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();

        assert_eq!(
            content_ids,
            vec![TWIN_STRIKE_ID, SEARING_BLOW_ID, SHRUG_IT_OFF_ID],
            "update snapshot if reward algorithm changes intentionally"
        );
    }

    #[test]
    fn combat_win_enters_reward_with_three_rng_choices() {
        let run = winning_combat_run();
        let pool = reward_pool_content_ids();

        assert_eq!(run.phase, RunPhase::Reward);
        assert!(run.combat.is_none());
        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.choices.len(), 3);
        assert_eq!(reward.gold_offer, REWARD_GOLD_AMOUNT);
        assert_eq!(reward.potion_offer, Some(Potion::Fire));
        assert_eq!(reward.relic_offer, Some(Relic::OddlySmoothStone));
        assert!(reward
            .choices
            .iter()
            .all(|card| pool.contains(&card.content_id)));
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
    fn take_card_reward_adds_choice_to_master_deck_and_stays_on_reward_screen() {
        let run = winning_combat_run();
        let deck_len_before = run.deck.len();
        let chosen = run.reward.as_ref().expect("reward screen").choices[0].id;
        let chosen_content = run.reward.as_ref().expect("reward screen").choices[0].content_id;

        let next = apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen })
            .expect("take reward");

        assert_eq!(next.phase, RunPhase::Reward);
        assert!(next.reward.as_ref().expect("reward").choices.is_empty());
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

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").gold_offer, 0);
        assert_eq!(next.deck, deck_before);
        assert_eq!(next.gold, gold_before + REWARD_GOLD_AMOUNT);
    }

    #[test]
    fn take_gold_reward_rejects_already_taken_gold() {
        let run = winning_combat_run();
        let next = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");

        let err = apply_run_action(&next, RunAction::TakeGoldReward).expect_err("gold taken");

        assert_eq!(err, SimError::IllegalAction("no gold reward offered"));
    }

    #[test]
    fn take_potion_reward_adds_fire_potion_to_belt() {
        let run = winning_combat_run();
        let potions_before = run.potions.len();

        let next = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").potion_offer, None);
        assert_eq!(next.potions.len(), potions_before + 1);
        assert_eq!(next.potions.last(), Some(&Potion::Fire));
    }

    #[test]
    fn take_potion_reward_rejects_full_belt() {
        let mut run = winning_combat_run();
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Fire];

        let err = apply_run_action(&run, RunAction::TakePotionReward).expect_err("belt full");

        assert_eq!(err, SimError::IllegalAction("potion belt is full"));
    }

    #[test]
    fn take_relic_reward_adds_oddly_smooth_stone() {
        let run = winning_combat_run();

        let next = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").relic_offer, None);
        assert_eq!(next.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn multiple_reward_offers_can_be_taken_before_skip() {
        let run = winning_combat_run();

        let run = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");
        let run = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");
        let run = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");
        let run = apply_run_action(&run, RunAction::SkipReward).expect("leave reward");

        assert_eq!(run.phase, RunPhase::Idle);
        assert!(run.reward.is_none());
        assert_eq!(run.gold, crate::STARTING_GOLD + REWARD_GOLD_AMOUNT);
        assert_eq!(run.potions, vec![Potion::Fire]);
        assert_eq!(run.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn take_relic_reward_rejects_duplicate_relic() {
        let mut run = winning_combat_run();
        run.relics.push(Relic::OddlySmoothStone);
        run.reward.as_mut().expect("reward screen").relic_offer = None;

        let err = apply_run_action(&run, RunAction::TakeRelicReward).expect_err("no offer");

        assert_eq!(err, SimError::IllegalAction("no relic reward offered"));
    }

    #[test]
    fn combat_fixture_starts_with_starting_gold() {
        let run = RunState::combat_fixture();

        assert_eq!(run.gold, crate::run::state::STARTING_GOLD);
    }
}
