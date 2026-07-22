use super::*;

pub(super) enum SeedStartPreDispatch {
    NotHandled,
    Handled,
    Boundary(SeedStartBoundary),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn seed_start_handle_overlay_command(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    phase: &mut SeedStartPhase,
    seed_sim: Option<&RunState>,
    pending_boss_relic_overlay: &mut Option<PendingBossRelicOverlayAssertion>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if action
        .command
        .split_whitespace()
        .next()
        .is_some_and(|head| head.eq_ignore_ascii_case("CLICK"))
        && pre
            .message
            .get("game_state")
            .and_then(|game| game.get("screen_name"))
            .and_then(Value::as_str)
            .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"))
    {
        let Some(sim) = seed_sim else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_ftue_dismiss".to_owned(),
                reason: "FTUE dismissal occurred without initialized deterministic replay"
                    .to_owned(),
            });
        };
        if sim.phase != RunPhase::Reward {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_ftue_dismiss".to_owned(),
                reason: format!(
                    "FTUE dismissal expected deterministic reward state, found {:?}",
                    sim.phase
                ),
            });
        }
        compare_subset(
            report,
            action,
            "dismiss FTUE overlay",
            seed_start_reward_observed_subset(&post.message),
            seed_start_reward_simulated_subset(sim),
        );
        *phase = SeedStartPhase::Reward;
        return SeedStartPreDispatch::Handled;
    }

    if action
        .command
        .split_whitespace()
        .next()
        .is_some_and(|head| head.eq_ignore_ascii_case("KEY"))
        && action
            .command
            .split_whitespace()
            .nth(1)
            .is_some_and(|key| key.eq_ignore_ascii_case("CANCEL"))
        && pre
            .message
            .get("game_state")
            .and_then(|game| game.get("screen_name"))
            .and_then(Value::as_str)
            .is_some_and(|screen| screen.eq_ignore_ascii_case("MASTER_DECK_VIEW"))
        && *phase == SeedStartPhase::Treasure
    {
        let Some(sim) = seed_sim else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_boss_reward_overlay".to_owned(),
                reason: "boss relic deck overlay closed without initialized deterministic replay"
                    .to_owned(),
            });
        };
        let diff_count = report.unexpected_diffs.len();
        compare_subset(
            report,
            action,
            "close boss relic deck overlay",
            seed_start_treasure_observed_subset(&post.message),
            seed_start_treasure_simulated_subset(sim),
        );
        let stable_matches = report.unexpected_diffs.len() == diff_count;
        if let Some(pending) = pending_boss_relic_overlay.take() {
            seed_start_reconcile_boss_relic_overlay(
                report,
                pending,
                stable_matches,
                action.step,
                reconciled_deferred_action_steps,
            );
        }
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_bootstrap_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    boss_unlocks: BossUnlockState,
    deck_ids: &[String],
    seed_sim: Option<&RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::BeforeStart
        && action.command.eq_ignore_ascii_case(&format!(
            "START {} {} {}",
            start.character, start.ascension, start.external_seed
        ))
    {
        compare_subset(
            report,
            action,
            "seed-start bootstrap",
            seed_start_bootstrap_observed_subset(&post.message),
            seed_start_bootstrap_simulated_subset(start, boss_unlocks, deck_ids),
        );
        *phase = SeedStartPhase::NeowTalk;
        return SeedStartPreDispatch::Handled;
    }
    if *phase == SeedStartPhase::NeowTalk && command_is_choose(&action.command, 0) {
        compare_subset(
            report,
            action,
            "Neow talk",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": seed_start_neow_choices(start.numeric_seed),
            }),
        );
        *phase = SeedStartPhase::NeowOptions;
        return SeedStartPreDispatch::Handled;
    }
    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_transform_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    neow_leave_visible_deck_ids: &mut Option<Vec<String>>,
    seed_sim: Option<&RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions
        && seed_start_selected_neow_option(start.numeric_seed, &action.command)
            .is_some_and(|option| option.reward == NeowRewardType::TransformCard)
    {
        compare_subset(
            report,
            action,
            "Neow transform grid",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "GRID",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": ["strike", "strike", "strike", "strike", "strike", "defend", "defend", "defend", "defend", "bash"],
            }),
        );
        *phase = SeedStartPhase::NeowTransformGrid;
        return SeedStartPreDispatch::Handled;
    }
    if *phase == SeedStartPhase::NeowTransformGrid && action.command.eq_ignore_ascii_case("PROCEED")
    {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "captured trace sent PROCEED while Neow transform grid only accepted choose; classified as a trace-client command hiccup".to_owned(),
        });
        return SeedStartPreDispatch::Handled;
    }
    if *phase == SeedStartPhase::NeowTransformGrid && command_is_choose(&action.command, 0) {
        compare_subset(
            report,
            action,
            "Neow transform Strike select",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "GRID",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": [],
            }),
        );
        *phase = SeedStartPhase::NeowTransformConfirm;
        return SeedStartPreDispatch::Handled;
    }
    if *phase == SeedStartPhase::NeowTransformConfirm
        && action.command.eq_ignore_ascii_case("CONFIRM")
    {
        let visible_deck_after_transform = ironclad_deck_after_transform_selection_keys();
        compare_subset(
            report,
            action,
            "Neow transform confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": visible_deck_after_transform,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": ["leave"],
            }),
        );
        *deck_ids = seed_start_deck_after_transform(start.numeric_seed);
        *neow_leave_visible_deck_ids = Some(visible_deck_after_transform);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }
    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_immediate_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    pending_neow_room_entry_curse: &mut Option<String>,
    pending_neow_room_entry_curse_advances_card_rng: &mut bool,
    neow_gold: &mut i32,
    neow_current_hp: &mut i32,
    neow_max_hp: &mut i32,
    seed_sim: &mut Option<RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::NeowOptions {
        return SeedStartPreDispatch::NotHandled;
    }
    let Some(option) = seed_start_selected_neow_option(start.numeric_seed, &action.command) else {
        return SeedStartPreDispatch::NotHandled;
    };

    if let Some((gold, current_hp, max_hp)) = seed_start_apply_neow_simple_option(option.clone()) {
        *neow_gold = gold;
        *neow_current_hp = current_hp;
        *neow_max_hp = max_hp;
        compare_subset(
            report,
            action,
            "Neow simple immediate reward",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": gold,
                "current_hp": current_hp,
                "max_hp": max_hp,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if option.reward == NeowRewardType::ThreeEnemyKill {
        let mut run = seed_start_carried_run(
            seed_sim.as_ref(),
            start.numeric_seed,
            start.ascension,
            &start.external_seed,
            deck_ids,
        );
        apply_neow_lament_reward(&mut run);
        *seed_sim = Some(run);
        compare_subset(
            report,
            action,
            "Neow's Lament",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if seed_start_neow_option_is_supported_curse_simple(option.clone()) {
        let mut run = seed_start_apply_neow_curse_simple_visible_option(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            option.clone(),
        );
        let mut curse_run = run.clone();
        let curse = match apply_neow_curse_drawback(&mut curse_run) {
            Ok(curse) => curse,
            Err(error) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_state".to_owned(),
                    reason: format!("core Neow curse drawback rejected simulator state: {error}"),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            }
        };
        *pending_neow_room_entry_curse = Some(deck_content_key(curse.curse).to_owned());
        *pending_neow_room_entry_curse_advances_card_rng = false;
        run.card_rng_counter = curse.card_rng_counter;
        *deck_ids = deck_content_keys(&run.deck);
        *neow_gold = run.gold;
        *neow_current_hp = run.player_hp;
        *neow_max_hp = run.player_max_hp;
        *seed_sim = Some(run);
        compare_subset(
            report,
            action,
            "Neow curse immediate reward",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": neow_gold,
                "current_hp": neow_current_hp,
                "max_hp": neow_max_hp,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if seed_start_neow_option_is_supported_relic_reward(option.clone()) {
        let mut run = seed_start_apply_neow_relic_reward_for_ascension(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            &option,
        );
        let mut visible_deck_ids = deck_content_keys(&run.deck);
        if option.drawback == NeowDrawback::Curse {
            if let Some(curse) = visible_deck_ids.pop() {
                *pending_neow_room_entry_curse = Some(curse);
                *pending_neow_room_entry_curse_advances_card_rng = false;
                run.deck = deck_instances_from_keys(&visible_deck_ids);
            }
        }
        *deck_ids = visible_deck_ids;
        *neow_gold = run.gold;
        *neow_current_hp = run.player_hp;
        *neow_max_hp = run.player_max_hp;
        let relic = seed_start_newest_trace_relic_name(&run);
        compare_subset(
            report,
            action,
            seed_start_neow_relic_reward_label(option.reward),
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": neow_gold,
                "current_hp": neow_current_hp,
                "max_hp": neow_max_hp,
                "deck_ids": deck_ids,
                "relic_ids": relic_ids_for_simulated_subset(&run),
                "choices": ["leave"],
            }),
        );
        if relic == "Toy Ornithopter" {
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: "Toy Ornithopter is only carried as a captured Neow relic in this trace; no potion-use transition is observed here, so potion-triggered healing remains covered by sts_core unit tests rather than seed-start trace parity".to_owned(),
            });
        }
        *seed_sim = Some(run);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_card_reward_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    neow_gold: &mut i32,
    neow_current_hp: &mut i32,
    neow_max_hp: &mut i32,
    neow_card_reward_option: &mut Option<GeneratedNeowOption>,
    neow_card_reward_choices: &mut Option<Vec<String>>,
    neow_card_reward_card_rng_counter: &mut Option<u32>,
    neow_leave_visible_deck_ids: &mut Option<Vec<String>>,
    delayed_neow_curse: &mut Option<String>,
    seed_sim: &mut Option<RunState>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions {
        let Some(option) = seed_start_selected_neow_option(start.numeric_seed, &action.command)
        else {
            return SeedStartPreDispatch::NotHandled;
        };

        if option.reward == NeowRewardType::OneRandomRareCard {
            let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
                start.numeric_seed,
                start.ascension,
                deck_ids,
                &option,
            );
            *deck_ids = deck_content_keys(&run.deck);
            *neow_gold = run.gold;
            *neow_current_hp = run.player_hp;
            *neow_max_hp = run.player_max_hp;
            compare_subset(
                report,
                action,
                seed_start_neow_card_reward_label(option.reward),
                seed_start_observed_subset(&post.message),
                json!({
                    "screen_type": "EVENT",
                    "ascension": start.ascension,
                    "floor": 0,
                    "gold": neow_gold,
                    "current_hp": neow_current_hp,
                    "max_hp": neow_max_hp,
                    "deck_ids": deck_ids,
                    "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                    "choices": ["leave"],
                }),
            );
            let reward = generate_neow_card_reward(start.numeric_seed, option.reward)
                .expect("matched generated Neow card reward option");
            *neow_leave_visible_deck_ids = Some(deck_ids.clone());
            for content_id in reward.cards {
                run.gain_deck_card(content_id)
                    .expect("canonical seed-start deck has card ID allocation headroom");
            }
            *deck_ids = deck_content_keys(&run.deck);
            *seed_sim = Some(run);
            *phase = SeedStartPhase::NeowLeave;
            return SeedStartPreDispatch::Handled;
        }

        if seed_start_neow_option_is_supported_card_reward(option.clone()) {
            let run = seed_start_apply_neow_reward_drawback_for_ascension(
                start.numeric_seed,
                start.ascension,
                deck_ids,
                &option,
            );
            *deck_ids = deck_content_keys(&run.deck);
            *neow_gold = run.gold;
            *neow_current_hp = run.player_hp;
            *neow_max_hp = run.player_max_hp;
            *neow_card_reward_choices = Some(seed_start_neow_card_reward_ids(
                start.numeric_seed,
                &option,
                Some(&run),
            ));
            *neow_card_reward_card_rng_counter = seed_start_neow_card_reward_card_rng_counter(
                start.numeric_seed,
                &option,
                Some(&run),
            );
            *neow_card_reward_option = Some(option.clone());
            if option.drawback == NeowDrawback::Curse {
                let card_rng_counter = match option.reward {
                    NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
                        generate_neow_colorless_reward(start.numeric_seed, option.reward)
                            .expect("matched generated Neow colorless reward option")
                            .card_rng_counter
                    }
                    _ => 0,
                };
                *delayed_neow_curse =
                    seed_start_neow_curse_deck_key(start.numeric_seed, card_rng_counter);
            }
            compare_subset(
                report,
                action,
                seed_start_neow_card_reward_label(option.reward),
                seed_start_reward_observed_subset(&post.message),
                json!({
                    "screen_type": "CARD_REWARD",
                    "floor": 0,
                    "gold": neow_gold,
                    "current_hp": neow_current_hp,
                    "max_hp": neow_max_hp,
                    "deck_ids": deck_ids,
                    "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                    "choices": seed_start_neow_card_reward_choice_names(start.numeric_seed, &option, Some(&run)),
                    "card_reward_ids": seed_start_neow_card_reward_id_values(start.numeric_seed, &option, Some(&run)),
                    "unobservable": {
                        "card_reward_rng_draws": true,
                        "card_reward_uuids": true,
                    },
                }),
            );
            *phase = SeedStartPhase::NeowCardReward;
            return SeedStartPreDispatch::Handled;
        }

        return SeedStartPreDispatch::NotHandled;
    }

    if *phase != SeedStartPhase::NeowCardReward {
        return SeedStartPreDispatch::NotHandled;
    }
    let Some(picked_card) =
        seed_start_pick_neow_card_reward(neow_card_reward_choices, &action.command)
    else {
        return SeedStartPreDispatch::NotHandled;
    };
    let option = neow_card_reward_option
        .as_ref()
        .expect("Neow card reward option is carried");
    let pre_pick_deck_ids = deck_ids.clone();
    deck_ids.push(picked_card.clone());
    let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
        start.numeric_seed,
        start.ascension,
        deck_ids,
        option,
    );
    if let Some(card_rng_counter) = *neow_card_reward_card_rng_counter {
        run.card_rng_counter = card_rng_counter;
    }
    let mut transient_deck = deck_ids.clone();
    if let Some(curse) = delayed_neow_curse.take() {
        let curse_order = match pending_neow_curse_order(pre, action) {
            Ok(order) => order,
            Err(reason) => {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].sent_at", action.step),
                    category: "invalid_input".to_owned(),
                    reason: reason.to_owned(),
                });
            }
        };
        // NeowReward.activate marks the curse pending before opening the reward. A target update can
        // obtain it while that screen remains open; otherwise CardRewardScreen obtains the selected
        // card first. The order is bound from pre-state/action timing, never post-state.
        *deck_ids = pre_pick_deck_ids;
        if curse_order == PendingNeowCurseOrder::BeforePickedCard {
            deck_ids.push(curse);
            deck_ids.push(picked_card);
        } else {
            deck_ids.push(picked_card);
            deck_ids.push(curse);
        }
        if curse_order == PendingNeowCurseOrder::BeforePickedCard {
            transient_deck = deck_ids.clone();
        }
        run.deck = deck_instances_from_keys(deck_ids);
        run.card_rng_counter = run.card_rng_counter.saturating_add(1);
    }
    *seed_sim = Some(run);
    let mut observed = seed_start_observed_subset(&post.message);
    let observed_deck = observed
        .as_object_mut()
        .and_then(|object| object.remove("deck_ids"))
        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
        .unwrap_or_default();
    let mut simulated = json!({
        "screen_type": "EVENT",
        "ascension": start.ascension,
        "floor": 0,
        "gold": neow_gold,
        "current_hp": neow_current_hp,
        "max_hp": neow_max_hp,
        "deck_ids": deck_ids,
        "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
        "choices": ["leave"],
    });
    let simulated_deck = simulated
        .as_object_mut()
        .and_then(|object| object.remove("deck_ids"))
        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
        .expect("Neow card pickup projection contains a deck");
    let mut diffs = subset_diffs(observed, simulated);
    match classify_deferred_deck_observation(&observed_deck, &transient_deck, &simulated_deck) {
        PendingDeckObservation::Settled if diffs.is_empty() => {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow colorless pickup".to_owned(),
            });
        }
        PendingDeckObservation::Deferred if diffs.is_empty() => {
            *pending_deck_assertion = Some(PendingDeckAssertion {
                action: action.clone(),
                label: "Neow colorless pickup".to_owned(),
                transient_decks: vec![transient_deck],
                expected_deck: simulated_deck,
            });
        }
        PendingDeckObservation::Diverged(deck_diffs) => {
            diffs.extend(deck_diffs);
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow colorless pickup".to_owned(),
                diffs,
            });
        }
        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow colorless pickup".to_owned(),
                diffs,
            });
        }
    }
    *phase = SeedStartPhase::NeowLeave;
    SeedStartPreDispatch::Handled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_potion_reward_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    neow_gold: i32,
    neow_current_hp: i32,
    neow_max_hp: i32,
    neow_potions_taken: &mut usize,
    seed_sim: &mut Option<RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions
        && seed_start_selected_neow_option(start.numeric_seed, &action.command)
            .is_some_and(|option| option.reward == NeowRewardType::ThreeSmallPotions)
    {
        let option_index =
            command_choose_index(&action.command).expect("matched generated three-potion option");
        *neow_potions_taken = 0;
        let mut run = seed_start_carried_run(
            seed_sim.as_ref(),
            start.numeric_seed,
            start.ascension,
            &start.external_seed,
            deck_ids,
        );
        run.gold = neow_gold;
        run.player_hp = neow_current_hp;
        run.player_max_hp = neow_max_hp;
        run.phase = RunPhase::Event;
        run.event = Some(neow_screen_for_stage(&run, 1));
        let next = match apply_event_action(
            &run,
            EventAction::Choose {
                choice_index: option_index,
            },
        ) {
            Ok(next) => next,
            Err(err) => {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_potion_reward".to_owned(),
                    reason: format!("core rejected generated Neow three-potion option: {err}"),
                });
            }
        };
        if next.phase != RunPhase::Reward
            || next
                .reward
                .as_ref()
                .is_none_or(|reward| reward.potion_offers.len() != 3)
        {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_neow_potion_reward".to_owned(),
                reason: "core Neow three-potion option did not open three potion rewards"
                    .to_owned(),
            });
        }
        compare_subset(
            report,
            action,
            "Neow three potion reward",
            seed_start_neow_potion_reward_observed_subset(&post.message),
            seed_start_neow_potion_reward_simulated_subset(&next),
        );
        *seed_sim = Some(next);
        *phase = SeedStartPhase::NeowPotionReward;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowPotionReward && command_is_choose(&action.command, 0) {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_neow_potion_reward".to_owned(),
                reason: "Neow potion pick has no authoritative core reward state".to_owned(),
            });
        };
        let next = match apply_run_action(sim, RunAction::TakePotionReward { index: 0 }) {
            Ok(next) => next,
            Err(err) => {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_potion_reward".to_owned(),
                    reason: format!("core rejected Neow potion pick: {err}"),
                });
            }
        };
        *neow_potions_taken += 1;
        compare_subset(
            report,
            action,
            &format!("Neow potion reward pick {neow_potions_taken}"),
            seed_start_neow_potion_reward_observed_subset(&post.message),
            seed_start_neow_potion_reward_simulated_subset(&next),
        );
        *seed_sim = Some(next);
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowPotionReward && action.command.eq_ignore_ascii_case("PROCEED")
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_neow_potion_reward".to_owned(),
                reason: "Neow potion proceed has no authoritative core reward state".to_owned(),
            });
        };
        let next = match apply_run_action(sim, RunAction::Proceed) {
            Ok(next) => next,
            Err(err) => {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_potion_reward".to_owned(),
                    reason: format!("core rejected Neow potion reward proceed: {err}"),
                });
            }
        };
        let mut observed = seed_start_map_return_observed_subset(&post.message);
        seed_start_insert_observed_potion_ids(&mut observed, &post.message);
        let mut simulated = match seed_start_simulated_map_return(&next) {
            Ok(simulated) => simulated,
            Err(reason) => {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_potion_map_projection".to_owned(),
                    reason,
                });
            }
        };
        seed_start_insert_simulated_potion_ids(&mut simulated, &next);
        compare_subset(
            report,
            action,
            "Neow potion reward proceed",
            observed,
            simulated,
        );
        *deck_ids = deck_content_keys(&next.deck);
        *seed_sim = Some(next);
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_grid_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    neow_gold: &mut i32,
    neow_current_hp: &mut i32,
    neow_max_hp: &mut i32,
    neow_leave_visible_deck_ids: &mut Option<Vec<String>>,
    delayed_neow_curse: &mut Option<String>,
    delayed_neow_transform_count: &mut usize,
    seed_sim: &mut Option<RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions {
        let Some(option) = seed_start_selected_neow_option(start.numeric_seed, &action.command)
        else {
            return SeedStartPreDispatch::NotHandled;
        };
        if !seed_start_neow_option_is_supported_grid_reward(option.clone()) {
            return SeedStartPreDispatch::NotHandled;
        }
        let mut run = seed_start_open_neow_grid_run_for_ascension(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            &option,
        );
        if option.drawback == NeowDrawback::Curse {
            let mut curse_run = run.clone();
            let curse = match apply_neow_curse_drawback(&mut curse_run) {
                Ok(curse) => curse,
                Err(error) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_neow_state".to_owned(),
                        reason: format!(
                            "core Neow curse drawback rejected simulator state: {error}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return SeedStartPreDispatch::Boundary(boundary);
                }
            };
            *delayed_neow_curse = Some(deck_content_key(curse.curse).to_owned());
            run.card_rng_counter = curse.card_rng_counter;
            *delayed_neow_transform_count = match option.reward {
                NeowRewardType::TransformCard => 1,
                NeowRewardType::TransformTwoCards => 2,
                _ => 0,
            };
        }
        *neow_gold = run.gold;
        *neow_current_hp = run.player_hp;
        *neow_max_hp = run.player_max_hp;
        compare_subset(
            report,
            action,
            seed_start_neow_grid_label(option.reward),
            seed_start_grid_observed_subset(&post.message),
            seed_start_grid_simulated_subset(&run),
        );
        *seed_sim = Some(run);
        *phase = SeedStartPhase::NeowGrid;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowGrid && command_choose_index(&action.command).is_some() {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow grid action without initialized run simulation".to_owned(),
            });
        };
        let index = command_choose_index(&action.command).expect("matched choose command");
        let Ok(next) = select_grid_card(sim, index) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow grid choose simulation failed".to_owned(),
            });
        };
        let observed_grid = seed_start_grid_observed_subset(&post.message);
        let selected_subset = if *delayed_neow_transform_count > 0 {
            let mut visible_deck_ids = deck_content_keys(&next.deck);
            if let Some(curse) = delayed_neow_curse.as_deref() {
                visible_deck_ids.push(curse.to_owned());
            }
            seed_start_grid_simulated_subset_with_deck(&next, visible_deck_ids)
        } else {
            seed_start_grid_simulated_subset(&next)
        };
        if !seed_start_neow_grid_auto_confirms_after_choose(&next) {
            compare_subset(
                report,
                action,
                "Neow grid select",
                observed_grid,
                selected_subset,
            );
            *seed_sim = Some(next);
            *phase = SeedStartPhase::NeowGridConfirm;
            return SeedStartPreDispatch::Handled;
        }
        let Ok(confirmed) = confirm_grid(&next) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow grid auto-confirm simulation failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&confirmed.deck);
        let confirmed_deck_ids = deck_ids.clone();
        let transform_count_before_confirm =
            seed_start_neow_grid_transform_count(&next).unwrap_or(*delayed_neow_transform_count);
        let mut visible_deck_ids = deck_ids.clone();
        if transform_count_before_confirm > 0 {
            visible_deck_ids = seed_start_visible_deck_after_neow_transform_selection(
                deck_ids,
                transform_count_before_confirm,
                delayed_neow_curse.as_deref(),
            );
        }
        if *delayed_neow_transform_count > 0 {
            *deck_ids = visible_deck_ids.clone();
            let transformed_start = confirmed_deck_ids
                .len()
                .saturating_sub(*delayed_neow_transform_count);
            deck_ids.extend(confirmed_deck_ids[transformed_start..].iter().cloned());
            if let Some(curse) = delayed_neow_curse.take() {
                if !deck_ids.contains(&curse) {
                    deck_ids.push(curse);
                }
            }
            *delayed_neow_transform_count = 0;
        }
        let mut carried_confirmed = confirmed.clone();
        if transform_count_before_confirm > 0 {
            carried_confirmed.deck = deck_instances_from_keys(deck_ids);
            *neow_leave_visible_deck_ids = Some(visible_deck_ids.clone());
        }
        compare_subset(
            report,
            action,
            "Neow grid confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": neow_gold,
                "current_hp": neow_current_hp,
                "max_hp": neow_max_hp,
                "deck_ids": visible_deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(carried_confirmed);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowGridConfirm && action.command.eq_ignore_ascii_case("CONFIRM") {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow grid confirm without initialized run simulation"
                    .to_owned(),
            });
        };
        let Ok(next) = confirm_grid(sim) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow grid confirm simulation failed".to_owned(),
            });
        };
        let had_delayed_transform = *delayed_neow_transform_count > 0;
        *deck_ids = deck_content_keys(&next.deck);
        if *delayed_neow_transform_count > 0 {
            for _ in 0..(*delayed_neow_transform_count).min(deck_ids.len()) {
                deck_ids.pop();
            }
            if let Some(curse) = delayed_neow_curse.take() {
                deck_ids.push(curse);
            }
            *delayed_neow_transform_count = 0;
        }
        let mut carried_next = next.clone();
        if had_delayed_transform {
            carried_next.deck = deck_instances_from_keys(deck_ids);
        }
        if next.card_grid.is_some() {
            compare_subset(
                report,
                action,
                "Neow grid confirm",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&next),
            );
            *seed_sim = Some(carried_next);
            *phase = SeedStartPhase::NeowGrid;
            return SeedStartPreDispatch::Handled;
        }
        compare_subset(
            report,
            action,
            "Neow grid confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": neow_gold,
                "current_hp": neow_current_hp,
                "max_hp": neow_max_hp,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(carried_next);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowGridConfirm
        && command_choose_index(&action.command).is_some()
        && seed_sim
            .as_ref()
            .is_some_and(seed_start_is_neow_multi_select_grid)
    {
        let sim = seed_sim
            .as_ref()
            .expect("matched initialized Neow multi-select grid");
        let index = command_choose_index(&action.command).expect("matched choose command");
        let Ok(next) = select_grid_card(sim, index) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow multi-select grid choose simulation failed".to_owned(),
            });
        };
        if !seed_start_neow_grid_auto_confirms_after_choose(&next) {
            let simulated = if *delayed_neow_transform_count > 0 {
                let mut visible_deck_ids = deck_content_keys(&next.deck);
                if let Some(curse) = delayed_neow_curse.as_deref() {
                    visible_deck_ids.push(curse.to_owned());
                }
                seed_start_grid_simulated_subset_with_deck(&next, visible_deck_ids)
            } else {
                seed_start_grid_simulated_subset(&next)
            };
            compare_subset(
                report,
                action,
                "Neow grid select",
                seed_start_grid_observed_subset(&post.message),
                simulated,
            );
            *seed_sim = Some(next);
            *phase = SeedStartPhase::NeowGridConfirm;
            return SeedStartPreDispatch::Handled;
        }
        let delayed_transform_count_before_confirm = *delayed_neow_transform_count;
        let delayed_curse_before_confirm = delayed_neow_curse.clone();
        let Ok(confirmed) = confirm_grid(&next) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_grid_path".to_owned(),
                reason: "seed-start Neow multi-select grid auto-confirm failed".to_owned(),
            });
        };
        *delayed_neow_transform_count = 0;
        *delayed_neow_curse = None;
        *deck_ids = deck_content_keys(&confirmed.deck);
        let confirmed_deck_ids = deck_ids.clone();
        let transform_count_before_confirm = seed_start_neow_grid_transform_count(&next)
            .unwrap_or(delayed_transform_count_before_confirm);
        let mut visible_deck_ids = deck_ids.clone();
        if transform_count_before_confirm > 0 {
            visible_deck_ids = seed_start_visible_deck_after_neow_transform_selection(
                deck_ids,
                transform_count_before_confirm,
                delayed_curse_before_confirm.as_deref(),
            );
        }
        if delayed_transform_count_before_confirm > 0 {
            *deck_ids = visible_deck_ids.clone();
            let transformed_start = confirmed_deck_ids
                .len()
                .saturating_sub(delayed_transform_count_before_confirm);
            deck_ids.extend(confirmed_deck_ids[transformed_start..].iter().cloned());
            if let Some(curse) = delayed_curse_before_confirm {
                if !deck_ids.contains(&curse) {
                    deck_ids.push(curse);
                }
            }
        }
        let mut carried_confirmed = confirmed;
        if transform_count_before_confirm > 0 {
            carried_confirmed.deck = deck_instances_from_keys(deck_ids);
            *neow_leave_visible_deck_ids = Some(visible_deck_ids.clone());
        }
        compare_subset(
            report,
            action,
            "Neow grid confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": neow_gold,
                "current_hp": neow_current_hp,
                "max_hp": neow_max_hp,
                "deck_ids": visible_deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(carried_confirmed);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_boss_swap_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &mut Vec<String>,
    neow_gold: &mut i32,
    neow_current_hp: &mut i32,
    neow_max_hp: &mut i32,
    seed_sim: &mut Option<RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions {
        let Some(option) = seed_start_selected_neow_option(start.numeric_seed, &action.command)
        else {
            return SeedStartPreDispatch::NotHandled;
        };
        if !seed_start_neow_option_is_supported_boss_swap(option) {
            return SeedStartPreDispatch::NotHandled;
        }
        let run = seed_start_apply_neow_boss_swap(start.numeric_seed, deck_ids);
        if seed_start_boss_swap_is_calling_bell_grid(&run) {
            compare_subset(
                report,
                action,
                "Neow boss swap Calling Bell grid",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&run),
            );
            *seed_sim = Some(run);
            *phase = SeedStartPhase::NeowBossSwapCallingBellGrid;
            return SeedStartPreDispatch::Handled;
        }
        if seed_start_boss_swap_is_astrolabe_grid(&run) {
            compare_subset(
                report,
                action,
                "Neow boss swap Astrolabe grid",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&run),
            );
            *seed_sim = Some(run);
            *phase = SeedStartPhase::NeowBossSwapAstrolabeGrid;
            return SeedStartPreDispatch::Handled;
        }
        if seed_start_boss_swap_is_pandoras_box_grid(&run) {
            compare_subset(
                report,
                action,
                "Neow boss swap Pandora's Box grid",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&run),
            );
            *seed_sim = Some(run);
            *phase = SeedStartPhase::NeowBossSwapPandorasBoxGrid;
            return SeedStartPreDispatch::Handled;
        }
        if seed_start_boss_swap_is_empty_cage_grid(&run) {
            compare_subset(
                report,
                action,
                "Neow boss swap Empty Cage grid",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&run),
            );
            *seed_sim = Some(run);
            *phase = SeedStartPhase::NeowBossSwapEmptyCageGrid;
            return SeedStartPreDispatch::Handled;
        }
        if seed_start_boss_swap_is_tiny_house_reward(&run) {
            compare_subset(
                report,
                action,
                "Neow boss swap Tiny House reward",
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&run),
            );
            *deck_ids = deck_content_keys(&run.deck);
            *neow_gold = run.gold;
            *neow_current_hp = run.player_hp;
            *neow_max_hp = run.player_max_hp;
            *seed_sim = Some(run);
            *phase = SeedStartPhase::Reward;
            return SeedStartPreDispatch::Handled;
        }
        if let Some(reason) = seed_start_unsupported_boss_swap_reason(&run) {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason,
            });
        }
        let relic_ids = seed_start_boss_swap_relic_ids(&run);
        let post_deck_ids = deck_content_keys(&run.deck);
        compare_subset(
            report,
            action,
            "Neow boss swap",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": post_deck_ids,
                "relic_ids": relic_ids,
                "choices": ["leave"],
            }),
        );
        *deck_ids = post_deck_ids;
        *seed_sim = Some(run);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapCallingBellGrid
        && (action.command.eq_ignore_ascii_case("PROCEED")
            || action.command.eq_ignore_ascii_case("CONFIRM"))
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Calling Bell boss-swap grid without initialized run simulation"
                    .to_owned(),
            });
        };
        let Ok(next) = confirm_grid(sim) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Calling Bell boss-swap grid confirm failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&next.deck);
        compare_subset(
            report,
            action,
            "Neow boss swap Calling Bell rewards",
            seed_start_reward_observed_subset(&post.message),
            seed_start_reward_simulated_subset(&next),
        );
        *seed_sim = Some(next);
        *phase = SeedStartPhase::NeowBossSwapCallingBellReward;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapCallingBellReward
        && command_choose_index(&action.command).is_some()
    {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason:
                    "seed-start Calling Bell boss-swap reward without initialized run simulation"
                        .to_owned(),
            });
        };
        let label = match seed_start_apply_reward_choose(sim, &action.command) {
            Ok(label) => label,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_neow_boss_swap".to_owned(),
                    reason,
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            }
        };
        *deck_ids = deck_content_keys(&sim.deck);
        compare_subset(
            report,
            action,
            &label,
            seed_start_reward_observed_subset(&post.message),
            seed_start_reward_simulated_subset(sim),
        );
        if seed_start_reward_sequence_complete(sim) {
            *phase = SeedStartPhase::Reward;
        }
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapAstrolabeGrid
        && command_choose_index(&action.command).is_some()
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Astrolabe boss-swap grid without initialized run simulation"
                    .to_owned(),
            });
        };
        let index = command_choose_index(&action.command).expect("matched choose command");
        let Ok(next) = select_grid_card(sim, index) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Astrolabe boss-swap grid choose failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&next.deck);
        if let Ok(confirmed) = confirm_grid(&next) {
            *deck_ids = deck_content_keys(&confirmed.deck);
            if confirmed.card_grid.is_none() {
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Astrolabe transformed",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                        "choices": ["leave"],
                    }),
                );
                *seed_sim = Some(confirmed);
                *phase = SeedStartPhase::NeowLeave;
                return SeedStartPreDispatch::Handled;
            }
        }
        if next.card_grid.is_some() {
            compare_subset(
                report,
                action,
                "Neow boss swap Astrolabe grid select",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&next),
            );
            *seed_sim = Some(next);
            return SeedStartPreDispatch::Handled;
        }
        compare_subset(
            report,
            action,
            "Neow boss swap Astrolabe transformed",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(next);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapPandorasBoxGrid
        && (action.command.eq_ignore_ascii_case("PROCEED")
            || action.command.eq_ignore_ascii_case("CONFIRM"))
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason:
                    "seed-start Pandora's Box boss-swap grid without initialized run simulation"
                        .to_owned(),
            });
        };
        let Ok(next) = confirm_grid(sim) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Pandora's Box boss-swap grid confirm failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&next.deck);
        compare_subset(
            report,
            action,
            "Neow boss swap Pandora's Box confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(next);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapEmptyCageGrid
        && command_choose_index(&action.command).is_some()
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Empty Cage boss-swap grid without initialized run simulation"
                    .to_owned(),
            });
        };
        let index = command_choose_index(&action.command).expect("matched choose command");
        let Ok(next) = select_grid_card(sim, index) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Empty Cage boss-swap grid choose failed".to_owned(),
            });
        };
        compare_subset(
            report,
            action,
            "Neow boss swap Empty Cage grid select",
            seed_start_grid_observed_subset(&post.message),
            seed_start_grid_simulated_subset(&next),
        );
        *seed_sim = Some(next);
        return SeedStartPreDispatch::Handled;
    }

    if *phase == SeedStartPhase::NeowBossSwapEmptyCageGrid
        && action.command.eq_ignore_ascii_case("CONFIRM")
    {
        let Some(sim) = seed_sim.as_ref() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Empty Cage boss-swap grid without initialized run simulation"
                    .to_owned(),
            });
        };
        let Ok(next) = confirm_grid(sim) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Empty Cage boss-swap grid confirm failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&next.deck);
        if next.card_grid.is_some() {
            compare_subset(
                report,
                action,
                "Neow boss swap Empty Cage grid confirm",
                seed_start_grid_observed_subset(&post.message),
                seed_start_grid_simulated_subset(&next),
            );
            *seed_sim = Some(next);
            return SeedStartPreDispatch::Handled;
        }
        compare_subset(
            report,
            action,
            "Neow boss swap Empty Cage confirm",
            seed_start_observed_subset(&post.message),
            json!({
                "screen_type": "EVENT",
                "ascension": start.ascension,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                "choices": ["leave"],
            }),
        );
        *seed_sim = Some(next);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_leave_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    deck_ids: &[String],
    neow_gold: i32,
    neow_current_hp: i32,
    neow_max_hp: i32,
    neow_leave_visible_deck_ids: &mut Option<Vec<String>>,
    delayed_neow_curse: &mut Option<String>,
    pending_neow_room_entry_curse: &mut Option<String>,
    pending_neow_room_entry_curse_advances_card_rng: &mut bool,
    seed_sim: &mut Option<RunState>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::NeowLeave || !command_is_choose(&action.command, 0) {
        return SeedStartPreDispatch::NotHandled;
    }
    if let Some(curse) = delayed_neow_curse.take() {
        *pending_neow_room_entry_curse = Some(curse);
        *pending_neow_room_entry_curse_advances_card_rng = true;
    }
    let initialized_seed_sim = seed_sim.is_none();
    if seed_sim.is_none() {
        let mut run = seed_start_seeded_idle_run(start.numeric_seed, start.ascension, deck_ids);
        run.gold = neow_gold;
        run.player_hp = neow_current_hp;
        run.player_max_hp = neow_max_hp;
        *seed_sim = Some(run);
    }
    if let Some(sim) = seed_sim.as_mut() {
        sim.phase = RunPhase::Idle;
        sim.event = None;
        sim.reward = None;
        sim.card_grid = None;
        if initialized_seed_sim {
            sim.deck = deck_instances_from_keys(deck_ids);
        }
    }
    let lagged_visible_deck = neow_leave_visible_deck_ids.take();
    let pre_room_entry_deck = deck_ids.to_vec();
    let settled_deck = pending_neow_room_entry_curse
        .as_ref()
        .map(|curse| seed_start_deck_with_pending_neow_curse(&pre_room_entry_deck, curse))
        .unwrap_or_else(|| pre_room_entry_deck.clone());
    let mut transient_decks = Vec::new();
    if pending_neow_room_entry_curse.is_some() {
        transient_decks.push(pre_room_entry_deck.clone());
    }
    if let Some(lagged) = lagged_visible_deck {
        transient_decks.push(lagged.clone());
        if let Some(curse) = pending_neow_room_entry_curse.as_ref() {
            transient_decks.push(seed_start_deck_with_pending_neow_curse(&lagged, curse));
        }
    }
    transient_decks.retain(|deck| deck != &settled_deck);
    transient_decks.sort();
    transient_decks.dedup();

    let mut observed = seed_start_observed_subset(&post.message);
    let observed_deck = observed
        .as_object_mut()
        .and_then(|object| object.remove("deck_ids"))
        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
        .unwrap_or_default();
    let mut simulated = json!({
        "screen_type": "MAP",
        "ascension": start.ascension,
        "floor": 0,
        "gold": neow_gold,
        "current_hp": neow_current_hp,
        "max_hp": neow_max_hp,
        "deck_ids": settled_deck,
        "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
        "choices": seed_start_first_map_choices(&start.external_seed),
    });
    let simulated_deck = simulated
        .as_object_mut()
        .and_then(|object| object.remove("deck_ids"))
        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
        .expect("Neow leave projection contains a deck");
    let mut diffs = subset_diffs(observed, simulated);
    let deck_observation = if observed_deck == simulated_deck {
        PendingDeckObservation::Settled
    } else if transient_decks.iter().any(|deck| deck == &observed_deck) {
        PendingDeckObservation::Deferred
    } else {
        PendingDeckObservation::Diverged(subset_diffs(json!(observed_deck), json!(simulated_deck)))
    };
    match deck_observation {
        PendingDeckObservation::Settled if diffs.is_empty() => {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow leave".to_owned(),
            });
        }
        PendingDeckObservation::Deferred if diffs.is_empty() => {
            *pending_deck_assertion = Some(PendingDeckAssertion {
                action: action.clone(),
                label: "Neow leave".to_owned(),
                transient_decks,
                expected_deck: simulated_deck,
            });
        }
        PendingDeckObservation::Diverged(deck_diffs) => {
            diffs.extend(deck_diffs);
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow leave".to_owned(),
                diffs,
            });
        }
        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow leave".to_owned(),
                diffs,
            });
        }
    }
    *phase = SeedStartPhase::Map;
    SeedStartPreDispatch::Handled
}

pub(super) fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    start: &StartRunCommand,
    report: &mut SimRealReport,
    boss_unlocks: BossUnlockState,
) -> SeedStartVerification {
    let mut phase = SeedStartPhase::BeforeStart;
    let mut _reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut normal_combat_index = 0usize;
    let mut event_room_index = 0usize;
    let mut map_path_xs: Vec<i32> = Vec::new();
    let mut neow_gold = 99;
    let mut neow_current_hp = 80;
    let mut neow_max_hp = 80;
    let mut neow_card_reward_option: Option<GeneratedNeowOption> = None;
    let mut neow_card_reward_choices: Option<Vec<String>> = None;
    let mut neow_card_reward_card_rng_counter: Option<u32> = None;
    let mut neow_leave_visible_deck_ids: Option<Vec<String>> = None;
    let mut neow_potions_taken = 0usize;
    let mut delayed_neow_curse: Option<String> = None;
    let mut pending_neow_room_entry_curse: Option<String> = None;
    let mut pending_neow_room_entry_curse_advances_card_rng = false;
    let mut delayed_neow_transform_count = 0usize;
    let mut deck_ids = ironclad_starter_deck_keys();
    let mut seed_sim: Option<RunState> = None;
    let mut smoke_bomb_ui: Option<SmokeBombUiState> = None;
    let mut pending_deck_assertion: Option<PendingDeckAssertion> = None;
    let mut pending_map_assertion: Option<PendingMapAssertion> = None;
    let mut pending_boss_relic_overlay: Option<PendingBossRelicOverlayAssertion> = None;
    let mut pending_combat_assertion: Option<PendingCombatAssertion> = None;
    let mut reconciled_deferred_action_steps = Vec::new();

    macro_rules! finish_boundary {
        ($boundary:expr) => {{
            let mut unresolved_deferred_action_steps = Vec::new();
            if let Some(pending) = pending_deck_assertion.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
            }
            if let Some(pending) = pending_map_assertion.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
            }
            if let Some(pending) = pending_boss_relic_overlay.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
            }
            if let Some(pending) = pending_combat_assertion.as_ref() {
                unresolved_deferred_action_steps.extend(
                    pending
                        .transitions
                        .iter()
                        .map(|transition| transition.action.step),
                );
            }
            if let Some(pending) = smoke_bomb_ui.as_ref() {
                match pending {
                    SmokeBombUiState::Escaping { action, .. } => {
                        unresolved_deferred_action_steps.push(action.step);
                    }
                    SmokeBombUiState::Reward { pending_proceeds } => {
                        unresolved_deferred_action_steps
                            .extend(pending_proceeds.iter().map(|action| action.step));
                    }
                }
            }
            unresolved_deferred_action_steps.sort_unstable();
            unresolved_deferred_action_steps.dedup();
            seed_start_finish_boundary(
                &seed_sim,
                $boundary,
                start.numeric_seed,
                boss_unlocks,
                reconciled_deferred_action_steps,
                unresolved_deferred_action_steps,
            )
        }};
    }

    macro_rules! require_map_projection {
        ($run:expr, $action:expr, $category:expr) => {{
            match seed_start_simulated_map_return($run) {
                Ok(projection) => projection,
                Err(reason) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", $action.step),
                        category: $category.to_owned(),
                        reason,
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: $action.step,
                        command: $action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
            }
        }};
    }

    for (pre, action, post) in transitions {
        if let Some(pending) = pending_deck_assertion.take() {
            if is_trace_observation_poll(action) {
                let observed_deck = seed_start_observed_deck(&post.message);
                match classify_deferred_deck_reconciliation(
                    &observed_deck,
                    &pending.transient_decks,
                    &pending.expected_deck,
                ) {
                    PendingDeckObservation::Settled => {
                        report.verified.push(VerifiedTransition {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                        });
                        reconciled_deferred_action_steps.push(pending.action.step);
                    }
                    PendingDeckObservation::Deferred => {
                        pending_deck_assertion = Some(pending);
                    }
                    PendingDeckObservation::Diverged(diffs) => {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                            diffs,
                        });
                    }
                }
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "deferred deck observation poll".to_owned(),
                });
                continue;
            } else {
                let observed_deck = seed_start_observed_deck(&pre.message);
                match classify_deferred_deck_reconciliation(
                    &observed_deck,
                    &pending.transient_decks,
                    &pending.expected_deck,
                ) {
                    PendingDeckObservation::Settled => {
                        report.verified.push(VerifiedTransition {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                        });
                        reconciled_deferred_action_steps.push(pending.action.step);
                    }
                    PendingDeckObservation::Deferred => {
                        let observed_post_deck = seed_start_observed_deck(&post.message);
                        match classify_deferred_deck_reconciliation(
                            &observed_post_deck,
                            &pending.transient_decks,
                            &pending.expected_deck,
                        ) {
                            PendingDeckObservation::Settled => {
                                report.verified.push(VerifiedTransition {
                                    action_step: pending.action.step,
                                    command: pending.action.command,
                                    label: pending.label,
                                });
                                reconciled_deferred_action_steps.push(pending.action.step);
                            }
                            PendingDeckObservation::Deferred => {
                                report.unexpected_diffs.push(UnexpectedDiff {
                                    action_step: pending.action.step,
                                    command: pending.action.command,
                                    label: pending.label,
                                    diffs: subset_diffs(
                                        json!(observed_post_deck),
                                        json!(pending.expected_deck),
                                    ),
                                });
                            }
                            PendingDeckObservation::Diverged(diffs) => {
                                report.unexpected_diffs.push(UnexpectedDiff {
                                    action_step: pending.action.step,
                                    command: pending.action.command,
                                    label: pending.label,
                                    diffs,
                                });
                            }
                        }
                    }
                    PendingDeckObservation::Diverged(diffs) => {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                            diffs,
                        });
                    }
                }
            }
        }
        if let Some(sim) = seed_sim.as_mut() {
            seed_start_apply_boss_unlocks(sim, start.numeric_seed, boss_unlocks);
        }
        // Target event eligibility can depend on CardCrawlGame.playtime (Secret
        // Portal). This non-seeded clock is recorded as an explicit transition
        // input; deterministic gameplay state is never hydrated from observations.
        if let (Some(sim), Some(playtime_seconds)) = (
            seed_sim.as_mut(),
            recorded_action_playtime_seconds(pre, action),
        ) {
            sim.playtime_seconds = playtime_seconds;
        }
        if pending_combat_assertion
            .as_ref()
            .is_some_and(|pending| pending.requires_stable_frame_before_next_command)
            && !is_trace_observation_poll(action)
        {
            let sim = seed_sim
                .as_ref()
                .expect("pending combat assertion keeps authoritative simulator state");
            let observed = seed_start_combat_observed_subset(&pre.message);
            let simulated = seed_start_simulated_combat_subset(sim, false);
            if seed_start_combat_subsets_match(observed, simulated) {
                let pending = pending_combat_assertion
                    .take()
                    .expect("pending combat assertion checked above");
                for transition in pending.transitions {
                    if transition.transient_matches {
                        report.verified.push(VerifiedTransition {
                            action_step: transition.action.step,
                            command: transition.action.command,
                            label: transition.label,
                        });
                        reconciled_deferred_action_steps.push(transition.action.step);
                    }
                }
            } else {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unreconciled_copied_attack_frame".to_owned(),
                    reason: "a new command arrived before the queued copied attack reached the captured pre-state".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
        }
        if pending_map_assertion.is_some() {
            if screen_type(&pre.message) == Some("MAP") {
                let pending = pending_map_assertion
                    .take()
                    .expect("pending map assertion checked above");
                let stable_matches =
                    seed_start_compare_pending_map_assertion(report, &pending, &pre.message);
                if pending.transient_matches && stable_matches {
                    report.verified.push(VerifiedTransition {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                    });
                    reconciled_deferred_action_steps.push(pending.action.step);
                }
                phase = SeedStartPhase::Map;
            } else if is_trace_observation_poll(action) {
                if screen_type(&post.message) == Some("MAP") {
                    let pending = pending_map_assertion
                        .take()
                        .expect("pending map assertion checked above");
                    let stable_matches =
                        seed_start_compare_pending_map_assertion(report, &pending, &post.message);
                    if pending.transient_matches && stable_matches {
                        report.verified.push(VerifiedTransition {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                        });
                        reconciled_deferred_action_steps.push(pending.action.step);
                    }
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "stable next-act map observation poll".to_owned(),
                    });
                    phase = SeedStartPhase::Map;
                    continue;
                }
                if seed_start_is_candidate_boss_act_transient_frame(&post.message) {
                    let pending = pending_map_assertion
                        .as_mut()
                        .expect("pending map assertion checked above");
                    pending.transient_matches &= seed_start_compare_deferred_subset(
                        report,
                        &pending.action,
                        "transient boss-act frame",
                        seed_start_boss_act_transient_observed_subset(&post.message),
                        seed_start_boss_act_transient_simulated_subset(),
                    );
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "transient boss-act observation poll".to_owned(),
                    });
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_pending_map_transition".to_owned(),
                    reason: format!(
                        "next-act map assertion reached unsupported poll screen {:?}",
                        screen_type(&post.message)
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            } else {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unresolved_pending_map_transition".to_owned(),
                    reason: "next-act map assertion did not reach a stable map before another semantic command".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
        }
        if action.command.eq_ignore_ascii_case("state")
            || smoke_bomb_ui.is_some() && action.command.eq_ignore_ascii_case("wait")
        {
            if pending_combat_assertion.is_some() {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_pending_combat_transition".to_owned(),
                        reason: "pending combat assertion lost its authoritative run state"
                            .to_owned(),
                    });
                };
                seed_start_compare_or_defer_combat_transition(
                    report,
                    action,
                    "combat observation poll",
                    &post.message,
                    seed_start_combat_observed_subset(&post.message),
                    seed_start_simulated_combat_subset(sim, false),
                    &mut pending_combat_assertion,
                    &mut reconciled_deferred_action_steps,
                );
                continue;
            }
            if let Some(pending) = pending_boss_relic_overlay.as_ref() {
                if seed_start_is_boss_relic_master_deck_overlay(&post.message) {
                    compare_subset(
                        report,
                        action,
                        "boss relic deck overlay observation poll",
                        seed_start_treasure_observed_subset(&post.message),
                        pending.simulated_overlay.clone(),
                    );
                    continue;
                }
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_boss_reward_overlay".to_owned(),
                        reason: "boss relic overlay poll lost its authoritative simulator state"
                            .to_owned(),
                    });
                };
                let diff_count = report.unexpected_diffs.len();
                compare_subset(
                    report,
                    action,
                    "boss relic overlay settled observation poll",
                    seed_start_treasure_observed_subset(&post.message),
                    seed_start_treasure_simulated_subset(sim),
                );
                let stable_matches = report.unexpected_diffs.len() == diff_count;
                let pending = pending_boss_relic_overlay
                    .take()
                    .expect("pending overlay checked above");
                seed_start_reconcile_boss_relic_overlay(
                    report,
                    pending,
                    stable_matches,
                    action.step,
                    &mut reconciled_deferred_action_steps,
                );
                continue;
            }
            if let Some(SmokeBombUiState::Escaping {
                source,
                action: escape_action,
                transient_matches,
            }) = smoke_bomb_ui.as_mut()
            {
                if screen_type(&post.message) == Some("NONE")
                    && post.message.pointer("/game_state/combat_state").is_some()
                {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb escape keeps its core destination");
                    *transient_matches &= seed_start_compare_deferred_combat_subset(
                        report,
                        escape_action,
                        "Smoke Bomb transient combat frame",
                        seed_start_smoke_bomb_transient_observed_subset(&post.message),
                        seed_start_smoke_bomb_transient_simulated_subset(source, destination),
                    );
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Smoke Bomb transient observation poll".to_owned(),
                    });
                    continue;
                }
                if screen_type(&post.message) == Some("COMBAT_REWARD") {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb escape keeps its core destination");
                    let stable_matches = seed_start_compare_deferred_subset(
                        report,
                        escape_action,
                        "Smoke Bomb escape settled to empty reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(destination),
                    );
                    if *transient_matches && stable_matches {
                        report.verified.push(VerifiedTransition {
                            action_step: escape_action.step,
                            command: escape_action.command.clone(),
                            label: "Smoke Bomb escape reconciled at empty reward".to_owned(),
                        });
                        reconciled_deferred_action_steps.push(escape_action.step);
                    }
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Smoke Bomb stable reward observation poll".to_owned(),
                    });
                    phase = SeedStartPhase::Reward;
                    smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                        pending_proceeds: Vec::new(),
                    });
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_smoke_bomb_ui_transition".to_owned(),
                    reason: format!(
                        "Smoke Bomb escape poll reached unsupported screen {:?}",
                        screen_type(&post.message)
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            if let Some(SmokeBombUiState::Reward { pending_proceeds }) = smoke_bomb_ui.as_ref() {
                if screen_type(&post.message) == Some("COMBAT_REWARD") {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb reward keeps its core destination");
                    compare_subset(
                        report,
                        action,
                        "Smoke Bomb empty reward observation poll",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(destination),
                    );
                    continue;
                }
                if !pending_proceeds.is_empty() && screen_type(&post.message) == Some("MAP") {
                    let pending_proceeds = pending_proceeds.clone();
                    let diff_count = report.unexpected_diffs.len();
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut pending_map_assertion,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    if report.unexpected_diffs.len() == diff_count {
                        for pending in pending_proceeds {
                            report.verified.push(VerifiedTransition {
                                action_step: pending.step,
                                command: pending.command,
                                label: "Smoke Bomb reward proceed reconciled at map".to_owned(),
                            });
                            reconciled_deferred_action_steps.push(pending.step);
                        }
                        smoke_bomb_ui = None;
                    }
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_smoke_bomb_ui_transition".to_owned(),
                    reason: format!(
                        "Smoke Bomb reward poll reached unsupported screen {:?}",
                        screen_type(&post.message)
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "trace client poll".to_owned(),
            });
            continue;
        }
        match seed_start_handle_overlay_command(
            pre,
            action,
            post,
            &mut phase,
            seed_sim.as_ref(),
            &mut pending_boss_relic_overlay,
            &mut reconciled_deferred_action_steps,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_bootstrap_phase(
            action,
            post,
            start,
            boss_unlocks,
            &deck_ids,
            seed_sim.as_ref(),
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_transform_phase(
            action,
            post,
            start,
            &mut deck_ids,
            &mut neow_leave_visible_deck_ids,
            seed_sim.as_ref(),
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_immediate_phase(
            action,
            post,
            start,
            &mut deck_ids,
            &mut pending_neow_room_entry_curse,
            &mut pending_neow_room_entry_curse_advances_card_rng,
            &mut neow_gold,
            &mut neow_current_hp,
            &mut neow_max_hp,
            &mut seed_sim,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_card_reward_phase(
            pre,
            action,
            post,
            start,
            &mut deck_ids,
            &mut neow_gold,
            &mut neow_current_hp,
            &mut neow_max_hp,
            &mut neow_card_reward_option,
            &mut neow_card_reward_choices,
            &mut neow_card_reward_card_rng_counter,
            &mut neow_leave_visible_deck_ids,
            &mut delayed_neow_curse,
            &mut seed_sim,
            &mut pending_deck_assertion,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_potion_reward_phase(
            action,
            post,
            start,
            &mut deck_ids,
            neow_gold,
            neow_current_hp,
            neow_max_hp,
            &mut neow_potions_taken,
            &mut seed_sim,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_grid_phase(
            action,
            post,
            start,
            &mut deck_ids,
            &mut neow_gold,
            &mut neow_current_hp,
            &mut neow_max_hp,
            &mut neow_leave_visible_deck_ids,
            &mut delayed_neow_curse,
            &mut delayed_neow_transform_count,
            &mut seed_sim,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_boss_swap_phase(
            action,
            post,
            start,
            &mut deck_ids,
            &mut neow_gold,
            &mut neow_current_hp,
            &mut neow_max_hp,
            &mut seed_sim,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_neow_leave_phase(
            action,
            post,
            start,
            &deck_ids,
            neow_gold,
            neow_current_hp,
            neow_max_hp,
            &mut neow_leave_visible_deck_ids,
            &mut delayed_neow_curse,
            &mut pending_neow_room_entry_curse,
            &mut pending_neow_room_entry_curse_advances_card_rng,
            &mut seed_sim,
            &mut pending_deck_assertion,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match phase {
            SeedStartPhase::Map
                if screen_type(&pre.message) == Some("MAP")
                    && command_choose_index(&action.command).is_some() =>
            {
                if let Some(sim) = seed_sim.as_ref() {
                    let mut transition_base = sim.clone();
                    seed_start_apply_boss_unlocks(
                        &mut transition_base,
                        start.numeric_seed,
                        boss_unlocks,
                    );
                    if let Some(curse) = pending_neow_room_entry_curse.take() {
                        let next_deck_ids = seed_start_deck_with_pending_neow_curse(
                            &deck_content_keys(&transition_base.deck),
                            &curse,
                        );
                        if pending_neow_room_entry_curse_advances_card_rng {
                            transition_base.card_rng_counter =
                                transition_base.card_rng_counter.saturating_add(1);
                        }
                        pending_neow_room_entry_curse_advances_card_rng = false;
                        transition_base.deck = deck_instances_from_keys(&next_deck_ids);
                    }
                    let legal_actions = match legal_map_decisions(&transition_base) {
                        Ok(actions) => actions,
                        Err(error) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_map_state".to_owned(),
                                reason: format!(
                                    "core legal-action boundary rejected map state: {error}"
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if let Some(choice_index) = choose_index(&action.command) {
                        if let Some(map_action) = legal_actions.get(choice_index).copied() {
                            let choice_x = transition_base.map.as_ref().and_then(|map_state| {
                                let node_id = match map_action {
                                    sts_core::MapAction::ChooseNode { node_id } => node_id,
                                };
                                map_state.map.node(node_id).map(|node| {
                                    let (x, _) = seed_start_map_node_xy(node.id);
                                    x
                                })
                            });
                            let Some(choice_x) = choice_x else {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "invalid_map_state".to_owned(),
                                    reason: "legal core map action references a missing node"
                                        .to_owned(),
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            };
                            map_path_xs.push(choice_x);
                            let Ok(next) = apply_map_action_on_run(&transition_base, map_action)
                            else {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_map_path".to_owned(),
                                    reason: "core map simulation rejected transition".to_owned(),
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            };
                            match next.phase {
                                RunPhase::Event => {
                                    let label = format!("map event node {}", event_room_index + 1);
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_event_observed_subset(&post.message),
                                        seed_start_event_simulated_subset(&next),
                                    );
                                    event_room_index += 1;
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Event;
                                }
                                RunPhase::Combat => {
                                    let label = seed_start_map_label(normal_combat_index);
                                    let observed =
                                        seed_start_encounter_observed_subset(&post.message);
                                    let simulated = seed_start_simulated_map_combat_subset(
                                        &next,
                                        normal_combat_index,
                                    );
                                    seed_start_compare_or_defer_combat_entry(
                                        report,
                                        action,
                                        &label,
                                        &post.message,
                                        observed,
                                        simulated,
                                        &mut pending_combat_assertion,
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Combat;
                                    normal_combat_index += 1;
                                }
                                RunPhase::Rest => {
                                    let label = format!("map rest node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_rest_observed_subset(&post.message),
                                        seed_start_rest_simulated_subset(&next),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Rest;
                                }
                                RunPhase::Treasure => {
                                    let label = format!("map treasure node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_treasure_observed_subset(&post.message),
                                        seed_start_treasure_simulated_subset(&next),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Treasure;
                                }
                                RunPhase::Shop => {
                                    let label = format!("map shop node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_shop_observed_subset(&post.message),
                                        seed_start_shop_room_simulated_subset(&next),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Shop;
                                }
                                RunPhase::Idle => {
                                    seed_start_compare_map_return(
                                        report,
                                        action,
                                        &post.message,
                                        require_map_projection!(
                                            &next,
                                            action,
                                            "invalid_map_projection"
                                        ),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Map;
                                }
                                RunPhase::Reward => {
                                    compare_subset(
                                        report,
                                        action,
                                        "map reward",
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(&next),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Reward;
                                }
                                RunPhase::Complete => {
                                    let boundary = SeedStartBoundary {
                                        path: format!("$.actions[step={}].command", action.step),
                                        category: "unsupported_map_path".to_owned(),
                                        reason: "map choice unexpectedly completed the run"
                                            .to_owned(),
                                    };
                                    report.unsupported.push(UnsupportedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        reason: boundary.reason.clone(),
                                    });
                                    return finish_boundary!(boundary);
                                }
                            }
                            continue;
                        }
                    }
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_path".to_owned(),
                    reason: "strict seed-start map transition could not be simulated; verifier refused to infer simulator state from the observed trace".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Treasure if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let simulated_return = {
                    let Some(sim) = seed_sim.as_mut() else {
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: "seed-start treasure action without initialized run simulation"
                                .to_owned(),
                        });
                    };
                    let previous_act = sim.current_act;
                    let next = apply_run_action(sim, RunAction::Proceed).map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    if next.current_act != previous_act {
                        map_path_xs.clear();
                        combat_index = 0;
                        normal_combat_index = 0;
                    }
                    let mut simulated_return =
                        require_map_projection!(&next, action, "invalid_treasure_map_projection");
                    if next.current_act != previous_act && previous_act != 1 {
                        seed_start_project_post_boss_transition_current_node(&mut simulated_return);
                    }
                    let act_changed = next.current_act != previous_act;
                    if next.phase != RunPhase::Idle || !act_changed {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_treasure_destination".to_owned(),
                            reason: format!(
                                "treasure proceed produced phase {:?} and act transition {} -> {}",
                                next.phase, previous_act, next.current_act
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    *sim = next;
                    simulated_return
                };
                if seed_start_is_candidate_boss_act_transient_frame(&post.message) {
                    let label = "boss chest proceed to settled next-act map";
                    let transient_matches = seed_start_compare_deferred_subset(
                        report,
                        action,
                        label,
                        seed_start_boss_act_transient_observed_subset(&post.message),
                        seed_start_boss_act_transient_simulated_subset(),
                    );
                    pending_map_assertion = Some(PendingMapAssertion {
                        action: action.clone(),
                        label: label.to_owned(),
                        simulated_map: simulated_return,
                        transient_matches,
                    });
                    phase = SeedStartPhase::Proceed;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "boss chest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    simulated_return,
                );
                phase = SeedStartPhase::Map;
                continue;
            }
            SeedStartPhase::Treasure if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: "seed-start treasure action without initialized run simulation"
                            .to_owned(),
                    });
                };
                if choose_index != 0 {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: format!("treasure chest choice {choose_index} is not available"),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                let next =
                    apply_run_action(sim, RunAction::OpenChest).map_err(|error| error.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let reward = next.reward.as_ref();
                let boss_reward = next.phase == RunPhase::Reward
                    && next.boss_chest_opened
                    && reward.is_some_and(|reward| !reward.boss_relic_choices.is_empty());
                let ordinary_reward = next.phase == RunPhase::Reward
                    && !next.boss_chest_opened
                    && reward.is_some_and(|reward| reward.boss_relic_choices.is_empty());
                if boss_reward {
                    compare_subset(
                        report,
                        action,
                        "open boss relic chest",
                        seed_start_boss_reward_observed_subset(&post.message),
                        seed_start_boss_reward_simulated_subset(&next),
                    );
                    phase = SeedStartPhase::BossReward;
                } else if ordinary_reward {
                    compare_subset(
                        report,
                        action,
                        "open treasure chest",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&next),
                    );
                    phase = SeedStartPhase::Reward;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_treasure_destination".to_owned(),
                        reason: format!(
                            "open chest produced inconsistent simulator destination: phase={:?}, boss_chest_opened={}, reward={}, boss_choices={}",
                            next.phase,
                            next.boss_chest_opened,
                            reward.is_some(),
                            reward.map_or(0, |reward| reward.boss_relic_choices.len()),
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                *sim = next;
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("SKIP") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest skip without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_run_action(sim, RunAction::CloseCardReward)
                    .map_err(|error| error.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if next.phase != RunPhase::Rest || next.reward.is_some() {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_rest_reward_continuation".to_owned(),
                        reason: format!(
                            "rest reward skip produced phase {:?} with reward_present={}",
                            next.phase,
                            next.reward.is_some()
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                compare_subset(
                    report,
                    action,
                    "rest skip card reward",
                    seed_start_rest_observed_subset(&post.message),
                    seed_start_rest_simulated_subset(&next),
                );
                *sim = next;
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest proceed without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_rest_action(sim, RestAction::Proceed).map_err(|e| e.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                compare_subset(
                    report,
                    action,
                    "rest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    require_map_projection!(&next, action, "invalid_rest_map_projection"),
                );
                *sim = next;
                phase = SeedStartPhase::Map;
                continue;
            }
            SeedStartPhase::Rest if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = if screen_type(&pre.message) == Some("REST") {
                    seed_start_rest_screen_actions(sim)
                        .map_err(|error| error.to_string())
                        .and_then(|actions| {
                            actions
                                .get(choose_index)
                                .copied()
                                .ok_or_else(|| "unsupported rest choice".to_owned())
                        })
                        .and_then(|action| {
                            apply_rest_action(sim, action).map_err(|e| e.to_string())
                        })
                } else if screen_type(&pre.message) == Some("CARD_REWARD") {
                    let card_id = reward_card_id_from_choose(sim, choose_index)
                        .ok_or_else(|| "bad rest card reward choose".to_owned());
                    match card_id {
                        Ok(card_id) => apply_run_action(sim, RunAction::TakeCardReward { card_id })
                            .map_err(|e| e.to_string()),
                        Err(reason) => Err(reason),
                    }
                } else {
                    Err("unsupported rest choice".to_owned())
                };
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let (observed, simulated, label) = if next.card_grid.is_some() {
                    (
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next),
                        "rest grid",
                    )
                } else {
                    match next.phase {
                        RunPhase::Reward
                            if next
                                .reward
                                .as_ref()
                                .is_some_and(RewardScreen::card_reward_is_active) =>
                        {
                            (
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next),
                                "rest card reward",
                            )
                        }
                        RunPhase::Rest if next.reward.is_none() => (
                            seed_start_rest_observed_subset(&post.message),
                            seed_start_rest_simulated_subset(&next),
                            "rest choice",
                        ),
                        phase => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_rest_destination".to_owned(),
                                reason: format!(
                                    "rest choice produced unsupported simulator phase {phase:?}"
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                };
                compare_subset(report, action, label, observed, simulated);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim
                    .reward
                    .as_ref()
                    .is_some_and(RewardScreen::card_reward_is_active)
                {
                    phase = SeedStartPhase::Reward;
                } else if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Event if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .ok_or_else(|| format!("bad event choose {}", action.command));
                let Ok(choose_index) = choose_index else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: choose_index.err().unwrap(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "seed-start event action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let Some(sim_choice_index) = seed_start_event_choice_index_for_communication_mod(
                    sim,
                    choose_index,
                    &pre.message,
                ) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: format!(
                            "event simulation could not map visible choice index {choose_index}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let delayed_event_deck_append_count = sim.event.as_ref().and_then(|screen| {
                    (screen.event == Event::Vampires
                        && screen.stage == 0
                        && sim_choice_index < screen.choices.len().saturating_sub(1))
                    .then_some(VAMPIRES_BITE_COUNT)
                });
                let spire_heart_stage = sim
                    .event
                    .as_ref()
                    .filter(|screen| screen.event == Event::SpireHeart)
                    .map(|screen| screen.stage);
                let Ok(next) = apply_event_action(
                    sim,
                    EventAction::Choose {
                        choice_index: sim_choice_index,
                    },
                ) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "event simulation rejected transition".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(spire_heart_stage) = spire_heart_stage {
                    if spire_heart_stage == 3 {
                        compare_subset(
                            report,
                            action,
                            "Spire Heart completion",
                            seed_start_game_over_observed_subset(&post.message),
                            seed_start_game_over_simulated_subset(&next),
                        );
                    } else {
                        compare_subset(
                            report,
                            action,
                            "Spire Heart choice",
                            seed_start_event_observed_subset(&post.message),
                            seed_start_event_simulated_subset(&next),
                        );
                    }
                    phase = if next.phase == RunPhase::Complete {
                        SeedStartPhase::Complete
                    } else {
                        SeedStartPhase::Event
                    };
                    *sim = next;
                    continue;
                }
                if next.phase == RunPhase::Combat {
                    if next.combat.is_none() {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_event_destination".to_owned(),
                            reason: "event choice entered combat phase without combat state"
                                .to_owned(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    let label = "event combat";
                    let observed = seed_start_encounter_observed_subset(&post.message);
                    let simulated = seed_start_simulated_combat_subset(&next, false);
                    seed_start_compare_or_defer_combat_entry(
                        report,
                        action,
                        label,
                        &post.message,
                        observed,
                        simulated,
                        &mut pending_combat_assertion,
                    );
                    *sim = next;
                    phase = SeedStartPhase::Combat;
                    continue;
                }
                let (mut observed, mut simulated) = if next.card_grid.is_some() {
                    (
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next),
                    )
                } else {
                    match next.phase {
                        RunPhase::Idle if next.event.is_none() => (
                            seed_start_map_return_observed_subset(&post.message),
                            require_map_projection!(&next, action, "invalid_event_map_projection"),
                        ),
                        RunPhase::Reward if next.reward.is_some() => (
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next),
                        ),
                        RunPhase::Event if next.event.is_some() => (
                            seed_start_event_observed_subset(&post.message),
                            seed_start_event_simulated_subset_with_delayed_deck_append(
                                &next,
                                delayed_event_deck_append_count,
                            ),
                        ),
                        _ => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_event_destination".to_owned(),
                                reason: format!(
                                    "event choice produced unsupported simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                };
                if next.phase == RunPhase::Event && !next.pending_obtain_cards.is_empty() {
                    let observed_deck = observed
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let simulated_deck = simulated
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let non_deck_diffs = subset_diffs(observed, simulated);
                    if !non_deck_diffs.is_empty() {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "event choice".to_owned(),
                            diffs: non_deck_diffs,
                        });
                    } else {
                        let expected_deck =
                            deck_content_keys_after_pending_obtain_cards_settle(&next);
                        match classify_deferred_deck_observation(
                            &observed_deck,
                            &simulated_deck,
                            &expected_deck,
                        ) {
                            PendingDeckObservation::Settled => {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: "event choice".to_owned(),
                                });
                            }
                            PendingDeckObservation::Deferred => {
                                pending_deck_assertion = Some(PendingDeckAssertion {
                                    action: action.clone(),
                                    label: "event choice".to_owned(),
                                    transient_decks: vec![simulated_deck],
                                    expected_deck,
                                });
                            }
                            PendingDeckObservation::Diverged(diffs) => {
                                report.unexpected_diffs.push(UnexpectedDiff {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: "event choice".to_owned(),
                                    diffs,
                                });
                            }
                        }
                    }
                } else {
                    compare_subset(report, action, "event choice", observed, simulated);
                }
                *sim = next.clone();
                if next.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if next.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Map;
                } else if next.phase == RunPhase::Reward {
                    phase = SeedStartPhase::Reward;
                }
            }
            SeedStartPhase::Map => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_action".to_owned(),
                    reason: "seed-start verifier saw a map command that was not a visible generated map choice".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Combat => {
                let command = action.command.trim();
                let command_head = command.split_whitespace().next().unwrap_or("");
                let is_play_command = command_head.eq_ignore_ascii_case("PLAY");
                let combat_decision = match seed_sim
                    .as_ref()
                    .map(seed_start_active_combat_decision)
                    .transpose()
                {
                    Ok(decision) => decision.flatten(),
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_combat_decision_state".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                };
                let potion_use = parse_potion_use(command);
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat action without initialized combat simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };

                if let Some(decision) = combat_decision {
                    if command.eq_ignore_ascii_case("WAIT") {
                        seed_start_compare_or_defer_combat_transition(
                            report,
                            action,
                            "combat decision refresh",
                            &post.message,
                            seed_start_combat_observed_subset(&post.message),
                            seed_start_simulated_combat_subset(sim, false),
                            &mut pending_combat_assertion,
                            &mut reconciled_deferred_action_steps,
                        );
                        continue;
                    }
                    let (decision_action, label) =
                        match seed_start_bind_combat_decision_command(decision, command) {
                            Ok(bound) => bound,
                            Err(reason) => {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_combat_decision_command".to_owned(),
                                    reason,
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            }
                        };
                    let next = apply_run_action(sim, decision_action);
                    let Ok(next) = next else {
                        push_sim_error(report, action, label, next.err().unwrap());
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!("seed-start {label} simulation failed"),
                        });
                    };
                    seed_start_compare_or_defer_combat_transition(
                        report,
                        action,
                        label,
                        &post.message,
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, false),
                        &mut pending_combat_assertion,
                        &mut reconciled_deferred_action_steps,
                    );
                    *sim = next;
                    continue;
                }

                if let Some(potion_use) = potion_use {
                    let is_smoke_bomb =
                        sim.potion_at_slot(potion_use.slot) == Some(Potion::SmokeBomb);
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    );
                    let Ok(next) = next else {
                        push_sim_error(report, action, "combat potion use", next.err().unwrap());
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat potion simulation failed".to_owned(),
                        });
                    };
                    if is_smoke_bomb {
                        if next.phase != RunPhase::Idle
                            || next.combat.is_some()
                            || next.reward.is_some()
                        {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_smoke_bomb_core_destination".to_owned(),
                                reason: format!(
                                    "Smoke Bomb core transition produced phase {:?}, combat={}, reward={}",
                                    next.phase,
                                    next.combat.is_some(),
                                    next.reward.is_some(),
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        if screen_type(&post.message) == Some("NONE")
                            && post.message.pointer("/game_state/combat_state").is_some()
                        {
                            let source = sim.clone();
                            let transient_matches = seed_start_compare_deferred_combat_subset(
                                report,
                                action,
                                "Smoke Bomb escape queued",
                                seed_start_smoke_bomb_transient_observed_subset(&post.message),
                                seed_start_smoke_bomb_transient_simulated_subset(&source, &next),
                            );
                            *sim = next;
                            smoke_bomb_ui = Some(SmokeBombUiState::Escaping {
                                source: Box::new(source),
                                action: action.clone(),
                                transient_matches,
                            });
                            continue;
                        }
                        if screen_type(&post.message) == Some("COMBAT_REWARD") {
                            compare_subset(
                                report,
                                action,
                                "Smoke Bomb escape settled to empty reward",
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next),
                            );
                            *sim = next;
                            phase = SeedStartPhase::Reward;
                            smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                                pending_proceeds: Vec::new(),
                            });
                            continue;
                        }
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_smoke_bomb_ui_transition".to_owned(),
                            reason: format!(
                                "Smoke Bomb command reached unsupported screen {:?}",
                                screen_type(&post.message)
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    if seed_start_run_has_combat_card_reward(&next) {
                        seed_start_compare_or_defer_combat_transition(
                            report,
                            action,
                            "combat potion card reward",
                            &post.message,
                            seed_start_combat_observed_subset(&post.message),
                            seed_start_simulated_combat_subset(&next, false),
                            &mut pending_combat_assertion,
                            &mut reconciled_deferred_action_steps,
                        );
                        *sim = next;
                        continue;
                    }
                    if next.phase == RunPhase::Reward && next.reward.is_some() {
                        compare_subset(
                            report,
                            action,
                            "reward-screen potion use",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Reward;
                        continue;
                    }
                    if next.phase != RunPhase::Combat || next.combat.is_none() {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_combat_potion_destination".to_owned(),
                            reason: format!(
                                "combat potion produced phase {:?}, combat={}, reward={}",
                                next.phase,
                                next.combat.is_some(),
                                next.reward.is_some(),
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    seed_start_compare_or_defer_combat_transition(
                        report,
                        action,
                        "combat potion use",
                        &post.message,
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, false),
                        &mut pending_combat_assertion,
                        &mut reconciled_deferred_action_steps,
                    );
                    *sim = next;
                    continue;
                }

                if command.eq_ignore_ascii_case("PROCEED")
                    && sim
                        .combat
                        .as_ref()
                        .is_some_and(|combat| combat.phase == CombatPhase::Lost)
                {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "death screen proceed".to_owned(),
                    });
                    continue;
                }

                if !(is_play_command || command_head.eq_ignore_ascii_case("END")) {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier does not support combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }

                let Some(combat) = sim.combat.as_ref() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_simulator_state".to_owned(),
                        reason:
                            "seed-start verifier entered its combat phase without core combat state"
                                .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(reason) = unsupported_seed_start_combat_command(combat, command) {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason,
                    });
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "unsupported card in seed-start combat".to_owned(),
                    });
                }

                let Some(combat_action) = combat_action_from_command(command, combat) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier could not parse combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };

                if is_final_combat_blow(sim, combat_action) {
                    let next = apply_combat_action_on_run(sim, combat_action);
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "seed-start combat victory",
                            next.err().unwrap(),
                        );
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat victory simulation failed".to_owned(),
                        });
                    };
                    let label = combat_label(command, sim);
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_victory_observed_subset(&post.message),
                        seed_start_victory_simulated_subset(&next),
                    );
                    let final_boss_complete = seed_start_is_final_boss_victory(&next);
                    seed_sim = Some(next);
                    phase = if final_boss_complete {
                        SeedStartPhase::Proceed
                    } else {
                        SeedStartPhase::Reward
                    };
                    continue;
                }

                let next = apply_combat_action_on_run(sim, combat_action);
                let Ok(next) = next else {
                    push_sim_error(
                        report,
                        action,
                        "seed-start combat transition",
                        next.err().unwrap(),
                    );
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat simulation rejected transition".to_owned(),
                    });
                };
                let label = combat_label(command, sim);
                let observed = seed_start_combat_observed_subset(&post.message);
                let simulated = seed_start_simulated_combat_subset(&next, false);
                let copied_attack = seed_start_copied_attack_expectation(combat, combat_action);
                let stable_projection_matches =
                    seed_start_combat_subsets_match(observed.clone(), simulated.clone());
                if seed_start_classify_copied_attack_frame(
                    stable_projection_matches,
                    copied_attack,
                    &post.message,
                ) == CopiedAttackFrame::Deferred
                {
                    let transient_matches = seed_start_compare_transient_combat_subset(
                        report, action, &label, observed, simulated,
                    );
                    let pending = pending_combat_assertion.get_or_insert_default();
                    pending.requires_stable_frame_before_next_command = true;
                    pending.transitions.push(PendingCombatTransition {
                        action: action.clone(),
                        label,
                        transient_matches,
                    });
                    *sim = next;
                    continue;
                }
                seed_start_compare_or_defer_combat_transition(
                    report,
                    action,
                    &label,
                    &post.message,
                    observed,
                    simulated,
                    &mut pending_combat_assertion,
                    &mut reconciled_deferred_action_steps,
                );
                *sim = next;
            }
            SeedStartPhase::Reward => {
                if matches!(smoke_bomb_ui, Some(SmokeBombUiState::Reward { .. }))
                    && action.command.eq_ignore_ascii_case("PROCEED")
                {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb reward keeps its core destination");
                    if screen_type(&post.message) == Some("COMBAT_REWARD") {
                        if seed_start_compare_deferred_subset(
                            report,
                            action,
                            "Smoke Bomb reward proceed awaiting map",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(destination),
                        ) {
                            let Some(SmokeBombUiState::Reward { pending_proceeds }) =
                                smoke_bomb_ui.as_mut()
                            else {
                                unreachable!("Smoke Bomb reward state checked above");
                            };
                            pending_proceeds.push(action.clone());
                        }
                        continue;
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        let pending_proceeds = match smoke_bomb_ui.as_ref() {
                            Some(SmokeBombUiState::Reward { pending_proceeds }) => {
                                pending_proceeds.clone()
                            }
                            _ => unreachable!("Smoke Bomb reward state checked above"),
                        };
                        let diff_count = report.unexpected_diffs.len();
                        if let Some(boundary) = seed_start_handle_proceed_to_map(
                            report,
                            action,
                            &post.message,
                            &mut phase,
                            &mut combat_index,
                            &mut _reward_step,
                            &mut map_path_xs,
                            &mut seed_sim,
                            &mut pending_map_assertion,
                        ) {
                            return finish_boundary!(boundary);
                        }
                        if report.unexpected_diffs.len() == diff_count {
                            for pending in pending_proceeds {
                                report.verified.push(VerifiedTransition {
                                    action_step: pending.step,
                                    command: pending.command,
                                    label: "Smoke Bomb reward proceed reconciled at map".to_owned(),
                                });
                                reconciled_deferred_action_steps.push(pending.step);
                            }
                            smoke_bomb_ui = None;
                        }
                        continue;
                    }
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_smoke_bomb_ui_transition".to_owned(),
                        reason: format!(
                            "Smoke Bomb reward proceed reached unsupported screen {:?}",
                            screen_type(&post.message)
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                if action.command.trim().eq_ignore_ascii_case("SKIP") {
                    let Some(sim) = seed_sim.as_mut() else {
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: "seed-start reward skip without initialized reward simulation"
                                .to_owned(),
                        });
                    };
                    let next = apply_run_action(sim, RunAction::CloseCardReward)
                        .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let (label, observed, simulated) = if next.card_grid.is_some() {
                        (
                            "skip card reward to grid",
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next),
                        )
                    } else {
                        match next.phase {
                            RunPhase::Reward if next.reward.is_some() => (
                                "skip combat card reward",
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next),
                            ),
                            RunPhase::Rest if next.reward.is_none() => (
                                "skip rest card reward",
                                seed_start_rest_observed_subset(&post.message),
                                seed_start_rest_simulated_subset(&next),
                            ),
                            RunPhase::Event if next.event.is_some() => (
                                "skip event card reward",
                                seed_start_event_observed_subset(&post.message),
                                seed_start_event_simulated_subset(&next),
                            ),
                            RunPhase::Shop if next.shop.is_some() => (
                                "skip shop card reward",
                                seed_start_shop_observed_subset(&post.message),
                                seed_start_shop_screen_simulated_subset(&next),
                            ),
                            phase => {
                                let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_reward_destination".to_owned(),
                                reason: format!(
                                    "card reward skip produced unsupported simulator phase {phase:?}"
                                ),
                            };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            }
                        }
                    };
                    compare_subset(report, action, label, observed, simulated);
                    *sim = next;
                    if seed_start_reward_sequence_complete(sim) {
                        phase = seed_start_phase_after_reward_completion(sim);
                    }
                    continue;
                }
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_final_boss_victory)
                    {
                        let sim = seed_sim
                            .as_mut()
                            .expect("final boss simulation checked above");
                        let next = seed_start_apply_final_boss_proceed(sim);
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        compare_subset(
                            report,
                            action,
                            "final boss proceed to Spire Heart",
                            seed_start_spire_heart_observed_subset(&post.message),
                            seed_start_spire_heart_simulated_subset(&next),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Event;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_boss_chest_proceed)
                    {
                        let Some(sim) = seed_sim.as_mut() else {
                            return finish_boundary!(SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            });
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        if next.phase != RunPhase::Treasure {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: format!(
                                    "boss combat proceed ended in simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(|sim| sim.phase == RunPhase::Reward && sim.event.is_some())
                    {
                        let sim = seed_sim.as_mut().expect("reward simulation checked above");
                        let reward_action = if seed_start_reward_sequence_complete(sim) {
                            RunAction::Proceed
                        } else {
                            RunAction::SkipReward
                        };
                        let next =
                            apply_run_action(sim, reward_action).map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        let (label, expected, next_phase) = if next.phase == RunPhase::Idle {
                            (
                                "empty Neow reward proceed to map",
                                json!({
                                    "screen_type": "MAP",
                                    "ascension": start.ascension,
                                    "floor": 0,
                                    "gold": next.gold,
                                    "current_hp": next.player_hp,
                                    "max_hp": next.player_max_hp,
                                    "deck_ids": deck_content_keys(&next.deck),
                                    "relic_ids": relic_ids_for_simulated_subset(&next),
                                    "choices": seed_start_first_map_choices(&start.external_seed),
                                }),
                                SeedStartPhase::Map,
                            )
                        } else {
                            (
                                "empty reward proceed to event",
                                json!({
                                    "screen_type": "EVENT",
                                    "ascension": start.ascension,
                                    "floor": next.current_floor,
                                    "gold": next.gold,
                                    "current_hp": next.player_hp,
                                    "max_hp": next.player_max_hp,
                                    "deck_ids": deck_content_keys(&next.deck),
                                    "relic_ids": relic_ids_for_simulated_subset(&next),
                                    "choices": ["leave"],
                                }),
                                SeedStartPhase::Event,
                            )
                        };
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_observed_subset(&post.message),
                            expected,
                        );
                        *sim = next;
                        phase = next_phase;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut pending_map_assertion,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    continue;
                }
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_reward_path".to_owned(),
                        reason: "seed-start reward action without initialized reward simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(potion_use) = parse_potion_use(&action.command) {
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    )
                    .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    *sim = next;
                    compare_subset(
                        report,
                        action,
                        "reward-screen potion use",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(sim),
                    );
                    continue;
                }

                let deck_before_reward_choice = deck_content_keys(&sim.deck);
                match seed_start_apply_reward_choose(sim, &action.command) {
                    Ok(label) => {
                        let (mut observed, mut simulated) = if sim.card_grid.is_some() {
                            (
                                seed_start_grid_observed_subset(&post.message),
                                seed_start_grid_simulated_subset(sim),
                            )
                        } else {
                            match sim.phase {
                                RunPhase::Reward if sim.reward.is_some() => (
                                    seed_start_reward_observed_subset(&post.message),
                                    seed_start_reward_simulated_subset(sim),
                                ),
                                RunPhase::Rest if sim.reward.is_none() => (
                                    seed_start_rest_observed_subset(&post.message),
                                    seed_start_rest_simulated_subset(sim),
                                ),
                                RunPhase::Event
                                    if sim.event.as_ref().is_some_and(|event| {
                                        event.event == Event::Neow && event.stage == 2
                                    }) =>
                                {
                                    // Neow's completed continuation is authoritative core
                                    // state, but the command-facing frame remains the empty
                                    // reward screen until PROCEED leaves the room.
                                    (
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(sim),
                                    )
                                }
                                RunPhase::Event if sim.event.is_some() => (
                                    seed_start_event_observed_subset(&post.message),
                                    seed_start_event_simulated_subset(sim),
                                ),
                                RunPhase::Shop if sim.shop.is_some() => (
                                    seed_start_shop_observed_subset(&post.message),
                                    seed_start_shop_screen_simulated_subset(sim),
                                ),
                                RunPhase::Treasure if sim.reward.is_none() => (
                                    seed_start_reward_observed_subset(&post.message),
                                    seed_start_reward_simulated_subset(sim),
                                ),
                                phase => {
                                    let boundary = SeedStartBoundary {
                                        path: format!(
                                            "$.actions[step={}].command",
                                            action.step
                                        ),
                                        category: "invalid_reward_destination".to_owned(),
                                        reason: format!(
                                            "reward choice produced unsupported simulator phase {phase:?}"
                                        ),
                                    };
                                    report.unsupported.push(UnsupportedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        reason: boundary.reason.clone(),
                                    });
                                    return finish_boundary!(boundary);
                                }
                            }
                        };
                        if label.starts_with("card reward pick ") && sim.phase == RunPhase::Reward {
                            let observed_deck = observed
                                .as_object_mut()
                                .and_then(|object| object.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            let simulated_deck = simulated
                                .as_object_mut()
                                .and_then(|object| object.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            let mut diffs = subset_diffs(observed, simulated);
                            let deck_observation = classify_deferred_deck_observation(
                                &observed_deck,
                                &deck_before_reward_choice,
                                &simulated_deck,
                            );
                            match deck_observation {
                                PendingDeckObservation::Settled if diffs.is_empty() => {
                                    report.verified.push(VerifiedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                    });
                                }
                                PendingDeckObservation::Deferred if diffs.is_empty() => {
                                    pending_deck_assertion = Some(PendingDeckAssertion {
                                        action: action.clone(),
                                        label: label.clone(),
                                        transient_decks: vec![deck_before_reward_choice],
                                        expected_deck: simulated_deck,
                                    });
                                }
                                PendingDeckObservation::Diverged(deck_diffs) => {
                                    diffs.extend(deck_diffs);
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                        diffs,
                                    });
                                }
                                PendingDeckObservation::Settled
                                | PendingDeckObservation::Deferred => {
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                        diffs,
                                    });
                                }
                            }
                        } else {
                            compare_subset(report, action, &label, observed, simulated);
                        }
                        _reward_step += 1;
                        if sim.card_grid.is_some() {
                            phase = SeedStartPhase::Grid;
                        } else if seed_start_reward_sequence_complete(sim) {
                            phase = seed_start_phase_after_reward_completion(sim);
                        }
                    }
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                }
            }
            SeedStartPhase::BossReward if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    });
                };
                if screen_type(&pre.message) == Some("BOSS_REWARD") {
                    let next = apply_run_action(
                        sim,
                        RunAction::ChooseBossRelicReward {
                            index: choose_index,
                        },
                    )
                    .map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_boss_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let visible_relics_before_pick = relic_ids_for_simulated_subset(sim);
                    let opened_master_deck_overlay =
                        seed_start_is_boss_relic_master_deck_overlay(&post.message);
                    if next.card_grid.is_some() {
                        compare_subset(
                            report,
                            action,
                            "boss relic reward grid",
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next),
                        );
                    } else if opened_master_deck_overlay {
                        let simulated_overlay = seed_start_boss_relic_deck_overlay_simulated_subset(
                            &next,
                            &visible_relics_before_pick,
                        );
                        let transient_matches = seed_start_compare_deferred_subset(
                            report,
                            action,
                            "boss relic reward deck overlay",
                            seed_start_treasure_observed_subset(&post.message),
                            simulated_overlay.clone(),
                        );
                        pending_boss_relic_overlay = Some(PendingBossRelicOverlayAssertion {
                            action: action.clone(),
                            simulated_overlay,
                            transient_matches,
                        });
                    } else {
                        compare_subset(
                            report,
                            action,
                            "boss relic reward",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next),
                        );
                    }
                    *sim = next;
                    phase = if sim.card_grid.is_some() {
                        SeedStartPhase::Grid
                    } else {
                        SeedStartPhase::Treasure
                    };
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "unsupported boss relic reward choice".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
            }
            SeedStartPhase::BossReward if action.command.trim().eq_ignore_ascii_case("SKIP") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_run_action(sim, RunAction::SkipReward).map_err(|e| e.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                compare_subset(
                    report,
                    action,
                    "boss relic reward skip",
                    seed_start_treasure_observed_subset(&post.message),
                    seed_start_treasure_simulated_subset(&next),
                );
                *sim = next;
                phase = SeedStartPhase::Treasure;
            }
            SeedStartPhase::Grid => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start grid action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let command = action.command.trim();
                let delayed_event_deck_append_count = (command_head_eq(command, "CHOOSE")
                    || command.eq_ignore_ascii_case("CONFIRM"))
                .then(|| {
                    sim.card_grid.as_ref().and_then(|grid| match grid.purpose {
                        GridPurpose::EventTransform { count }
                        | GridPurpose::EventTransformReturnToEvent { count, .. } => {
                            Some(usize::from(count))
                        }
                        GridPurpose::EventObtainCard
                        | GridPurpose::EventObtainCardReturnToEvent { .. } => Some(1),
                        _ => None,
                    })
                })
                .flatten();
                let next = seed_start_apply_grid_command(sim, command);
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let destination = match seed_start_grid_destination(&next) {
                    Ok(destination) => destination,
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_grid_destination".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                };
                let (label, mut observed, mut simulated, next_phase) = match destination {
                    SeedStartGridDestination::Grid => (
                        "grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next),
                        SeedStartPhase::Grid,
                    ),
                    SeedStartGridDestination::Shop => (
                        "shop grid",
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_screen_simulated_subset(&next),
                        SeedStartPhase::Shop,
                    ),
                    SeedStartGridDestination::Event => (
                        "event grid",
                        seed_start_event_observed_subset(&post.message),
                        seed_start_event_simulated_subset_with_delayed_deck_append(
                            &next,
                            delayed_event_deck_append_count,
                        ),
                        SeedStartPhase::Event,
                    ),
                    SeedStartGridDestination::Rest => (
                        "rest grid",
                        seed_start_rest_observed_subset(&post.message),
                        seed_start_rest_simulated_subset(&next),
                        SeedStartPhase::Rest,
                    ),
                    SeedStartGridDestination::Reward => (
                        "grid",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&next),
                        if seed_start_reward_sequence_complete(&next) {
                            seed_start_phase_after_reward_completion(&next)
                        } else {
                            SeedStartPhase::Reward
                        },
                    ),
                    SeedStartGridDestination::Treasure => (
                        "boss relic grid confirm",
                        seed_start_treasure_observed_subset(&post.message),
                        seed_start_treasure_simulated_subset(&next),
                        SeedStartPhase::Treasure,
                    ),
                    SeedStartGridDestination::Proceed => (
                        "grid proceed",
                        seed_start_observed_subset(&post.message),
                        seed_start_proceed_simulated_subset(&next),
                        SeedStartPhase::Proceed,
                    ),
                };
                if destination == SeedStartGridDestination::Event
                    && delayed_event_deck_append_count.is_some()
                    && next.card_grid.is_none()
                {
                    let observed_deck = observed
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let simulated_deck = simulated
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let mut diffs = subset_diffs(observed, simulated);
                    let expected_deck = deck_content_keys(&next.deck);
                    match classify_deferred_deck_observation(
                        &observed_deck,
                        &simulated_deck,
                        &expected_deck,
                    ) {
                        PendingDeckObservation::Settled if diffs.is_empty() => {
                            report.verified.push(VerifiedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                            });
                        }
                        PendingDeckObservation::Deferred if diffs.is_empty() => {
                            pending_deck_assertion = Some(PendingDeckAssertion {
                                action: action.clone(),
                                label: label.to_owned(),
                                transient_decks: vec![simulated_deck],
                                expected_deck,
                            });
                        }
                        PendingDeckObservation::Diverged(deck_diffs) => {
                            diffs.extend(deck_diffs);
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                                diffs,
                            });
                        }
                        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                                diffs,
                            });
                        }
                    }
                } else {
                    compare_subset(report, action, label, observed, simulated);
                }
                *sim = next;
                phase = next_phase;
            }
            SeedStartPhase::Shop => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_shop_path".to_owned(),
                        reason: "seed-start shop action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let command = action.command.trim();
                if command.eq_ignore_ascii_case("LEAVE") {
                    let next = match apply_run_action(sim, RunAction::LeaveShop) {
                        Ok(next) => next,
                        Err(err) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason: format!("core rejected shop merchant leave: {err}"),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if seed_start_shop_destination(&next) != Ok(SeedStartShopDestination::Room) {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_shop_destination".to_owned(),
                            reason: seed_start_shop_destination(&next).err().unwrap_or_else(|| {
                                "shop merchant leave did not reach the shop room".to_owned()
                            }),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    compare_subset(
                        report,
                        action,
                        "leave shop merchant",
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_room_simulated_subset(&next),
                    );
                    *sim = next;
                    continue;
                }
                if command.eq_ignore_ascii_case("PROCEED") {
                    let next = match apply_run_action(sim, RunAction::Proceed) {
                        Ok(next) => next,
                        Err(err) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason: format!("core rejected shop room proceed: {err}"),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if seed_start_shop_destination(&next) != Ok(SeedStartShopDestination::Map) {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_shop_destination".to_owned(),
                            reason: seed_start_shop_destination(&next).err().unwrap_or_else(|| {
                                "shop room proceed did not reach the map".to_owned()
                            }),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    compare_subset(
                        report,
                        action,
                        "leave shop room",
                        seed_start_map_return_observed_subset(&post.message),
                        require_map_projection!(&next, action, "invalid_shop_map_projection"),
                    );
                    *sim = next;
                    phase = SeedStartPhase::Map;
                    continue;
                }
                if command_head_eq(command, "CHOOSE") {
                    let choose_index = choose_index(command)
                        .expect("malformed CHOOSE rejected before phase dispatch");
                    let (shop_action, label) = match seed_start_bind_shop_choose(sim, choose_index)
                    {
                        Ok(bound) => bound,
                        Err(reason) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason,
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    let next = apply_run_action(sim, shop_action).map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_shop_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let destination = match seed_start_shop_destination(&next) {
                        Ok(destination) => destination,
                        Err(reason) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_shop_destination".to_owned(),
                                reason,
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if screen_type(&post.message) == Some("NONE") {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "trace_client_shop_transient".to_owned(),
                            reason: format!(
                                "shop {shop_action:?} reached a transient NONE frame before its core-owned {destination:?} destination became observable"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    match destination {
                        SeedStartShopDestination::Screen => {
                            let mut observed = seed_start_shop_observed_subset(&post.message);
                            let mut simulated = seed_start_shop_screen_simulated_subset(&next);
                            let observed_deck = observed
                                .as_object_mut()
                                .and_then(|fields| fields.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            if let Some(fields) = simulated.as_object_mut() {
                                fields.remove("deck_ids");
                            }
                            let mut diffs = subset_diffs(observed, simulated);
                            let transient_deck = deck_content_keys(&sim.deck);
                            let expected_deck = deck_content_keys(&next.deck);
                            match classify_deferred_deck_observation(
                                &observed_deck,
                                &transient_deck,
                                &expected_deck,
                            ) {
                                PendingDeckObservation::Settled if diffs.is_empty() => {
                                    report.verified.push(VerifiedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                    });
                                }
                                PendingDeckObservation::Deferred if diffs.is_empty() => {
                                    pending_deck_assertion = Some(PendingDeckAssertion {
                                        action: action.clone(),
                                        label: label.to_owned(),
                                        transient_decks: vec![transient_deck],
                                        expected_deck,
                                    });
                                }
                                PendingDeckObservation::Diverged(deck_diffs) => {
                                    diffs.extend(deck_diffs);
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                        diffs,
                                    });
                                }
                                PendingDeckObservation::Settled
                                | PendingDeckObservation::Deferred => {
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                        diffs,
                                    });
                                }
                            }
                        }
                        SeedStartShopDestination::Grid => compare_subset(
                            report,
                            action,
                            label,
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next),
                        ),
                        SeedStartShopDestination::Reward => compare_subset(
                            report,
                            action,
                            label,
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next),
                        ),
                        destination => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_shop_destination".to_owned(),
                                reason: format!(
                                    "shop CHOOSE {choose_index} produced unsupported destination {destination:?}"
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                    *sim = next;
                    phase = match destination {
                        SeedStartShopDestination::Grid => SeedStartPhase::Grid,
                        SeedStartShopDestination::Reward => SeedStartPhase::Reward,
                        SeedStartShopDestination::Screen => SeedStartPhase::Shop,
                        _ => unreachable!("shop CHOOSE destination checked above"),
                    };
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_shop_path".to_owned(),
                    reason: format!(
                        "seed-start verifier does not support shop command {command:?}"
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Proceed => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_final_boss_victory)
                    {
                        let sim = seed_sim
                            .as_mut()
                            .expect("final boss simulation checked above");
                        let next = seed_start_apply_final_boss_proceed(sim);
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        compare_subset(
                            report,
                            action,
                            "final boss proceed to Spire Heart",
                            seed_start_spire_heart_observed_subset(&post.message),
                            seed_start_spire_heart_simulated_subset(&next),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Event;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_boss_chest_proceed)
                    {
                        let Some(sim) = seed_sim.as_mut() else {
                            return finish_boundary!(SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            });
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        if next.phase != RunPhase::Treasure {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: format!(
                                    "boss combat proceed ended in simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut pending_map_assertion,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    continue;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: "seed-start verifier expected reward-to-map PROCEED command"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
            }
            SeedStartPhase::Complete if action.command.eq_ignore_ascii_case("PROCEED") => {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_complete_path".to_owned(),
                        reason: "terminal proceed without initialized run simulation".to_owned(),
                    });
                };
                if sim.phase != RunPhase::Complete
                    || !sim
                        .event
                        .as_ref()
                        .is_some_and(|event| event.event == Event::SpireHeart && event.stage == 4)
                {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_complete_path".to_owned(),
                        reason: "terminal proceed requires completed Spire Heart state".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                compare_subset(
                    report,
                    action,
                    "leave completed run",
                    json!({
                        "in_game": post.message.get("in_game").and_then(Value::as_bool),
                    }),
                    json!({ "in_game": false }),
                );
                continue;
            }
            _ => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unexpected_seed_start_command".to_owned(),
                    reason: format!(
                        "seed-start bootstrap harness did not expect command '{}' in phase {:?}",
                        action.command, phase
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
        }
    }

    finish_boundary!(SeedStartBoundary {
        path: "$.actions[verified]".to_owned(),
        category: "none".to_owned(),
        reason: "seed-start verifier checked every verifiable transition in the trace".to_owned(),
    })
}
