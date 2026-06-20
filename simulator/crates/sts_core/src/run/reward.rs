use crate::{
    card::{CardInstance, CardRarity},
    combat::{apply_combat_action, CombatPhase},
    content::cards::{ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID},
    content::reward_pool::{RewardCardEntry, IRONCLAD_REWARD_ENTRIES},
    ids::CardId,
    potion::{Potion, PotionRarity, IRONCLAD_POTION_POOL, MAX_POTIONS},
    relic::RelicTier,
    rng::{RngStream, SimulatorRng, StsRng},
    run::potion::apply_potion_action,
    run::shop::apply_shop_action,
    CombatAction, RewardScreen, RunAction, RunPhase, RunState, SimError, SimResult,
};

const REWARD_CARD_COUNT: usize = 3;
const NORMAL_COMBAT_GOLD_MIN: i32 = 10;
const NORMAL_COMBAT_GOLD_MAX: i32 = 20;
const BASE_POTION_DROP_CHANCE: i32 = 40;
const ACT_4: i32 = 4;

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

fn roll_reward_rarity(rng: &mut StsRng, card_rarity_factor: i32) -> CardRarity {
    let roll = rng.random_int(99) + card_rarity_factor;
    if roll < 3 {
        CardRarity::Rare
    } else if roll < 40 {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
    }
}

fn roll_placeholder_reward_rarity(rng: &mut SimulatorRng) -> CardRarity {
    let roll = rng.next_usize(RngStream::RewardRarity, "reward_rarity", 140);
    if roll < 100 {
        CardRarity::Common
    } else if roll < 137 {
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
        let requested = roll_placeholder_reward_rarity(rng);
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

#[must_use]
pub fn target_card_reward_choices(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
) -> Vec<CardInstance> {
    let mut choices = Vec::with_capacity(REWARD_CARD_COUNT);

    for index in 0..REWARD_CARD_COUNT {
        let requested = roll_reward_rarity(rng, *card_rarity_factor);
        let rarity = resolve_rarity(requested, IRONCLAD_REWARD_ENTRIES);
        match requested {
            CardRarity::Common => *card_rarity_factor = (*card_rarity_factor - 1).max(-40),
            CardRarity::Rare => *card_rarity_factor = 5,
            CardRarity::Uncommon => {}
        }

        let mut content_id;
        loop {
            let candidate_indices: Vec<usize> = IRONCLAD_REWARD_ENTRIES
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.rarity == rarity)
                .map(|(index, _)| index)
                .collect();
            let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
            content_id = IRONCLAD_REWARD_ENTRIES[candidate_indices[pick]].content_id;
            if !choices
                .iter()
                .any(|choice: &CardInstance| choice.content_id == content_id)
            {
                break;
            }
        }

        choices.push(CardInstance::new(
            CardId::new(next_card_id + index as u64),
            content_id,
        ));
    }

    choices
}

pub fn target_normal_combat_gold(rng: &mut StsRng) -> i32 {
    rng.random_int_range(NORMAL_COMBAT_GOLD_MIN, NORMAL_COMBAT_GOLD_MAX)
}

pub fn target_relic_tier(rng: &mut StsRng, act: i32) -> RelicTier {
    let common_chance = if act == ACT_4 { 0 } else { 50 };
    let uncommon_chance = if act == ACT_4 { 100 } else { 33 };
    let roll = rng.random_int_range(0, 99);

    if roll < common_chance {
        RelicTier::Common
    } else if roll < common_chance + uncommon_chance {
        RelicTier::Uncommon
    } else {
        RelicTier::Rare
    }
}

pub fn target_elite_relic_tier(rng: &mut StsRng) -> RelicTier {
    let roll = rng.random_int(99);
    if roll < 50 {
        RelicTier::Common
    } else if roll > 82 {
        RelicTier::Rare
    } else {
        RelicTier::Uncommon
    }
}

fn target_random_potion(rng: &mut StsRng) -> Potion {
    let rarity = match rng.random_int_range(0, 99) {
        roll if roll < 65 => PotionRarity::Common,
        roll if roll < 90 => PotionRarity::Uncommon,
        _ => PotionRarity::Rare,
    };

    loop {
        let index = rng.random_int((IRONCLAD_POTION_POOL.len() - 1) as i32) as usize;
        let potion = IRONCLAD_POTION_POOL[index];
        if potion.rarity() == rarity {
            return potion;
        }
    }
}

pub fn target_potion_reward_offer(
    rng: &mut StsRng,
    potion_chance: &mut i32,
    reward_count: usize,
    potion_belt_count: usize,
) -> Option<Potion> {
    if potion_belt_count >= MAX_POTIONS {
        return None;
    }

    let mut chance = BASE_POTION_DROP_CHANCE + *potion_chance;
    if reward_count >= 4 {
        chance = 0;
    }

    if rng.random_int(99) >= chance {
        *potion_chance += 10;
        None
    } else {
        *potion_chance -= 10;
        Some(target_random_potion(rng))
    }
}

pub fn enter_reward_screen(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let mut card_rng = StsRng::with_counter(run.reward_rng_seed as i64, run.card_rng_counter);
    let choices =
        target_card_reward_choices(&mut card_rng, &mut run.card_rarity_factor, next_card_id);
    run.card_rng_counter = card_rng.counter();

    let mut treasure_rng =
        StsRng::with_counter(run.treasure_rng_seed as i64, run.treasure_rng_counter);
    let gold_offer = target_normal_combat_gold(&mut treasure_rng);
    run.treasure_rng_counter = treasure_rng.counter();

    let mut potion_rng = StsRng::with_counter(run.potion_rng_seed as i64, run.potion_rng_counter);
    let potion_offer = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        1 + 1,
        run.potions.len(),
    );
    run.potion_rng_counter = potion_rng.counter();

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices,
        gold_offer,
        potion_offer,
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
        BASH_ID, BODY_SLAM_ID, CLEAVE_ID, CLOTHESLINE_ID, HAVOC_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID,
        TWIN_STRIKE_ID,
    };
    use crate::relic::Relic;

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
    fn some_placeholder_seed_rolls_havoc_from_modeled_pool() {
        let havoc_found = (0_u64..10_000).any(|seed| {
            let mut rng = SimulatorRng::new(seed);
            card_reward_choices(&mut rng, 1)
                .iter()
                .any(|card| card.content_id == HAVOC_ID)
        });

        assert!(havoc_found);
    }

    #[test]
    fn placeholder_seed_7_reward_cards_match_golden_snapshot() {
        let mut rng = SimulatorRng::new(7);
        let choices = card_reward_choices(&mut rng, 100);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();

        assert_eq!(
            content_ids,
            vec![CLOTHESLINE_ID, SHRUG_IT_OFF_ID, CLEAVE_ID],
            "update snapshot if reward algorithm changes intentionally"
        );
    }

    #[test]
    fn target_card_reward_choices_use_sts_card_rng_and_rarity_factor() {
        let mut rng = StsRng::new(22_079_335_079);
        let mut card_rarity_factor = 5;

        let choices = target_card_reward_choices(&mut rng, &mut card_rarity_factor, 100);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();

        assert_eq!(
            content_ids,
            vec![BODY_SLAM_ID, TWIN_STRIKE_ID, CLOTHESLINE_ID]
        );
        assert_eq!(rng.counter(), 6);
        assert_eq!(card_rarity_factor, 2);
    }

    #[test]
    fn combat_win_enters_reward_with_target_card_rng() {
        let mut run = winning_combat_run();

        run.reward_rng_seed = 22_079_335_079;
        run.card_rng_counter = 0;
        run.card_rarity_factor = 5;
        enter_reward_screen(&mut run);

        let reward = run.reward.expect("reward screen present");
        let content_ids: Vec<_> = reward.choices.iter().map(|card| card.content_id).collect();
        assert_eq!(
            content_ids,
            vec![BODY_SLAM_ID, TWIN_STRIKE_ID, CLOTHESLINE_ID]
        );
        assert_eq!(run.card_rarity_factor, 2);
        assert_eq!(run.card_rng_counter, 6);
    }

    #[test]
    fn target_card_reward_counter_persists_between_rewards() {
        let mut run = winning_combat_run();

        run.reward_rng_seed = 22_079_335_079;
        run.card_rng_counter = 0;
        run.card_rarity_factor = 5;
        enter_reward_screen(&mut run);
        let first_counter = run.card_rng_counter;
        let first_choices: Vec<_> = run
            .reward
            .as_ref()
            .expect("first reward")
            .choices
            .iter()
            .map(|card| card.content_id)
            .collect();

        enter_reward_screen(&mut run);
        let second_choices: Vec<_> = run
            .reward
            .as_ref()
            .expect("second reward")
            .choices
            .iter()
            .map(|card| card.content_id)
            .collect();

        assert_eq!(first_counter, 6);
        assert!(run.card_rng_counter > first_counter);
        assert_ne!(second_choices, first_choices);
    }

    #[test]
    fn combat_win_enters_reward_with_three_rng_choices() {
        let run = winning_combat_run();
        let pool = reward_pool_content_ids();

        assert_eq!(run.phase, RunPhase::Reward);
        assert!(run.combat.is_none());
        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.choices.len(), 3);
        assert_eq!(reward.gold_offer, 11);
        assert_eq!(reward.potion_offer, None);
        assert_eq!(run.potion_chance, 10);
        assert_eq!(run.potion_rng_counter, 1);
        assert_eq!(reward.relic_offer, None);
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
        assert_eq!(next.gold, gold_before + 11);
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
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);
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
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);

        let err = apply_run_action(&run, RunAction::TakePotionReward).expect_err("belt full");

        assert_eq!(err, SimError::IllegalAction("potion belt is full"));
    }

    #[test]
    fn take_relic_reward_adds_oddly_smooth_stone() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").relic_offer = Some(Relic::OddlySmoothStone);

        let next = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").relic_offer, None);
        assert_eq!(next.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn multiple_reward_offers_can_be_taken_before_skip() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);
        run.reward.as_mut().expect("reward").relic_offer = Some(Relic::OddlySmoothStone);

        let run = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");
        let run = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");
        let run = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");
        let run = apply_run_action(&run, RunAction::SkipReward).expect("leave reward");

        assert_eq!(run.phase, RunPhase::Idle);
        assert!(run.reward.is_none());
        assert_eq!(run.gold, crate::STARTING_GOLD + 11);
        assert_eq!(run.potions, vec![Potion::Fire]);
        assert_eq!(run.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn normal_combat_gold_uses_target_treasure_rng_range() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(target_normal_combat_gold(&mut rng), 19);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_relic_tier_uses_act_one_thresholds() {
        let mut uncommon_rng = StsRng::new(22_079_335_079);
        let mut common_rng = StsRng::new(22_079_335_079);
        common_rng.random_int(99);
        let mut rare_rng = StsRng::new(22_079_335_079);
        for _ in 0..10 {
            rare_rng.random_int(99);
        }

        assert_eq!(target_relic_tier(&mut common_rng, 1), RelicTier::Common);
        assert_eq!(target_relic_tier(&mut rare_rng, 1), RelicTier::Rare);
        assert_eq!(target_relic_tier(&mut uncommon_rng, 1), RelicTier::Uncommon);
    }

    #[test]
    fn target_relic_tier_uses_act_four_thresholds() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(target_relic_tier(&mut rng, 4), RelicTier::Uncommon);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_elite_relic_tier_uses_target_thresholds() {
        let mut uncommon_rng = StsRng::new(22_079_335_079);
        let mut common_rng = StsRng::new(22_079_335_079);
        common_rng.random_int(99);
        let mut rare_rng = StsRng::new(22_079_335_079);
        for _ in 0..10 {
            rare_rng.random_int(99);
        }

        assert_eq!(target_elite_relic_tier(&mut common_rng), RelicTier::Common);
        assert_eq!(target_elite_relic_tier(&mut rare_rng), RelicTier::Rare);
        assert_eq!(
            target_elite_relic_tier(&mut uncommon_rng),
            RelicTier::Uncommon
        );
    }

    #[test]
    fn target_potion_reward_miss_increases_chance_and_consumes_drop_roll() {
        let mut rng = StsRng::new(0);
        let mut potion_chance = 0;

        let offer = target_potion_reward_offer(&mut rng, &mut potion_chance, 2, 0);

        assert_eq!(offer, None);
        assert_eq!(potion_chance, 10);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_potion_reward_hit_decreases_chance_and_rolls_pool() {
        let mut rng = StsRng::new(0);
        let mut potion_chance = 70;

        let offer = target_potion_reward_offer(&mut rng, &mut potion_chance, 2, 0);

        assert!(offer.is_some());
        assert_eq!(potion_chance, 60);
        assert!(rng.counter() > 1);
    }

    #[test]
    fn combat_win_persists_treasure_rng_counter() {
        let mut run = winning_combat_run();

        run.treasure_rng_seed = 22_079_335_079;
        run.treasure_rng_counter = 0;
        enter_reward_screen(&mut run);

        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.gold_offer, 19);
        assert_eq!(run.treasure_rng_counter, 1);
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
