use super::*;

pub(super) fn ironclad_starter_deck_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
        "Defend_R", "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(super) fn ironclad_deck_after_transform_selection_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R", "Defend_R",
        "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(super) fn seed_start_generated_transform_card(numeric_seed: i64) -> Option<String> {
    generate_neow_transform_reward(numeric_seed, &[STRIKE_R_ID])
        .cards
        .first()
        .map(|card| deck_content_key(*card).to_owned())
}

pub(super) fn seed_start_deck_after_transform(numeric_seed: i64) -> Vec<String> {
    let mut deck = ironclad_deck_after_transform_selection_keys();
    if let Some(card) = seed_start_generated_transform_card(numeric_seed) {
        deck.push(card);
    }
    deck
}

pub(super) fn seed_start_neow_choices(numeric_seed: i64) -> Vec<String> {
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .map(|option| option.label)
        .collect()
}

pub(super) fn seed_start_selected_neow_option(
    numeric_seed: i64,
    command: &str,
) -> Option<GeneratedNeowOption> {
    let index = command_choose_index(command)?;
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .nth(index)
}

pub(super) fn seed_start_apply_neow_simple_option(
    option: GeneratedNeowOption,
) -> Option<(i32, i32, i32)> {
    if !seed_start_neow_drawback_is_simple(option.drawback)
        || !seed_start_neow_reward_is_simple(option.reward)
    {
        return None;
    }

    let mut run = RunState::map_fixture();
    run.gold = 99;
    apply_neow_simple_drawback(&mut run, option.drawback).expect("matched simple Neow drawback");
    apply_neow_simple_reward(&mut run, option.reward)
        .expect("canonical seed-start immediate Neow reward is representable");
    Some((run.gold, run.player_hp, run.player_max_hp))
}

pub(super) fn seed_start_neow_drawback_is_simple(drawback: NeowDrawback) -> bool {
    matches!(
        drawback,
        NeowDrawback::None
            | NeowDrawback::TenPercentHpLoss
            | NeowDrawback::NoGold
            | NeowDrawback::PercentDamage
    )
}

pub(super) fn seed_start_neow_reward_is_simple(reward: NeowRewardType) -> bool {
    matches!(
        reward,
        NeowRewardType::TenPercentHpBonus
            | NeowRewardType::TwentyPercentHpBonus
            | NeowRewardType::HundredGold
            | NeowRewardType::TwoFiftyGold
    )
}

pub(super) fn seed_start_neow_option_is_supported_curse_simple(
    option: GeneratedNeowOption,
) -> bool {
    option.drawback == NeowDrawback::Curse && seed_start_neow_reward_is_simple(option.reward)
}

pub(super) fn seed_start_neow_option_is_supported_card_reward(option: GeneratedNeowOption) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::ThreeCards
                | NeowRewardType::RandomColorless
                | NeowRewardType::RandomColorlessTwo
                | NeowRewardType::ThreeRareCards
        )
}

pub(super) fn seed_start_neow_option_is_supported_grid_reward(option: GeneratedNeowOption) -> bool {
    (seed_start_neow_drawback_is_simple(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RemoveCard
                | NeowRewardType::RemoveTwo
                | NeowRewardType::UpgradeCard
                | NeowRewardType::TransformCard
                | NeowRewardType::TransformTwoCards
        ))
        || (option.drawback == NeowDrawback::Curse
            && option.reward == NeowRewardType::TransformTwoCards)
}

pub(super) fn seed_start_neow_option_is_supported_relic_reward(
    option: GeneratedNeowOption,
) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RandomCommonRelic | NeowRewardType::OneRareRelic
        )
}

pub(super) fn seed_start_neow_option_is_supported_boss_swap(option: GeneratedNeowOption) -> bool {
    option.drawback == NeowDrawback::None && option.reward == NeowRewardType::BossRelic
}

pub(super) fn seed_start_seeded_idle_run(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
) -> RunState {
    let mut run = RunState::seeded_ironclad(numeric_seed as u64, ascension);
    run.phase = RunPhase::Idle;
    run.event = None;
    run.reward = None;
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.combat = None;
    run.deck = deck_instances_from_keys(deck_ids);
    run
}

#[cfg(test)]
pub(super) fn seed_start_apply_neow_curse_simple_option(
    numeric_seed: i64,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = RunState::map_fixture();
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.deck = deck_instances_from_keys(deck_ids);
    run.relics = vec![Relic::BurningBlood];
    apply_neow_curse_drawback(&mut run)
        .expect("canonical seed-start deck has card ID allocation headroom");
    apply_neow_simple_reward(&mut run, option.reward)
        .expect("canonical seed-start immediate Neow reward is representable");
    run
}

pub(super) fn seed_start_apply_neow_curse_simple_visible_option(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    apply_neow_simple_reward(&mut run, option.reward)
        .expect("canonical seed-start immediate Neow reward is representable");
    run
}

pub(super) fn seed_start_neow_drawback_is_supported_for_reward_screen(
    drawback: NeowDrawback,
) -> bool {
    seed_start_neow_drawback_is_simple(drawback) || drawback == NeowDrawback::Curse
}

#[cfg(test)]
pub(super) fn seed_start_apply_neow_reward_drawback(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_apply_neow_reward_drawback_for_ascension(numeric_seed, 0, deck_ids, option)
}

pub(super) fn seed_start_apply_neow_reward_drawback_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => {
            apply_neow_simple_drawback(&mut run, drawback).expect("matched simple Neow drawback")
        }
    }
    run
}

#[cfg(test)]
pub(super) fn seed_start_open_neow_grid_run(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_open_neow_grid_run_for_ascension(numeric_seed, 0, deck_ids, option)
}

pub(super) fn seed_start_open_neow_grid_run_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => {
            apply_neow_simple_drawback(&mut run, drawback).expect("matched simple Neow drawback")
        }
    }
    open_neow_reward_grid(&mut run, option.reward).expect("matched grid-opening Neow reward");
    run
}

pub(super) fn seed_start_neow_curse_deck_key(
    numeric_seed: i64,
    card_rng_counter: u32,
) -> Option<String> {
    let mut run = RunState::map_fixture();
    run.reward_rng_seed = numeric_seed as u64;
    run.card_rng_counter = card_rng_counter;
    apply_neow_curse_drawback(&mut run)
        .expect("canonical seed-start deck has card ID allocation headroom");
    run.deck
        .last()
        .map(|card| deck_content_key(card.content_id).to_owned())
}

pub(super) fn seed_start_is_neow_multi_select_grid(run: &RunState) -> bool {
    run.card_grid.as_ref().is_some_and(|grid| {
        matches!(
            grid.purpose,
            GridPurpose::NeowTransform { .. } | GridPurpose::NeowRemove { remaining: 2.. }
        )
    })
}

pub(super) fn seed_start_neow_grid_auto_confirms_after_choose(run: &RunState) -> bool {
    seed_start_is_neow_multi_select_grid(run)
        && run
            .card_grid
            .as_ref()
            .is_some_and(grid_selection_ready_for_confirm)
}

pub(super) fn seed_start_neow_grid_transform_count(run: &RunState) -> Option<usize> {
    run.card_grid.as_ref().and_then(|grid| match grid.purpose {
        GridPurpose::NeowTransform { count } => Some(usize::from(count)),
        _ => None,
    })
}

pub(super) fn seed_start_visible_deck_after_neow_transform_selection(
    deck_ids: &[String],
    transform_count: usize,
    delayed_curse: Option<&str>,
) -> Vec<String> {
    let mut visible = deck_ids.to_vec();
    for _ in 0..transform_count.min(visible.len()) {
        visible.pop();
    }
    if let Some(curse) = delayed_curse {
        visible.push(curse.to_owned());
    }
    visible
}

pub(super) fn seed_start_apply_neow_boss_swap(numeric_seed: i64, deck_ids: &[String]) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, 0, deck_ids);
    run.gold = 99;
    run.relics = vec![Relic::BurningBlood];
    run.event = Some(neow_screen_for_stage(&run, 2));
    seed_start_prepare_neow_relic_equip(&mut run);
    apply_neow_boss_swap(&mut run).expect("canonical seed-start boss swap is representable");
    run
}

pub(super) fn seed_start_prepare_neow_relic_equip(run: &mut RunState) {
    // Captured session-352 shows Neow-spawned Whetstone using the second
    // miscRng draw for its onEquip shuffle. Session-32 proves that boss-swap
    // Tiny House uses the same offset before choosing the upgraded starter
    // instance. The exact UI/update-site draw before relic equip is not
    // exposed by CommunicationMod, so keep this scoped to seed-start Neow
    // relic replay instead of changing ordinary relic pickup.
    if run.misc_rng_counter == 0 {
        run.misc_rng_counter = 1;
    }
}

pub(super) fn seed_start_boss_swap_relic_ids(run: &RunState) -> Vec<String> {
    run.relics
        .iter()
        .map(|relic| relic.key())
        .filter(|key| *key != RelicKey::BurningBlood)
        .filter_map(|key| {
            let name = relic_key_trace_name(key);
            (name != "Unknown Relic").then(|| name.to_owned())
        })
        .collect()
}

pub(super) fn seed_start_boss_swap_is_calling_bell_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::CallingBellCurse)
}

pub(super) fn seed_start_boss_swap_is_astrolabe_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::Astrolabe)
}

pub(super) fn seed_start_boss_swap_is_pandoras_box_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::PandorasBox)
}

pub(super) fn seed_start_boss_swap_is_empty_cage_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| matches!(grid.purpose, GridPurpose::EmptyCage { .. }))
}

pub(super) fn seed_start_boss_swap_is_tiny_house_reward(run: &RunState) -> bool {
    run.relics.contains(&Relic::TinyHouse) && run.reward.is_some()
}

pub(super) fn seed_start_unsupported_boss_swap_reason(run: &RunState) -> Option<String> {
    if run.card_grid.is_some() {
        return Some(
            "Neow boss-swap produced a grid-opening boss relic without a dedicated seed-start follow-up; downstream parity remains classified"
                .to_owned(),
        );
    }
    if run.reward.is_some() {
        return Some(
            "Neow boss-swap produced a reward-screen boss relic; reward follow-up is classified outside this narrow verifier slice"
                .to_owned(),
        );
    }
    let unmapped = run
        .relics
        .iter()
        .map(|relic| relic.key())
        .find(|key| relic_key_trace_name(*key) == "Unknown Relic");
    unmapped.map(|key| {
        format!(
            "Neow boss-swap relic {key:?} is not trace-name mapped in sts_verify, so downstream parity remains classified"
        )
    })
}

pub(super) fn seed_start_neow_grid_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RemoveCard => "Neow remove card grid",
        NeowRewardType::RemoveTwo => "Neow remove two grid",
        NeowRewardType::UpgradeCard => "Neow upgrade grid",
        NeowRewardType::TransformTwoCards => "Neow transform two grid",
        _ => "Neow grid",
    }
}

pub(super) fn seed_start_neow_card_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::ThreeCards => "Neow card reward choices",
        NeowRewardType::OneRandomRareCard => "Neow random rare card reward",
        NeowRewardType::RandomColorlessTwo => "Neow rare colorless reward choices",
        NeowRewardType::ThreeRareCards => "Neow rare card reward choices",
        _ => "Neow card reward choices",
    }
}

pub(super) fn seed_start_neow_card_reward_choice_names(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| content_key(content_id).to_ascii_lowercase())
        .collect()
}

pub(super) fn seed_start_neow_card_reward_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| {
            let key = content_key(content_id);
            if key == "Hand Of Greed" {
                "HandOfGreed".to_owned()
            } else {
                key.to_owned()
            }
        })
        .collect()
}

pub(super) fn seed_start_neow_card_reward_id_values(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<Value> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| json!(content_id.get()))
        .collect()
}

pub(super) fn seed_start_neow_card_reward_card_rng_counter(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Option<u32> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            let generated = if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward)
            }
            .expect("matched generated Neow colorless reward option");
            Some(generated.card_rng_counter)
        }
        _ => None,
    }
}

pub(super) fn seed_start_neow_card_reward_content_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<ContentId> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            if option.drawback == NeowDrawback::Curse {
                generate_neow_colorless_reward(numeric_seed, option.reward)
                    .expect("matched generated Neow colorless reward option")
                    .cards
            } else if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
                .expect("matched generated Neow colorless reward option")
                .cards
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward)
                    .expect("matched generated Neow colorless reward option")
                    .cards
            }
        }
        _ => {
            generate_neow_card_reward(numeric_seed, option.reward)
                .expect("matched generated Neow card reward option")
                .cards
        }
    }
}

#[cfg(test)]
pub(super) fn seed_start_neow_potion_names(numeric_seed: i64) -> Vec<String> {
    generate_neow_three_potions(numeric_seed)
        .potions
        .into_iter()
        .map(|potion| potion_trace_name(potion).to_owned())
        .collect()
}

#[cfg(test)]
pub(super) fn seed_start_apply_neow_relic_reward(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_apply_neow_relic_reward_for_ascension(numeric_seed, 0, deck_ids, option)
}

pub(super) fn seed_start_apply_neow_relic_reward_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {
            run.reward_rng_seed = numeric_seed as u64;
            apply_neow_curse_drawback(&mut run)
                .expect("canonical seed-start deck has card ID allocation headroom");
        }
        drawback => {
            apply_neow_simple_drawback(&mut run, drawback).expect("matched simple Neow drawback")
        }
    }
    seed_start_prepare_neow_relic_equip(&mut run);
    apply_neow_relic_reward(&mut run, option.reward)
        .expect("canonical seed-start relic reward is representable");
    run
}

pub(super) fn seed_start_newest_trace_relic_name(run: &RunState) -> String {
    run.relics
        .iter()
        .last()
        .map(|relic| relic_key_trace_name(relic.key()).to_owned())
        .unwrap_or_else(|| "Unknown Relic".to_owned())
}

pub(super) fn seed_start_neow_relic_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RandomCommonRelic => "Neow common relic",
        NeowRewardType::OneRareRelic => "Neow rare relic",
        _ => "Neow relic",
    }
}

pub(super) fn seed_start_pick_neow_card_reward(
    reward_choices: &Option<Vec<String>>,
    command: &str,
) -> Option<String> {
    let index = command_choose_index(command)?;
    reward_choices.as_ref()?.get(index).cloned()
}
