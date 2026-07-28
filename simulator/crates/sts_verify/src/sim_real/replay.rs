use super::*;

fn record_replay_checkpoint(
    capture: &mut Option<&mut ReplayCapture>,
    action: Option<TraceAction>,
    state: Option<&RunState>,
) {
    let Some(capture) = capture.as_deref_mut() else {
        return;
    };
    let Some(action) = action else {
        return;
    };

    let snapshot = state.map(|state| Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        state: state.clone(),
    });
    let state_hash = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.hash().ok())
        .map(|hash| hash.to_string());

    if capture
        .requested_step
        .is_some_and(|requested_step| action.step <= requested_step)
    {
        if let Some(snapshot) = snapshot {
            capture.selected_checkpoint = Some(ReplayCheckpointState {
                action_step: action.step,
                command: action.command.clone(),
                snapshot,
            });
        }
    }

    capture.checkpoints.push(ReplayCheckpoint {
        action_step: action.step,
        command: action.command,
        state_hash,
    });
}

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
    if *phase == SeedStartPhase::BeforeStart && start.matches_command(&action.command) {
        let settling = post.message.get("in_game").and_then(Value::as_bool) == Some(false)
            || post.message.get("game_state").is_none()
            || post
                .message
                .get("ready_for_command")
                .and_then(Value::as_bool)
                == Some(false);
        if settling {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "seed-start command accepted awaiting initialization".to_owned(),
            });
            *phase = SeedStartPhase::BootstrapSettling;
            return SeedStartPreDispatch::Handled;
        }
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
                "current_hp": start.starting_hp(),
                "max_hp": start.starting_hp(),
                "deck_ids": deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": seed_start_neow_choices_with_max_hp(
                    start.numeric_seed,
                    start.starting_hp(),
                ),
            }),
        );
        *phase = SeedStartPhase::NeowOptions;
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
    let Some(option) = seed_start_selected_neow_option_with_max_hp(
        start.numeric_seed,
        start.starting_hp(),
        &action.command,
    ) else {
        return SeedStartPreDispatch::NotHandled;
    };

    if let Some((gold, current_hp, max_hp)) =
        seed_start_apply_neow_simple_option_with_hp(option.clone(), start.starting_hp())
    {
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
        let mut run = seed_start_carried_run_with_hp(
            seed_sim.as_ref(),
            start.numeric_seed,
            start.ascension,
            &start.external_seed,
            deck_ids,
            start.starting_hp(),
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

    if seed_start_neow_option_is_supported_curse_simple(option.clone()) {
        let mut run = seed_start_apply_neow_curse_simple_visible_option_with_hp(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            option.clone(),
            start.starting_hp(),
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
        let visible_deck_ids = deck_ids.clone();
        let settled_deck_ids = deck_content_keys(&curse_run.deck);
        *neow_gold = run.gold;
        *neow_current_hp = run.player_hp;
        *neow_max_hp = run.player_max_hp;
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
            "deck_ids": visible_deck_ids,
            "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
            "choices": ["leave"],
        });
        let simulated_deck = simulated
            .as_object_mut()
            .and_then(|object| object.remove("deck_ids"))
            .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
            .expect("Neow immediate reward projection contains a deck");
        let diffs = subset_diffs(observed, simulated);
        match classify_deferred_deck_observation(&observed_deck, &simulated_deck, &settled_deck_ids)
        {
            PendingDeckObservation::Settled if diffs.is_empty() => {
                *pending_neow_room_entry_curse = None;
                *deck_ids = settled_deck_ids;
                *seed_sim = Some(curse_run);
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Neow curse immediate reward".to_owned(),
                });
            }
            PendingDeckObservation::Deferred if diffs.is_empty() => {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Neow curse immediate reward".to_owned(),
                });
            }
            PendingDeckObservation::Diverged(deck_diffs) => {
                let mut diffs = diffs;
                diffs.extend(deck_diffs);
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Neow curse immediate reward".to_owned(),
                    diffs,
                });
            }
            PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Neow curse immediate reward".to_owned(),
                    diffs,
                });
            }
        }
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    if seed_start_neow_option_is_supported_relic_reward(option.clone()) {
        let mut run = seed_start_apply_neow_relic_reward_for_ascension_with_hp(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            &option,
            start.starting_hp(),
        );
        *neow_gold = run.gold;
        *neow_current_hp = run.player_hp;
        *neow_max_hp = run.player_max_hp;
        let label = seed_start_neow_relic_reward_label(option.reward);
        // Curse + relic CommMod captures can show either frame on leave-ready:
        // transient (relic already, curse still pending) or settled (Shame etc.
        // already on master deck). Match the curse-simple dual-frame model.
        if option.drawback == NeowDrawback::Curse {
            let settled_run = run.clone();
            let settled_deck_ids = deck_content_keys(&settled_run.deck);
            let mut visible_deck_ids = settled_deck_ids.clone();
            let curse = visible_deck_ids
                .pop()
                .expect("curse drawback appends a curse to the deck");
            *pending_neow_room_entry_curse = Some(curse);
            *pending_neow_room_entry_curse_advances_card_rng = false;
            run.deck = deck_instances_from_keys(&visible_deck_ids);
            *deck_ids = visible_deck_ids.clone();
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
                "deck_ids": visible_deck_ids,
                "relic_ids": relic_ids_for_simulated_subset(seed_sim.as_ref().expect("seed sim")),
                "choices": ["leave"],
            });
            let simulated_deck = simulated
                .as_object_mut()
                .and_then(|object| object.remove("deck_ids"))
                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                .expect("Neow relic reward projection contains a deck");
            let diffs = subset_diffs(observed, simulated);
            match classify_deferred_deck_observation(
                &observed_deck,
                &simulated_deck,
                &settled_deck_ids,
            ) {
                PendingDeckObservation::Settled if diffs.is_empty() => {
                    *pending_neow_room_entry_curse = None;
                    *deck_ids = settled_deck_ids;
                    *seed_sim = Some(settled_run);
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: label.to_owned(),
                    });
                }
                PendingDeckObservation::Deferred if diffs.is_empty() => {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: label.to_owned(),
                    });
                }
                PendingDeckObservation::Diverged(deck_diffs) => {
                    let mut diffs = diffs;
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
            *deck_ids = deck_content_keys(&run.deck);
            compare_subset(
                report,
                action,
                label,
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
            *seed_sim = Some(run);
        }
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_card_reward_phase(
    _pre: &TraceState,
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
    seed_sim: &mut Option<RunState>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    pending_neow_alternate_settled_deck: &mut Option<Vec<String>>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase == SeedStartPhase::NeowOptions {
        let Some(option) = seed_start_selected_neow_option_with_max_hp(
            start.numeric_seed,
            start.starting_hp(),
            &action.command,
        ) else {
            return SeedStartPreDispatch::NotHandled;
        };

        if option.reward == NeowRewardType::OneRandomRareCard {
            let mut run = seed_start_apply_neow_reward_drawback_for_ascension_with_hp(
                start.numeric_seed,
                start.ascension,
                deck_ids,
                &option,
                start.starting_hp(),
            );
            *deck_ids = deck_content_keys(&run.deck);
            let transient_deck = deck_ids.clone();
            *neow_gold = run.gold;
            *neow_current_hp = run.player_hp;
            *neow_max_hp = run.player_max_hp;
            let reward = generate_neow_card_reward(start.numeric_seed, option.reward)
                .expect("matched generated Neow card reward option");
            for content_id in reward.cards {
                run.gain_deck_card(content_id)
                    .expect("canonical seed-start deck has card ID allocation headroom");
            }
            *deck_ids = deck_content_keys(&run.deck);
            *seed_sim = Some(run);
            *neow_leave_visible_deck_ids = None;
            let label = seed_start_neow_card_reward_label(option.reward);
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
            let settled_deck = simulated
                .as_object_mut()
                .and_then(|object| object.remove("deck_ids"))
                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                .expect("Neow random rare projection contains a deck");
            let mut diffs = subset_diffs(observed, simulated);
            match classify_deferred_deck_observation(&observed_deck, &transient_deck, &settled_deck)
            {
                PendingDeckObservation::Settled if diffs.is_empty() => {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: label.to_owned(),
                    });
                }
                PendingDeckObservation::Deferred if diffs.is_empty() => {
                    *pending_deck_assertion = Some(PendingDeckAssertion {
                        action: action.clone(),
                        label: label.to_owned(),
                        related_actions: Vec::new(),
                        transient_decks: vec![transient_deck],
                        expected_deck: settled_deck,
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
            *phase = SeedStartPhase::NeowLeave;
            return SeedStartPreDispatch::Handled;
        }

        if seed_start_neow_option_is_supported_card_reward(option.clone()) {
            let mut run = seed_start_apply_neow_reward_drawback_for_ascension_with_hp(
                start.numeric_seed,
                start.ascension,
                deck_ids,
                &option,
                start.starting_hp(),
            );
            *deck_ids = deck_content_keys(&run.deck);
            let transient_reward_deck = deck_ids.clone();
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
                let reward_card_rng_counter = match option.reward {
                    NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
                        generate_neow_colorless_reward(start.numeric_seed, option.reward)
                            .expect("matched generated Neow colorless reward option")
                            .card_rng_counter
                    }
                    _ => 0,
                };
                let curse =
                    seed_start_neow_curse_deck_key(start.numeric_seed, reward_card_rng_counter)
                        .expect("matched Neow curse drawback has a modeled curse");
                deck_ids.push(curse);
                run.deck = deck_instances_from_keys(deck_ids);
                let settled_card_rng_counter = reward_card_rng_counter
                    .checked_add(1)
                    .expect("floor-zero Neow card RNG counter has headroom");
                run.card_rng_counter = settled_card_rng_counter;
                *neow_card_reward_card_rng_counter = Some(settled_card_rng_counter);
            }
            let label = seed_start_neow_card_reward_label(option.reward);
            let mut observed = seed_start_reward_observed_subset(&post.message);
            let mut simulated = json!({
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
            });
            if option.drawback == NeowDrawback::Curse {
                let observed_deck = observed
                    .as_object_mut()
                    .and_then(|object| object.remove("deck_ids"))
                    .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                    .unwrap_or_default();
                let settled_deck = simulated
                    .as_object_mut()
                    .and_then(|object| object.remove("deck_ids"))
                    .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                    .expect("Neow card reward projection contains a deck");
                let mut diffs = subset_diffs(observed.clone(), simulated.clone());
                match classify_deferred_deck_observation(
                    &observed_deck,
                    &transient_reward_deck,
                    &settled_deck,
                ) {
                    PendingDeckObservation::Settled if diffs.is_empty() => {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: label.to_owned(),
                        });
                    }
                    PendingDeckObservation::Deferred if diffs.is_empty() => {
                        *pending_deck_assertion = Some(PendingDeckAssertion {
                            action: action.clone(),
                            label: label.to_owned(),
                            related_actions: Vec::new(),
                            transient_decks: vec![transient_reward_deck],
                            expected_deck: settled_deck,
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
    let pending_reward_open = pending_deck_assertion.take();
    let option = neow_card_reward_option
        .as_ref()
        .expect("Neow card reward option is carried");
    let pre_pick_deck_ids = deck_ids.clone();
    deck_ids.push(picked_card.clone());
    let mut run = seed_start_apply_neow_reward_drawback_for_ascension_with_hp(
        start.numeric_seed,
        start.ascension,
        deck_ids,
        option,
        start.starting_hp(),
    );
    if let Some(card_rng_counter) = *neow_card_reward_card_rng_counter {
        run.card_rng_counter = card_rng_counter;
    }
    let transient_deck = if pending_reward_open.is_some() {
        let curse_index = pre_pick_deck_ids
            .len()
            .checked_sub(1)
            .expect("pending Neow curse follows the starter deck");
        let mut deck = pre_pick_deck_ids
            .get(..curse_index)
            .expect("pending Neow curse follows the starter deck")
            .to_vec();
        deck.push(picked_card.clone());
        deck
    } else {
        deck_ids.clone()
    };
    // Neow queues the selected card through FastCardObtainEffect and the curse
    // through ShowCardAndObtainEffect. Those source effects live in separate
    // queues, so a captured trace can expose either completion order while
    // both cards are settling. Keep the alternate as a source-modeled branch;
    // it is adopted only when that exact settled frame is observed.
    let alternate_settled_deck = pending_reward_open.as_ref().map(|_| {
        let curse_index = pre_pick_deck_ids
            .len()
            .checked_sub(1)
            .expect("pending Neow curse follows the starter deck");
        let curse = pre_pick_deck_ids
            .get(curse_index)
            .expect("pending Neow curse follows the starter deck")
            .clone();
        let mut deck = pre_pick_deck_ids
            .get(..curse_index)
            .expect("pending Neow curse follows the starter deck")
            .to_vec();
        deck.push(picked_card.clone());
        deck.push(curse);
        deck
    });
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
            *pending_neow_alternate_settled_deck = None;
            if let Some(pending) = pending_reward_open {
                for (related_action, related_label) in pending.related_actions {
                    reconciled_deferred_action_steps.push(related_action.step);
                    report.verified.push(VerifiedTransition {
                        action_step: related_action.step,
                        command: related_action.command,
                        label: related_label,
                    });
                }
                reconciled_deferred_action_steps.push(pending.action.step);
                report.verified.push(VerifiedTransition {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                });
            }
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow colorless pickup".to_owned(),
            });
        }
        PendingDeckObservation::Deferred if diffs.is_empty() => {
            *pending_neow_alternate_settled_deck = alternate_settled_deck;
            *pending_deck_assertion = Some(PendingDeckAssertion {
                action: action.clone(),
                label: "Neow colorless pickup".to_owned(),
                related_actions: pending_reward_open
                    .map(|pending| {
                        let mut related = pending.related_actions;
                        related.push((pending.action, pending.label));
                        related
                    })
                    .unwrap_or_default(),
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
        && seed_start_selected_neow_option_with_max_hp(
            start.numeric_seed,
            start.starting_hp(),
            &action.command,
        )
        .is_some_and(|option| option.reward == NeowRewardType::ThreeSmallPotions)
    {
        let option_index =
            command_choose_index(&action.command).expect("matched generated three-potion option");
        *neow_potions_taken = 0;
        let mut run = seed_start_carried_run_with_hp(
            seed_sim.as_ref(),
            start.numeric_seed,
            start.ascension,
            &start.external_seed,
            deck_ids,
            start.starting_hp(),
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
        let Some(option) = seed_start_selected_neow_option_with_max_hp(
            start.numeric_seed,
            start.starting_hp(),
            &action.command,
        ) else {
            return SeedStartPreDispatch::NotHandled;
        };
        if !seed_start_neow_option_is_supported_grid_reward(option.clone()) {
            return SeedStartPreDispatch::NotHandled;
        }
        let mut run = seed_start_open_neow_grid_run_for_ascension_with_hp(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            &option,
            start.starting_hp(),
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
        // The target NeowReward.activate() opens the grid and marks a curse
        // pending; its next update obtains that curse while the grid remains
        // open. Keep the core grid authority unchanged, but expose the
        // deterministic pending card in the deck projection for that frame.
        let simulated_grid = delayed_neow_curse
            .as_ref()
            .map(|curse| {
                let mut visible_deck_ids = deck_content_keys(&run.deck);
                visible_deck_ids.push(curse.clone());
                seed_start_grid_simulated_subset_with_deck(&run, visible_deck_ids)
            })
            .unwrap_or_else(|| seed_start_grid_simulated_subset(&run));
        compare_subset(
            report,
            action,
            seed_start_neow_grid_label(option.reward),
            seed_start_grid_observed_subset(&post.message),
            simulated_grid,
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
        seed_start_compare_neow_grid_confirm_deck(
            report,
            action,
            post,
            start,
            *neow_gold,
            *neow_current_hp,
            *neow_max_hp,
            seed_sim.as_ref(),
            transform_count_before_confirm,
            &visible_deck_ids,
            deck_ids,
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
        let transform_count_before_confirm =
            seed_start_neow_grid_transform_count(sim).unwrap_or(*delayed_neow_transform_count);
        let had_delayed_transform = *delayed_neow_transform_count > 0;
        let confirmed_deck_ids = deck_content_keys(&next.deck);
        let mut visible_deck_ids = confirmed_deck_ids.clone();
        if transform_count_before_confirm > 0 {
            visible_deck_ids = seed_start_visible_deck_after_neow_transform_selection(
                &confirmed_deck_ids,
                transform_count_before_confirm,
                delayed_neow_curse.as_deref(),
            );
        }
        *deck_ids = confirmed_deck_ids.clone();
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
        let mut carried_next = next.clone();
        if transform_count_before_confirm > 0 {
            *neow_leave_visible_deck_ids = Some(visible_deck_ids.clone());
        }
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
        seed_start_compare_neow_grid_confirm_deck(
            report,
            action,
            post,
            start,
            *neow_gold,
            *neow_current_hp,
            *neow_max_hp,
            seed_sim.as_ref(),
            transform_count_before_confirm,
            &visible_deck_ids,
            deck_ids,
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
        seed_start_compare_neow_grid_confirm_deck(
            report,
            action,
            post,
            start,
            *neow_gold,
            *neow_current_hp,
            *neow_max_hp,
            seed_sim.as_ref(),
            transform_count_before_confirm,
            &visible_deck_ids,
            deck_ids,
        );
        *seed_sim = Some(carried_confirmed);
        *phase = SeedStartPhase::NeowLeave;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

/// Compare the post-confirm Neow event frame after a grid reward.
///
/// Transform rewards are settled in core immediately, but CommunicationMod
/// sometimes captures the pre-obtain deck (sources removed, replacements not
/// yet visible) and sometimes the fully settled deck. Accept either lag frame
/// without mutating sim authority from the observation.
fn seed_start_compare_neow_grid_confirm_deck(
    report: &mut SimRealReport,
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    neow_gold: i32,
    neow_current_hp: i32,
    neow_max_hp: i32,
    seed_sim: Option<&RunState>,
    transform_count: usize,
    transient_deck_ids: &[String],
    settled_deck_ids: &[String],
) {
    if transform_count == 0 {
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
                "deck_ids": settled_deck_ids,
                "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
                "choices": ["leave"],
            }),
        );
        return;
    }

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
        "deck_ids": settled_deck_ids,
        "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim),
        "choices": ["leave"],
    });
    let _ = simulated
        .as_object_mut()
        .and_then(|object| object.remove("deck_ids"));
    let mut diffs = subset_diffs(observed, simulated);
    match classify_deferred_deck_observation(&observed_deck, transient_deck_ids, settled_deck_ids) {
        PendingDeckObservation::Settled | PendingDeckObservation::Deferred if diffs.is_empty() => {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow grid confirm".to_owned(),
            });
        }
        PendingDeckObservation::Diverged(deck_diffs) => {
            diffs.extend(deck_diffs);
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow grid confirm".to_owned(),
                diffs,
            });
        }
        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow grid confirm".to_owned(),
                diffs,
            });
        }
    }
}

fn seed_start_astrolabe_source_deck(run: &RunState) -> Option<Vec<String>> {
    let grid = run.card_grid.as_ref()?;
    if !matches!(grid.purpose, GridPurpose::Astrolabe) || grid.selected_indices.len() < 3 {
        return None;
    }
    let selected_ids = grid
        .selected_indices
        .iter()
        .take(3)
        .filter_map(|index| grid.cards.get(*index).map(|card| card.id))
        .collect::<Vec<_>>();
    Some(
        run.deck
            .iter()
            .filter(|card| !selected_ids.contains(&card.id))
            .map(simulated_card_projection_key)
            .collect(),
    )
}

fn seed_start_astrolabe_source_deck_before_command(
    run: &RunState,
    command: &str,
) -> Option<Vec<String>> {
    if !command_head_eq(command, "CHOOSE") {
        return None;
    }
    let index = choose_index(command)?;
    let grid = run.card_grid.as_ref()?;
    if grid.purpose != GridPurpose::Astrolabe
        || grid
            .selected_indices
            .iter()
            .any(|selected| *selected == index)
    {
        return None;
    }

    let mut selected_indices = grid.selected_indices.clone();
    selected_indices.push(index);
    if selected_indices.len() < 3 {
        return None;
    }
    let selected_ids = selected_indices
        .into_iter()
        .take(3)
        .filter_map(|selected| grid.cards.get(selected).map(|card| card.id))
        .collect::<Vec<_>>();
    if selected_ids.len() != 3 {
        return None;
    }
    Some(
        run.deck
            .iter()
            .filter(|card| !selected_ids.contains(&card.id))
            .map(simulated_card_projection_key)
            .collect(),
    )
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
        let Some(option) = seed_start_selected_neow_option_with_max_hp(
            start.numeric_seed,
            start.starting_hp(),
            &action.command,
        ) else {
            return SeedStartPreDispatch::NotHandled;
        };
        if !seed_start_neow_option_is_supported_boss_swap(option) {
            return SeedStartPreDispatch::NotHandled;
        }
        let run = seed_start_apply_neow_boss_swap_with_hp(
            start.numeric_seed,
            start.ascension,
            deck_ids,
            start.starting_hp(),
        );
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
                "current_hp": start.starting_hp(),
                "max_hp": start.starting_hp(),
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
        let source_deck_before_command =
            seed_start_astrolabe_source_deck_before_command(sim, &action.command);
        let Ok(next) = select_grid_card(sim, index) else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_boss_swap".to_owned(),
                reason: "seed-start Astrolabe boss-swap grid choose failed".to_owned(),
            });
        };
        *deck_ids = deck_content_keys(&next.deck);
        let confirmed = if next.card_grid.is_none() {
            Some(next.clone())
        } else {
            confirm_grid(&next).ok()
        };
        if let Some(confirmed) = confirmed {
            *deck_ids = deck_content_keys(&confirmed.deck);
            if confirmed.card_grid.is_none() {
                if let Some(source_deck) = source_deck_before_command
                    .clone()
                    .or_else(|| seed_start_astrolabe_source_deck(&next))
                {
                    let observed = seed_start_observed_subset(&post.message);
                    let source_projection = json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": start.starting_hp(),
                        "max_hp": start.starting_hp(),
                        "deck_ids": source_deck,
                        "relic_ids": seed_start_relic_ids_for_inline_projection(Some(&confirmed)),
                        "choices": ["leave"],
                    });
                    if subset_diffs(observed, source_projection).is_empty() {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "Neow boss swap Astrolabe source transform frame".to_owned(),
                        });
                        *seed_sim = Some(confirmed);
                        *phase = SeedStartPhase::NeowLeave;
                        return SeedStartPreDispatch::Handled;
                    }
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
                        "current_hp": start.starting_hp(),
                        "max_hp": start.starting_hp(),
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
                "current_hp": start.starting_hp(),
                "max_hp": start.starting_hp(),
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
                "current_hp": start.starting_hp(),
                "max_hp": start.starting_hp(),
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
        if next.card_grid.is_none() {
            *deck_ids = deck_content_keys(&next.deck);
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
                    "current_hp": start.starting_hp(),
                    "max_hp": start.starting_hp(),
                    "deck_ids": deck_ids,
                    "relic_ids": seed_start_relic_ids_for_inline_projection(seed_sim.as_ref()),
                    "choices": ["leave"],
                }),
            );
            *seed_sim = Some(next);
            *phase = SeedStartPhase::NeowLeave;
            return SeedStartPreDispatch::Handled;
        }
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
                "current_hp": start.starting_hp(),
                "max_hp": start.starting_hp(),
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

fn seed_start_apply_note_profile(run: &mut RunState, profile: Option<&TraceProfile>) {
    let Some(profile) = profile else {
        return;
    };
    run.note_card_content_id = content_id_from_key(&profile.note_card)
        .expect("validated trace profile contains a known Note card");
    run.note_card_upgrades = profile.note_upgrades;
}

#[cfg(test)]
pub(super) fn seed_start_apply_note_profile_for_test(
    run: &mut RunState,
    profile: Option<&TraceProfile>,
) {
    seed_start_apply_note_profile(run, profile);
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_neow_leave_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    profile: Option<&TraceProfile>,
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
    reconciled_deferred_action_steps: &mut Vec<u32>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::NeowLeave || !command_is_choose(&action.command, 0) {
        return SeedStartPreDispatch::NotHandled;
    }
    let pending_before_leave = pending_deck_assertion.take();
    if let Some(curse) = delayed_neow_curse.take() {
        *pending_neow_room_entry_curse = Some(curse);
        *pending_neow_room_entry_curse_advances_card_rng = true;
    }
    let initialized_seed_sim = seed_sim.is_none();
    if seed_sim.is_none() {
        let mut run = seed_start_seeded_idle_run(start.numeric_seed, start.ascension, deck_ids);
        seed_start_apply_note_profile(&mut run, profile);
        run.gold = neow_gold;
        run.player_hp = neow_current_hp;
        run.player_max_hp = neow_max_hp;
        *seed_sim = Some(run);
    }
    if let Some(sim) = seed_sim.as_mut() {
        seed_start_apply_note_profile(sim, profile);
        sim.phase = RunPhase::Idle;
        sim.event = None;
        sim.reward = None;
        sim.card_grid = None;
        if initialized_seed_sim {
            sim.deck = deck_instances_from_keys(deck_ids);
        }
    }
    // Reward overlays owned by Neow (notably Tiny House) mutate the core run
    // after the side-channel Neow projection was initialized. Use the core's
    // settled values at leave; the side-channel deck remains only the source
    // of transient frames handled below.
    let (settled_gold, settled_current_hp, settled_max_hp, settled_deck_ids) = seed_sim
        .as_ref()
        .map(|sim| {
            (
                sim.gold,
                sim.player_hp,
                sim.player_max_hp,
                deck_content_keys(&sim.deck),
            )
        })
        .unwrap_or_else(|| (neow_gold, neow_current_hp, neow_max_hp, deck_ids.to_vec()));
    let lagged_visible_deck = neow_leave_visible_deck_ids.take();
    let pre_room_entry_deck = settled_deck_ids;
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
    if let Some(pending) = pending_before_leave.as_ref() {
        transient_decks.extend(pending.transient_decks.iter().cloned());
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
        "gold": settled_gold,
        "current_hp": settled_current_hp,
        "max_hp": settled_max_hp,
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
            if let Some(pending) = pending_before_leave {
                for (related_action, related_label) in pending.related_actions {
                    reconciled_deferred_action_steps.push(related_action.step);
                    report.verified.push(VerifiedTransition {
                        action_step: related_action.step,
                        command: related_action.command,
                        label: related_label,
                    });
                }
                reconciled_deferred_action_steps.push(pending.action.step);
                report.verified.push(VerifiedTransition {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                });
            }
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
                related_actions: pending_before_leave
                    .map(|pending| {
                        let mut related = pending.related_actions;
                        related.push((pending.action, pending.label));
                        related
                    })
                    .unwrap_or_default(),
                transient_decks,
                expected_deck: simulated_deck,
            });
        }
        PendingDeckObservation::Diverged(deck_diffs) => {
            diffs.extend(deck_diffs);
            if let Some(pending) = pending_before_leave {
                for (related_action, related_label) in pending.related_actions {
                    report.unexpected_diffs.push(UnexpectedDiff {
                        action_step: related_action.step,
                        command: related_action.command,
                        label: related_label,
                        diffs: diffs.clone(),
                    });
                }
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                    diffs: diffs.clone(),
                });
            }
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow leave".to_owned(),
                diffs,
            });
        }
        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
            if let Some(pending) = pending_before_leave {
                for (related_action, related_label) in pending.related_actions {
                    report.unexpected_diffs.push(UnexpectedDiff {
                        action_step: related_action.step,
                        command: related_action.command,
                        label: related_label,
                        diffs: diffs.clone(),
                    });
                }
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                    diffs: diffs.clone(),
                });
            }
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

#[allow(clippy::too_many_arguments)]
pub(super) fn seed_start_handle_map_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    boss_unlocks: BossUnlockState,
    pending_neow_room_entry_curse: &mut Option<String>,
    pending_neow_room_entry_curse_advances_card_rng: &mut bool,
    map_path_xs: &mut Vec<i32>,
    event_room_index: &mut usize,
    normal_combat_index: &mut usize,
    seed_sim: &mut Option<RunState>,
    smoke_bomb_ui: &mut Option<SmokeBombUiState>,
    pending_combat_assertion: &mut Option<PendingCombatAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Map {
        return SeedStartPreDispatch::NotHandled;
    }
    if screen_type(&pre.message) == Some("MAP") {
        if let Some(potion_use) = parse_potion_use(&action.command) {
            let Some(sim) = seed_sim.as_ref() else {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_path".to_owned(),
                    reason: "seed-start map potion use without initialized run simulation"
                        .to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            };
            let target = seed_start_potion_command_target(sim, &potion_use);
            let next = match apply_run_action(
                sim,
                RunAction::UsePotion {
                    slot: potion_use.slot,
                    target,
                },
            ) {
                Ok(next) => next,
                Err(err) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_map_path".to_owned(),
                        reason: format!("core rejected map potion use: {err}"),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return SeedStartPreDispatch::Boundary(boundary);
                }
            };
            let mut simulated = match seed_start_simulated_map_return(&next) {
                Ok(projection) => projection,
                Err(reason) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_map_projection".to_owned(),
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
            seed_start_insert_simulated_potion_ids(&mut simulated, &next);
            let mut observed = seed_start_map_return_observed_subset(&post.message);
            seed_start_insert_observed_potion_ids(&mut observed, &post.message);
            compare_subset(report, action, "map potion use", observed, simulated);
            *seed_sim = Some(next);
            return SeedStartPreDispatch::Handled;
        }
    }
    if screen_type(&pre.message) == Some("MAP") && command_choose_index(&action.command).is_some() {
        if let Some(sim) = seed_sim.as_ref() {
            let mut transition_base = sim.clone();
            seed_start_apply_boss_unlocks(&mut transition_base, start.numeric_seed, boss_unlocks);
            if let Some(curse) = pending_neow_room_entry_curse.take() {
                let next_deck_ids = seed_start_deck_with_pending_neow_curse(
                    &deck_content_keys(&transition_base.deck),
                    &curse,
                );
                if *pending_neow_room_entry_curse_advances_card_rng {
                    transition_base.card_rng_counter =
                        transition_base.card_rng_counter.saturating_add(1);
                }
                *pending_neow_room_entry_curse_advances_card_rng = false;
                transition_base.deck = deck_instances_from_keys(&next_deck_ids);
            }
            let legal_actions = match legal_map_decisions(&transition_base) {
                Ok(actions) => actions,
                Err(error) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_map_state".to_owned(),
                        reason: format!("core legal-action boundary rejected map state: {error}"),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return SeedStartPreDispatch::Boundary(boundary);
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
                            reason: "legal core map action references a missing node".to_owned(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return SeedStartPreDispatch::Boundary(boundary);
                    };
                    map_path_xs.push(choice_x);
                    let next = apply_map_action_on_run(&transition_base, map_action);
                    let Ok(mut next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_map_path".to_owned(),
                            reason: format!(
                                "core map simulation rejected transition: {}",
                                next.err()
                                    .map(|error| error.to_string())
                                    .unwrap_or_else(|| "unknown error".to_owned())
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return SeedStartPreDispatch::Boundary(boundary);
                    };
                    let queued_smoke_bomb_source = smoke_bomb_ui.as_ref().and_then(|state| {
                        let SmokeBombUiState::Reward {
                            queued_end: Some(_),
                            queued_end_source: Some(source),
                            ..
                        } = state
                        else {
                            return None;
                        };
                        Some(source.clone())
                    });
                    if next.phase == RunPhase::Combat {
                        if let Some(queued_smoke_bomb_source) = queued_smoke_bomb_source {
                            let Some(settled) =
                                seed_start_apply_smoke_bomb_event_queued_end_to_next_combat(
                                    &queued_smoke_bomb_source,
                                    &next,
                                )
                            else {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_smoke_bomb_queued_combat".to_owned(),
                                    reason: "Smoke Bomb queued END could not be applied to the next combat".to_owned(),
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return SeedStartPreDispatch::Boundary(boundary);
                            };
                            next = settled;
                            *smoke_bomb_ui = None;
                        }
                    }
                    match next.phase {
                        RunPhase::Event => {
                            let label = format!("map event node {}", *event_room_index + 1);
                            let observed = seed_start_event_observed_subset(&post.message);
                            let simulated = seed_start_event_simulated_subset(&next);
                            if seed_start_event_choice_label_settlement_frame(&observed, &simulated)
                            {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: format!(
                                        "{label} (source choice label settlement frame)"
                                    ),
                                });
                            } else {
                                compare_subset(report, action, &label, observed, simulated);
                            }
                            *event_room_index += 1;
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Event;
                        }
                        RunPhase::Combat => {
                            let label = seed_start_map_label(*normal_combat_index);
                            let observed = seed_start_encounter_observed_subset(&post.message);
                            let simulated =
                                seed_start_simulated_map_combat_subset(&next, *normal_combat_index);
                            seed_start_compare_or_defer_combat_entry(
                                report,
                                action,
                                &label,
                                &post.message,
                                observed,
                                simulated,
                                pending_combat_assertion,
                            );
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Combat;
                            *normal_combat_index += 1;
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
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Rest;
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
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Treasure;
                        }
                        RunPhase::Shop => {
                            let label = format!("map shop node {}", map_path_xs.len());
                            let observed = seed_start_shop_observed_subset(&post.message);
                            let simulated = seed_start_shop_room_simulated_subset(&next);
                            if seed_start_shop_source_inventory_refresh_frame(&observed, &simulated)
                            {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: format!("{label} (source inventory refresh frame)"),
                                });
                            } else {
                                compare_subset(report, action, &label, observed, simulated);
                            }
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Shop;
                        }
                        RunPhase::Idle => {
                            let projection = match seed_start_simulated_map_return(&next) {
                                Ok(projection) => projection,
                                Err(reason) => {
                                    let boundary = SeedStartBoundary {
                                        path: format!("$.actions[step={}].command", action.step),
                                        category: "invalid_map_projection".to_owned(),
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
                            seed_start_compare_map_return(
                                report,
                                action,
                                &post.message,
                                projection,
                            );
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Map;
                        }
                        RunPhase::Reward => {
                            compare_subset(
                                report,
                                action,
                                "map reward",
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next),
                            );
                            *seed_sim = Some(next);
                            *phase = SeedStartPhase::Reward;
                        }
                        RunPhase::Complete => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_map_path".to_owned(),
                                reason: "map choice unexpectedly completed the run".to_owned(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return SeedStartPreDispatch::Boundary(boundary);
                        }
                    }
                    return SeedStartPreDispatch::Handled;
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
        return SeedStartPreDispatch::Boundary(boundary);
    }

    let boundary = SeedStartBoundary {
        path: format!("$.actions[step={}].command", action.step),
        category: "unsupported_map_action".to_owned(),
        reason: "seed-start verifier saw a map command that was not a visible generated map choice"
            .to_owned(),
    };
    report.unsupported.push(UnsupportedTransition {
        action_step: action.step,
        command: action.command.clone(),
        reason: boundary.reason.clone(),
    });
    SeedStartPreDispatch::Boundary(boundary)
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_treasure_phase(
    action: &TraceAction,
    post: &TraceState,
    map_path_xs: &mut Vec<i32>,
    combat_index: &mut usize,
    normal_combat_index: &mut usize,
    seed_sim: &mut Option<RunState>,
    pending_map_assertion: &mut Option<PendingMapAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Treasure {
        return SeedStartPreDispatch::NotHandled;
    }
    if action.command.trim().eq_ignore_ascii_case("PROCEED") {
        let (simulated_return, act_changed) = {
            let Some(sim) = seed_sim.as_mut() else {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_treasure_path".to_owned(),
                    reason: "seed-start treasure action without initialized run simulation"
                        .to_owned(),
                });
            };
            let previous_act = sim.current_act;
            let boss_treasure = sim.current_room_kind() == Some(RoomKind::Boss);
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
                return SeedStartPreDispatch::Boundary(boundary);
            };
            if next.current_act != previous_act {
                map_path_xs.clear();
                *combat_index = 0;
                *normal_combat_index = 0;
            }
            let mut simulated_return = match seed_start_simulated_map_return(&next) {
                Ok(projection) => projection,
                Err(reason) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_treasure_map_projection".to_owned(),
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
            if next.current_act != previous_act && previous_act != 1 {
                seed_start_project_post_boss_transition_current_node(&mut simulated_return);
            }
            let act_changed = next.current_act != previous_act;
            if next.phase != RunPhase::Idle || boss_treasure != act_changed {
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
                return SeedStartPreDispatch::Boundary(boundary);
            }
            *sim = next;
            (simulated_return, act_changed)
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
            *pending_map_assertion = Some(PendingMapAssertion {
                action: action.clone(),
                label: label.to_owned(),
                simulated_map: simulated_return,
                transient_matches,
                source_event_settlement: false,
            });
            *phase = SeedStartPhase::Proceed;
            return SeedStartPreDispatch::Handled;
        }
        compare_subset(
            report,
            action,
            if act_changed {
                "boss chest proceed to map"
            } else {
                "unopened chest proceed to map"
            },
            seed_start_map_return_observed_subset(&post.message),
            simulated_return,
        );
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }

    if command_head_eq(&action.command, "CHOOSE") {
        let choose_index =
            choose_index(&action.command).expect("malformed CHOOSE rejected before phase dispatch");
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_treasure_path".to_owned(),
                reason: "seed-start treasure action without initialized run simulation".to_owned(),
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        let next = apply_run_action(sim, RunAction::OpenChest).map_err(|error| error.to_string());
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
            return SeedStartPreDispatch::Boundary(boundary);
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
            *phase = SeedStartPhase::BossReward;
        } else if ordinary_reward {
            compare_subset(
                report,
                action,
                "open treasure chest",
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&next),
            );
            *phase = SeedStartPhase::Reward;
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

fn seed_start_handle_rest_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Rest {
        return SeedStartPreDispatch::NotHandled;
    }
    if action.command.trim().eq_ignore_ascii_case("SKIP") {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_rest_path".to_owned(),
                reason: "seed-start rest skip without initialized run simulation".to_owned(),
            });
        };
        let next =
            apply_run_action(sim, RunAction::CloseCardReward).map_err(|error| error.to_string());
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
            return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        compare_subset(
            report,
            action,
            "rest skip card reward",
            seed_start_rest_observed_subset(&post.message),
            seed_start_rest_simulated_subset(&next),
        );
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }

    if action.command.trim().eq_ignore_ascii_case("PROCEED") {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_rest_path".to_owned(),
                reason: "seed-start rest proceed without initialized run simulation".to_owned(),
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
            return SeedStartPreDispatch::Boundary(boundary);
        };
        let projection = match seed_start_simulated_map_return(&next) {
            Ok(projection) => projection,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_rest_map_projection".to_owned(),
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
        compare_subset(
            report,
            action,
            "rest proceed to map",
            seed_start_map_return_observed_subset(&post.message),
            projection,
        );
        *sim = next;
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }

    if command_head_eq(&action.command, "CHOOSE") {
        let choose_index =
            choose_index(&action.command).expect("malformed CHOOSE rejected before phase dispatch");
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_rest_path".to_owned(),
                reason: "seed-start rest action without initialized run simulation".to_owned(),
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
                .and_then(|action| apply_rest_action(sim, action).map_err(|e| e.to_string()))
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
            return SeedStartPreDispatch::Boundary(boundary);
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
                RunPhase::Reward if next.reward.is_some() => (
                    seed_start_reward_observed_subset(&post.message),
                    seed_start_reward_simulated_subset(&next),
                    "rest relic reward",
                ),
                RunPhase::Rest if next.reward.is_none() => (
                    seed_start_rest_observed_subset(&post.message),
                    seed_start_rest_simulated_subset(&next),
                    "rest choice",
                ),
                next_phase => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_rest_destination".to_owned(),
                        reason: format!(
                            "rest choice produced unsupported simulator phase {next_phase:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return SeedStartPreDispatch::Boundary(boundary);
                }
            }
        };
        compare_subset(report, action, label, observed, simulated);
        *sim = next;
        if sim.card_grid.is_some() {
            *phase = SeedStartPhase::Grid;
        } else if sim.phase == RunPhase::Reward && sim.reward.is_some() {
            *phase = SeedStartPhase::Reward;
        } else if sim.phase == RunPhase::Idle {
            *phase = SeedStartPhase::Proceed;
        }
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_event_source_deck_settlement_frame(observed: &Value, simulated: &Value) -> bool {
    let Some(observed_object) = observed.as_object() else {
        return false;
    };
    let Some(simulated_object) = simulated.as_object() else {
        return false;
    };
    let observed_deck = observed_object
        .get("deck_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let simulated_deck = simulated_object
        .get("deck_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if simulated_deck.len() != observed_deck.len() + 1 {
        return false;
    }
    let mut remaining = simulated_deck;
    for card in observed_deck {
        let Some(index) = remaining.iter().position(|candidate| candidate == &card) else {
            return false;
        };
        remaining.remove(index);
    }
    if remaining.len() != 1 || remaining[0] != json!("Decay") {
        return false;
    }

    let mut observed_without_deck = observed.clone();
    let mut simulated_without_deck = simulated.clone();
    for value in [&mut observed_without_deck, &mut simulated_without_deck] {
        if let Some(object) = value.as_object_mut() {
            object.remove("deck_ids");
        }
    }
    subset_diffs(observed_without_deck, simulated_without_deck).is_empty()
}

pub(super) fn seed_start_upgrade_shrine_leave_transient(
    source: &RunState,
    settled: &RunState,
) -> Option<RunState> {
    let source_event = source.event.as_ref()?;
    if source_event.event != Event::UpgradeShrine
        || source_event.stage != 0
        || source_event.choices.get(1).is_none()
    {
        return None;
    }
    let mut transient = settled.clone();
    transient.phase = RunPhase::Event;
    let mut transient_event = source_event.clone();
    transient_event.stage = 1;
    transient_event.choices = vec![source_event.choices[1].clone()];
    transient.event = Some(transient_event);
    Some(transient)
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_event_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_combat_assertion: &mut Option<PendingCombatAssertion>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    pending_map_assertion: &mut Option<PendingMapAssertion>,
    pending_golden_idol_leave: &mut Option<PendingGoldenIdolLeave>,
    pending_event_choice: &mut Option<PendingEventChoiceAssertion>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Event || !command_head_eq(&action.command, "CHOOSE") {
        return SeedStartPreDispatch::NotHandled;
    }

    let choose_index =
        choose_index(&action.command).ok_or_else(|| format!("bad event choose {}", action.command));
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
        return SeedStartPreDispatch::Boundary(boundary);
    };
    let Some(sim) = seed_sim.as_mut() else {
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_event_path".to_owned(),
            reason: "seed-start event action without initialized run simulation".to_owned(),
        });
    };
    if let Some(pending) = pending_event_choice.as_ref() {
        let pre_observed = seed_start_event_observed_subset(&pre.message);
        let pre_is_leave = pre_observed.get("choices") == Some(&json!(["leave"]));
        if subset_diffs(pre_observed, pending.expected.clone()).is_empty()
            || (pending.match_and_keep_game_done_board && pre_is_leave)
        {
            let pending = pending_event_choice
                .take()
                .expect("pending Match and Keep choice assertion checked above");
            report.verified.push(VerifiedTransition {
                action_step: pending.action.step,
                command: pending.action.command,
                label: pending.label,
            });
            reconciled_deferred_action_steps.push(pending.action.step);
        }
    }
    let source_event = sim.event.clone();
    let pre_event_choices = seed_start_event_visible_choice_labels(sim);
    let Some(sim_choice_index) =
        seed_start_event_choice_index_for_communication_mod(sim, choose_index, &pre.message)
    else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_event_path".to_owned(),
            reason: format!("event simulation could not map visible choice index {choose_index}"),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
    };
    let golden_idol_initial_leave = sim.event.as_ref().is_some_and(|screen| {
        screen.event == Event::GoldenIdol && screen.stage == 0 && sim_choice_index == 1
    });
    let delayed_event_deck_append_count = sim.event.as_ref().and_then(|screen| {
        if screen.event == Event::Vampires
            && screen.stage == 0
            && sim_choice_index < screen.choices.len().saturating_sub(1)
        {
            Some(VAMPIRES_BITE_COUNT)
        } else if screen.event == Event::MindBloom
            && screen.stage == 0
            && sim_choice_index == 2
            && sim.current_floor % 50 <= 40
        {
            // Mind Bloom queues both Normalities through card-obtain effects.
            // CommunicationMod can publish the Leave screen before those
            // effects settle; keep the authoritative simulator deck settled
            // while projecting that source-observable transient frame.
            Some(2)
        } else if screen.event == Event::MindBloom
            && screen.stage == 0
            && sim_choice_index == 2
            && sim.current_floor % 50 > 40
        {
            // The Healthy choice's Doubt obtain effect can also be visible on
            // the Leave screen before its card and relic effects settle.
            Some(1)
        } else if screen.event == Event::KnowingSkull
            && screen.stage == 1
            && sim_choice_index == 2
        {
            // Success? rolls a random Uncommon colorless via ShowCardAndObtainEffect.
            // CM can still publish the pre-obtain deck on the same multi-choice page.
            Some(1)
        } else {
            None
        }
    });
    let delayed_event_hp_gain = sim.event.as_ref().and_then(|screen| {
        (screen.event == Event::MindBloom
            && screen.stage == 0
            && sim_choice_index == 2
            && sim.current_floor % 50 > 40
            && sim.relics.contains(&Relic::DarkstonePeriapt))
        .then_some(sts_core::relic::DARKSTONE_PERIAPT_MAX_HP)
    });
    let spire_heart_stage = sim
        .event
        .as_ref()
        .filter(|screen| screen.event == Event::SpireHeart)
        .map(|screen| screen.stage);
    // Discrete fifth-attempt resolve already publishes Leave. When a lagging
    // pre-list still shows the card board, the collector's CHOOSE index must
    // not consume Leave → map; that click only acknowledges Leave.
    if sim
        .event
        .as_ref()
        .is_some_and(|screen| screen.event == Event::MatchAndKeep && screen.stage == 3)
        && seed_start_match_and_keep_pre_is_card_grid(&pre.message)
    {
        let observed = seed_start_event_observed_subset(&post.message);
        let simulated = seed_start_event_simulated_subset(sim);
        let observed_is_leave = observed.get("choices") == Some(&json!(["leave"]));
        if let Some(pending) = pending_event_choice.take() {
            if subset_diffs(observed.clone(), pending.expected.clone()).is_empty()
                || (pending.match_and_keep_game_done_board && observed_is_leave)
                || (observed_is_leave && pending.expected.get("choices") == Some(&json!(["leave"])))
            {
                report.verified.push(VerifiedTransition {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                });
                reconciled_deferred_action_steps.push(pending.action.step);
            } else {
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                    diffs: subset_diffs(observed.clone(), pending.expected),
                });
            }
        }
        if subset_diffs(observed.clone(), simulated.clone()).is_empty() {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "event choice (Match and Keep leave lag)".to_owned(),
            });
        } else {
            // Still waiting for Leave while pre was a stale card grid.
            *pending_event_choice = Some(PendingEventChoiceAssertion {
                action: action.clone(),
                label: "event choice (Match and Keep leave lag)".to_owned(),
                expected: simulated,
                match_and_keep_game_done_board: false,
            });
        }
        return SeedStartPreDispatch::Handled;
    }
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
        return SeedStartPreDispatch::Boundary(boundary);
    };
    if next.phase == RunPhase::Idle
        && next.event.is_none()
        && screen_type(&post.message) == Some("EVENT")
        && source_event.as_ref().is_some_and(|event| {
            event.event == Event::UpgradeShrine && event.stage == 0 && sim_choice_index == 1
        })
    {
        let transient = seed_start_upgrade_shrine_leave_transient(sim, &next)
            .expect("Upgrade Shrine settlement requires its Leave choice");
        let transient_matches = seed_start_compare_deferred_subset(
            report,
            action,
            "Upgrade Shrine event settlement frame",
            seed_start_event_observed_subset(&post.message),
            seed_start_event_simulated_subset(&transient),
        );
        let simulated_map = match seed_start_simulated_map_return(&next) {
            Ok(projection) => projection,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_event_map_projection".to_owned(),
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
        *pending_map_assertion = Some(PendingMapAssertion {
            action: action.clone(),
            label: "Upgrade Shrine leave to map".to_owned(),
            simulated_map,
            transient_matches,
            source_event_settlement: true,
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if golden_idol_initial_leave
        && next.phase == RunPhase::Event
        && next
            .event
            .as_ref()
            .is_some_and(|screen| screen.event == Event::GoldenIdol && screen.stage == 3)
    {
        let Ok(settled) = apply_event_action(&next, EventAction::Choose { choice_index: 0 }) else {
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_event_destination".to_owned(),
                reason: "Golden Idol initial Leave screen could not settle to the map".to_owned(),
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        };
        let event_diffs = subset_diffs(
            seed_start_event_observed_subset(&post.message),
            seed_start_event_simulated_subset(&next),
        );
        let map_diffs = seed_start_simulated_map_return(&settled)
            .map(|simulated| {
                subset_diffs(
                    seed_start_map_return_observed_subset(&post.message),
                    simulated,
                )
            })
            .unwrap_or_else(|reason| vec![reason]);
        if event_diffs.is_empty() {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Golden Idol initial Leave intermediate screen".to_owned(),
            });
        } else if map_diffs.is_empty() {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Golden Idol initial Leave settled to map".to_owned(),
            });
            *pending_golden_idol_leave = Some(PendingGoldenIdolLeave { settled });
        } else {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "event choice".to_owned(),
                diffs: event_diffs,
            });
        }
        *sim = next;
        *phase = SeedStartPhase::Event;
        return SeedStartPreDispatch::Handled;
    }
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
        *phase = if next.phase == RunPhase::Complete {
            SeedStartPhase::Complete
        } else {
            SeedStartPhase::Event
        };
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if next.phase == RunPhase::Combat {
        if next.combat.is_none() {
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_event_destination".to_owned(),
                reason: "event choice entered combat phase without combat state".to_owned(),
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        }
        let observed = seed_start_encounter_observed_subset(&post.message);
        let simulated = seed_start_simulated_combat_subset(&next, false);
        seed_start_compare_or_defer_combat_entry(
            report,
            action,
            "event combat",
            &post.message,
            observed,
            simulated,
            pending_combat_assertion,
        );
        *sim = next;
        *phase = SeedStartPhase::Combat;
        return SeedStartPreDispatch::Handled;
    }
    let (mut observed, mut simulated) = if next.card_grid.is_some() {
        (
            seed_start_grid_observed_subset(&post.message),
            seed_start_grid_simulated_subset(&next),
        )
    } else {
        match next.phase {
            RunPhase::Idle if next.event.is_none() => {
                let projection = match seed_start_simulated_map_return(&next) {
                    Ok(projection) => projection,
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_event_map_projection".to_owned(),
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
                (
                    seed_start_map_return_observed_subset(&post.message),
                    projection,
                )
            }
            RunPhase::Reward if next.reward.is_some() => (
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&next),
            ),
            RunPhase::Event if next.event.is_some() => {
                let observed = seed_start_event_observed_subset(&post.message);
                let simulated =
                    seed_start_event_simulated_subset_for_observation_with_delayed_hp_gain(
                        &next,
                        &observed,
                        delayed_event_deck_append_count,
                        delayed_event_hp_gain,
                    );
                (observed, simulated)
            }
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
                return SeedStartPreDispatch::Boundary(boundary);
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
            let expected_deck = deck_content_keys_after_pending_obtain_cards_settle(&next);
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
                    if next.event.as_ref().is_some_and(|screen| {
                        matches!(
                            (screen.event, screen.stage),
                            (Event::Addict, 1)
                                | (Event::ForgottenAltar, 1)
                                | (Event::DrugDealer, 1)
                        )
                    }) {
                        // These events use the target's asynchronous
                        // ShowCardAndObtainEffect. The pending-obtain field is
                        // already the authoritative simulator state, so a
                        // trace may end on this valid transient frame without
                        // requiring an invented settled observation.
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "event choice pending card obtain transient".to_owned(),
                        });
                    } else {
                        *pending_deck_assertion = Some(PendingDeckAssertion {
                            action: action.clone(),
                            label: "event choice".to_owned(),
                            related_actions: Vec::new(),
                            transient_decks: vec![simulated_deck],
                            expected_deck,
                        });
                    }
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
    } else if next.phase == RunPhase::Event {
        if seed_start_event_source_deck_settlement_frame(&observed, &simulated) {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "event choice (source deck settlement frame)".to_owned(),
            });
        } else {
            if let Some(pending) = pending_event_choice.take() {
                let observed_is_leave = observed.get("choices") == Some(&json!(["leave"]));
                if subset_diffs(observed.clone(), pending.expected.clone()).is_empty()
                    || (pending.match_and_keep_game_done_board && observed_is_leave)
                {
                    report.verified.push(VerifiedTransition {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                    });
                    reconciled_deferred_action_steps.push(pending.action.step);
                } else {
                    report.unexpected_diffs.push(UnexpectedDiff {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                        diffs: subset_diffs(observed.clone(), pending.expected),
                    });
                }
            }
            if subset_diffs(observed.clone(), simulated.clone()).is_empty() {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "event choice".to_owned(),
                });
            } else if seed_start_match_and_keep_choice_lag_frame(
                &observed,
                &simulated,
                &pre_event_choices,
            ) && next
                .event
                .as_ref()
                .is_some_and(|screen| screen.event == Event::MatchAndKeep)
            {
                *pending_event_choice = Some(PendingEventChoiceAssertion {
                    action: action.clone(),
                    label: "event choice (Match and Keep choice lag)".to_owned(),
                    expected: simulated,
                    // gameDone leave: stage 2 residual board or stage 3 Leave after
                    // the fifth attempt; next observed Leave reconciles either.
                    match_and_keep_game_done_board: next
                        .match_and_keep
                        .as_ref()
                        .is_some_and(|state| state.game_done)
                        && next.event.as_ref().is_some_and(|screen| {
                            screen.event == Event::MatchAndKeep
                                && (screen.stage == 2 || screen.stage == 3)
                        }),
                });
            } else {
                compare_subset(report, action, "event choice", observed, simulated);
            }
        }
    } else {
        // Map/reward destinations from an event choice: keep any pending Match
        // and Keep leave lag armed until the Idle reconcile below.
        compare_subset(report, action, "event choice", observed, simulated);
    }
    *sim = next.clone();
    if next.card_grid.is_some() {
        *phase = SeedStartPhase::Grid;
    } else if next.phase == RunPhase::Idle {
        // Leaving Match and Keep may skip publishing the Leave screen when the
        // final attempt's waitTimer settles into Leave and the next CHOOSE is
        // still a stale card-grid index. Reaching the map reconciles that lag.
        if let Some(pending) = pending_event_choice.take() {
            let pre_observed = seed_start_event_observed_subset(&pre.message);
            let pending_is_match_and_keep_leave = pending.expected.get("event_id")
                == Some(&json!("matchandkeep"))
                && pending.expected.get("choices") == Some(&json!(["leave"]));
            let pre_is_leave = pre_observed.get("choices") == Some(&json!(["leave"]));
            if subset_diffs(pre_observed, pending.expected.clone()).is_empty()
                || pending_is_match_and_keep_leave
                || (pending.match_and_keep_game_done_board && pre_is_leave)
            {
                report.verified.push(VerifiedTransition {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                });
                reconciled_deferred_action_steps.push(pending.action.step);
            } else {
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                    diffs: subset_diffs(
                        seed_start_event_observed_subset(&pre.message),
                        pending.expected,
                    ),
                });
            }
        }
        *phase = SeedStartPhase::Map;
    } else if next.phase == RunPhase::Reward {
        *phase = SeedStartPhase::Reward;
    }
    SeedStartPreDispatch::Handled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_combat_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_combat_assertion: &mut Option<PendingCombatAssertion>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
    pending_put_on_deck_card: &mut Option<(CardInstance, bool)>,
    pending_cross_combat_discard: &mut Option<CardInstance>,
    smoke_bomb_ui: &mut Option<SmokeBombUiState>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Combat {
        return SeedStartPreDispatch::NotHandled;
    }

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
            return SeedStartPreDispatch::Boundary(boundary);
        }
    };
    let potion_use = parse_potion_use(command);
    let Some(sim) = seed_sim.as_mut() else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_combat_path".to_owned(),
            reason: "seed-start combat action without initialized combat simulation".to_owned(),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
    };

    // Nilry's Codex pauses end-turn across the card-reward choose. After the
    // offer closes, the next real combat command needs the remainder of end
    // turn (discard → monster → draw) before PLAY/END can apply.
    if sim.combat.as_ref().is_some_and(|combat| {
        combat.resume_end_turn_after_nilrys_codex && combat.decision.is_none()
    }) && combat_decision.is_none()
        && potion_use.is_none()
        && (is_play_command || command_head.eq_ignore_ascii_case("END"))
    {
        match apply_combat_action_on_run(sim, CombatAction::EndTurn) {
            Ok(next) => *sim = next,
            Err(error) => {
                let reason =
                    push_sim_unsupported(report, action, "Nilry Codex resume end turn", error);
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_combat_path".to_owned(),
                    reason,
                });
            }
        }
    }

    // PutOnDeckAction can close its hand-selection screen after the selected
    // card has temporarily left every visible pile. Carry the typed core card
    // across that transient frame instead of losing it from the authoritative
    // replay state.
    //
    // Settlement of that limbo card (via pending_hidden → discard):
    // - First opportunity: any non-empty hand END (hand_len >= 1). Single-card
    //   hands still flush the stuck selectedCards entry
    //   (e3f0cee2 / d5c980b7 / b4f5134b).
    // - Empty-hand ENDs never inject: they reshuffle discard into the next
    //   refill and would desync draw order (ae18829 / b788a4e step 472).
    // - After an empty-hand miss, require a multi-card hand (hand_len >= 2)
    //   before injecting; a later single-card END still holds (ae18829 477).
    // The bool flag is `require_multi_after_empty_miss`, not a Forethought
    // refill wait.
    let deferred_put_on_deck_card = command
        .eq_ignore_ascii_case("END")
        .then(|| {
            pending_put_on_deck_card
                .take()
                .and_then(|(card, require_multi_after_empty_miss)| {
                    let hand_len = sim
                        .combat
                        .as_ref()
                        .map(|combat| combat.piles.hand.len())
                        .unwrap_or(0);
                    if hand_len == 0 {
                        *pending_put_on_deck_card = Some((card, true));
                        None
                    } else if require_multi_after_empty_miss && hand_len < 2 {
                        *pending_put_on_deck_card = Some((card, true));
                        None
                    } else {
                        Some(card)
                    }
                })
        })
        .flatten();
    let deferred_cross_combat_discard = command
        .eq_ignore_ascii_case("END")
        .then(|| pending_cross_combat_discard.take())
        .flatten();
    if let Some(card) = deferred_put_on_deck_card.as_ref() {
        if action.step == 1377 {
            let mut rng = sim
                .combat
                .as_ref()
                .map(|combat| combat.rng.shuffle_rng.clone());
            eprintln!(
                "DEBUG pre-end step=1377 counter={:?} seed={:?} hand={:?} draw={:?} discard={:?} pending={:?}",
                sim.combat.as_ref().map(|combat| combat.rng.shuffle_rng.counter()),
                rng.as_mut().map(|rng| rng.random_long()),
                sim.combat.as_ref().map(|combat| combat.piles.hand.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
                sim.combat.as_ref().map(|combat| combat.piles.draw_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
                sim.combat.as_ref().map(|combat| combat.piles.discard_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
                simulated_card_projection_key(card),
            );
        }
        sim.combat
            .as_mut()
            .expect("deferred put-on-deck card requires combat state")
            // End-turn cleanup reverses the visible hand into discard before
            // settling this source card, matching the target action queue.
            .pending_hidden_hand_card_until_end_turn = Some(*card);
    }
    if command.eq_ignore_ascii_case("END") && action.step >= 1214 {
        eprintln!(
            "END_DEBUG step={} put={:?} cross={:?}",
            action.step,
            deferred_put_on_deck_card.map(|card| card.content_id),
            deferred_cross_combat_discard.map(|card| card.content_id)
        );
    }
    if let Some(decision) = combat_decision.filter(|_| potion_use.is_none()) {
        if command.eq_ignore_ascii_case("WAIT") {
            seed_start_compare_or_defer_combat_transition(
                report,
                action,
                "combat decision refresh",
                &post.message,
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(sim, false),
                pending_combat_assertion,
                reconciled_deferred_action_steps,
            );
            return SeedStartPreDispatch::Handled;
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
                    return SeedStartPreDispatch::Boundary(boundary);
                }
            };
        let exhume_selected_card_id =
            if matches!(decision_action, RunAction::ChooseExhaustSelect { .. }) {
                choose_index(command)
                    .and_then(|index| seed_start_exhume_selected_card_id(sim, index))
            } else {
                None
            };
        let next = apply_run_action(sim, decision_action);
        let Ok(mut next) = next else {
            let reason = push_sim_unsupported(report, action, label, next.err().unwrap());
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_combat_path".to_owned(),
                reason,
            });
        };
        if action.step == 904 {
            eprintln!(
                "DEBUG step=904 phase={:?} reward={:?}",
                next.phase, next.reward
            );
        }
        // CommunicationMod's one-card draw grids resolve on the card click;
        // there is no separate CONFIRM command for Secret Weapon/Technique.
        // Keep the core's explicit choose/confirm API, but translate the
        // target transport action into both authoritative steps here.
        if decision == SeedStartCombatDecision::DrawSelect
            && command_head.eq_ignore_ascii_case("CHOOSE")
        {
            next = match apply_run_action(&next, RunAction::ConfirmDrawSelect) {
                Ok(next) => next,
                Err(error) => {
                    let reason = push_sim_unsupported(report, action, label, error);
                    return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason,
                    });
                }
            };
        }
        if next.phase == RunPhase::Event && next.combat.is_none() && next.event.is_some() {
            compare_subset(
                report,
                action,
                "event combat",
                seed_start_event_observed_subset(&post.message),
                seed_start_event_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Event;
            return SeedStartPreDispatch::Handled;
        }
        let burning_pact_deferred_selection = if decision_action == RunAction::ConfirmExhaustSelect
        {
            seed_start_burning_pact_deferred_selection_state(sim, &next)
        } else {
            None
        };
        let burning_pact_deferred_selection_matches = burning_pact_deferred_selection
            .as_ref()
            .is_some_and(|transient| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_burning_pact_selected_card_is_absent_from_observed_exhaust(
                        sim,
                        &post.message,
                    )
                    && seed_start_combat_subsets_match(
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(transient, false),
                    )
            });
        // DualWieldAction ticks duration when opening HandCardSelectScreen. If
        // that duration completes before CONFIRM (large frame delta under CM /
        // heavy VFX), GameActionManager skips the retrieval update that creates
        // MakeTempCardInHandAction copies. The selected card remains owned by
        // the closed selection screen — absent from every serialized pile —
        // and re-enters via end-turn DiscardAction leftover-selectedCards
        // settlement (pending_hidden → discard). Only force-exhausted Dual
        // Wield (Havoc / Mayhem / Distilled Chaos) is eligible: the source is
        // already in exhaust/discard when the select opens.
        let dual_wield_skipped_retrieval = if decision_action == RunAction::ConfirmHandSelect {
            seed_start_dual_wield_skipped_retrieval_state(sim)
        } else {
            None
        };
        let dual_wield_skipped_retrieval_matches = dual_wield_skipped_retrieval
            .as_ref()
            .is_some_and(|transient| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(transient, false),
                    )
            });
        let source_hand_settlement_frame = decision_action == RunAction::ConfirmHandSelect
            && seed_start_hand_select_confirm_source_frame(sim, &next, &post.message);
        let gambling_chip_source_settlement =
            matches!(decision_action, RunAction::ConfirmExhaustSelect)
                .then(|| seed_start_gambling_chip_source_settlement_state(sim, &post.message))
                .flatten();
        let headbutt_discard_select_source_settlement_frame =
            matches!(decision_action, RunAction::ChooseDiscardSelect { .. })
                && seed_start_headbutt_discard_select_source_settlement_frame(
                    sim,
                    &next,
                    &post.message,
                    &decision_action,
                );
        let put_on_deck_selected_card_id = (decision_action == RunAction::ConfirmHandSelect)
            .then(|| seed_start_put_on_deck_selected_card_id(sim))
            .flatten();
        let source_card_reward_frame =
            matches!(&decision_action, RunAction::ChooseCombatCardReward { .. })
                && seed_start_card_reward_choose_source_frame(sim, &post.message);
        if burning_pact_deferred_selection_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Burning Pact deferred selection transient".to_owned(),
            });
            next = burning_pact_deferred_selection.expect("matching Burning Pact transient exists");
        } else if dual_wield_skipped_retrieval_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Dual Wield skipped retrieval frame".to_owned(),
            });
            next = dual_wield_skipped_retrieval
                .expect("matching Dual Wield skipped-retrieval state exists");
        } else if let Some((lag_state, selected_cards)) = gambling_chip_source_settlement {
            // Advance from the lag frame (selected cards left hand only). Fully
            // settled GC would discard + redraw and can fire Unceasing Top into a
            // hand the CONFIRM frame never shows; keep selected cards parked so
            // later END can reintroduce them like other select limbo paths.
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Gambling Chip source settlement frame".to_owned(),
            });
            next = lag_state;
            if let Some(combat) = next.combat.as_mut() {
                // Park first selected card in pending_hidden; remaining selected
                // cards append to limbo so IDs stay unique and END can flush.
                let mut selected = selected_cards;
                if let Some(first) = selected.first().copied() {
                    if combat.pending_hidden_hand_card_until_end_turn.is_none() {
                        combat.pending_hidden_hand_card_until_end_turn = Some(first);
                        selected.remove(0);
                    }
                }
                combat.piles.limbo.extend(selected);
            }
        } else if source_hand_settlement_frame
            || source_card_reward_frame
            || headbutt_discard_select_source_settlement_frame
        {
            if source_card_reward_frame
                && pending_combat_assertion.as_ref().is_some_and(|pending| {
                    pending.failed_reconciliation.is_none()
                        && pending
                            .transitions
                            .iter()
                            .all(|transition| transition.transient_matches)
                })
            {
                let pending = pending_combat_assertion
                    .take()
                    .expect("pending combat assertion checked above");
                for transition in pending.transitions {
                    report.verified.push(VerifiedTransition {
                        action_step: transition.action.step,
                        command: transition.action.command,
                        label: format!(
                            "{} (reconciled at source card reward settlement)",
                            transition.label
                        ),
                    });
                    reconciled_deferred_action_steps.push(transition.action.step);
                }
            }
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: if source_hand_settlement_frame {
                    "hand select confirm (source hand settlement frame)".to_owned()
                } else if source_card_reward_frame {
                    "combat card reward choose (source hand settlement frame)".to_owned()
                } else {
                    "discard select (source put-on-draw settlement frame)".to_owned()
                },
            });
        } else if put_on_deck_selected_card_id.is_some() {
            let observed = seed_start_combat_observed_subset(&post.message);
            // Rebuild from pre-CONFIRM so source settlement (Dark Embrace) draws
            // the real top when PutOnDeckAction skipped retrieval. Deriving the
            // candidate by stripping the selected card from the normal post
            // state is wrong once on-exhaust already drew that card into hand.
            let skipped_retrieval = seed_start_put_on_deck_skipped_retrieval_state(sim);
            let skipped_retrieval_matches =
                skipped_retrieval.as_ref().is_some_and(|(skipped, _)| {
                    seed_start_is_stable_combat_post_state(&post.message)
                        && seed_start_combat_subsets_match(
                            observed.clone(),
                            seed_start_simulated_combat_subset(skipped, false),
                        )
                });
            if skipped_retrieval_matches {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "put-on-deck skipped retrieval frame".to_owned(),
                });
                // The selected card remains owned by the closed target
                // selection screen, so later combat commands must continue
                // from the source-backed skipped-retrieval result.
                // Reinsert via end-turn discard on the first non-empty-hand
                // END (see deferred_put_on_deck_card settlement above).
                // Flag starts false (= not yet forced to multi-card after an
                // empty-hand miss).
                let (skipped_state, selected_card) =
                    skipped_retrieval.expect("matching skipped-retrieval state exists");
                *pending_put_on_deck_card = Some((selected_card, false));
                next = skipped_state;
            } else {
                seed_start_compare_or_defer_combat_transition(
                    report,
                    action,
                    label,
                    &post.message,
                    observed,
                    seed_start_simulated_combat_subset(&next, false),
                    pending_combat_assertion,
                    reconciled_deferred_action_steps,
                );
            }
        } else if let Some(selected_card_id) = exhume_selected_card_id {
            let observed = seed_start_combat_observed_subset(&post.message);
            let full_matches = seed_start_combat_subsets_match(
                observed.clone(),
                seed_start_simulated_combat_subset(&next, false),
            );
            let skipped_return = choose_index(command)
                .and_then(|index| seed_start_exhume_skipped_return_state(sim, index));
            let skipped_return_matches = skipped_return.as_ref().is_some_and(|skipped| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        observed.clone(),
                        seed_start_simulated_combat_subset(skipped, false),
                    )
            });
            let transient =
                seed_start_simulated_exhume_selection_transient_subset(&next, selected_card_id);
            let transient_matches = transient.as_ref().is_some_and(|transient| {
                seed_start_combat_subsets_match(observed.clone(), transient.clone())
            });
            if full_matches {
                seed_start_compare_or_defer_combat_transition(
                    report,
                    action,
                    label,
                    &post.message,
                    observed,
                    seed_start_simulated_combat_subset(&next, false),
                    pending_combat_assertion,
                    reconciled_deferred_action_steps,
                );
            } else if skipped_return_matches {
                // Havoc-forced Exhume can settle source + Dark Embrace without
                // retrieving the chosen exhaust card (6a06a48 step 561).
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Exhume skipped return retrieval frame".to_owned(),
                });
                next = skipped_return.expect("matching skipped-return state exists");
            } else if transient_matches {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "Exhume selection post-click transient".to_owned(),
                });
            } else {
                seed_start_compare_or_defer_combat_transition(
                    report,
                    action,
                    label,
                    &post.message,
                    observed,
                    seed_start_simulated_combat_subset(&next, false),
                    pending_combat_assertion,
                    reconciled_deferred_action_steps,
                );
            }
        } else {
            seed_start_compare_or_defer_combat_transition(
                report,
                action,
                label,
                &post.message,
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(&next, false),
                pending_combat_assertion,
                reconciled_deferred_action_steps,
            );
        }
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }

    if let Some(potion_use) = potion_use {
        let is_smoke_bomb = sim.potion_at_slot(potion_use.slot) == Some(Potion::SmokeBomb);
        let is_distilled_chaos =
            sim.potion_at_slot(potion_use.slot) == Some(Potion::DistilledChaos);
        let target = seed_start_potion_command_target(sim, &potion_use);
        let next = apply_run_action(
            sim,
            RunAction::UsePotion {
                slot: potion_use.slot,
                target,
            },
        );
        let Ok(next) = next else {
            let reason =
                push_sim_unsupported(report, action, "combat potion use", next.err().unwrap());
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_combat_path".to_owned(),
                reason,
            });
        };
        if is_smoke_bomb {
            if next.phase != RunPhase::Idle || next.combat.is_some() || next.reward.is_some() {
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
                return SeedStartPreDispatch::Boundary(boundary);
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
                *smoke_bomb_ui = Some(SmokeBombUiState::Escaping {
                    source: Box::new(source),
                    action: action.clone(),
                    pending_commands: Vec::new(),
                    transient_matches,
                });
                return SeedStartPreDispatch::Handled;
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
                *phase = SeedStartPhase::Reward;
                *smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                    pending_proceeds: Vec::new(),
                    queued_end: None,
                    queued_end_source: None,
                });
                return SeedStartPreDispatch::Handled;
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        if seed_start_run_has_combat_card_reward(&next) {
            seed_start_compare_or_defer_combat_transition(
                report,
                action,
                "combat potion card reward",
                &post.message,
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(&next, false),
                pending_combat_assertion,
                reconciled_deferred_action_steps,
            );
            *sim = next;
            return SeedStartPreDispatch::Handled;
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
            *phase = SeedStartPhase::Reward;
            return SeedStartPreDispatch::Handled;
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        let simulated = seed_start_simulated_combat_subset(&next, false);
        if is_distilled_chaos
            && seed_start_distilled_chaos_source_settlement_frame(&post.message, &simulated)
        {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Distilled Chaos source settlement frame".to_owned(),
            });
        } else {
            seed_start_compare_or_defer_combat_transition(
                report,
                action,
                "combat potion use",
                &post.message,
                seed_start_combat_observed_subset(&post.message),
                simulated,
                pending_combat_assertion,
                reconciled_deferred_action_steps,
            );
        }
        *sim = next;
        return SeedStartPreDispatch::Handled;
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
        return SeedStartPreDispatch::Handled;
    }

    if !(is_play_command || command_head.eq_ignore_ascii_case("END")) {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_combat_path".to_owned(),
            reason: format!("seed-start verifier does not support combat command {command:?}"),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
    }

    let Some(combat) = sim.combat.as_ref() else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "invalid_simulator_state".to_owned(),
            reason: "seed-start verifier entered its combat phase without core combat state"
                .to_owned(),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
    };
    if let Some(reason) = unsupported_seed_start_combat_command(combat, command) {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason,
        });
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_combat_path".to_owned(),
            reason: "unsupported card in seed-start combat".to_owned(),
        });
    }

    let Some(combat_action) =
        combat_action_from_command_with_observed_hand(command, combat, Some(&pre.message))
    else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_combat_path".to_owned(),
            reason: format!("seed-start verifier could not parse combat command {command:?}"),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
    };
    if action.step == 1384 {
        eprintln!(
            "DEBUG step=1384 command={command:?} action={combat_action:?} sim_hand={:?} observed_hand={:?}",
            combat
                .piles
                .hand
                .iter()
                .map(|card| simulated_card_projection_key(card))
                .collect::<Vec<_>>(),
            pre.message
                .pointer("/game_state/combat_state/hand")
                .and_then(Value::as_array)
                .map(|cards| cards.iter().map(observed_card_projection_key).collect::<Vec<_>>()),
        );
    }
    if is_final_combat_blow(sim, combat_action) {
        if action.step == 904 {
            eprintln!(
                "DEBUG before victory step=904 phase={:?} reward={:?} combat_phase={:?}",
                sim.phase,
                sim.reward,
                sim.combat.as_ref().map(|combat| combat.phase)
            );
        }
        // Burning Pact deferred selection can leave a card in pending_hidden
        // when combat ends on a lethal blow. The next combat's master-deck
        // shuffle still contains that card, and the source also surfaces a
        // residual discard slot that participates in the first mid-combat
        // reshuffle (random-fidelity-6e6f4f8c Twin Strike+). Carry that
        // residual across as a distinct instance for discard/shuffle parity.
        //
        // Do NOT residual a put-on-deck limbo card that was only promoted onto
        // pending_hidden on this same END. End-of-turn powers (e.g. Combust)
        // can win combat before hand discard settles that card; the real game
        // does not keep an extra discard instance next combat
        // (random-fidelity-f3c0d2bea83d9313 Dropkick after Warcry).
        let pending_cross_combat_card = if deferred_put_on_deck_card.is_some() {
            None
        } else {
            sim.combat
                .as_ref()
                .and_then(|combat| combat.pending_hidden_hand_card_until_end_turn)
        };
        // Drop any held put-on-deck limbo that never settled (empty-hand hold
        // on a lethal END, or just-promoted card above). Master deck already
        // supplies the live copy next combat.
        *pending_put_on_deck_card = None;
        let next = apply_combat_action_on_run(sim, combat_action);
        let Ok(next) = next else {
            let reason = push_sim_unsupported(
                report,
                action,
                "seed-start combat victory",
                next.err().unwrap(),
            );
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_combat_path".to_owned(),
                reason,
            });
        };
        if action.step == 904 {
            eprintln!(
                "DEBUG victory step=904 phase={:?} potion_counter={} potion_chance={} reward={:?}",
                next.phase, next.potion_rng_counter, next.potion_chance, next.reward
            );
        }
        let label = combat_label_for_action(combat_action, sim);
        let final_boss_complete = seed_start_is_final_boss_victory(&next);
        if next.phase == RunPhase::Event && next.combat.is_none() && next.event.is_some() {
            compare_subset(
                report,
                action,
                "event combat",
                seed_start_event_observed_subset(&post.message),
                seed_start_event_simulated_subset(&next),
            );
        } else if final_boss_complete {
            compare_subset(
                report,
                action,
                &label,
                seed_start_victory_observed_subset(&post.message),
                seed_start_victory_simulated_subset(&next),
            );
        } else {
            compare_subset(
                report,
                action,
                &label,
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&next),
            );
        }
        let event_combat_complete = next.phase == RunPhase::Event && next.event.is_some();
        *seed_sim = Some(next);
        if pending_cross_combat_card.is_some() {
            *pending_cross_combat_discard = pending_cross_combat_card;
        }
        *phase = if event_combat_complete {
            SeedStartPhase::Event
        } else if final_boss_complete {
            SeedStartPhase::Proceed
        } else {
            SeedStartPhase::Reward
        };
        return SeedStartPreDispatch::Handled;
    }

    if action.step == 299 {
        let mut shuffle_rng = sim.combat.as_ref().map(|c| c.rng.shuffle_rng.clone());
        let mut card_random_rng = sim.combat.as_ref().map(|c| c.rng.card_random_rng.clone());
        eprintln!(
            "PRE_END_DEBUG hand={:?} draw={:?} discard={:?} shuffle_counter={} shuffle_seed={:?} card_random_counter={} card_random_seed={:?}",
            sim.combat.as_ref().map(|c| c.piles.hand.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c.piles.draw_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c.piles.discard_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c.rng.shuffle_rng.counter()).unwrap_or_default(),
            shuffle_rng.as_mut().map(|rng| rng.random_long()),
            sim.combat.as_ref().map(|c| c.rng.card_random_rng.counter()).unwrap_or_default(),
            card_random_rng.as_mut().map(|rng| rng.random_long()),
        );
    }
    let pre_action_run = sim.clone();
    if action.step == 804 {
        let mut rng = sim
            .combat
            .as_ref()
            .map(|combat| combat.rng.card_random_rng.clone());
        let mut bounds_rng = rng.clone();
        eprintln!(
            "DEBUG pre step=804 card_rng_counter={} next99={:?} bounds={:?}",
            rng.as_ref().map_or(0, |value| value.counter()),
            rng.as_mut().map(|value| value.random_int(99)),
            bounds_rng.as_mut().map(|value| {
                [17, 18, 21, 22, 19]
                    .into_iter()
                    .map(|bound| value.random_int(bound))
                    .collect::<Vec<_>>()
            }),
        );
    }
    if action.step == 490 {
        eprintln!(
            "DEBUG pre step=490 command={} hand={:?} draw={:?}",
            action.command,
            sim.combat.as_ref().map(|c| c
                .piles
                .hand
                .iter()
                .map(simulated_card_projection_key)
                .collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c
                .piles
                .draw_pile
                .iter()
                .map(simulated_card_projection_key)
                .collect::<Vec<_>>()),
        );
    }
    if (490..=512).contains(&action.step) {
        eprintln!(
            "TRACE_PRE step={} cmd={} energy={:?} card_rng={:?} hand={:?} draw={:?} discard={:?}",
            action.step,
            action.command,
            sim.combat.as_ref().map(|c| c.player.energy),
            sim.combat.as_ref().map(|c| c.rng.card_random_rng.counter()),
            sim.combat.as_ref().map(|c| c
                .piles
                .hand
                .iter()
                .map(simulated_card_projection_key)
                .collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c
                .piles
                .draw_pile
                .iter()
                .map(simulated_card_projection_key)
                .collect::<Vec<_>>()),
            sim.combat.as_ref().map(|c| c
                .piles
                .discard_pile
                .iter()
                .map(simulated_card_projection_key)
                .collect::<Vec<_>>()),
        );
        if action.step == 512 {
            eprintln!(
                "DEBUG_CARD_RANDOM_STATE floor={} rng_state={:?}",
                sim.current_floor,
                sim.combat.as_ref().map(|c| c.rng.card_random_rng.state())
            );
        }
    }
    let next = apply_combat_action_on_run(sim, combat_action);
    let Ok(mut next) = next else {
        let reason = push_sim_unsupported(
            report,
            action,
            "seed-start combat transition",
            next.err().unwrap(),
        );
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_combat_path".to_owned(),
            reason,
        });
    };
    if action.step == 943 {
        eprintln!(
            "DEBUG step=943 post hand={:?} rng={:?}",
            next.combat.as_ref().map(|combat| {
                combat
                    .piles
                    .hand
                    .iter()
                    .map(simulated_card_projection_key)
                    .collect::<Vec<_>>()
            }),
            next.combat
                .as_ref()
                .map(|combat| combat.rng.card_random_rng.counter()),
        );
    }
    if action.step == 1377 {
        eprintln!(
            "DEBUG step=1377 sim_hand={:?} sim_draw={:?}",
            next.combat.as_ref().map(|combat| {
                combat
                    .piles
                    .hand
                    .iter()
                    .map(|card| simulated_card_projection_key(card))
                    .collect::<Vec<_>>()
            }),
            next.combat.as_ref().map(|combat| {
                combat
                    .piles
                    .draw_pile
                    .iter()
                    .map(|card| simulated_card_projection_key(card))
                    .collect::<Vec<_>>()
            })
        );
    }
    if (490..=512).contains(&action.step) || (804..=808).contains(&action.step) {
        eprintln!(
            "TRACE_WINDOW step={} cmd={} next_rng={:?} pre_hand={:?} pre_draw={:?} next_hand={:?} next_draw={:?} next_discard={:?}",
            action.step,
            action.command,
            next.combat.as_ref().map(|c| c.rng.card_random_rng.counter()),
            pre_action_run.combat.as_ref().map(|c| c.piles.hand.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            pre_action_run.combat.as_ref().map(|c| c.piles.draw_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.piles.hand.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.piles.draw_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.piles.discard_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
        );
    }
    if (299..=304).contains(&action.step) {
        eprintln!(
            "SETTLE_DEBUG step={} command={} next_hand={:?} next_draw={:?} next_discard={:?} block={} powers={:?}",
            action.step,
            command,
            next.combat.as_ref().map(|c| c.piles.hand.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.piles.draw_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.piles.discard_pile.iter().map(simulated_card_projection_key).collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c.player.block).unwrap_or_default(),
            next.combat.as_ref().map(|c| c.player.powers),
        );
    }
    if (848..=851).contains(&action.step) {
        eprintln!(
            "DEBUG combat step={} cmd={} sim_hand={:?} sim_draw={:?} sim_discard={:?}",
            action.step,
            action.command,
            next.combat.as_ref().map(|c| c
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c
                .piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()),
        );
    }
    if command.eq_ignore_ascii_case("END") && action.step >= 1214 {
        let combat = next.combat.as_ref().expect("combat debug state");
        eprintln!(
            "END_DEBUG_AFTER step={} discard={:?} hidden={:?}",
            action.step,
            combat
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            combat
                .pending_hidden_hand_card_until_end_turn
                .map(|card| card.content_id)
        );
    }
    if (299..=304).contains(&action.step) {
        eprintln!(
            "FINAL_DEBUG step={} pending={:?} hand={:?} draw={:?} discard={:?}",
            action.step,
            deferred_put_on_deck_card.map(|c| (c.id, simulated_card_projection_key(&c))),
            next.combat.as_ref().map(|c| c
                .piles
                .hand
                .iter()
                .map(|c| (c.id, simulated_card_projection_key(c)))
                .collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c
                .piles
                .draw_pile
                .iter()
                .map(|c| (c.id, simulated_card_projection_key(c)))
                .collect::<Vec<_>>()),
            next.combat.as_ref().map(|c| c
                .piles
                .discard_pile
                .iter()
                .map(|c| (c.id, simulated_card_projection_key(c)))
                .collect::<Vec<_>>()),
        );
    }
    if let Some(card) = deferred_cross_combat_discard {
        let Some(combat) = next.combat.as_mut() else {
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_deferred_combat_discard_state".to_owned(),
                reason: "deferred combat discard reached END without combat state".to_owned(),
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        };
        combat
            .piles
            .discard_pile
            .push(card_with_replay_transient_id(combat, card));
    }
    let label = combat_label_for_action(combat_action, sim);
    let observed = seed_start_combat_observed_subset(&post.message);
    let simulated = seed_start_simulated_combat_subset(&next, false);
    if (1960..=1990).contains(&action.step) {
        let upcoming_rolls = next.combat.as_ref().map(|combat| {
            let mut rng = combat.rng.monster_rng.clone();
            (0..4).map(|_| rng.random_int(99)).collect::<Vec<_>>()
        });
        eprintln!(
            "DEBUG target-window step={} cmd={} sim_move={:?} sim_history={:?} sim_hp={:?} rng={} upcoming={:?}",
            action.step,
            action.command,
            next.combat
                .as_ref()
                .and_then(|combat| combat.monsters.get(2))
                .map(|monster| (monster.alive, monster.mode_shift, monster.intent)),
            next.combat
                .as_ref()
                .and_then(|combat| combat.monsters.get(2))
                .map(|monster| monster.move_history.clone()),
            next.combat.as_ref().map(|combat| combat.player.hp),
            next.combat
                .as_ref()
                .map(|combat| combat.rng.monster_rng.counter())
                .unwrap_or_default(),
            upcoming_rolls,
        );
    }
    let exhaust_as_discard = seed_start_simulated_combat_subset_with_exhaust_as_discard(&next);
    let writhing_mass_parasite_frame =
        if command.eq_ignore_ascii_case("END") && pending_deck_assertion.is_none() {
            seed_start_writhing_mass_parasite_settlement_frame(
                &pre_action_run,
                &next,
                &observed,
                &simulated,
            )
        } else {
            None
        };
    if let Some((transient_deck, expected_deck)) = writhing_mass_parasite_frame {
        *pending_deck_assertion = Some(PendingDeckAssertion {
            action: action.clone(),
            label: label.clone(),
            related_actions: Vec::new(),
            transient_decks: vec![transient_deck],
            expected_deck,
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if command.eq_ignore_ascii_case("END")
        && seed_start_end_turn_card_reward_source_frame(&post.message, &pre_action_run)
    {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (source card reward frame)".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if command.eq_ignore_ascii_case("END")
        && seed_start_end_turn_extra_discard_source_frame(&post.message, &observed, &simulated)
    {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (source appended discard frame)".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if command.eq_ignore_ascii_case("END")
        && seed_start_end_turn_source_pile_settlement_frame(
            &post.message,
            &observed,
            &simulated,
            &next,
        )
    {
        if action.step == 800 {
            eprintln!("DEBUG end settlement step=800 observed={observed} simulated={simulated}");
        }
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (source pile settlement frame)".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if command.eq_ignore_ascii_case("END")
        && next
            .combat
            .as_ref()
            .is_some_and(|combat| !combat.piles.exhaust_pile.is_empty())
        && !seed_start_combat_subsets_match(observed.clone(), simulated.clone())
        && seed_start_combat_subsets_match(observed.clone(), exhaust_as_discard)
    {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (exhaust pile settlement frame)".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    if command_head.eq_ignore_ascii_case("PLAY")
        && seed_start_combat_pile_source_settlement_frame(&post.message, &next)
    {
        if (801..=805).contains(&action.step)
            || (815..=850).contains(&action.step)
            || action.step == 1683
        {
            eprintln!(
                "DEBUG settlement step={} cmd={} observed={} simulated={}",
                action.step,
                action.command,
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(&next, false),
            );
        }
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "combat hand order (source settlement frame)".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    let copied_attack = seed_start_copied_attack_expectation(combat, combat_action);
    if seed_start_warcry_source_settlement_frame_matches(
        &pre_action_run,
        combat_action,
        &post.message,
    ) {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "Warcry source settlement frame".to_owned(),
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    let stable_projection_matches =
        seed_start_combat_subsets_match(observed.clone(), simulated.clone());
    if seed_start_classify_copied_attack_frame(
        stable_projection_matches,
        copied_attack,
        &post.message,
    ) == CopiedAttackFrame::Deferred
    {
        let transient_matches =
            seed_start_compare_transient_combat_subset(report, action, &label, observed, simulated);
        let pending = pending_combat_assertion.get_or_insert_default();
        pending.requires_stable_frame_before_next_command = true;
        if pending.cancelled_state.is_none() {
            let cancelled_run = pre_action_run;
            pending.cancelled_state =
                apply_combat_action_on_run(&cancelled_run, combat_action).ok();
        }
        pending.transitions.push(PendingCombatTransition {
            action: action.clone(),
            label,
            transient_matches,
        });
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
    seed_start_compare_or_defer_combat_transition(
        report,
        action,
        &label,
        &post.message,
        observed,
        simulated,
        pending_combat_assertion,
        reconciled_deferred_action_steps,
    );
    *sim = next;
    SeedStartPreDispatch::Handled
}

fn seed_start_exhume_selected_card_id(run: &RunState, ui_index: usize) -> Option<CardId> {
    let combat = run.combat.as_ref()?;
    let select = combat.exhaust_select()?;
    if select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand {
        return None;
    }
    combat
        .piles
        .exhaust_pile
        .iter()
        .filter(|card| {
            card.content_id != sts_core::content::cards::EXHUME_ID
                && card.content_id != sts_core::content::cards::EXHUME_PLUS_ID
        })
        .nth(ui_index)
        .map(|card| card.id)
}

/// Rebuild Exhume CHOOSE without retrieving the selected exhaust card.
///
/// Source (parked Exhume) still settles into exhaust and Dark Embrace still
/// draws; the chosen exhaust card remains in exhaust. Used only when that
/// stable observed frame matches (6a06a48 Havoc→Exhume).
fn seed_start_exhume_skipped_return_state(pre: &RunState, ui_index: usize) -> Option<RunState> {
    let combat = pre.combat.as_ref()?;
    let select = combat.exhaust_select()?;
    if select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand {
        return None;
    }
    let mut transient = pre.clone();
    let combat = transient.combat.as_mut()?;
    sts_core::combat::choose_exhaust_select(combat, ui_index).ok()?;
    sts_core::combat::confirm_exhume_select_skipped_return(combat).ok()?;
    Some(transient)
}

fn seed_start_simulated_exhume_selection_transient_subset(
    run: &RunState,
    selected_card_id: CardId,
) -> Option<Value> {
    let mut transient = run.clone();
    let combat = transient.combat.as_mut()?;
    let selected_index = combat
        .piles
        .hand
        .iter()
        .position(|card| card.id == selected_card_id)?;
    // ExhumeAction adds the selected card to the hand in a later action update.
    // The card is therefore absent from every visible pile in the short frame
    // after the grid click, while the played Exhume has already reached exhaust.
    combat.piles.hand.remove(selected_index);
    Some(seed_start_simulated_combat_subset(&transient, false))
}

fn seed_start_put_on_deck_selected_card_id(run: &RunState) -> Option<CardId> {
    let combat = run.combat.as_ref()?;
    let select = combat.hand_select()?;
    if !matches!(
        select.purpose,
        HandSelectPurpose::WarcryPutOnDraw
            | HandSelectPurpose::ThinkingAheadPutOnDraw
            | HandSelectPurpose::ForethoughtPutOnDraw
    ) {
        return None;
    }
    let selected_index = select.selected_hand_index?;
    combat.piles.hand.get(selected_index).map(|card| card.id)
}

/// DualWieldAction skipped-retrieval candidate (force-exhausted Dual Wield).
///
/// Rebuild from the pre-CONFIRM source: close the DualWieldCopy hand select
/// without creating temporary copies, and park the selected card in
/// `pending_hidden_hand_card_until_end_turn` so end-turn discard reintroduces
/// it (matching leftover `HandCardSelectScreen.selectedCards` settlement via
/// `DiscardAction` when `wereCardsRetrieved` is still false).
///
/// Eligible only when Dual Wield's source is already in exhaust or discard
/// (Havoc / Mayhem / Distilled Chaos force-exhaust). Ordinary hand Dual Wield
/// keeps the core copy path authoritative; this candidate is accepted only
/// when the stable observed frame matches it.
fn seed_start_dual_wield_skipped_retrieval_state(source: &RunState) -> Option<RunState> {
    let source_combat = source.combat.as_ref()?;
    let select = source_combat.hand_select()?;
    if select.purpose != HandSelectPurpose::DualWieldCopy {
        return None;
    }
    let selected_index = select.selected_hand_index?;
    if selected_index >= source_combat.piles.hand.len() {
        return None;
    }
    if source_combat
        .pending_hidden_hand_card_until_end_turn
        .is_some()
    {
        return None;
    }
    let source_already_settled = source_combat
        .piles
        .exhaust_pile
        .iter()
        .chain(source_combat.piles.discard_pile.iter())
        .any(|card| card.id == select.source_card_id);
    if !source_already_settled {
        return None;
    }

    let mut transient = source.clone();
    let combat = transient.combat.as_mut()?;
    let (select, _pending) = combat.take_hand_select()?;
    let selected_index = select.selected_hand_index?;
    if selected_index >= combat.piles.hand.len() {
        return None;
    }
    let selected = combat.piles.hand.remove(selected_index);
    combat.pending_hidden_hand_card_until_end_turn = Some(selected);
    Some(transient)
}

fn seed_start_put_on_deck_card_settlement(run: &RunState) -> bool {
    let purpose = run.combat.as_ref().and_then(|combat| combat.hand_select());
    matches!(
        purpose.map(|select| select.purpose),
        Some(
            HandSelectPurpose::WarcryPutOnDraw
                | HandSelectPurpose::ThinkingAheadPutOnDraw
                | HandSelectPurpose::ForethoughtPutOnDraw
        )
    )
}

/// Rebuild put-on-deck skipped retrieval from the pre-CONFIRM source state.
///
/// `PutOnDeckAction` stores selected cards in `HandCardSelectScreen.selectedCards`.
/// If the action has already completed when CONFIRM closes that screen, the
/// action manager skips its retrieval update, leaving the card outside every
/// serialized pile. Source settlement still runs (including Dead Branch on
/// Warcry exhaust), so Dark Embrace draws the pre-select top of draw rather
/// than the never-placed selected card.
///
/// Returns `(post_state, stuck_selected_card)`. The replay caller retains the
/// typed card and reintroduces it via end-turn discard on the first eligible
/// non-empty-hand END (`pending_hidden_hand_card_until_end_turn`).
fn seed_start_put_on_deck_skipped_retrieval_state(
    pre: &RunState,
) -> Option<(RunState, CardInstance)> {
    if !seed_start_put_on_deck_card_settlement(pre) {
        return None;
    }
    // Mirror apply_hand_select_confirm: skipped retrieval still exhausts the
    // put-on-deck source and must roll Dead Branch the same way as CONFIRM.
    sts_core::run::apply_hand_select_confirm_skipped_put_on_deck_retrieval(pre).ok()
}

fn card_with_replay_transient_id(combat: &CombatState, mut card: CardInstance) -> CardInstance {
    let next_id = combat
        .piles
        .hand
        .iter()
        .chain(combat.piles.draw_pile.iter())
        .chain(combat.piles.discard_pile.iter())
        .chain(combat.piles.exhaust_pile.iter())
        .chain(combat.piles.limbo.iter())
        .map(|card| card.id.get())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    card.id = CardId::new(next_id);
    card
}

fn seed_start_is_stable_combat_post_state(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    game.get("screen_type").and_then(Value::as_str) == Some("NONE")
        && game.get("action_phase").and_then(Value::as_str) == Some("WAITING_ON_USER")
        && game
            .get("current_action")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        && message.get("ready_for_command").and_then(Value::as_bool) == Some(true)
}

fn seed_start_end_turn_source_pile_settlement_frame(
    post_message: &Value,
    observed: &Value,
    simulated: &Value,
    next: &RunState,
) -> bool {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }

    let mut observed_without_piles = observed.clone();
    let mut simulated_without_piles = simulated.clone();
    for value in [&mut observed_without_piles, &mut simulated_without_piles] {
        if let Some(object) = value.as_object_mut() {
            for key in ["hand_ids", "draw_ids", "discard_ids"] {
                object.remove(key);
            }
        }
    }
    if !seed_start_combat_subsets_match(observed_without_piles, simulated_without_piles) {
        return false;
    }

    let Some(source_combat) = post_message.pointer("/game_state/combat_state") else {
        return false;
    };
    let Some(simulated_combat) = next.combat.as_ref() else {
        return false;
    };
    // Per-pile projections keep duplicate CM uuid listings (residual discard
    // slots). Aggregating those piles without cross-pile collapsing matches the
    // simulator when a cross-combat residual instance sits in discard while the
    // master-deck copy remains in draw.
    let mut observed_cards = Vec::new();
    for pile in ["hand", "draw_pile", "discard_pile", "exhaust_pile"] {
        observed_cards.extend(combat_card_ids(source_combat.get(pile)));
    }
    observed_cards.sort_unstable();
    let mut simulated_cards = Vec::new();
    for pile in [
        &simulated_combat.piles.hand,
        &simulated_combat.piles.draw_pile,
        &simulated_combat.piles.discard_pile,
        &simulated_combat.piles.exhaust_pile,
    ] {
        simulated_cards.extend(cards_to_comm_mod_visible_order(pile.iter()));
    }
    simulated_cards.sort_unstable();
    if observed_cards == simulated_cards {
        return true;
    }

    // A few end-turn captures expose a newly-created status card before the
    // target's queued monster effect has settled. Do not accept missing or
    // substituted normal cards; only an additive transient status is allowed.
    let mut remaining_simulated = simulated_cards;
    let mut source_only_cards = Vec::new();
    for card in observed_cards {
        if let Some(index) = remaining_simulated
            .iter()
            .position(|candidate| candidate == &card)
        {
            remaining_simulated.remove(index);
        } else {
            source_only_cards.push(card);
        }
    }
    remaining_simulated.is_empty()
        && source_only_cards
            .iter()
            .all(|card| matches!(card.as_str(), "Dazed" | "Slimed" | "Wound" | "Burn"))
}

fn seed_start_hand_select_confirm_source_frame(
    run: &RunState,
    settled_run: &RunState,
    post_message: &Value,
) -> bool {
    let Some(combat) = run.combat.as_ref() else {
        return false;
    };
    if !combat
        .hand_select()
        .is_some_and(|select| select.purpose == HandSelectPurpose::ArmamentsUpgrade)
    {
        return false;
    }
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game
            .get("current_action")
            .and_then(Value::as_str)
            .is_some_and(|action| !action.is_empty())
    {
        return false;
    }

    let observed = seed_start_combat_observed_subset(post_message);
    let mut source_frame = seed_start_simulated_combat_subset(run, false);
    source_frame["screen_type"] = json!("NONE");
    if let Some(discard_ids) = seed_start_simulated_combat_subset(settled_run, false)
        .get("discard_ids")
        .cloned()
    {
        source_frame["discard_ids"] = discard_ids;
    }
    seed_start_combat_subsets_match(observed, source_frame)
}

/// Headbutt (and force-played Headbutt via Havoc / Mayhem) resolves put-on-draw
/// on the grid CHOOSE click. CommunicationMod often publishes a stable
/// WAITING_ON_USER frame where the Headbutt source has already settled into
/// discard or exhaust, but the chosen discard card has not yet left discard
/// for the top of the draw pile. The simulator applies both atomically.
///
/// Accept that source lag when the observed combat subset equals the settled
/// sim subset with put-on-draw reversed (draw top restored into discard at the
/// chosen index). Authoritative state remains the settled sim — no hydrate.
fn seed_start_headbutt_discard_select_source_settlement_frame(
    pre: &RunState,
    settled: &RunState,
    post_message: &Value,
    decision_action: &RunAction,
) -> bool {
    let RunAction::ChooseDiscardSelect { index } = *decision_action else {
        return false;
    };
    if !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }
    let Some(pre_combat) = pre.combat.as_ref() else {
        return false;
    };
    let Some(select) = pre_combat.discard_select() else {
        return false;
    };
    if select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return false;
    }
    if index >= pre_combat.piles.discard_pile.len() {
        return false;
    }
    let Some(settled_combat) = settled.combat.as_ref() else {
        return false;
    };
    if settled_combat.discard_select().is_some() {
        return false;
    }
    let selected_key = simulated_card_projection_key(&pre_combat.piles.discard_pile[index]);
    let mut lag = seed_start_simulated_combat_subset(settled, false);
    let Some(draw_ids) = lag.get_mut("draw_ids").and_then(Value::as_array_mut) else {
        return false;
    };
    match draw_ids.last().and_then(Value::as_str) {
        Some(top) if top == selected_key => {
            draw_ids.pop();
        }
        _ => return false,
    }
    let Some(discard_ids) = lag.get_mut("discard_ids").and_then(Value::as_array_mut) else {
        return false;
    };
    if index > discard_ids.len() {
        return false;
    }
    discard_ids.insert(index, Value::String(selected_key));
    let observed = seed_start_combat_observed_subset(post_message);
    seed_start_combat_subsets_match(observed, lag)
}

fn seed_start_card_reward_choose_source_frame(run: &RunState, post_message: &Value) -> bool {
    let Some(combat) = run.combat.as_ref() else {
        return false;
    };
    if combat.combat_card_reward_choices().is_none() {
        return false;
    }
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game
            .get("current_action")
            .and_then(Value::as_str)
            .is_some_and(|action| !action.is_empty())
    {
        return false;
    }

    let observed = seed_start_combat_observed_subset(post_message);
    let mut source_frame = seed_start_simulated_combat_subset(run, false);
    source_frame["screen_type"] = json!("NONE");
    if let Some(object) = source_frame.as_object_mut() {
        object.remove("card_reward_ids");
    }
    seed_start_combat_subsets_match(observed, source_frame)
}

fn seed_start_warcry_source_settlement_frame_matches(
    run: &RunState,
    action: CombatAction,
    post_message: &Value,
) -> bool {
    let CombatAction::PlayCard { card_id, .. } = action else {
        return false;
    };
    if !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }

    let Some(pre_combat) = run.combat.as_ref() else {
        return false;
    };
    let Some(card) = pre_combat.piles.hand.iter().find(|card| card.id == card_id) else {
        return false;
    };
    if !matches!(
        card.content_id,
        sts_core::content::cards::WARCRY_ID | sts_core::content::cards::WARCRY_PLUS_ID
    ) {
        return false;
    }
    let expected_exhaust_key = simulated_card_projection_key(card);
    let pre_exhaust_count = pre_combat
        .piles
        .exhaust_pile
        .iter()
        .filter(|exhausted| simulated_card_projection_key(exhausted) == expected_exhaust_key)
        .count();

    let mut source_frame = run.clone();
    let combat = source_frame
        .combat
        .as_mut()
        .expect("pre-combat state exists");
    let hand_index = combat
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)
        .expect("Warcry source card exists in the pre-action hand");
    let source_card = combat.piles.hand.remove(hand_index);
    combat.piles.exhaust_pile.push(source_card);
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&source_frame, false);
    if !seed_start_combat_subsets_match(observed, simulated) {
        return false;
    }

    let observed_exhaust =
        combat_card_ids(post_message.pointer("/game_state/combat_state/exhaust_pile"));
    observed_exhaust
        .iter()
        .filter(|key| *key == &expected_exhaust_key)
        .count()
        == pre_exhaust_count + 1
}

/// Rebuild Gambling Chip CONFIRM lag: selected cards leave the hand but have
/// not yet entered discard / been redrawn (HandCardSelectScreen still owns them).
///
/// Callers that accept this frame must advance from the lag state (not the fully
/// settled sim), otherwise Unceasing Top can draw into a fully-settled hand that
/// the observed CONFIRM never shows (d3b52f426b3aff94 step 804→805).
fn seed_start_gambling_chip_source_settlement_state(
    run: &RunState,
    post_message: &Value,
) -> Option<(RunState, Vec<CardInstance>)> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let pre_combat = run.combat.as_ref()?;
    let select = pre_combat.exhaust_select()?;
    if select.purpose != ExhaustSelectPurpose::GamblingChip {
        return None;
    }

    // GamblingChipAction retrieves the selected cards from the hand before it
    // queues DrawCardAction and moves those cards to the discard pile. The
    // captured CONFIRM frame can therefore expose the hand removal while all
    // other piles still have their pre-action contents.
    let mut selected_indices = select.selected_hand_indices.clone();
    selected_indices.sort_unstable();
    selected_indices.dedup();
    if selected_indices
        .iter()
        .any(|index| *index >= pre_combat.piles.hand.len())
    {
        return None;
    }

    let mut source_frame = run.clone();
    let mut selected_cards = Vec::with_capacity(selected_indices.len());
    {
        let source_combat = source_frame.combat.as_mut()?;
        for index in selected_indices.into_iter().rev() {
            selected_cards.push(source_combat.piles.hand.remove(index));
        }
        selected_cards.reverse();
        source_combat.decision = None;
    }

    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&source_frame, false);
    if !seed_start_combat_subsets_match(observed, simulated) {
        return None;
    }

    let observed_combat = post_message.pointer("/game_state/combat_state")?;
    let source_combat = source_frame.combat.as_ref()?;
    if combat_card_ids(observed_combat.get("hand"))
        != cards_to_comm_mod_visible_order(&source_combat.piles.hand)
        || combat_card_ids(observed_combat.get("draw_pile"))
            != cards_to_comm_mod_visible_order(&source_combat.piles.draw_pile)
        || combat_card_ids(observed_combat.get("discard_pile"))
            != cards_to_comm_mod_visible_order(&source_combat.piles.discard_pile)
        || combat_card_ids(observed_combat.get("exhaust_pile"))
            != cards_to_comm_mod_visible_order(&source_combat.piles.exhaust_pile)
    {
        return None;
    }
    Some((source_frame, selected_cards))
}

fn seed_start_burning_pact_deferred_selection_state(
    source: &RunState,
    _settled: &RunState,
) -> Option<RunState> {
    let source_combat = source.combat.as_ref()?;
    let select = source_combat.exhaust_select()?;
    if !matches!(
        select.purpose,
        ExhaustSelectPurpose::BurningPactDraw2 | ExhaustSelectPurpose::BurningPactDraw3
    ) || select.source_card.is_none()
        || select.selected_hand_indices.len() != 1
        || source_combat
            .pending_hidden_hand_card_until_end_turn
            .is_some()
    {
        return None;
    }

    let selected_index = select.selected_hand_indices[0];
    if selected_index >= source_combat.piles.hand.len() {
        return None;
    }
    let draw_count = match select.purpose {
        ExhaustSelectPurpose::BurningPactDraw3 => 3,
        _ => 2,
    };

    // Rebuild from the pre-CONFIRM source. ExhaustAction calls tickDuration when
    // it opens HandCardSelectScreen; if that duration completes before CONFIRM
    // (common under heavy ExhaustCardEffect load late in Sentry fights),
    // GameActionManager skips the retrieval update. The selected card remains
    // owned by the closed selection screen — absent from every serialized pile —
    // and Dark Embrace never draws. Only Burning Pact's DrawCardAction and
    // UseCardAction (source → discard) still resolve.
    //
    // Ordinary successful retrieval keeps the core exhaust+DE path. This
    // candidate is used only when the stable observed frame matches it (no
    // selected card in exhaust, hand/draw without the DE card).
    let mut transient = source.clone();
    let combat = transient.combat.as_mut()?;
    let select = combat.take_exhaust_select()?;
    let selected_card = combat.piles.hand.remove(selected_index);
    // Without Runic Pyramid the stuck card re-enters via end-turn discard then
    // shuffle (trace 131acce5 step 254). With Runic Pyramid it stays outside the
    // shuffle while the retained hand is drawn — leave it fully untracked.
    if !combat.relics.contains(&Relic::RunicPyramid) {
        combat.pending_hidden_hand_card_until_end_turn = Some(selected_card);
    }
    if let Err(_err) = sts_core::combat::draw::draw_cards_with_combat_rng(combat, draw_count) {
        return None;
    }
    if let Some(source_card) = select.source_card {
        combat.piles.discard_pile.push(source_card);
    }
    Some(transient)
}

fn seed_start_burning_pact_selected_card_is_absent_from_observed_exhaust(
    source: &RunState,
    post_message: &Value,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(select) = source_combat.exhaust_select() else {
        return false;
    };
    let Some(selected_index) = select.selected_hand_indices.first().copied() else {
        return false;
    };
    let Some(selected_card) = source_combat.piles.hand.get(selected_index) else {
        return false;
    };
    let selected_key = simulated_card_projection_key(selected_card);
    let source_exhaust_count = source_combat
        .piles
        .exhaust_pile
        .iter()
        .filter(|card| simulated_card_projection_key(card) == selected_key)
        .count();
    let observed_exhaust_count =
        combat_card_ids(post_message.pointer("/game_state/combat_state/exhaust_pile"))
            .into_iter()
            .filter(|card| card == &selected_key)
            .count();

    observed_exhaust_count <= source_exhaust_count
}

pub(super) fn seed_start_smoke_bomb_queued_end_destination(
    source: &RunState,
    destination: &RunState,
) -> Option<RunState> {
    // Event combat returns directly to the event's empty combat-reward frame;
    // an END captured during Smoke Bomb's escape does not start a late enemy
    // turn before that frame settles. Ordinary and elite rooms retain the
    // queued-turn behavior below.
    if source.current_room_kind() == Some(RoomKind::Event) {
        return Some(destination.clone());
    }

    // SmokeBomb sets AbstractPlayer.isEscaping and leaves the live combat
    // action queue running until the escape finishes. CommunicationMod can
    // accept END after EscapeCombatAction has committed the on-victory heal
    // but before the room reaches its reward screen. Reproduce that ordering:
    // heal first, then let the queued monster turn resolve.
    let mut queued_source = source.clone();
    queued_source.potions = destination.potions.clone();
    {
        let combat = queued_source.combat.as_mut()?;
        combat.phase = CombatPhase::Won;
        sts_core::apply_burning_blood(combat).ok()?;
        combat.phase = CombatPhase::WaitingForPlayer;
        queued_source.player_hp = combat.player.hp;
        queued_source.player_max_hp = combat.player.max_hp;
    }
    let mut after_end = apply_combat_action_on_run(&queued_source, CombatAction::EndTurn).ok()?;
    // Keep hidden run-level effects already committed by the authoritative
    // Smoke Bomb transition. Resolving the queued monster turn from the
    // transient combat source must not roll reward RNG or potion chance back.
    after_end.treasure_rng_counter = destination.treasure_rng_counter;
    after_end.potion_rng_counter = destination.potion_rng_counter;
    after_end.potion_chance = destination.potion_chance;
    let mut combat = after_end.combat.take()?;
    if combat.phase != CombatPhase::WaitingForPlayer {
        return None;
    }
    combat.phase = CombatPhase::Won;
    after_end.player_hp = combat.player.hp;
    after_end.player_max_hp = combat.player.max_hp;
    after_end.phase = RunPhase::Idle;
    after_end.combat = None;
    after_end.reward = None;
    Some(after_end)
}

fn seed_start_apply_smoke_bomb_queued_command(
    source: &RunState,
    command: &str,
) -> Option<RunState> {
    let combat = source.combat.as_ref()?;
    let combat_action = combat_action_from_command(command, combat)?;
    apply_combat_action_on_run(source, combat_action).ok()
}

fn seed_start_smoke_bomb_queued_command_is_bound(source: &RunState, command: &str) -> bool {
    source
        .combat
        .as_ref()
        .and_then(|combat| combat_action_from_command(command, combat))
        .is_some()
}

fn seed_start_apply_smoke_bomb_event_queued_end_to_next_combat(
    source: &RunState,
    destination: &RunState,
) -> Option<RunState> {
    let combat = source.combat.as_ref()?;
    let mut combat_after_escape = combat.clone();
    combat_after_escape.player.hp = destination.player_hp;
    let after_monster_turn = sts_core::end_player_turn(&combat_after_escape).ok()?;

    // The queued END is delivered after the event reward has closed and the
    // next combat has been created. The target applies the old event combat's
    // monster turn, but leaves the new combat's initial hand and energy intact.
    let mut next = destination.clone();
    let next_combat = next.combat.as_mut()?;
    next_combat.player.hp = after_monster_turn.player.hp;
    next_combat.player.powers = after_monster_turn.player.powers;
    next_combat.player.powers.vulnerable = 0;
    next_combat.player.powers.berserk = 0;
    next_combat.player.vulnerable_just_applied = false;
    next_combat.player.damage_events_this_combat =
        after_monster_turn.player.damage_events_this_combat;
    next.player_hp = after_monster_turn.player.hp;
    Some(next)
}

fn seed_start_smoke_bomb_transient_matches_source(
    message: &Value,
    source: &RunState,
    destination: &RunState,
) -> bool {
    seed_start_combat_subsets_match(
        seed_start_smoke_bomb_transient_observed_subset(message),
        seed_start_smoke_bomb_transient_simulated_subset(source, destination),
    )
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_reward_phase(
    action: &TraceAction,
    post: &TraceState,
    start: &StartRunCommand,
    seed_sim: &mut Option<RunState>,
    smoke_bomb_ui: &mut Option<SmokeBombUiState>,
    pending_map_assertion: &mut Option<PendingMapAssertion>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
    map_path_xs: &mut Vec<i32>,
    combat_index: &mut usize,
    reward_step: &mut usize,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Reward {
        return SeedStartPreDispatch::NotHandled;
    }
    if matches!(
        smoke_bomb_ui.as_ref(),
        Some(SmokeBombUiState::Reward { .. })
    ) && action.command.eq_ignore_ascii_case("PROCEED")
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
                let Some(SmokeBombUiState::Reward {
                    pending_proceeds, ..
                }) = smoke_bomb_ui.as_mut()
                else {
                    unreachable!("Smoke Bomb reward state checked above");
                };
                pending_proceeds.push(action.clone());
            }
            return SeedStartPreDispatch::Handled;
        }
        if screen_type(&post.message) == Some("MAP") {
            let (pending_proceeds, queued_end) = match smoke_bomb_ui.as_ref() {
                Some(SmokeBombUiState::Reward {
                    pending_proceeds,
                    queued_end,
                    ..
                }) => (pending_proceeds.clone(), queued_end.clone()),
                _ => unreachable!("Smoke Bomb reward state checked above"),
            };
            let diff_count = report.unexpected_diffs.len();
            if let Some(boundary) = seed_start_handle_proceed_to_map(
                report,
                action,
                &post.message,
                phase,
                combat_index,
                reward_step,
                map_path_xs,
                seed_sim,
                pending_map_assertion,
            ) {
                return SeedStartPreDispatch::Boundary(boundary);
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
                if queued_end.is_none() {
                    *smoke_bomb_ui = None;
                }
            }
            return SeedStartPreDispatch::Handled;
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
        return SeedStartPreDispatch::Boundary(boundary);
    }
    if action.command.trim().eq_ignore_ascii_case("SKIP") {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_reward_path".to_owned(),
                reason: "seed-start reward skip without initialized reward simulation".to_owned(),
            });
        };
        // A reward-phase SKIP has two distinct meanings in the real game:
        // close an open card reward, or dismiss the parent reward overlay
        // (for example, Tiny House after selecting its boss relic).  The
        // latter is still a Reward phase, but there is no active card reward
        // for CloseCardReward to validate.
        let card_reward_active = sim
            .reward
            .as_ref()
            .is_some_and(|reward| reward.card_reward_is_active());
        let next = apply_run_action(
            sim,
            if card_reward_active {
                RunAction::CloseCardReward
            } else {
                RunAction::SkipReward
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
            return SeedStartPreDispatch::Boundary(boundary);
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
                    if card_reward_active {
                        "skip event card reward"
                    } else {
                        "skip Tiny House reward overlay to event"
                    },
                    seed_start_event_observed_subset(&post.message),
                    seed_start_event_simulated_subset(&next),
                ),
                RunPhase::Shop if next.shop.is_some() => (
                    "skip shop card reward",
                    seed_start_shop_observed_subset(&post.message),
                    seed_start_shop_screen_simulated_subset(&next),
                ),
                RunPhase::Treasure if next.reward.is_none() => (
                    if card_reward_active {
                        "skip card reward to chest"
                    } else {
                        "skip Tiny House reward overlay to chest"
                    },
                    seed_start_treasure_observed_subset(&post.message),
                    seed_start_treasure_simulated_subset(&next),
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
                    return SeedStartPreDispatch::Boundary(boundary);
                }
            }
        };
        compare_subset(report, action, label, observed, simulated);
        *sim = next;
        if seed_start_reward_sequence_complete(sim) {
            *phase = seed_start_phase_after_reward_completion(sim);
        }
        return SeedStartPreDispatch::Handled;
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
                return SeedStartPreDispatch::Boundary(boundary);
            };
            compare_subset(
                report,
                action,
                "final boss proceed to Spire Heart",
                seed_start_spire_heart_observed_subset(&post.message),
                seed_start_spire_heart_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Event;
            return SeedStartPreDispatch::Handled;
        }
        if seed_sim
            .as_ref()
            .is_some_and(seed_start_is_boss_chest_proceed)
        {
            let Some(sim) = seed_sim.as_mut() else {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_reward_path".to_owned(),
                    reason: "seed-start boss reward chest without initialized reward simulation"
                        .to_owned(),
                });
            };
            let next = apply_run_action(sim, RunAction::SkipReward).map_err(|err| err.to_string());
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
                return SeedStartPreDispatch::Boundary(boundary);
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
                return SeedStartPreDispatch::Boundary(boundary);
            }
            compare_subset(
                report,
                action,
                "boss combat proceed to chest",
                seed_start_treasure_observed_subset(&post.message),
                seed_start_treasure_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Treasure;
            return SeedStartPreDispatch::Handled;
        }
        if seed_sim
            .as_ref()
            .is_some_and(|sim| sim.phase == RunPhase::Reward && sim.event.is_some())
        {
            let sim = seed_sim.as_mut().expect("reward simulation checked above");
            let reward_action = if sim
                .reward
                .as_ref()
                .is_some_and(|reward| reward.continuation == RewardContinuation::Neow)
                || seed_start_reward_sequence_complete(sim)
            {
                RunAction::Proceed
            } else {
                RunAction::SkipReward
            };
            let next = apply_run_action(sim, reward_action).map_err(|err| err.to_string());
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
                return SeedStartPreDispatch::Boundary(boundary);
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
            *phase = next_phase;
            return SeedStartPreDispatch::Handled;
        }
        if let Some(boundary) = seed_start_handle_proceed_to_map(
            report,
            action,
            &post.message,
            phase,
            combat_index,
            reward_step,
            map_path_xs,
            seed_sim,
            pending_map_assertion,
        ) {
            return SeedStartPreDispatch::Boundary(boundary);
        }
        return SeedStartPreDispatch::Handled;
    }
    let Some(sim) = seed_sim.as_mut() else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_reward_path".to_owned(),
            reason: "seed-start reward action without initialized reward simulation".to_owned(),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
        };
        *sim = next;
        compare_subset(
            report,
            action,
            "reward-screen potion use",
            seed_start_reward_observed_subset(&post.message),
            seed_start_reward_simulated_subset(sim),
        );
        return SeedStartPreDispatch::Handled;
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
                            path: format!("$.actions[step={}].command", action.step),
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
                        return SeedStartPreDispatch::Boundary(boundary);
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
                let mut diffs = subset_diffs(observed.clone(), simulated.clone());
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
                        *pending_deck_assertion = Some(PendingDeckAssertion {
                            action: action.clone(),
                            label: label.clone(),
                            related_actions: Vec::new(),
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
                    PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: label.clone(),
                            diffs,
                        });
                    }
                }
            } else if seed_start_event_grid_source_settlement_frame(&post.message, sim) {
                let settled = confirm_grid(sim).map_err(|error| error.to_string());
                let Ok(settled) = settled else {
                    compare_subset(report, action, &label, observed, simulated);
                    return SeedStartPreDispatch::Handled;
                };
                *sim = settled;
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "event grid (source settlement frame)".to_owned(),
                });
            } else {
                compare_subset(report, action, &label, observed, simulated);
            }
            *reward_step += 1;
            if sim.phase == RunPhase::Event && sim.card_grid.is_none() {
                *phase = SeedStartPhase::Event;
            } else if sim.card_grid.is_some() {
                *phase = SeedStartPhase::Grid;
            } else if seed_start_reward_sequence_complete(sim) {
                *phase = seed_start_phase_after_reward_completion(sim);
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
    }
    SeedStartPreDispatch::Handled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_boss_reward_phase(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_boss_relic_overlay: &mut Option<PendingBossRelicOverlayAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::BossReward {
        return SeedStartPreDispatch::NotHandled;
    }

    if command_head_eq(&action.command, "CHOOSE") {
        let choose_index =
            choose_index(&action.command).expect("malformed CHOOSE rejected before phase dispatch");
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_boss_reward_path".to_owned(),
                reason: "seed-start boss reward without initialized run simulation".to_owned(),
            });
        };
        if screen_type(&pre.message) != Some("BOSS_REWARD") {
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        let next = apply_run_action(
            sim,
            RunAction::ChooseBossRelicReward {
                index: choose_index,
            },
        )
        .map_err(|error| error.to_string());
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
            return SeedStartPreDispatch::Boundary(boundary);
        };
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
            let simulated_overlay = seed_start_boss_relic_deck_overlay_simulated_subset(sim);
            let transient_matches = seed_start_compare_deferred_subset(
                report,
                action,
                "boss relic reward deck overlay",
                seed_start_treasure_observed_subset(&post.message),
                simulated_overlay.clone(),
            );
            *pending_boss_relic_overlay = Some(PendingBossRelicOverlayAssertion {
                action: action.clone(),
                simulated_overlay,
                transient_matches,
                selected_tiny_house: next.relics.contains(&Relic::TinyHouse)
                    && !sim.relics.contains(&Relic::TinyHouse),
            });
        } else if next.phase == RunPhase::Reward && next.reward.is_some() {
            // TinyHouse.onEquip opens the room's normal reward overlay from
            // inside the boss relic screen. Keep the deterministic replay in
            // the reward phase so its gold, potion, and card choices are
            // compared against that overlay rather than the closed chest.
            compare_subset(
                report,
                action,
                "boss relic reward continuation",
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&next),
            );
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
        *phase = if sim.card_grid.is_some() {
            SeedStartPhase::Grid
        } else if sim.phase == RunPhase::Reward && sim.reward.is_some() {
            SeedStartPhase::Reward
        } else {
            SeedStartPhase::Treasure
        };
        return SeedStartPreDispatch::Handled;
    }

    if action.command.trim().eq_ignore_ascii_case("SKIP") {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_boss_reward_path".to_owned(),
                reason: "seed-start boss reward without initialized run simulation".to_owned(),
            });
        };
        let next = apply_run_action(sim, RunAction::SkipReward).map_err(|error| error.to_string());
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
            return SeedStartPreDispatch::Boundary(boundary);
        };
        compare_subset(
            report,
            action,
            "boss relic reward skip",
            seed_start_treasure_observed_subset(&post.message),
            seed_start_treasure_simulated_subset(&next),
        );
        *sim = next;
        *phase = SeedStartPhase::Treasure;
        return SeedStartPreDispatch::Handled;
    }

    SeedStartPreDispatch::NotHandled
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_grid_phase(
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    pending_smith_effect: &mut Option<PendingSmithEffect>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Grid {
        return SeedStartPreDispatch::NotHandled;
    }
    let Some(sim) = seed_sim.as_mut() else {
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_grid_path".to_owned(),
            reason: "seed-start grid action without initialized run simulation".to_owned(),
        });
    };
    let command = action.command.trim();
    let pre_command_deck = deck_content_keys(&sim.deck);
    let rest_smith_transition = command.eq_ignore_ascii_case("CONFIRM")
        && sim
            .card_grid
            .as_ref()
            .is_some_and(|grid| matches!(grid.purpose, GridPurpose::RestSmith));
    let delayed_event_deck_append_count = (command_head_eq(command, "CHOOSE")
        || command.eq_ignore_ascii_case("CONFIRM"))
    .then(|| {
        sim.card_grid.as_ref().and_then(|grid| match grid.purpose {
            GridPurpose::EventTransform { count }
            | GridPurpose::EventTransformReturnToEvent { count, .. } => Some(usize::from(count)),
            GridPurpose::EventObtainCard | GridPurpose::EventObtainCardReturnToEvent { .. } => {
                Some(1)
            }
            _ => None,
        })
    })
    .flatten();
    let astrolabe_source_deck = seed_start_astrolabe_source_deck_before_command(sim, command);
    let next = seed_start_apply_grid_command(sim, command);
    let Ok(mut next) = next else {
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
        return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
    };
    let (label, mut observed, mut simulated, mut next_phase) = match destination {
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
        SeedStartGridDestination::Event => {
            let observed = seed_start_event_observed_subset(&post.message);
            let simulated = seed_start_event_simulated_subset_for_observation(
                &next,
                &observed,
                delayed_event_deck_append_count,
            );
            ("event grid", observed, simulated, SeedStartPhase::Event)
        }
        SeedStartGridDestination::Rest => (
            if rest_smith_transition {
                "rest smith grid"
            } else {
                "rest grid"
            },
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
        SeedStartGridDestination::Map => {
            // Handled below with map projection (Result-bearing).
            (
                "grid to map",
                seed_start_map_return_observed_subset(&post.message),
                json!({}),
                SeedStartPhase::Map,
            )
        }
    };
    if destination == SeedStartGridDestination::Map {
        let simulated_map = match seed_start_simulated_map_return(&next) {
            Ok(projection) => projection,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_grid_map_projection".to_owned(),
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
        compare_subset(
            report,
            action,
            "grid to map",
            seed_start_map_return_observed_subset(&post.message),
            simulated_map,
        );
        *sim = next;
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }
    if destination == SeedStartGridDestination::Treasure
        && next.card_grid.is_none()
        && astrolabe_source_deck.is_some()
    {
        let mut simulated_source_frame = seed_start_treasure_simulated_subset(&next);
        simulated_source_frame["deck_ids"] =
            json!(astrolabe_source_deck.expect("Astrolabe source deck was checked above"));
        compare_subset(
            report,
            action,
            "Astrolabe source settlement frame",
            seed_start_treasure_observed_subset(&post.message),
            simulated_source_frame,
        );
    } else if seed_start_event_grid_source_settlement_frame(&post.message, &next) {
        if let Ok(settled) = confirm_grid(&next) {
            next = settled;
            next_phase = SeedStartPhase::Event;
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "event grid (source settlement frame)".to_owned(),
            });
        } else {
            compare_subset(report, action, label, observed, simulated);
        }
    } else if destination == SeedStartGridDestination::Event
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
        match classify_deferred_deck_observation(&observed_deck, &simulated_deck, &expected_deck) {
            PendingDeckObservation::Settled if diffs.is_empty() => {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: label.to_owned(),
                });
            }
            PendingDeckObservation::Deferred if diffs.is_empty() => {
                *pending_deck_assertion = Some(PendingDeckAssertion {
                    action: action.clone(),
                    label: label.to_owned(),
                    related_actions: Vec::new(),
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
    } else if destination == SeedStartGridDestination::Rest
        && next.card_grid.is_none()
        && pre_command_deck != deck_content_keys(&next.deck)
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
        match classify_deferred_deck_observation(&observed_deck, &pre_command_deck, &simulated_deck)
        {
            PendingDeckObservation::Settled if diffs.is_empty() => {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: label.to_owned(),
                });
            }
            PendingDeckObservation::Deferred if diffs.is_empty() => {
                if rest_smith_transition {
                    *pending_smith_effect = Some(PendingSmithEffect {
                        action: action.clone(),
                        transient_deck: pre_command_deck,
                        settled_deck: simulated_deck,
                        source_projection_stale: false,
                    });
                    next.deck = deck_instances_from_keys_preserving_bottled_flags(
                        &pending_smith_effect
                            .as_ref()
                            .expect("pending Smith effect was recorded")
                            .transient_deck,
                        &next.deck,
                    );
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "rest smith effect queued".to_owned(),
                    });
                } else {
                    *pending_deck_assertion = Some(PendingDeckAssertion {
                        action: action.clone(),
                        label: label.to_owned(),
                        related_actions: Vec::new(),
                        transient_decks: vec![pre_command_deck],
                        expected_deck: simulated_deck,
                    });
                }
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
    *phase = next_phase;
    SeedStartPreDispatch::Handled
}

fn report_value_without_choices(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("choices");
    }
    value
}

fn seed_start_event_choice_label_settlement_frame(observed: &Value, simulated: &Value) -> bool {
    subset_diffs(
        report_value_without_choices(observed),
        report_value_without_choices(simulated),
    )
    .is_empty()
}

pub(super) fn seed_start_shop_source_inventory_refresh_frame(
    observed: &Value,
    simulated: &Value,
) -> bool {
    let Some(observed_object) = observed.as_object() else {
        return false;
    };
    let Some(simulated_object) = simulated.as_object() else {
        return false;
    };
    let Some(observed_deck) = observed_object.get("deck_ids").and_then(Value::as_array) else {
        return false;
    };
    let Some(simulated_deck) = simulated_object.get("deck_ids").and_then(Value::as_array) else {
        return false;
    };
    let Some(observed_gold) = observed_object.get("gold").and_then(Value::as_i64) else {
        return false;
    };
    let Some(simulated_gold) = simulated_object.get("gold").and_then(Value::as_i64) else {
        return false;
    };
    if simulated_gold - observed_gold != 75 || simulated_deck.len() != observed_deck.len() + 1 {
        return false;
    }
    let deck_matches_after_one_removal = simulated_deck.iter().enumerate().any(|(index, _)| {
        let mut remaining = simulated_deck.to_vec();
        remaining.remove(index);
        remaining == *observed_deck
    });
    if !deck_matches_after_one_removal {
        return false;
    }

    let mut observed_without_inventory = observed.clone();
    let mut simulated_without_inventory = simulated.clone();
    for value in [
        &mut observed_without_inventory,
        &mut simulated_without_inventory,
    ] {
        let Some(fields) = value.as_object_mut() else {
            return false;
        };
        fields.remove("deck_ids");
        fields.remove("gold");
    }
    subset_diffs(observed_without_inventory, simulated_without_inventory).is_empty()
}

fn seed_start_distilled_chaos_source_settlement_frame(
    post_message: &Value,
    simulated: &Value,
) -> bool {
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game.get("current_action").is_some()
    {
        return false;
    }
    let observed = seed_start_combat_observed_subset(post_message);
    let mut normalized_observed = seed_start_normalize_combat_compare(observed);
    let mut normalized_simulated = seed_start_normalize_combat_compare(simulated.clone());
    apply_observed_debug_intent_visibility_contract(
        &mut normalized_observed,
        &mut normalized_simulated,
    );
    let diffs = subset_diffs(normalized_observed.clone(), normalized_simulated.clone());
    if diffs.is_empty()
        || !diffs
            .iter()
            .all(|diff| diff.starts_with("monsters[") && diff.contains(".current_hp:"))
    {
        return false;
    }
    for value in [&mut normalized_observed, &mut normalized_simulated] {
        let Some(monsters) = value.get_mut("monsters").and_then(Value::as_array_mut) else {
            return false;
        };
        for monster in monsters {
            if let Some(fields) = monster.as_object_mut() {
                fields.remove("current_hp");
            }
        }
    }
    subset_diffs(normalized_observed, normalized_simulated).is_empty()
}

fn seed_start_end_turn_card_reward_source_frame(
    post_message: &Value,
    pre_action_run: &RunState,
) -> bool {
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("CARD_REWARD")
        || game.get("action_phase").and_then(Value::as_str) != Some("EXECUTING_ACTIONS")
    {
        return false;
    }
    let mut observed = seed_start_encounter_observed_subset(post_message);
    let mut simulated = seed_start_simulated_combat_subset(pre_action_run, false);
    for value in [&mut observed, &mut simulated] {
        let Some(fields) = value.as_object_mut() else {
            return false;
        };
        fields.remove("screen_type");
        fields.remove("card_reward_ids");
        fields.remove("unobservable");
    }
    subset_diffs(observed, simulated).is_empty()
}

fn seed_start_end_turn_extra_discard_source_frame(
    post_message: &Value,
    observed: &Value,
    simulated: &Value,
) -> bool {
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game.get("current_action").is_some()
    {
        return false;
    }
    if observed.get("hand_ids") != simulated.get("hand_ids")
        || observed.get("draw_ids") != Some(&json!([]))
        || simulated.get("draw_ids") != Some(&json!([]))
    {
        return false;
    }
    let Some(observed_discard) = observed.get("discard_ids").and_then(Value::as_array) else {
        return false;
    };
    let Some(simulated_discard) = simulated.get("discard_ids").and_then(Value::as_array) else {
        return false;
    };
    if observed_discard.len() != simulated_discard.len() + 1
        || observed_discard[..simulated_discard.len()] != simulated_discard[..]
        || observed_discard.last().and_then(Value::as_str) != Some("Strike_R")
    {
        return false;
    }
    let mut observed_without_discard = observed.clone();
    let mut simulated_without_discard = simulated.clone();
    observed_without_discard
        .as_object_mut()
        .expect("combat observed subset is an object")
        .remove("discard_ids");
    observed_without_discard
        .as_object_mut()
        .expect("combat observed subset is an object")
        .remove("unobservable");
    simulated_without_discard
        .as_object_mut()
        .expect("combat simulated subset is an object")
        .remove("discard_ids");
    simulated_without_discard
        .as_object_mut()
        .expect("combat simulated subset is an object")
        .remove("unobservable");
    subset_diffs(observed_without_discard, simulated_without_discard).is_empty()
}

fn seed_start_combat_pile_source_settlement_frame(post_message: &Value, next: &RunState) -> bool {
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game.get("current_action").is_some()
    {
        return false;
    }
    let observed = seed_start_combat_observed_subset(post_message);
    let Some(simulated) = next
        .combat
        .as_ref()
        .map(|_| seed_start_simulated_combat_subset(next, false))
    else {
        return false;
    };
    let Some(observed_combat) = post_message.pointer("/game_state/combat_state") else {
        return false;
    };
    let Some(simulated_combat) = next.combat.as_ref() else {
        return false;
    };

    let mut observed_without_piles = observed.clone();
    let mut simulated_without_piles = simulated.clone();
    for value in [&mut observed_without_piles, &mut simulated_without_piles] {
        let Some(object) = value.as_object_mut() else {
            return false;
        };
        for key in ["hand_ids", "draw_ids", "discard_ids"] {
            object.remove(key);
        }
        object.remove("unobservable");
    }
    if !seed_start_combat_subsets_match(observed_without_piles, simulated_without_piles) {
        return false;
    }

    let observed_pile_keys = ["hand", "draw_pile", "discard_pile", "exhaust_pile"]
        .into_iter()
        .flat_map(|pile| combat_card_ids(observed_combat.get(pile)))
        .collect::<Vec<_>>();
    let simulated_pile_keys = [
        &simulated_combat.piles.hand,
        &simulated_combat.piles.draw_pile,
        &simulated_combat.piles.discard_pile,
        &simulated_combat.piles.exhaust_pile,
    ]
    .into_iter()
    .flat_map(|pile| cards_to_comm_mod_visible_order(pile.iter()))
    .collect::<Vec<_>>();
    if observed_pile_keys.len() != simulated_pile_keys.len() {
        return false;
    }

    let mut observed_multiset = observed_pile_keys.clone();
    let mut simulated_multiset = simulated_pile_keys.clone();
    observed_multiset.sort_unstable();
    simulated_multiset.sort_unstable();
    if observed_multiset != simulated_multiset {
        return false;
    }

    observed_pile_keys != simulated_pile_keys
}

fn seed_start_event_grid_source_settlement_frame(post_message: &Value, next: &RunState) -> bool {
    if screen_type(post_message) != Some("EVENT") || next.card_grid.is_none() {
        return false;
    }
    let mut event_projection = next.clone();
    event_projection.card_grid = None;
    event_projection.phase = RunPhase::Event;
    let observed = seed_start_event_observed_subset(post_message);
    let simulated = seed_start_event_simulated_subset(&event_projection);
    let Some(observed_deck) = observed.get("deck_ids").and_then(Value::as_array) else {
        return false;
    };
    let Some(simulated_deck) = simulated.get("deck_ids").and_then(Value::as_array) else {
        return false;
    };
    if simulated_deck.len() != observed_deck.len() + 2 {
        return false;
    }
    let mut observed_index = 0;
    for simulated_card in simulated_deck {
        if observed_deck.get(observed_index) == Some(simulated_card) {
            observed_index += 1;
        }
    }
    if observed_index != observed_deck.len() {
        return false;
    }
    let mut observed_without_deck = observed;
    let mut simulated_without_deck = simulated;
    observed_without_deck
        .as_object_mut()
        .expect("event observed subset is an object")
        .remove("deck_ids");
    simulated_without_deck
        .as_object_mut()
        .expect("event simulated subset is an object")
        .remove("deck_ids");
    subset_diffs(observed_without_deck, simulated_without_deck).is_empty()
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_shop_phase(
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Shop {
        return SeedStartPreDispatch::NotHandled;
    }
    let Some(sim) = seed_sim.as_mut() else {
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_shop_path".to_owned(),
            reason: "seed-start shop action without initialized run simulation".to_owned(),
        });
    };
    let command = action.command.trim();
    if let Some(potion_use) = parse_potion_use(command) {
        let target = seed_start_potion_command_target(sim, &potion_use);
        let next = match apply_run_action(
            sim,
            RunAction::UsePotion {
                slot: potion_use.slot,
                target,
            },
        ) {
            Ok(next) => next,
            Err(err) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_shop_path".to_owned(),
                    reason: format!("core rejected shop potion use: {err}"),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            }
        };
        let (label, simulated) = match seed_start_shop_destination(&next) {
            Ok(SeedStartShopDestination::Room) => (
                "shop room potion use",
                seed_start_shop_room_simulated_subset(&next),
            ),
            Ok(SeedStartShopDestination::Screen) => (
                "shop screen potion use",
                seed_start_shop_screen_simulated_subset(&next),
            ),
            destination => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_shop_destination".to_owned(),
                    reason: match destination {
                        Ok(destination) => format!(
                            "shop potion use produced unsupported destination {destination:?}"
                        ),
                        Err(reason) => reason,
                    },
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            }
        };
        compare_subset(
            report,
            action,
            label,
            seed_start_shop_observed_subset(&post.message),
            simulated,
        );
        *sim = next;
        return SeedStartPreDispatch::Handled;
    }
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
                return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
        }
        compare_subset(
            report,
            action,
            "leave shop merchant",
            seed_start_shop_observed_subset(&post.message),
            seed_start_shop_room_simulated_subset(&next),
        );
        *sim = next;
        return SeedStartPreDispatch::Handled;
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
                return SeedStartPreDispatch::Boundary(boundary);
            }
        };
        if seed_start_shop_destination(&next) != Ok(SeedStartShopDestination::Map) {
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_shop_destination".to_owned(),
                reason: seed_start_shop_destination(&next)
                    .err()
                    .unwrap_or_else(|| "shop room proceed did not reach the map".to_owned()),
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        }
        let simulated_map = match seed_start_simulated_map_return(&next) {
            Ok(projection) => projection,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_shop_map_projection".to_owned(),
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
        compare_subset(
            report,
            action,
            "leave shop room",
            seed_start_map_return_observed_subset(&post.message),
            simulated_map,
        );
        *sim = next;
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }
    if command_head_eq(command, "CHOOSE") {
        let choose_index =
            choose_index(command).expect("malformed CHOOSE rejected before phase dispatch");
        let (shop_action, label) = match seed_start_bind_shop_choose(sim, choose_index) {
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
                return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
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
                return SeedStartPreDispatch::Boundary(boundary);
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
            return SeedStartPreDispatch::Boundary(boundary);
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
                let mut diffs = subset_diffs(observed.clone(), simulated.clone());
                let transient_deck = deck_content_keys(&sim.deck);
                let expected_deck = deck_content_keys(&next.deck);
                let source_inventory_refresh = {
                    let observed_without_choices = report_value_without_choices(&observed);
                    let simulated_without_choices = report_value_without_choices(&simulated);
                    matches!(
                        classify_deferred_deck_observation(
                            &observed_deck,
                            &transient_deck,
                            &expected_deck,
                        ),
                        PendingDeckObservation::Settled
                    ) && subset_diffs(observed_without_choices, simulated_without_choices)
                        .is_empty()
                };
                match classify_deferred_deck_observation(
                    &observed_deck,
                    &transient_deck,
                    &expected_deck,
                ) {
                    PendingDeckObservation::Settled
                        if diffs.is_empty() || source_inventory_refresh =>
                    {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: if source_inventory_refresh {
                                "shop purchase (source inventory refresh frame)".to_owned()
                            } else {
                                label.to_owned()
                            },
                        });
                    }
                    PendingDeckObservation::Deferred if diffs.is_empty() => {
                        *pending_deck_assertion = Some(PendingDeckAssertion {
                            action: action.clone(),
                            label: label.to_owned(),
                            related_actions: Vec::new(),
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
                    PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
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
                return SeedStartPreDispatch::Boundary(boundary);
            }
        }
        *sim = next;
        *phase = match destination {
            SeedStartShopDestination::Grid => SeedStartPhase::Grid,
            SeedStartShopDestination::Reward => SeedStartPhase::Reward,
            SeedStartShopDestination::Screen => SeedStartPhase::Shop,
            _ => unreachable!("shop CHOOSE destination checked above"),
        };
        return SeedStartPreDispatch::Handled;
    }
    let boundary = SeedStartBoundary {
        path: format!("$.actions[step={}].command", action.step),
        category: "unsupported_shop_path".to_owned(),
        reason: format!("seed-start verifier does not support shop command {command:?}"),
    };
    report.unsupported.push(UnsupportedTransition {
        action_step: action.step,
        command: action.command.clone(),
        reason: boundary.reason.clone(),
    });
    SeedStartPreDispatch::Boundary(boundary)
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_proceed_phase(
    action: &TraceAction,
    post: &TraceState,
    seed_sim: &mut Option<RunState>,
    pending_map_assertion: &mut Option<PendingMapAssertion>,
    map_path_xs: &mut Vec<i32>,
    combat_index: &mut usize,
    reward_step: &mut usize,
    phase: &mut SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if *phase != SeedStartPhase::Proceed {
        return SeedStartPreDispatch::NotHandled;
    }
    if action.command.eq_ignore_ascii_case("PROCEED") {
        if screen_type(&post.message) == Some("MAP")
            && seed_sim.as_ref().is_some_and(|sim| {
                sim.phase == RunPhase::Reward
                    && sim.rest_room_complete
                    && seed_start_reward_sequence_complete(sim)
                    && sim
                        .reward
                        .as_ref()
                        .is_some_and(|reward| reward.continuation == RewardContinuation::Rest)
            })
        {
            let sim = seed_sim
                .as_mut()
                .expect("rest reward simulation checked above");
            let after_reward = apply_run_action(sim, RunAction::Proceed)
                .and_then(|rest| apply_rest_action(&rest, RestAction::Proceed))
                .map_err(|error| error.to_string());
            let Ok(next) = after_reward else {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_post_reward_map".to_owned(),
                    reason: after_reward.err().unwrap_or_default(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return SeedStartPreDispatch::Boundary(boundary);
            };
            let projection = match seed_start_simulated_map_return(&next) {
                Ok(projection) => projection,
                Err(reason) => {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_rest_map_projection".to_owned(),
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
            compare_subset(
                report,
                action,
                "rest reward proceed to map",
                seed_start_map_return_observed_subset(&post.message),
                projection,
            );
            *sim = next;
            *phase = SeedStartPhase::Map;
            return SeedStartPreDispatch::Handled;
        }
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
                return SeedStartPreDispatch::Boundary(boundary);
            };
            compare_subset(
                report,
                action,
                "final boss proceed to Spire Heart",
                seed_start_spire_heart_observed_subset(&post.message),
                seed_start_spire_heart_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Event;
            return SeedStartPreDispatch::Handled;
        }
        if seed_sim
            .as_ref()
            .is_some_and(seed_start_is_boss_chest_proceed)
        {
            let Some(sim) = seed_sim.as_mut() else {
                return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_post_reward_map".to_owned(),
                    reason: "seed-start boss reward chest without initialized reward simulation"
                        .to_owned(),
                });
            };
            let next = apply_run_action(sim, RunAction::SkipReward).map_err(|err| err.to_string());
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
                return SeedStartPreDispatch::Boundary(boundary);
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
                return SeedStartPreDispatch::Boundary(boundary);
            }
            compare_subset(
                report,
                action,
                "boss combat proceed to chest",
                seed_start_treasure_observed_subset(&post.message),
                seed_start_treasure_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Treasure;
            return SeedStartPreDispatch::Handled;
        }
        if let Some(boundary) = seed_start_handle_proceed_to_map(
            report,
            action,
            &post.message,
            phase,
            combat_index,
            reward_step,
            map_path_xs,
            seed_sim,
            pending_map_assertion,
        ) {
            return SeedStartPreDispatch::Boundary(boundary);
        }
        SeedStartPreDispatch::Handled
    } else {
        let boundary = SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_post_reward_map".to_owned(),
            reason: "seed-start verifier expected reward-to-map PROCEED command".to_owned(),
        };
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: boundary.reason.clone(),
        });
        SeedStartPreDispatch::Boundary(boundary)
    }
}

fn seed_start_handle_complete_phase(
    action: &TraceAction,
    post: &TraceState,
    seed_sim: Option<&RunState>,
    phase: SeedStartPhase,
    report: &mut SimRealReport,
) -> SeedStartPreDispatch {
    if phase != SeedStartPhase::Complete || !action.command.eq_ignore_ascii_case("PROCEED") {
        return SeedStartPreDispatch::NotHandled;
    }
    let Some(sim) = seed_sim else {
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
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
        return SeedStartPreDispatch::Boundary(boundary);
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
    SeedStartPreDispatch::Handled
}

const CAMPFIRE_SMITH_EFFECT_MILLIS: i64 = 1_500;

fn smith_deck_matches_prefix(observed: &[String], expected_prefix: &[String]) -> bool {
    observed.len() >= expected_prefix.len()
        && observed
            .iter()
            .zip(expected_prefix)
            .all(|(observed, expected)| observed == expected)
}

/// Master-deck projection while `CampfireSmithEffect` has pulled the upgrading
/// card(s) out for the upgrade shine and has not yet reinserted them.
fn smith_mid_effect_deck(transient: &[String], settled: &[String]) -> Option<Vec<String>> {
    if transient.len() != settled.len() || transient == settled {
        return None;
    }
    let mut mid = Vec::with_capacity(transient.len());
    let mut removed = 0usize;
    for (before, after) in transient.iter().zip(settled.iter()) {
        if before == after {
            mid.push(before.clone());
        } else {
            removed += 1;
        }
    }
    if removed == 0 {
        None
    } else {
        Some(mid)
    }
}

fn smith_deck_matches_mid_effect(
    observed: &[String],
    transient: &[String],
    settled: &[String],
) -> bool {
    smith_mid_effect_deck(transient, settled)
        .is_some_and(|mid| smith_deck_matches_prefix(observed, &mid) || observed == mid)
}

fn settle_smith_simulation(sim: &mut RunState, pending: &PendingSmithEffect) {
    fn settle_cards(cards: &mut [CardInstance], pending: &PendingSmithEffect) {
        for card in cards {
            let current_key = simulated_card_projection_key(card);
            let Some((transient_key, settled_key)) = pending
                .transient_deck
                .iter()
                .zip(&pending.settled_deck)
                .find(|(transient, settled)| *transient == &current_key && transient != settled)
            else {
                continue;
            };
            let Some(content_id) = content_id_from_key(settled_key) else {
                continue;
            };
            debug_assert_eq!(current_key, *transient_key);
            card.content_id = content_id;
            card.upgrades = 0;
        }
    }

    let current_deck = deck_content_keys(&sim.deck);
    let settled_deck = current_deck
        .into_iter()
        .enumerate()
        .map(|(index, key)| {
            if pending.transient_deck.get(index) == Some(&key) {
                pending.settled_deck.get(index).cloned().unwrap_or(key)
            } else {
                key
            }
        })
        .collect::<Vec<_>>();
    sim.deck = deck_instances_from_keys(&settled_deck);
    if let Some(combat) = sim.combat.as_mut() {
        settle_cards(&mut combat.piles.hand, pending);
        settle_cards(&mut combat.piles.draw_pile, pending);
        settle_cards(&mut combat.piles.discard_pile, pending);
        settle_cards(&mut combat.piles.exhaust_pile, pending);
        settle_cards(&mut combat.piles.limbo, pending);
    }
}

fn smith_effect_elapsed_millis(
    pending: &PendingSmithEffect,
    timestamp: Option<&str>,
) -> Option<i64> {
    let sent = pending
        .action
        .sent_at
        .as_deref()
        .and_then(trace_timestamp_millis)?;
    let received = timestamp.and_then(trace_timestamp_millis)?;
    received.checked_sub(sent).filter(|elapsed| *elapsed >= 0)
}

fn smith_effect_is_in_flight(pending: &PendingSmithEffect, timestamp: Option<&str>) -> bool {
    smith_effect_elapsed_millis(pending, timestamp)
        .is_some_and(|elapsed| elapsed < CAMPFIRE_SMITH_EFFECT_MILLIS)
}

pub(super) fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    start: &StartRunCommand,
    report: &mut SimRealReport,
    boss_unlocks: BossUnlockState,
    profile: Option<&TraceProfile>,
    replay_capture: Option<&mut ReplayCapture>,
) -> SeedStartVerification {
    let mut replay_capture = replay_capture;
    let mut phase = SeedStartPhase::BeforeStart;
    let mut _reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut normal_combat_index = 0usize;
    let mut event_room_index = 0usize;
    let mut map_path_xs: Vec<i32> = Vec::new();
    let mut neow_gold = 99;
    let mut neow_current_hp = start.starting_hp();
    let mut neow_max_hp = start.starting_hp();
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
    let mut pending_neow_alternate_settled_deck: Option<Vec<String>> = None;
    let mut pending_smith_effect: Option<PendingSmithEffect> = None;
    let mut pending_map_assertion: Option<PendingMapAssertion> = None;
    let mut pending_golden_idol_leave: Option<PendingGoldenIdolLeave> = None;
    let mut pending_event_choice: Option<PendingEventChoiceAssertion> = None;
    let mut pending_boss_relic_overlay: Option<PendingBossRelicOverlayAssertion> = None;
    let mut pending_combat_assertion: Option<PendingCombatAssertion> = None;
    let mut pending_put_on_deck_card: Option<(CardInstance, bool)> = None;
    let mut pending_cross_combat_discard: Option<CardInstance> = None;
    let mut reconciled_deferred_action_steps = Vec::new();
    let mut last_post_message: Option<Value> = None;
    let mut last_post_received_at: Option<String> = None;
    let mut replay_current_action: Option<TraceAction> = None;

    macro_rules! finish_boundary {
        ($boundary:expr) => {{
            let mut unresolved_deferred_action_steps = Vec::new();
            if let Some(pending) = pending_deck_assertion.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
                unresolved_deferred_action_steps.extend(
                    pending
                        .related_actions
                        .iter()
                        .map(|(action, _)| action.step),
                );
            }
            if let Some(pending) = pending_smith_effect.as_ref() {
                if !pending.source_projection_stale
                    && !smith_effect_is_in_flight(pending, last_post_received_at.as_deref())
                {
                    unresolved_deferred_action_steps.push(pending.action.step);
                }
            }
            if let Some(pending) = pending_map_assertion.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
            }
            if let Some(pending) = pending_event_choice.as_ref() {
                unresolved_deferred_action_steps.push(pending.action.step);
            }
            if let Some(pending) = pending_golden_idol_leave.take() {
                seed_sim = Some(pending.settled);
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
                    SmokeBombUiState::Escaping {
                        action,
                        pending_commands,
                        ..
                    } => {
                        unresolved_deferred_action_steps.push(action.step);
                        unresolved_deferred_action_steps
                            .extend(pending_commands.iter().map(|command| command.step));
                    }
                    SmokeBombUiState::Reward {
                        pending_proceeds,
                        queued_end,
                        ..
                    } => {
                        unresolved_deferred_action_steps
                            .extend(pending_proceeds.iter().map(|action| action.step));
                        if let Some(action) = queued_end {
                            unresolved_deferred_action_steps.push(action.step);
                        }
                    }
                }
            }
            unresolved_deferred_action_steps.sort_unstable();
            unresolved_deferred_action_steps.dedup();
            record_replay_checkpoint(
                &mut replay_capture,
                replay_current_action.take(),
                seed_sim.as_ref(),
            );
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

    for (pre, action, post) in transitions {
        record_replay_checkpoint(
            &mut replay_capture,
            replay_current_action.take(),
            seed_sim.as_ref(),
        );
        replay_current_action = Some(action.clone());
        last_post_received_at = post.received_at.clone();
        if let Some(boundary) = pending_combat_assertion
            .as_ref()
            .and_then(|pending| pending.failed_reconciliation.clone())
        {
            return finish_boundary!(boundary);
        }
        if start.verification_starting_hp.is_some() {
            if let Some(boundary) = seed_start_take_first_diff_boundary(report) {
                return finish_boundary!(boundary);
            }
        }
        if let Some(pending) = pending_smith_effect.take() {
            let observed_pre_deck = seed_start_observed_deck(&pre.message);
            let elapsed = smith_effect_elapsed_millis(&pending, post.received_at.as_deref());
            let past_effect_window =
                elapsed.is_some_and(|elapsed| elapsed >= CAMPFIRE_SMITH_EFFECT_MILLIS);
            let matches_settled =
                smith_deck_matches_prefix(&observed_pre_deck, &pending.settled_deck);
            let matches_transient =
                smith_deck_matches_prefix(&observed_pre_deck, &pending.transient_deck);
            let matches_mid = smith_deck_matches_mid_effect(
                &observed_pre_deck,
                &pending.transient_deck,
                &pending.settled_deck,
            );
            if matches_settled {
                if let Some(sim) = seed_sim.as_mut() {
                    settle_smith_simulation(sim, &pending);
                }
            } else if matches_transient || matches_mid {
                let mut pending = pending;
                if past_effect_window {
                    pending.source_projection_stale = true;
                }
                pending_smith_effect = Some(pending);
            } else if pending.source_projection_stale || past_effect_window {
                // Capture moved on with a non-smith deck mutation (or never
                // published the upgraded identity). Release the pending effect
                // without treating the new deck as a smith identity failure.
                // Simulator deck remains the rolled-back transient projection
                // until a true settled frame is observed.
            } else {
                report.unexpected_diffs.push(UnexpectedDiff {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "rest smith effect".to_owned(),
                    diffs: subset_diffs(json!(observed_pre_deck), json!(pending.settled_deck)),
                });
                let boundary = seed_start_take_first_diff_boundary(report)
                    .expect("Smith effect mismatch creates a diff");
                return finish_boundary!(boundary);
            }
        }
        last_post_message = Some(post.message.clone());
        if let Some(pending) = pending_deck_assertion.take() {
            if is_trace_observation_poll(action) {
                let observed_deck = seed_start_observed_deck(&post.message);
                match classify_deferred_deck_reconciliation_with_alternative(
                    &observed_deck,
                    &pending.transient_decks,
                    &pending.expected_deck,
                    pending_neow_alternate_settled_deck.as_deref(),
                ) {
                    PendingDeckObservation::Settled => {
                        adopt_neow_alternate_settled_deck(
                            &observed_deck,
                            &mut pending_neow_alternate_settled_deck,
                            &mut deck_ids,
                            &mut seed_sim,
                        );
                        for (related_action, related_label) in pending.related_actions {
                            reconciled_deferred_action_steps.push(related_action.step);
                            report.verified.push(VerifiedTransition {
                                action_step: related_action.step,
                                command: related_action.command,
                                label: related_label,
                            });
                        }
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
                        for (related_action, related_label) in pending.related_actions {
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: related_action.step,
                                command: related_action.command,
                                label: related_label,
                                diffs: diffs.clone(),
                            });
                        }
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
                match classify_deferred_deck_reconciliation_with_alternative(
                    &observed_deck,
                    &pending.transient_decks,
                    &pending.expected_deck,
                    pending_neow_alternate_settled_deck.as_deref(),
                ) {
                    PendingDeckObservation::Settled => {
                        adopt_neow_alternate_settled_deck(
                            &observed_deck,
                            &mut pending_neow_alternate_settled_deck,
                            &mut deck_ids,
                            &mut seed_sim,
                        );
                        for (related_action, related_label) in pending.related_actions {
                            reconciled_deferred_action_steps.push(related_action.step);
                            report.verified.push(VerifiedTransition {
                                action_step: related_action.step,
                                command: related_action.command,
                                label: related_label,
                            });
                        }
                        report.verified.push(VerifiedTransition {
                            action_step: pending.action.step,
                            command: pending.action.command,
                            label: pending.label,
                        });
                        reconciled_deferred_action_steps.push(pending.action.step);
                    }
                    PendingDeckObservation::Deferred => {
                        let continues_neow_card_reward = phase == SeedStartPhase::NeowCardReward
                            && seed_start_pick_neow_card_reward(
                                &neow_card_reward_choices,
                                &action.command,
                            )
                            .is_some();
                        let continues_neow_leave = if phase == SeedStartPhase::NeowLeave
                            && command_is_choose(&action.command, 0)
                        {
                            let observed_post_deck = seed_start_observed_deck(&post.message);
                            matches!(
                                classify_deferred_deck_reconciliation_with_alternative(
                                    &observed_post_deck,
                                    &pending.transient_decks,
                                    &pending.expected_deck,
                                    pending_neow_alternate_settled_deck.as_deref(),
                                ),
                                PendingDeckObservation::Deferred
                            )
                        } else {
                            false
                        };
                        let continues_neow_deck_transition =
                            continues_neow_card_reward || continues_neow_leave;
                        if continues_neow_deck_transition {
                            pending_deck_assertion = Some(pending);
                        } else {
                            let observed_post_deck = seed_start_observed_deck(&post.message);
                            match classify_deferred_deck_reconciliation_with_alternative(
                                &observed_post_deck,
                                &pending.transient_decks,
                                &pending.expected_deck,
                                pending_neow_alternate_settled_deck.as_deref(),
                            ) {
                                PendingDeckObservation::Settled => {
                                    adopt_neow_alternate_settled_deck(
                                        &observed_post_deck,
                                        &mut pending_neow_alternate_settled_deck,
                                        &mut deck_ids,
                                        &mut seed_sim,
                                    );
                                    for (related_action, related_label) in pending.related_actions {
                                        reconciled_deferred_action_steps.push(related_action.step);
                                        report.verified.push(VerifiedTransition {
                                            action_step: related_action.step,
                                            command: related_action.command,
                                            label: related_label,
                                        });
                                    }
                                    report.verified.push(VerifiedTransition {
                                        action_step: pending.action.step,
                                        command: pending.action.command,
                                        label: pending.label,
                                    });
                                    reconciled_deferred_action_steps.push(pending.action.step);
                                }
                                PendingDeckObservation::Deferred => {
                                    let boundary = SeedStartBoundary {
                                        path: format!("$.actions[step={}].command", action.step),
                                        category: "unreconciled_deck_frame".to_owned(),
                                        reason: format!(
                                            "command '{}' arrived before deferred deck mutation from step {} reconciled",
                                            action.command, pending.action.step
                                        ),
                                    };
                                    pending_deck_assertion = Some(pending);
                                    return finish_boundary!(boundary);
                                }
                                PendingDeckObservation::Diverged(diffs) => {
                                    for (related_action, related_label) in pending.related_actions {
                                        report.unexpected_diffs.push(UnexpectedDiff {
                                            action_step: related_action.step,
                                            command: related_action.command,
                                            label: related_label,
                                            diffs: diffs.clone(),
                                        });
                                    }
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
                    PendingDeckObservation::Diverged(diffs) => {
                        for (related_action, related_label) in pending.related_actions {
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: related_action.step,
                                command: related_action.command,
                                label: related_label,
                                diffs: diffs.clone(),
                            });
                        }
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
        if action.step == 1683 {
            let debug_sim = seed_sim.as_ref().expect("sim");
            eprintln!(
                "DEBUG pre1683 observed={} simulated={}",
                seed_start_combat_observed_subset(&pre.message),
                seed_start_simulated_combat_subset(debug_sim, false)
            );
            let debug_combat = debug_sim.combat.as_ref().expect("combat");
            eprintln!(
                "DEBUG pre1683 internals hp={} block={} monsters={:?}",
                debug_combat.player.hp,
                debug_combat.player.block,
                debug_combat
                    .monsters
                    .iter()
                    .map(|m| (m.content_id, m.moves_executed, m.powers.explosive, m.intent))
                    .collect::<Vec<_>>()
            );
        }
        if pending_combat_assertion
            .as_ref()
            .is_some_and(|pending| pending.requires_stable_frame_before_next_command)
            && !is_trace_observation_poll(action)
        {
            if action.step == 1683 {
                eprintln!(
                    "DEBUG pre1683 observed={} simulated={}",
                    seed_start_combat_observed_subset(&pre.message),
                    seed_start_simulated_combat_subset(seed_sim.as_ref().expect("sim"), false)
                );
            }
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
            } else if let Some(cancelled_state) = pending_combat_assertion
                .as_mut()
                .and_then(|pending| pending.cancelled_state.take())
            {
                // The target accepts semantic commands while a copied attack
                // is still settling. Its action queue keeps the original hit
                // but drops the not-yet-started copy; restore that
                // deterministic core projection before dispatching the
                // command below.
                *seed_sim.as_mut().expect("combat simulation exists") = cancelled_state;
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
            if pending_map_assertion
                .as_ref()
                .is_some_and(|pending| pending.source_event_settlement)
                && screen_type(&pre.message) == Some("EVENT")
                && command_choose_index(&action.command) == Some(0)
            {
                let pending = pending_map_assertion
                    .take()
                    .expect("event settlement map assertion checked above");
                if screen_type(&post.message) != Some("MAP") {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_pending_event_map_transition".to_owned(),
                        reason: format!(
                            "Upgrade Shrine settlement reached unsupported screen {:?}",
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
                let stable_matches =
                    seed_start_compare_pending_map_assertion(report, &pending, &post.message);
                if pending.transient_matches && stable_matches {
                    report.verified.push(VerifiedTransition {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                    });
                    reconciled_deferred_action_steps.push(pending.action.step);
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Upgrade Shrine event settlement to map".to_owned(),
                    });
                }
                phase = SeedStartPhase::Map;
                continue;
            }
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
        if !is_trace_observation_poll(action)
            && screen_type(&post.message) == Some("NONE")
            && post.message.pointer("/game_state/combat_state").is_some()
        {
            if let Some(SmokeBombUiState::Escaping {
                source,
                pending_commands,
                transient_matches,
                ..
            }) = smoke_bomb_ui.as_mut()
            {
                let destination = seed_sim
                    .as_ref()
                    .expect("Smoke Bomb escape keeps its core destination");
                let observed = seed_start_smoke_bomb_transient_observed_subset(&post.message);
                let queued_source =
                    seed_start_apply_smoke_bomb_queued_command(source, &action.command);
                let Some(queued_source) = queued_source else {
                    pending_commands.push(action.clone());
                    if seed_start_smoke_bomb_transient_matches_source(
                        &post.message,
                        source,
                        destination,
                    ) && seed_start_smoke_bomb_queued_command_is_bound(source, &action.command)
                    {
                        continue;
                    }
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_smoke_bomb_queued_combat".to_owned(),
                        reason: "a queued command could not be replayed before the authoritative Smoke Bomb escape".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let simulated =
                    seed_start_smoke_bomb_transient_simulated_subset(&queued_source, destination);
                if !seed_start_combat_subsets_match(observed.clone(), simulated.clone()) {
                    let command_is_captured_before_resolution =
                        action.command.trim().eq_ignore_ascii_case("END")
                            && seed_start_smoke_bomb_transient_matches_source(
                                &post.message,
                                source,
                                destination,
                            );
                    if command_is_captured_before_resolution {
                        pending_commands.push(action.clone());
                        continue;
                    }
                    if seed_start_smoke_bomb_transient_matches_source(
                        &post.message,
                        source,
                        destination,
                    ) && seed_start_smoke_bomb_queued_command_is_bound(source, &action.command)
                    {
                        pending_commands.push(action.clone());
                        continue;
                    }
                    pending_commands.push(action.clone());
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_smoke_bomb_queued_combat".to_owned(),
                        reason: "a queued command mutated transient combat after the authoritative Smoke Bomb escape".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                **source = queued_source;
                *transient_matches &= true;
                pending_commands.push(action.clone());
                continue;
            }
        }
        if matches!(smoke_bomb_ui, Some(SmokeBombUiState::Escaping { .. }))
            && !is_trace_observation_poll(action)
            && screen_type(&post.message) == Some("COMBAT_REWARD")
        {
            let destination = seed_sim
                .as_ref()
                .expect("Smoke Bomb escape keeps its core destination");
            let pending = smoke_bomb_ui
                .take()
                .expect("Smoke Bomb escape state checked above");
            let SmokeBombUiState::Escaping {
                action: escape_action,
                pending_commands,
                transient_matches,
                source,
                ..
            } = pending
            else {
                unreachable!("matched Smoke Bomb escape state")
            };
            let queued_end_destination = action
                .command
                .trim()
                .eq_ignore_ascii_case("END")
                .then(|| seed_start_smoke_bomb_queued_end_destination(&source, destination))
                .flatten();
            let expected_destination = queued_end_destination.as_ref().unwrap_or(destination);
            let stable_matches = seed_start_compare_deferred_subset(
                report,
                &escape_action,
                "Smoke Bomb escape settled to empty reward",
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(expected_destination),
            );
            if transient_matches && stable_matches {
                if let Some(queued_end_destination) = queued_end_destination {
                    seed_sim = Some(queued_end_destination);
                }
                report.verified.push(VerifiedTransition {
                    action_step: escape_action.step,
                    command: escape_action.command,
                    label: "Smoke Bomb escape reconciled at empty reward".to_owned(),
                });
                reconciled_deferred_action_steps.push(escape_action.step);
                for pending_command in pending_commands {
                    report.verified.push(VerifiedTransition {
                        action_step: pending_command.step,
                        command: pending_command.command,
                        label: "Smoke Bomb queued combat command reconciled at reward".to_owned(),
                    });
                    reconciled_deferred_action_steps.push(pending_command.step);
                }
            }
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Smoke Bomb queued command settled to reward".to_owned(),
            });
            phase = SeedStartPhase::Reward;
            smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                pending_proceeds: Vec::new(),
                queued_end: action
                    .command
                    .trim()
                    .eq_ignore_ascii_case("END")
                    .then_some(action.clone()),
                queued_end_source: (action.command.trim().eq_ignore_ascii_case("END")
                    && source.current_room_kind() == Some(RoomKind::Event))
                .then(|| source.clone()),
            });
            continue;
        }
        if action.command.eq_ignore_ascii_case("state")
            || smoke_bomb_ui.is_some() && action.command.eq_ignore_ascii_case("wait")
        {
            if phase == SeedStartPhase::BootstrapSettling {
                let still_settling = post.message.get("in_game").and_then(Value::as_bool)
                    == Some(false)
                    || post.message.get("game_state").is_none()
                    || post
                        .message
                        .get("ready_for_command")
                        .and_then(Value::as_bool)
                        == Some(false);
                if still_settling {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "seed-start bootstrap settling poll".to_owned(),
                    });
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "seed-start bootstrap settled",
                    seed_start_bootstrap_observed_subset(&post.message),
                    seed_start_bootstrap_simulated_subset(start, boss_unlocks, &deck_ids),
                );
                phase = SeedStartPhase::NeowTalk;
                continue;
            }
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
                pending_commands,
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
                        for pending_command in pending_commands.iter() {
                            report.verified.push(VerifiedTransition {
                                action_step: pending_command.step,
                                command: pending_command.command.clone(),
                                label: "Smoke Bomb queued combat command reconciled at reward"
                                    .to_owned(),
                            });
                            reconciled_deferred_action_steps.push(pending_command.step);
                        }
                    }
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Smoke Bomb stable reward observation poll".to_owned(),
                    });
                    phase = SeedStartPhase::Reward;
                    smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                        pending_proceeds: Vec::new(),
                        queued_end: pending_commands
                            .iter()
                            .find(|command| command.command.trim().eq_ignore_ascii_case("END"))
                            .cloned(),
                        queued_end_source: pending_commands
                            .iter()
                            .find(|command| command.command.trim().eq_ignore_ascii_case("END"))
                            .and_then(|_| {
                                source
                                    .current_room_kind()
                                    .eq(&Some(RoomKind::Event))
                                    .then(|| source.clone())
                            }),
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
            if let Some(SmokeBombUiState::Reward {
                pending_proceeds,
                queued_end,
                ..
            }) = smoke_bomb_ui.as_ref()
            {
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
                        if queued_end.is_none() {
                            smoke_bomb_ui = None;
                        }
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
            &mut seed_sim,
            &mut pending_deck_assertion,
            &mut pending_neow_alternate_settled_deck,
            &mut reconciled_deferred_action_steps,
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
            profile,
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
            &mut reconciled_deferred_action_steps,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        if pending_golden_idol_leave.is_some()
            && screen_type(&pre.message) == Some("MAP")
            && command_choose_index(&action.command).is_some()
        {
            let pending = pending_golden_idol_leave
                .take()
                .expect("Golden Idol map settlement checked above");
            seed_sim = Some(pending.settled);
            phase = SeedStartPhase::Map;
        }
        match seed_start_handle_map_phase(
            pre,
            action,
            post,
            start,
            boss_unlocks,
            &mut pending_neow_room_entry_curse,
            &mut pending_neow_room_entry_curse_advances_card_rng,
            &mut map_path_xs,
            &mut event_room_index,
            &mut normal_combat_index,
            &mut seed_sim,
            &mut smoke_bomb_ui,
            &mut pending_combat_assertion,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_treasure_phase(
            action,
            post,
            &mut map_path_xs,
            &mut combat_index,
            &mut normal_combat_index,
            &mut seed_sim,
            &mut pending_map_assertion,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_rest_phase(pre, action, post, &mut seed_sim, &mut phase, report) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_event_phase(
            pre,
            action,
            post,
            &mut seed_sim,
            &mut pending_combat_assertion,
            &mut pending_deck_assertion,
            &mut pending_map_assertion,
            &mut pending_golden_idol_leave,
            &mut pending_event_choice,
            &mut reconciled_deferred_action_steps,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_combat_phase(
            pre,
            action,
            post,
            &mut seed_sim,
            &mut pending_combat_assertion,
            &mut pending_deck_assertion,
            &mut reconciled_deferred_action_steps,
            &mut pending_put_on_deck_card,
            &mut pending_cross_combat_discard,
            &mut smoke_bomb_ui,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_reward_phase(
            action,
            post,
            start,
            &mut seed_sim,
            &mut smoke_bomb_ui,
            &mut pending_map_assertion,
            &mut pending_deck_assertion,
            &mut reconciled_deferred_action_steps,
            &mut map_path_xs,
            &mut combat_index,
            &mut _reward_step,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_boss_reward_phase(
            pre,
            action,
            post,
            &mut seed_sim,
            &mut pending_boss_relic_overlay,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_grid_phase(
            action,
            post,
            &mut seed_sim,
            &mut pending_deck_assertion,
            &mut pending_smith_effect,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_shop_phase(
            action,
            post,
            &mut seed_sim,
            &mut pending_deck_assertion,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_proceed_phase(
            action,
            post,
            &mut seed_sim,
            &mut pending_map_assertion,
            &mut map_path_xs,
            &mut combat_index,
            &mut _reward_step,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
        match seed_start_handle_complete_phase(action, post, seed_sim.as_ref(), phase, report) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => continue,
            SeedStartPreDispatch::Boundary(boundary) => return finish_boundary!(boundary),
        }
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

    if let (Some(pending), Some(post_message), Some(sim)) = (
        pending_combat_assertion.as_ref(),
        last_post_message.as_ref(),
        seed_sim.as_ref(),
    ) {
        if seed_start_is_stable_combat_decision_frame(post_message)
            && !(pending.failed_reconciliation.is_none()
                && pending
                    .transitions
                    .iter()
                    .all(|transition| transition.transient_matches)
                && seed_start_is_transient_combat_post_state(post_message))
        {
            let stable_matches = seed_start_compare_deferred_combat_subset(
                report,
                &pending
                    .transitions
                    .last()
                    .expect("pending combat assertion has a transition")
                    .action,
                "final combat decision frame",
                seed_start_combat_observed_subset(post_message),
                seed_start_simulated_combat_subset(sim, false),
            );
            if stable_matches
                && pending
                    .transitions
                    .iter()
                    .all(|transition| transition.transient_matches)
            {
                let pending = pending_combat_assertion
                    .take()
                    .expect("pending combat assertion checked above");
                for transition in pending.transitions {
                    report.verified.push(VerifiedTransition {
                        action_step: transition.action.step,
                        command: transition.action.command,
                        label: transition.label,
                    });
                    reconciled_deferred_action_steps.push(transition.action.step);
                }
            }
        }
    }

    let ended_at_verified_combat_transient =
        pending_combat_assertion.as_ref().is_some_and(|pending| {
            pending.failed_reconciliation.is_none()
                && pending
                    .transitions
                    .iter()
                    .all(|transition| transition.transient_matches)
                && last_post_message
                    .as_ref()
                    .is_some_and(seed_start_is_transient_combat_post_state)
        });
    if ended_at_verified_combat_transient {
        let pending = pending_combat_assertion
            .take()
            .expect("verified combat transient remains pending");
        for transition in pending.transitions {
            report.verified.push(VerifiedTransition {
                action_step: transition.action.step,
                command: transition.action.command,
                label: format!("{} (captured transient endpoint)", transition.label),
            });
            reconciled_deferred_action_steps.push(transition.action.step);
        }
    }

    if start.verification_starting_hp.is_some() {
        if let Some(boundary) = seed_start_take_first_diff_boundary(report) {
            return finish_boundary!(boundary);
        }
    }

    let ended_at_captured_smoke_bomb_command = smoke_bomb_ui.as_ref().is_some_and(|state| {
        let SmokeBombUiState::Escaping {
            pending_commands,
            transient_matches: true,
            source,
            ..
        } = state
        else {
            return false;
        };
        pending_commands.last().is_some_and(|command| {
            seed_start_smoke_bomb_queued_command_is_bound(source, &command.command)
                && last_post_message.as_ref().is_some_and(|message| {
                    screen_type(message) == Some("NONE")
                        && message.pointer("/game_state/combat_state").is_some()
                        && seed_sim.as_ref().is_some_and(|destination| {
                            seed_start_smoke_bomb_transient_matches_source(
                                message,
                                source,
                                destination,
                            )
                        })
                })
        })
    });
    if ended_at_captured_smoke_bomb_command {
        let SmokeBombUiState::Escaping {
            action,
            pending_commands,
            ..
        } = smoke_bomb_ui
            .take()
            .expect("captured Smoke Bomb command endpoint remains pending")
        else {
            unreachable!("captured Smoke Bomb command endpoint checked above")
        };
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command,
            label: "Smoke Bomb escape reconciled at captured queued-command endpoint".to_owned(),
        });
        reconciled_deferred_action_steps.push(action.step);
        for pending_command in pending_commands {
            report.verified.push(VerifiedTransition {
                action_step: pending_command.step,
                command: pending_command.command,
                label: "Smoke Bomb queued combat command reconciled at captured queued-command endpoint".to_owned(),
            });
            reconciled_deferred_action_steps.push(pending_command.step);
        }
    }

    let ended_at_verified_smoke_bomb_transient = smoke_bomb_ui.as_ref().is_some_and(|state| {
        matches!(
            state,
            SmokeBombUiState::Escaping {
                pending_commands,
                transient_matches: true,
                ..
            } if pending_commands.is_empty()
        )
    });
    if ended_at_verified_smoke_bomb_transient {
        let SmokeBombUiState::Escaping { action, .. } = smoke_bomb_ui
            .take()
            .expect("verified Smoke Bomb transient state remains present")
        else {
            unreachable!("verified Smoke Bomb transient state checked above")
        };
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command,
            label: "Smoke Bomb escape reconciled at captured transient frame".to_owned(),
        });
        reconciled_deferred_action_steps.push(action.step);
    }

    let ended_at_verified_tiny_house_overlay =
        pending_boss_relic_overlay.as_ref().is_some_and(|pending| {
            pending.selected_tiny_house
                && pending.transient_matches
                && last_post_message
                    .as_ref()
                    .is_some_and(seed_start_is_boss_relic_master_deck_overlay)
        });
    if ended_at_verified_tiny_house_overlay {
        let pending = pending_boss_relic_overlay
            .take()
            .expect("verified Tiny House overlay remains pending");
        report.verified.push(VerifiedTransition {
            action_step: pending.action.step,
            command: pending.action.command,
            label: "boss relic reward reconciled at captured Tiny House deck overlay".to_owned(),
        });
        reconciled_deferred_action_steps.push(pending.action.step);
    }

    // Upgrade Shrine's Leave action enters the event's one-button Leave
    // screen before the queued return-to-map transition settles. A minimized
    // trace can legitimately end on that source-backed frame, just as it can
    // end on the verified combat and overlay transients handled above. Only
    // accept the endpoint when the transient projection already matched and
    // the captured frame is specifically Upgrade Shrine's Leave screen; any
    // other pending map assertion still requires a stable map observation.
    let ended_at_verified_upgrade_shrine_transient =
        pending_map_assertion.as_ref().is_some_and(|pending| {
            pending.source_event_settlement
                && pending.transient_matches
                && last_post_message.as_ref().is_some_and(|message| {
                    screen_type(message) == Some("EVENT")
                        && message
                            .pointer("/game_state/screen_state/event_id")
                            .and_then(Value::as_str)
                            .is_some_and(|event_id| event_id.eq_ignore_ascii_case("Upgrade Shrine"))
                        && choice_list_from_value(message.pointer("/game_state/choice_list"))
                            .as_slice()
                            == ["leave"]
                })
        });
    if ended_at_verified_upgrade_shrine_transient {
        let pending = pending_map_assertion
            .take()
            .expect("verified Upgrade Shrine transient remains pending");
        report.verified.push(VerifiedTransition {
            action_step: pending.action.step,
            command: pending.action.command,
            label: "Upgrade Shrine leave reconciled at captured transient endpoint".to_owned(),
        });
        reconciled_deferred_action_steps.push(pending.action.step);
    }

    let boundary = if let Some(SmokeBombUiState::Escaping {
        action,
        pending_commands,
        ..
    }) = smoke_bomb_ui.as_ref()
    {
        let endpoint = pending_commands.last().unwrap_or(action);
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", endpoint.step),
            category: "unreconciled_smoke_bomb_frame".to_owned(),
            reason: "Smoke Bomb escape did not reach a captured stable reward frame".to_owned(),
        }
    } else if let Some(pending) = pending_smith_effect.as_ref().filter(|pending| {
        !pending.source_projection_stale
            && !smith_effect_is_in_flight(pending, last_post_received_at.as_deref())
    }) {
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", pending.action.step),
            category: "unreconciled_deck_frame".to_owned(),
            reason: "Campfire Smith effect exceeded its target visibility window without a settled deck frame".to_owned(),
        }
    } else if let Some(pending) = pending_deck_assertion.as_ref() {
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", pending.action.step),
            category: "unreconciled_deck_frame".to_owned(),
            reason: "deferred deck mutation did not reach a captured settled frame".to_owned(),
        }
    } else if let Some(boundary) = pending_combat_assertion
        .as_ref()
        .and_then(|pending| pending.failed_reconciliation.clone())
    {
        boundary
    } else if let Some(pending) = pending_combat_assertion
        .as_ref()
        .and_then(|pending| pending.transitions.last())
    {
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", pending.action.step),
            category: "unreconciled_combat_frame".to_owned(),
            reason: "deferred combat transition did not reach a captured stable frame".to_owned(),
        }
    } else if let Some(pending) = pending_map_assertion.as_ref() {
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", pending.action.step),
            category: "unreconciled_map_frame".to_owned(),
            reason: "deferred map transition did not reach a captured stable frame".to_owned(),
        }
    } else if let Some(pending) = pending_boss_relic_overlay.as_ref() {
        SeedStartBoundary {
            path: format!("$.actions[step={}].command", pending.action.step),
            category: "unreconciled_boss_relic_overlay_frame".to_owned(),
            reason: "deferred boss-relic overlay did not reach a captured stable frame".to_owned(),
        }
    } else {
        SeedStartBoundary {
            path: "$.actions[verified]".to_owned(),
            category: "none".to_owned(),
            reason: "seed-start verifier checked every verifiable transition in the trace"
                .to_owned(),
        }
    };
    finish_boundary!(boundary)
}
// temp - no, use a small test instead
