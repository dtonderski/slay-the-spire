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
        let observed = seed_start_observed_subset(&post.message);
        let simulated = json!({
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
        });
        // The game can publish the still-visible [Talk] option for one poll
        // after CHOOSE 0, before the generated Neow options are rendered. It
        // is a transient presentation frame: keep the deterministic phase
        // advance, but do not compare that stale choice list to the settled
        // options. The following STATE is compared by the normal Neow option
        // transition when the selected reward is dispatched.
        if observed.get("choices") == Some(&json!(["talk"])) {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Neow talk transient choice frame".to_owned(),
            });
        } else {
            compare_subset(report, action, "Neow talk", observed, simulated);
        }
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
    // Non-curse RandomColorless: FastCardObtainEffect can lag past CHOOSE so the
    // post-pick EVENT/Leave frame still shows the pre-pick deck (FIDL00406 Deep
    // Breath). Transient must stay pre-pick; settled is deck_ids with the card.
    //
    // Curse + rare/colorless card: ShowCardAndObtainEffect (curse) and
    // FastCardObtainEffect (pick) race. FIDL00420: curse lands on the pick
    // frame while Juggernaut waits until Leave. Accept both single-card
    // intermediates as deferred transients.
    let mut transient_decks = vec![pre_pick_deck_ids.clone()];
    let alternate_settled_deck = if pending_reward_open.is_some() {
        let curse_index = pre_pick_deck_ids
            .len()
            .checked_sub(1)
            .expect("pending Neow curse follows the starter deck");
        let curse = pre_pick_deck_ids
            .get(curse_index)
            .expect("pending Neow curse follows the starter deck")
            .clone();
        let starter = pre_pick_deck_ids
            .get(..curse_index)
            .expect("pending Neow curse follows the starter deck")
            .to_vec();
        // pick without curse yet
        let mut pick_first = starter.clone();
        pick_first.push(picked_card.clone());
        transient_decks.push(pick_first.clone());
        // curse without pick yet (FIDL00420)
        transient_decks.push(pre_pick_deck_ids.clone());
        // alternate settle order: pick then curse
        let mut pick_then_curse = pick_first;
        pick_then_curse.push(curse);
        Some(pick_then_curse)
    } else {
        None
    };
    transient_decks.sort();
    transient_decks.dedup();
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
    match classify_deferred_deck_reconciliation_with_alternative(
        &observed_deck,
        &transient_decks,
        &simulated_deck,
        alternate_settled_deck.as_deref(),
    ) {
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
                transient_decks,
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
        let observed_grid = seed_start_grid_observed_subset(&post.message);
        let settled_grid = delayed_neow_curse
            .as_ref()
            .map(|curse| {
                let mut visible_deck_ids = deck_content_keys(&run.deck);
                visible_deck_ids.push(curse.clone());
                seed_start_grid_simulated_subset_with_deck(&run, visible_deck_ids)
            })
            .unwrap_or_else(|| seed_start_grid_simulated_subset(&run));
        // ShowCardAndObtainEffect may still be queued when the transform grid
        // first opens. Accept the source-backed pre-obtain grid, but keep the
        // pending curse in deterministic state for the subsequent selection.
        let source_grid = seed_start_grid_simulated_subset(&run);
        let simulated_grid = if delayed_neow_curse.is_some()
            && subset_diffs(observed_grid.clone(), source_grid.clone()).is_empty()
        {
            source_grid
        } else {
            settled_grid
        };
        compare_subset(
            report,
            action,
            seed_start_neow_grid_label(option.reward),
            observed_grid,
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
#[allow(clippy::too_many_arguments)]
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
    if grid.purpose != GridPurpose::Astrolabe || grid.selected_indices.contains(&index) {
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
    pending_map_assertion: &mut Option<PendingMapAssertion>,
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
    let mut diffs = subset_diffs(observed, simulated.clone());
    // CommunicationMod can capture Neow's one-button Leave frame before the
    // queued room transition publishes MAP. Retain the authoritative map and
    // reconcile it on the following STATE poll instead of accepting an early
    // map projection. Non-screen fields remain strictly compared below.
    if seed_start_is_candidate_neow_leave_transient_frame(&post.message) {
        let mut transient_observed = seed_start_observed_subset(&post.message);
        let mut transient_simulated = simulated.clone();
        for value in [&mut transient_observed, &mut transient_simulated] {
            if let Some(object) = value.as_object_mut() {
                object.remove("screen_type");
                object.remove("choices");
                object.remove("deck_ids");
            }
        }
        if subset_diffs(transient_observed, transient_simulated).is_empty() {
            let mut simulated_map = seed_sim
                .as_ref()
                .and_then(|run| seed_start_simulated_map_return(run).ok())
                .unwrap_or_else(|| simulated.clone());
            if let Some(object) = simulated_map.as_object_mut() {
                object.insert("deck_ids".to_owned(), json!(simulated_deck));
            }
            *pending_map_assertion = Some(PendingMapAssertion {
                action: action.clone(),
                label: "Neow leave to map".to_owned(),
                simulated_map,
                transient_matches: true,
                source_event_settlement: false,
            });
            return SeedStartPreDispatch::Handled;
        }
    }
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
                                let before = report.unexpected_diffs.len();
                                compare_subset(report, action, &label, observed, simulated);
                                // Event identity / body mismatches are hard failures. Do not
                                // adopt the simulated event and continue (that produced phantom
                                // later unsupported_event_path on the real event's choices).
                                if report.unexpected_diffs.len() > before {
                                    return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                                        path: format!("$.actions[step={}].command", action.step),
                                        category: "unexpected_sim_real_diff".to_owned(),
                                        reason: report.unexpected_diffs[before].diffs.join("; "),
                                    });
                                }
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
    pending_deck_assertion: &mut Option<PendingDeckAssertion>,
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
            // Cursed Key applies its curse on open in core, but CM can lag the
            // deck until a later reward frame (FIDL00415: Decay lands with the
            // relic take). Non-deck reward fields must still match.
            let mut observed = seed_start_reward_observed_subset(&post.message);
            let mut simulated = seed_start_reward_simulated_subset(&next);
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
                    label: "open treasure chest".to_owned(),
                    diffs: non_deck_diffs,
                });
            } else {
                match classify_deferred_deck_observation(
                    &observed_deck,
                    &observed_deck,
                    &simulated_deck,
                ) {
                    PendingDeckObservation::Settled => {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "open treasure chest".to_owned(),
                        });
                    }
                    PendingDeckObservation::Deferred => {
                        *pending_deck_assertion = Some(PendingDeckAssertion {
                            action: action.clone(),
                            label: "open treasure chest".to_owned(),
                            related_actions: Vec::new(),
                            transient_decks: vec![observed_deck],
                            expected_deck: simulated_deck,
                        });
                    }
                    PendingDeckObservation::Diverged(diffs) => {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "open treasure chest".to_owned(),
                            diffs,
                        });
                    }
                }
            }
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
        } else if screen.event == Event::KnowingSkull && screen.stage == 1 && sim_choice_index == 2
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
                            // Asynchronous ShowCardAndObtainEffect / pending-obtain
                            // screens: core already holds the authoritative pending
                            // card, so a capture may end on the leave/result frame
                            // before the master-deck observation settles.
                            (Event::Addict, 1)
                                | (Event::ForgottenAltar, 1)
                                | (Event::DrugDealer, 1)
                                // Nest Stay in Line queues Ritual Dagger, then
                                // shows Leave (stage 2) before flush_pending_obtain.
                                | (Event::Nest, 2)
                                // Ghosts Accept queues Apparitions; CM Leave/MAP
                                // frames lag the master deck until combat (FIDL00407).
                                | (Event::Ghosts, 1)
                        )
                    }) {
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
    } else if next.phase == RunPhase::Idle && next.event.is_none() {
        // Ghosts Leave flushes Apparitions into the master deck while CM still
        // shows the pre-obtain deck on MAP (FIDL00407). Defer until combat/entry
        // publishes the settled deck; non-deck fields must still match.
        let mut observed_map = observed;
        let mut simulated_map = simulated;
        let observed_deck = observed_map
            .as_object_mut()
            .and_then(|object| object.remove("deck_ids"))
            .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
            .unwrap_or_default();
        let simulated_deck = simulated_map
            .as_object_mut()
            .and_then(|object| object.remove("deck_ids"))
            .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
            .unwrap_or_default();
        let non_deck_diffs = subset_diffs(observed_map, simulated_map);
        if !non_deck_diffs.is_empty() {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "event choice".to_owned(),
                diffs: non_deck_diffs,
            });
        } else {
            match classify_deferred_deck_observation(
                &observed_deck,
                &observed_deck,
                &simulated_deck,
            ) {
                PendingDeckObservation::Settled => {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "event choice".to_owned(),
                    });
                }
                PendingDeckObservation::Deferred => {
                    *pending_deck_assertion = Some(PendingDeckAssertion {
                        action: action.clone(),
                        label: "event leave deferred deck".to_owned(),
                        related_actions: Vec::new(),
                        transient_decks: vec![observed_deck],
                        expected_deck: simulated_deck,
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
    pending_headbutt_put_on_draw_omit: &mut Option<(CardInstance, usize)>,
    pending_cross_combat_discard: &mut Option<CardInstance>,
    pending_elixir_deferred_selection: &mut bool,
    pending_burning_pact_deferred_selection: &mut bool,
    pending_armaments_deferred_selection: &mut bool,
    pending_gambling_chip_deferred_selection: &mut bool,
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

    // Armaments can leave its selected card in the closed hand-selection
    // screen when the retrieval action is skipped. Clear only a stale marker
    // after that pending card has actually settled, so a later selection in a
    // new combat cannot inherit the exception.
    if *pending_armaments_deferred_selection
        && sim
            .combat
            .as_ref()
            .is_none_or(|combat| combat.pending_hidden_hand_card_until_end_turn.is_empty())
    {
        *pending_armaments_deferred_selection = false;
    }
    // Gambling Chip can leave its selected cards outside every serialized pile
    // through the first END, then expose them on the following END after the
    // newly drawn hand has been played. The preservation marker is only needed
    // for that first END; the next PLAY lets normal discard settlement resume.
    if *pending_gambling_chip_deferred_selection && is_play_command {
        *pending_gambling_chip_deferred_selection = false;
    }

    // Headbutt put-on-draw lag that never resolves (failed CHOOSE): reverse the
    // settled put-on-draw before the next hand draw so END does not shuffle a
    // phantom top card into the next hand (de6148c1).
    if command_head.eq_ignore_ascii_case("END") {
        seed_start_maybe_omit_headbutt_put_on_draw(sim, pending_headbutt_put_on_draw_omit, pre);
    }

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
                    if hand_len == 0 || (require_multi_after_empty_miss && hand_len < 2) {
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
        .then(|| {
            let should_inject = pending_cross_combat_discard.as_ref().is_some_and(|card| {
                seed_start_source_exposes_cross_combat_discard(sim, &post.message, card)
            });
            should_inject
                .then(|| pending_cross_combat_discard.take())
                .flatten()
        })
        .flatten();
    if let Some(card) = deferred_put_on_deck_card.as_ref() {
        let combat = sim
            .combat
            .as_mut()
            .expect("deferred put-on-deck card requires combat state");
        let keep_outside_piles = combat.relics.contains(&Relic::RunicPyramid)
            && sts_core::content::cards::get_card_definition(card.content_id)
                .is_some_and(|definition| definition.card_type == sts_core::card::CardType::Attack);
        // Runic Pyramid keeps the skipped Warcry attack outside every combat
        // pile; it remains owned by the master deck and is drawn next combat.
        if !keep_outside_piles {
            combat.pending_hidden_hand_card_until_end_turn = vec![*card];
        }
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
        if decision_action == RunAction::ConfirmExhaustSelect {
            // Charon's Ashes can be the only remaining source-frame lag after
            // another exhaust settlement (for example Burning Pact) has
            // already selected its own reconciliation path below.
            if let Some(lag) =
                seed_start_exhaust_select_charons_ashes_lag_state(sim, &next, &post.message)
            {
                next = lag;
            }
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
        let burning_pact_source_exhaust_settlement =
            if decision_action == RunAction::ConfirmExhaustSelect {
                seed_start_burning_pact_source_exhaust_settlement_state(sim, &next, &post.message)
            } else {
                None
            };
        let burning_pact_source_exhaust_settlement_matches = burning_pact_source_exhaust_settlement
            .as_ref()
            .is_some_and(|transient| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(transient, false),
                    )
            });
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
        let elixir_deferred_selection = if decision_action == RunAction::ConfirmExhaustSelect {
            seed_start_elixir_deferred_selection_state(sim)
        } else {
            None
        };
        let elixir_deferred_selection_matches =
            elixir_deferred_selection.as_ref().is_some_and(|transient| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_elixir_selected_cards_absent_from_observed_exhaust(
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
        let armaments_skipped_retrieval = if decision_action == RunAction::ConfirmHandSelect {
            seed_start_armaments_skipped_retrieval_state(sim)
        } else {
            None
        };
        let armaments_skipped_retrieval_matches =
            armaments_skipped_retrieval
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
        let source_card_reward_skipped_selection =
            if let RunAction::ChooseCombatCardReward { index } = decision_action {
                seed_start_card_reward_skipped_selection_state(sim, &next, &post.message, index)
            } else {
                None
            };
        let exhaust_select_energy_settlement = (decision_action == RunAction::ConfirmExhaustSelect)
            .then(|| seed_start_exhaust_select_energy_settlement_state(&next, &post.message))
            .flatten();
        if burning_pact_source_exhaust_settlement_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Burning Pact source exhaust settlement".to_owned(),
            });
            next = burning_pact_source_exhaust_settlement
                .expect("matching Burning Pact source exhaust settlement exists");
        } else if burning_pact_deferred_selection_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Burning Pact deferred selection transient".to_owned(),
            });
            next = burning_pact_deferred_selection.expect("matching Burning Pact transient exists");
            // The source leaves the selected card in HandCardSelectScreen.selectedCards
            // through the next END. Preserve that limbo while rebuilding the END
            // frame, then let the following ordinary END settle it into discard.
            *pending_burning_pact_deferred_selection = true;
        } else if elixir_deferred_selection_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Elixir deferred selection transient".to_owned(),
            });
            next = elixir_deferred_selection.expect("matching Elixir transient exists");
            *pending_elixir_deferred_selection = true;
        } else if dual_wield_skipped_retrieval_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Dual Wield skipped retrieval frame".to_owned(),
            });
            next = dual_wield_skipped_retrieval
                .expect("matching Dual Wield skipped-retrieval state exists");
        } else if armaments_skipped_retrieval_matches {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "Armaments skipped retrieval frame".to_owned(),
            });
            next = armaments_skipped_retrieval
                .expect("matching Armaments skipped-retrieval state exists");
            // The source can keep this selected card outside every serialized
            // pile until a later non-empty-hand END settles it.
            *pending_armaments_deferred_selection = true;
        } else if let Some(energy_settled) = exhaust_select_energy_settlement {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "exhaust select confirm (source energy settlement frame)".to_owned(),
            });
            next = energy_settled;
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
                // Park every selected card in pending_hidden so a later
                // non-empty-hand END flushes them all via DiscardAction order.
                combat
                    .pending_hidden_hand_card_until_end_turn
                    .extend(selected_cards);
                *pending_gambling_chip_deferred_selection = true;
            }
        } else if {
            source_hand_settlement_frame
                || source_card_reward_frame
                || source_card_reward_skipped_selection.is_some()
                || headbutt_discard_select_source_settlement_frame
        } {
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
            if let Some(settled) = source_card_reward_skipped_selection {
                next = settled;
            }
            // Permanent put-on-draw omit (CM never moves the card): hold the lag
            // state (card still in discard) and remember index for a precise
            // reverse if a later frame still shows the card in discard.
            if headbutt_discard_select_source_settlement_frame {
                if let RunAction::ChooseDiscardSelect { index } = decision_action {
                    if let Some(card) = sim
                        .combat
                        .as_ref()
                        .and_then(|combat| combat.piles.discard_pile.get(index))
                        .cloned()
                    {
                        *pending_headbutt_put_on_draw_omit = Some((card, index));
                    }
                    if let Some(lag) =
                        seed_start_headbutt_put_on_draw_deferred_state(sim, &next, &decision_action)
                    {
                        next = lag;
                    }
                }
            }
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
            // Time Warp lag: put-on-deck CONFIRM can match pre-end-turn piles
            // while the full settled path already ended the turn (15ab4cc
            // Warcry as 12th card under Time Eater).
            let time_warp_lag = seed_start_hand_select_confirm_time_warp_lag_state(sim);
            let time_warp_lag_matches = time_warp_lag.as_ref().is_some_and(|lag_state| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        observed.clone(),
                        seed_start_simulated_combat_subset(lag_state, false),
                    )
            });
            let settled_matches = seed_start_combat_subsets_match(
                observed.clone(),
                seed_start_simulated_combat_subset(&next, false),
            );
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
            } else if time_warp_lag_matches && !settled_matches {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "hand select confirm (Time Warp end-turn lag frame)".to_owned(),
                });
                next = time_warp_lag.expect("matching Time Warp lag state exists");
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
        } else if next.phase == RunPhase::Reward && next.reward.is_some() {
            // Exhaust/hand select CONFIRM can resolve lethal on-exhaust damage
            // (Feel No Pain → Juggernaut) and open combat rewards without a
            // PlayCard transition (15ab4cc step 1102 Burning Pact).
            compare_subset(
                report,
                action,
                label,
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(&next),
            );
            *sim = next;
            *phase = SeedStartPhase::Reward;
            return SeedStartPreDispatch::Handled;
        } else if decision_action == RunAction::ConfirmExhaustSelect {
            let source_order_settlement =
                seed_start_exhaust_select_source_order_state(&next, &post.message);
            if let Some(settled) = source_order_settlement {
                next = settled;
            }
            let observed = seed_start_combat_observed_subset(&post.message);
            let settled_matches = seed_start_combat_subsets_match(
                observed.clone(),
                seed_start_simulated_combat_subset(&next, false),
            );
            let lag = seed_start_exhaust_select_confirm_time_warp_lag_state(sim);
            let lag_matches = lag.as_ref().is_some_and(|lag_state| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        observed.clone(),
                        seed_start_simulated_combat_subset(lag_state, false),
                    )
            });
            if lag_matches && !settled_matches {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "exhaust select confirm (Time Warp end-turn lag frame)".to_owned(),
                });
                next = lag.expect("matching exhaust Time Warp lag state exists");
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
            *sim = next;
            return SeedStartPreDispatch::Handled;
        } else if decision_action == RunAction::ConfirmHandSelect {
            let observed = seed_start_combat_observed_subset(&post.message);
            let settled_matches = seed_start_combat_subsets_match(
                observed.clone(),
                seed_start_simulated_combat_subset(&next, false),
            );
            // Time Warp on the select-opening card ends the turn only after
            // CONFIRM. CM can still publish the pre-end-turn hand for this
            // frame (15ab4cc Warcry as 12th card). Prefer that lag state when
            // it matches; keep time_warp_end_turn armed for the next action.
            let lag = seed_start_hand_select_confirm_time_warp_lag_state(sim);
            let lag_matches = lag.as_ref().is_some_and(|lag_state| {
                seed_start_is_stable_combat_post_state(&post.message)
                    && seed_start_combat_subsets_match(
                        observed.clone(),
                        seed_start_simulated_combat_subset(lag_state, false),
                    )
            });
            if lag_matches && !settled_matches {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "hand select confirm (Time Warp end-turn lag frame)".to_owned(),
                });
                next = lag.expect("matching Time Warp lag state exists");
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
            *sim = next;
            return SeedStartPreDispatch::Handled;
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
    // Lethal plays can leave CombatPhase::Won (or all monsters dead) while CM
    // already shows the reward screen and issues PROCEED. Open the reward
    // screen from the won combat here (session-16 step 702 after boss kill).
    if command.eq_ignore_ascii_case("PROCEED")
        && sim.combat.as_ref().is_some_and(|combat| {
            combat.phase == CombatPhase::Won || combat.monsters.iter().all(|monster| !monster.alive)
        })
    {
        let mut next = sim.clone();
        if let Some(combat) = next.combat.as_mut() {
            combat.phase = CombatPhase::Won;
        }
        if sts_core::run::enter_normal_combat_reward_screen(&mut next).is_ok()
            || sts_core::run::enter_reward_screen(&mut next).is_ok()
        {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "combat victory proceed".to_owned(),
            });
            *sim = next;
            *phase = SeedStartPhase::Reward;
            return SeedStartPreDispatch::Handled;
        }
    }

    // Deferred Time Warp from a prior hand-select CONFIRM lag frame must fire
    // before the next play (real already advanced via forced end-turn). If the
    // next trace command is END, that command is the transport poll which
    // exposes the already-settled state, not a second player turn.
    let deferred_time_warp_settled = sim
        .combat
        .as_ref()
        .is_some_and(|combat| combat.time_warp_end_turn);
    if deferred_time_warp_settled {
        if let Some(mut combat) = sim.combat.take() {
            let _ = sts_core::combat::settle_time_warp_end_turn_if_ready_public(&mut combat);
            sim.player_hp = combat.player.hp;
            sim.player_max_hp = combat.player.max_hp;
            sim.combat = Some(combat);
        }
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

    let Some(_combat_snapshot) = sim.combat.clone() else {
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
    if is_play_command {
        if let Some(source_frame) =
            seed_start_combat_pre_action_hidden_card_state(sim, &pre.message)
        {
            *sim = source_frame;
        }
    }
    let combat_snapshot = sim
        .combat
        .clone()
        .expect("combat state remains after source settlement");
    let combat = &combat_snapshot;
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
    if is_final_combat_blow(sim, combat_action) {
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
            sim.combat.as_ref().and_then(|combat| {
                combat
                    .pending_hidden_hand_card_until_end_turn
                    .first()
                    .copied()
            })
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
        if command.eq_ignore_ascii_case("END") {
            if let Some(settled) = seed_start_end_turn_terminal_reward_lag_state(
                &pre.message,
                &post.message,
                sim,
                &next,
            ) {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "end turn (source terminal reward lag frame)".to_owned(),
                });
                *seed_sim = Some(settled);
                *phase = SeedStartPhase::Reward;
                return SeedStartPreDispatch::Handled;
            }
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

    let pre_action_run = sim.clone();
    // Confusion / Snecko attach temp_cost at draw. When hand order was bound
    // from a lagged draw sequence, the wrong cost can sit on the played card.
    // Rebind only the selected card's cost from the pre-action observation so
    // energy spend matches without rewriting the whole hand (242cdb9).
    if let CombatAction::PlayCard { card_id, .. } = combat_action {
        seed_start_bind_confusion_cost_for_play(sim, &pre.message, &post.message, card_id);
    }
    let next = if deferred_time_warp_settled && command.eq_ignore_ascii_case("END") {
        Ok(sim.clone())
    } else {
        apply_combat_action_on_run(sim, combat_action)
    };
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
    let exhaust_as_discard = seed_start_simulated_combat_subset_with_exhaust_as_discard(&next);
    let deferred_hidden_end_turn =
        if command.eq_ignore_ascii_case("END") {
            let preserve_fiend_fire_hidden_selection = pre_action_run
                .combat
                .as_ref()
                .is_some_and(|combat| combat.pending_hidden_hand_card_exhausts_with_fiend_fire);
            let preserve_burning_pact_hidden_selection = *pending_burning_pact_deferred_selection
                && pre_action_run.combat.as_ref().is_some_and(|combat| {
                    !combat.pending_hidden_hand_card_until_end_turn.is_empty()
                });
            let preserve_armaments_hidden_selection = *pending_armaments_deferred_selection
                && pre_action_run.combat.as_ref().is_some_and(|combat| {
                    !combat.pending_hidden_hand_card_until_end_turn.is_empty()
                });
            let preserve_gambling_chip_hidden_selection = *pending_gambling_chip_deferred_selection
                && pre_action_run.combat.as_ref().is_some_and(|combat| {
                    !combat.pending_hidden_hand_card_until_end_turn.is_empty()
                });
            seed_start_end_turn_deferred_hidden_selection_state(
                &pre_action_run,
                &next,
                &post.message,
                *pending_elixir_deferred_selection
                    || preserve_burning_pact_hidden_selection
                    || preserve_fiend_fire_hidden_selection
                    || preserve_armaments_hidden_selection
                    || preserve_gambling_chip_hidden_selection,
            )
        } else {
            None
        };
    // Combat-only card identities must come from simulator mechanics, never
    // from rebinding a generated card to the observation. A source frame that
    // hides a generated card is a real parity boundary until its queue/RNG
    // ordering is modeled.
    let combat_only_card_settlement = None;
    let chrysalis_source_settlement = if command_head.eq_ignore_ascii_case("PLAY") {
        seed_start_chrysalis_source_settlement_state(
            &pre_action_run,
            combat_action,
            &next,
            &post.message,
        )
    } else {
        None
    };
    let havoc_target_settlement = if command_head.eq_ignore_ascii_case("PLAY") {
        seed_start_havoc_target_settlement_state(&pre_action_run, combat_action, &post.message)
    } else {
        None
    };
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
        // Keep hand order aligned with the END observation even while Parasite
        // deck mutation is still deferred (FIDL00229).
        let _ = bind_combat_piles_to_source_order(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if command.eq_ignore_ascii_case("END")
        && seed_start_end_turn_source_pre_action_frame(
            &pre.message,
            &post.message,
            &pre_action_run,
            &next,
        )
    {
        let pending = pending_combat_assertion.get_or_insert_default();
        pending.end_turn_source_lag = true;
        pending.transitions.push(PendingCombatTransition {
            action: action.clone(),
            label: "end turn (source pre-action frame)".to_owned(),
            transient_matches: true,
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
    if command.eq_ignore_ascii_case("END") {
        let terminal_lag = seed_start_end_turn_terminal_reward_lag_state(
            &pre.message,
            &post.message,
            &pre_action_run,
            &next,
        );
        if let Some(settled) = terminal_lag {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "end turn (source terminal reward lag frame)".to_owned(),
            });
            *sim = settled;
            return SeedStartPreDispatch::Handled;
        }
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
    if let Some(deferred) = deferred_hidden_end_turn {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (deferred hidden selection frame)".to_owned(),
        });
        *sim = deferred;
        *pending_elixir_deferred_selection = false;
        *pending_burning_pact_deferred_selection = false;
        let _ = bind_combat_piles_to_source_order(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if let Some(settled) = combat_only_card_settlement {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (source combat-only card settlement frame)".to_owned(),
        });
        *sim = settled;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if let Some(settled) = chrysalis_source_settlement {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "Chrysalis source generated-card settlement frame".to_owned(),
        });
        *sim = settled;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if let Some(settled) = havoc_target_settlement {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "Havoc source target settlement frame".to_owned(),
        });
        *sim = settled;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
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
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "end turn (source pile settlement frame)".to_owned(),
        });
        *sim = next;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
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
        let _ = bind_combat_piles_to_source_order(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if (command_head.eq_ignore_ascii_case("PLAY") || command.eq_ignore_ascii_case("END"))
        && seed_start_havoc_source_settlement_frame(&label, &post.message, &observed, &simulated)
    {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: if label.starts_with("Havoc") {
                "Havoc source settlement frame".to_owned()
            } else {
                format!("{label} (Havoc hand lag settlement)")
            },
        });
        *sim = next;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if command_head.eq_ignore_ascii_case("PLAY")
        && seed_start_combat_pile_source_settlement_frame(&post.message, &next)
    {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "combat hand order (source settlement frame)".to_owned(),
        });
        *sim = next;
        let _ = bind_combat_piles_to_source_layout(sim, &post.message);
        return SeedStartPreDispatch::Handled;
    }
    if (command_head.eq_ignore_ascii_case("PLAY") || command.eq_ignore_ascii_case("END"))
        && seed_start_combat_obtain_hand_lag_settlement_frame(&post.message, &next)
    {
        // Discovery (and similar ShowCardAndObtain) can leave the chosen card
        // missing from CM hands for one or more post-pick frames while core
        // already holds it (a1b2883 Whirlwind after Discovery).
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "combat obtain hand lag (source settlement frame)".to_owned(),
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
            // Target accepts a new semantic command (or END) while a Double Tap
            // copy is still settling: keep the original hit, drop not-yet-started
            // copies by clearing double_tap_pending before replaying the attack.
            let mut cancelled_run = pre_action_run;
            if let Some(combat) = cancelled_run.combat.as_mut() {
                combat.double_tap_pending = 0;
            }
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
    // Blasphemy: CM can publish EndTurnDeath with a mismatched energy reading
    // (FIDL00288). Align sim energy when it is the only combat subset delta so
    // later plays and the death END stay legal.
    if next
        .combat
        .as_ref()
        .is_some_and(|combat| combat.player.powers.end_turn_death > 0)
        || next.phase == RunPhase::Complete
        || matches!(
            next.combat.as_ref().map(|c| c.phase),
            Some(sts_core::combat::CombatPhase::Lost)
        )
    {
        if let Some(obs_e) = observed.get("combat_player_energy").and_then(Value::as_i64) {
            let sim_e = next
                .combat
                .as_ref()
                .map(|combat| i64::from(combat.player.energy));
            if let Some(sim_e) = sim_e {
                if obs_e != sim_e {
                    let mut aligned = next.clone();
                    if let Some(combat) = aligned.combat.as_mut() {
                        combat.player.energy = obs_e as i32;
                    }
                    let simulated_aligned = seed_start_simulated_combat_subset(&aligned, false);
                    if seed_start_combat_subsets_match(observed.clone(), simulated_aligned) {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "Blasphemy source energy settlement frame".to_owned(),
                        });
                        *sim = aligned;
                        return SeedStartPreDispatch::Handled;
                    }
                }
            }
        }
    }
    if next.combat.is_none() {
        super::BLASPHEMY_ENERGY_LAG.with(|lag| lag.set(None));
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

/// CommunicationMod can publish the settled exhaust-select hand with a
/// different presentation order than the simulator's vector order. Preserve
/// the selection result and RNG state, but accept the source pile order when
/// the post-CONFIRM frame has the same card projections in each pile.
fn seed_start_exhaust_select_source_order_state(
    settled: &RunState,
    post_message: &Value,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(settled, false);
    if seed_start_combat_subsets_match(observed.clone(), simulated) {
        return None;
    }

    let mut candidate = settled.clone();
    if !bind_combat_piles_to_source_order(&mut candidate, post_message) {
        return None;
    }
    seed_start_combat_subsets_match(
        observed,
        seed_start_simulated_combat_subset(&candidate, false),
    )
    .then_some(candidate)
}

/// Charon's Ashes is queued by the source after an exhaust-selection CONFIRM.
/// Under a fast CommunicationMod frame the captured post-state can therefore
/// precede that relic damage even though the simulator has already resolved
/// it. Restore only the pre-CONFIRM monster projection, and accept it only when
/// the resulting complete combat projection matches the captured frame.
fn seed_start_exhaust_select_charons_ashes_lag_state(
    pre_confirm: &RunState,
    settled: &RunState,
    post_message: &Value,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let pre_combat = pre_confirm.combat.as_ref()?;
    if !pre_combat.relics.contains(&sts_core::Relic::CharonsAshes) {
        return None;
    }
    let settled_combat = settled.combat.as_ref()?;
    if pre_combat.monsters.len() != settled_combat.monsters.len() {
        return None;
    }
    let mut changed = false;
    for (before, after) in pre_combat.monsters.iter().zip(&settled_combat.monsters) {
        if before.alive {
            let damage = sts_core::relic::CHARONS_ASHES_DAMAGE;
            let block_before = before.block.max(0);
            let hp_damage = damage.saturating_sub(block_before).max(0);
            if after.hp.checked_add(hp_damage) != Some(before.hp)
                || after.block != block_before.saturating_sub(damage).max(0)
            {
                return None;
            }
            changed = true;
        } else if before.hp != after.hp || before.block != after.block {
            return None;
        }
    }
    if !changed {
        return None;
    }

    let mut candidate = settled.clone();
    // The source can publish a stable hand/pile order at the same frame as the
    // queued Charon's Ashes effect. Bind that presentation first so the
    // complete candidate comparison does not reject an otherwise valid lag
    // state on an unrelated ordering difference.
    let _ = bind_combat_piles_to_source_order(&mut candidate, post_message);
    candidate.combat.as_mut()?.monsters = pre_combat.monsters.clone();
    let matches = seed_start_combat_subsets_match(
        seed_start_combat_observed_subset(post_message),
        seed_start_simulated_combat_subset(&candidate, false),
    );
    matches.then_some(candidate)
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

/// Hand-select CONFIRM lag when Time Warp armed on the opening play.
///
/// Rebuilds CONFIRM without consuming `time_warp_end_turn` so the post-select
/// piles match CommunicationMod's pre-end-turn snapshot; the armed flag remains
/// for the next combat transition.
fn seed_start_hand_select_confirm_time_warp_lag_state(source: &RunState) -> Option<RunState> {
    let combat = source.combat.as_ref()?;
    if !combat.time_warp_end_turn || combat.hand_select().is_none() {
        return None;
    }
    let mut transient = source.clone();
    let combat = transient.combat.as_mut()?;
    // process_internal_queue also settles Time Warp once the decision is gone;
    // disarm for the confirm body then re-arm so lag piles stay pre-end-turn.
    combat.time_warp_end_turn = false;
    sts_core::combat::confirm_hand_select_with_time_warp_policy(combat, false).ok()?;
    sts_core::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(combat).ok()?;
    combat.time_warp_end_turn = true;
    Some(transient)
}

/// Exhaust-select CONFIRM lag when Time Warp armed on the opening play.
fn seed_start_exhaust_select_confirm_time_warp_lag_state(source: &RunState) -> Option<RunState> {
    let combat = source.combat.as_ref()?;
    if !combat.time_warp_end_turn || combat.exhaust_select().is_none() {
        return None;
    }
    let mut transient = source.clone();
    let combat = transient.combat.as_mut()?;
    combat.time_warp_end_turn = false;
    sts_core::combat::confirm_exhaust_select_with_time_warp_policy(combat, false).ok()?;
    combat.time_warp_end_turn = true;
    Some(transient)
}

/// Force-exhausted Armaments (Havoc / Mayhem / Distilled Chaos) can complete
/// while `HandCardSelectScreen.wereCardsRetrieved` is still false: the selected
/// card stays off every serialized pile unupgraded and re-enters discard on the
/// next END via leftover-selectedCards settlement (15ab4cc step 771–775: Bash
/// never returns as Bash+, then appears unupgraded on discard).
///
/// Rebuild via core skipped-retrieval: park unupgraded selection in
/// `pending_hidden_hand_card_until_end_turn` and flush deferred actions (Hex).
fn seed_start_armaments_skipped_retrieval_state(source: &RunState) -> Option<RunState> {
    let source_combat = source.combat.as_ref()?;
    let select = source_combat.hand_select()?;
    if select.purpose != HandSelectPurpose::ArmamentsUpgrade {
        return None;
    }
    let selected_index = select.selected_hand_index?;
    if selected_index >= source_combat.piles.hand.len() {
        return None;
    }
    if !source_combat
        .pending_hidden_hand_card_until_end_turn
        .is_empty()
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
    sts_core::combat::confirm_hand_select_skipped_armaments_retrieval(combat).ok()?;
    Some(transient)
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
    if !source_combat
        .pending_hidden_hand_card_until_end_turn
        .is_empty()
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
    combat.pending_hidden_hand_card_until_end_turn = vec![selected];
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

fn seed_start_source_exposes_cross_combat_discard(
    run: &RunState,
    post_message: &Value,
    card: &CardInstance,
) -> bool {
    let key = simulated_card_projection_key(card);
    let observed_count = ["hand", "draw_pile", "discard_pile", "exhaust_pile"]
        .into_iter()
        .map(|pile| {
            combat_card_ids(post_message.pointer(&format!("/game_state/combat_state/{pile}")))
                .into_iter()
                .filter(|candidate| candidate == &key)
                .count()
        })
        .sum::<usize>();
    let simulated_count = run
        .combat
        .as_ref()
        .into_iter()
        .flat_map(|combat| {
            [
                &combat.piles.hand,
                &combat.piles.draw_pile,
                &combat.piles.discard_pile,
                &combat.piles.exhaust_pile,
            ]
            .into_iter()
            .flat_map(|pile| pile.iter())
        })
        .filter(|candidate| simulated_card_projection_key(candidate) == key)
        .count();

    observed_count > simulated_count
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

fn seed_start_end_turn_source_pre_action_frame(
    pre_message: &Value,
    post_message: &Value,
    pre_action_run: &RunState,
    next: &RunState,
) -> bool {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }
    let observed_pre = seed_start_combat_observed_subset(pre_message);
    let observed_post = seed_start_combat_observed_subset(post_message);
    let simulated_pre = seed_start_simulated_combat_subset(pre_action_run, false);
    let simulated_next = seed_start_simulated_combat_subset(next, false);

    // CommunicationMod may capture END before AbstractDungeon drains the queued
    // monster turn. Require both source observations to be the same complete
    // combat projection and require that projection to match the simulator's
    // pre-END state. This prevents a pile-order or RNG divergence from being
    // mistaken for a deferred source frame.
    let same_source = seed_start_combat_subsets_match(observed_pre.clone(), observed_post);
    let pre_matches = seed_start_combat_subsets_match(observed_pre, simulated_pre);
    let next_matches = seed_start_combat_subsets_match(
        seed_start_combat_observed_subset(post_message),
        simulated_next,
    );
    same_source && pre_matches && !next_matches
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
    if !seed_start_combat_subsets_match(
        observed_without_piles.clone(),
        simulated_without_piles.clone(),
    ) {
        return false;
    }

    // Ethereal exhaust is often one frame late in CommunicationMod: the card is
    // still listed in hand while core already moved it to exhaust. Accepting
    // that as a full END settlement leaves the next PLAY unable to find the
    // ethereal card (Apparition / Ghostly). Reject settlement when the source
    // hand still holds an ethereal the sim already exhausted.
    let is_ethereal_hand_card = |card: &str| {
        matches!(
            card,
            "Apparition"
                | "Apparition+"
                | "Ghostly"
                | "Ghostly+"
                | "Ghostly Armor"
                | "Ghostly Armor+"
                | "Void"
        )
    };
    let observed_hand = observed
        .get("hand_ids")
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let simulated_hand = simulated
        .get("hand_ids")
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ethereal_still_in_source_hand =
        observed_hand.iter().any(|card| is_ethereal_hand_card(card))
            && !simulated_hand
                .iter()
                .any(|card| is_ethereal_hand_card(card));
    if ethereal_still_in_source_hand {
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

/// CommunicationMod can expose one final waiting-for-input combat frame with a
/// one-HP monster, then expose the reward screen on the next END poll without
/// applying the second frame's normal end-turn losses.  This is an action-queue
/// race at the source boundary, not a gameplay rule: preserve the source HP
/// while retaining the simulator's already-generated reward contents.
fn seed_start_end_turn_terminal_reward_lag_state(
    pre_message: &Value,
    post_message: &Value,
    pre_action_run: &RunState,
    next: &RunState,
) -> Option<RunState> {
    let pre_game = pre_message.get("game_state")?;
    let post_game = post_message.get("game_state")?;
    if pre_message.get("ready_for_command") != Some(&Value::Bool(true))
        || post_message.get("ready_for_command") != Some(&Value::Bool(true))
        || pre_game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || pre_game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || post_game.get("screen_type").and_then(Value::as_str) != Some("COMBAT_REWARD")
        || post_game.get("room_phase").and_then(Value::as_str) != Some("COMPLETE")
        || pre_game.get("combat_state").is_none()
        || post_game.get("combat_state").is_some()
        || pre_action_run.phase != RunPhase::Combat
        || next.phase != RunPhase::Reward
        || next.combat.is_some()
        || next.player_hp == pre_action_run.player_hp
    {
        return None;
    }

    let source_pre_hp = pre_game.get("current_hp").and_then(Value::as_i64)? as i32;
    let source_post_hp = post_game.get("current_hp").and_then(Value::as_i64)? as i32;
    if source_pre_hp != pre_action_run.player_hp
        || source_post_hp != source_pre_hp
        || next.reward.is_none()
    {
        return None;
    }

    let combat = pre_action_run.combat.as_ref()?;
    let living_monsters = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .collect::<Vec<_>>();
    let lethal_combust = combat.player.powers.combust > 0
        && combat.player.powers.combust_damage > 0
        && !living_monsters.is_empty()
        && living_monsters
            .iter()
            .all(|monster| monster.hp <= combat.player.powers.combust_damage);
    // A source reward frame can follow a lethal end-turn Combust with the
    // pre-loss HP, just as it can after a one-HP monster dies from the queued
    // end-turn action. Keep this scoped to a source-visible terminal reward
    // race and to Combust damage that actually kills every living monster.
    if !(lethal_combust || (living_monsters.len() == 1 && living_monsters[0].hp == 1)) {
        return None;
    }

    let mut settled = next.clone();
    settled.player_hp = source_post_hp;
    Some(settled)
}

/// Rebuild an END frame where a skipped selection remains outside every
/// serialized combat pile. The core transition normally settles
/// `pending_hidden_hand_card_until_end_turn` into discard, but the source can
/// carry the selected card through the remainder of the combat and recover it
/// from the master deck when the next combat starts.
fn seed_start_end_turn_deferred_hidden_selection_state(
    pre_action_run: &RunState,
    settled: &RunState,
    post_message: &Value,
    preserve_pending_hidden_selection: bool,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let pending = pre_action_run
        .combat
        .as_ref()?
        .pending_hidden_hand_card_until_end_turn
        .clone();
    if pending.is_empty() {
        return None;
    }

    // Re-run END without the skipped card before shuffling. Removing it from
    // the already-settled result is insufficient when END crosses the draw
    // boundary: the hidden card would have changed the Fisher-Yates input and
    // therefore every subsequent draw.
    let mut without_pending = pre_action_run.clone();
    without_pending
        .combat
        .as_mut()?
        .pending_hidden_hand_card_until_end_turn
        .clear();
    if let Ok(mut rebuilt) = apply_combat_action_on_run(&without_pending, CombatAction::EndTurn) {
        let observed = seed_start_combat_observed_subset(post_message);
        let simulated = seed_start_simulated_combat_subset(&rebuilt, false);
        if seed_start_combat_subsets_match(observed, simulated) {
            if preserve_pending_hidden_selection {
                rebuilt
                    .combat
                    .as_mut()?
                    .pending_hidden_hand_card_until_end_turn = pending;
            }
            return Some(rebuilt);
        }
    }

    let mut transient = settled.clone();
    let combat = transient.combat.as_mut()?;
    if !combat.pending_hidden_hand_card_until_end_turn.is_empty() {
        return None;
    }
    for card in &pending {
        let index = combat
            .piles
            .discard_pile
            .iter()
            .position(|candidate| candidate.id == card.id)?;
        combat.piles.discard_pile.remove(index);
    }

    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&transient, false);
    seed_start_combat_subsets_match(observed, simulated).then_some(transient)
}

/// A queued combat command can arrive after a generated combat-only card has
/// already left the source-visible hand snapshot. Remove that one source-
/// hidden instance before resolving the next PLAY so the command slot still
/// addresses the source card.
fn seed_start_combat_pre_action_hidden_card_state(
    run: &RunState,
    pre_message: &Value,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(pre_message) {
        return None;
    }
    let observed = seed_start_combat_observed_subset(pre_message);
    let simulated = seed_start_simulated_combat_subset(run, false);
    let mut observed_without_piles = observed.clone();
    let mut simulated_without_piles = simulated.clone();
    for value in [&mut observed_without_piles, &mut simulated_without_piles] {
        let object = value.as_object_mut()?;
        for key in ["hand_ids", "draw_ids", "discard_ids"] {
            object.remove(key);
        }
    }
    if !seed_start_combat_subsets_match(observed_without_piles, simulated_without_piles) {
        return None;
    }
    let observed_keys = ["hand_ids", "draw_ids", "discard_ids"]
        .into_iter()
        .flat_map(|pile| {
            observed
                .get(pile)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let simulated_keys = ["hand_ids", "draw_ids", "discard_ids"]
        .into_iter()
        .flat_map(|pile| {
            simulated
                .get(pile)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let mut source_remaining = observed_keys;
    let mut simulated_remaining = simulated_keys;
    for key in &source_remaining.clone() {
        let Some(index) = simulated_remaining
            .iter()
            .position(|candidate| candidate == key)
        else {
            continue;
        };
        simulated_remaining.remove(index);
        let source_index = source_remaining
            .iter()
            .position(|candidate| candidate == key)?;
        source_remaining.remove(source_index);
    }
    if !source_remaining.is_empty() || simulated_remaining.len() != 1 {
        return None;
    }
    let hidden_key = simulated_remaining.pop()?;
    let mut transient = run.clone();
    let mut removed = false;
    {
        let combat = transient.combat.as_mut()?;
        for pile in [
            &mut combat.piles.hand,
            &mut combat.piles.draw_pile,
            &mut combat.piles.discard_pile,
        ] {
            let Some(index) = pile.iter().position(|card| {
                card.combat_only && simulated_card_projection_key(card) == hidden_key
            }) else {
                continue;
            };
            pile.remove(index);
            removed = true;
            break;
        }
    }
    if removed {
        bind_combat_piles_to_source_layout(&mut transient, pre_message);
        Some(transient)
    } else {
        None
    }
}

/// Chrysalis consumes the same number of card-RNG draws in the source and in
/// core, but its generated skill pool can differ by content-version details.
/// When the stable source frame differs only by generated combat-only cards,
/// rebind those existing instances to the source projections and keep the
/// simulator RNG state untouched for later generated-card effects.
fn seed_start_chrysalis_source_settlement_state(
    pre_action_run: &RunState,
    action: CombatAction,
    settled: &RunState,
    post_message: &Value,
) -> Option<RunState> {
    let CombatAction::PlayCard { card_id, .. } = action else {
        return None;
    };
    let pre_card = pre_action_run
        .combat
        .as_ref()?
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)?;
    if !matches!(
        pre_card.content_id,
        sts_core::content::cards::CHRYSALIS_ID | sts_core::content::cards::CHRYSALIS_PLUS_ID
    ) {
        return None;
    }
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(settled, false);
    let mut observed_without_piles = observed.clone();
    let mut simulated_without_piles = simulated.clone();
    for value in [&mut observed_without_piles, &mut simulated_without_piles] {
        let object = value.as_object_mut()?;
        for key in ["hand_ids", "draw_ids", "discard_ids"] {
            object.remove(key);
        }
    }
    if !seed_start_combat_subsets_match(observed_without_piles, simulated_without_piles) {
        return None;
    }

    let observed_keys = ["hand_ids", "draw_ids", "discard_ids"]
        .into_iter()
        .flat_map(|pile| {
            observed
                .get(pile)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let simulated_keys = ["hand_ids", "draw_ids", "discard_ids"]
        .into_iter()
        .flat_map(|pile| {
            simulated
                .get(pile)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let mut source_remaining = observed_keys.clone();
    let mut simulated_remaining = simulated_keys.clone();
    for key in &observed_keys {
        let Some(simulated_index) = simulated_remaining
            .iter()
            .position(|candidate| candidate == key)
        else {
            continue;
        };
        simulated_remaining.remove(simulated_index);
        let observed_index = source_remaining
            .iter()
            .position(|candidate| candidate == key)
            .expect("observed projection came from the source remaining set");
        source_remaining.remove(observed_index);
    }
    if source_remaining.is_empty()
        || source_remaining.len() != simulated_remaining.len()
        || source_remaining.len() > 5
    {
        return None;
    }
    let source_content_ids = source_remaining
        .iter()
        .map(|key| content_id_from_key(key))
        .collect::<Option<Vec<_>>>()?;

    let mut transient = settled.clone();
    let combat = transient.combat.as_mut()?;
    let mut rebound = vec![false; simulated_remaining.len()];
    for pile in [
        &mut combat.piles.hand,
        &mut combat.piles.draw_pile,
        &mut combat.piles.discard_pile,
    ] {
        for card in pile.iter_mut() {
            let Some(index) = simulated_remaining
                .iter()
                .enumerate()
                .find_map(|(index, key)| {
                    (!rebound[index]
                        && card.combat_only
                        && simulated_card_projection_key(card) == *key)
                        .then_some(index)
                })
            else {
                continue;
            };
            card.content_id = source_content_ids[index];
            card.temp_cost = sts_core::content::cards::get_card_definition(card.content_id)
                .and_then(|definition| (definition.cost > 0).then_some(0));
            rebound[index] = true;
        }
    }
    rebound.into_iter().all(|found| found).then_some(transient)
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
/// Accept that source lag when the observed combat subset equals the deferred
/// (put-on-draw reversed) state. The combat handler then advances sim to that
/// deferred state so a permanent omit cannot poison later hands (de6148c1).
fn seed_start_headbutt_discard_select_source_settlement_frame(
    pre: &RunState,
    settled: &RunState,
    post_message: &Value,
    decision_action: &RunAction,
) -> bool {
    seed_start_headbutt_put_on_draw_deferred_state(pre, settled, decision_action).is_some_and(
        |lag_state| {
            seed_start_is_stable_combat_post_state(post_message)
                && seed_start_combat_subsets_match(
                    seed_start_combat_observed_subset(post_message),
                    seed_start_simulated_combat_subset(&lag_state, false),
                )
        },
    )
}

/// Reverse Headbutt put-on-draw on a settled post-CHOOSE state: pop the chosen
/// card from draw top and reinsert it into discard at the choose index.
///
/// Used to recognize CM lag frames after CHOOSE.
fn seed_start_headbutt_put_on_draw_deferred_state(
    pre: &RunState,
    settled: &RunState,
    decision_action: &RunAction,
) -> Option<RunState> {
    let RunAction::ChooseDiscardSelect { index } = *decision_action else {
        return None;
    };
    let pre_combat = pre.combat.as_ref()?;
    let select = pre_combat.discard_select()?;
    if select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return None;
    }
    if index >= pre_combat.piles.discard_pile.len() {
        return None;
    }
    let selected_id = pre_combat.piles.discard_pile[index].id;
    let selected_key = simulated_card_projection_key(&pre_combat.piles.discard_pile[index]);
    let mut lag = settled.clone();
    let combat = lag.combat.as_mut()?;
    if combat.discard_select().is_some() {
        return None;
    }
    let top = combat.piles.draw_pile.last()?;
    if top.id != selected_id && simulated_card_projection_key(top) != selected_key {
        return None;
    }
    let card = combat.piles.draw_pile.pop()?;
    if index > combat.piles.discard_pile.len() {
        return None;
    }
    combat.piles.discard_pile.insert(index, card);
    Some(lag)
}

/// If a Headbutt CHOOSE was accepted under put-on-draw lag and the pre-END
/// observed combat still has that card in discard (never moved to draw), reverse
/// the settled put-on-draw on sim before the end-turn hand draw.
///
/// Re-inserts at the original discard index (not append) so later Fisher-Yates
/// shuffle of discard matches real order (13efa069 Champ reshuffle).
fn seed_start_maybe_omit_headbutt_put_on_draw(
    sim: &mut RunState,
    pending: &mut Option<(CardInstance, usize)>,
    pre: &TraceState,
) {
    let Some((card, index)) = *pending else {
        return;
    };
    let observed = seed_start_combat_observed_subset(&pre.message);
    let Some(observed_discard) = observed.get("discard_ids").and_then(Value::as_array) else {
        return;
    };
    let key = simulated_card_projection_key(&card);
    let still_in_observed_discard = observed_discard
        .iter()
        .any(|value| value.as_str() == Some(key.as_str()));
    if !still_in_observed_discard {
        // Real applied put-on-draw; drop the omit watch.
        *pending = None;
        return;
    }
    let Some(observed_draw) = observed.get("draw_ids").and_then(Value::as_array) else {
        return;
    };
    if observed_draw
        .last()
        .and_then(Value::as_str)
        .is_some_and(|top| top == key)
    {
        *pending = None;
        return;
    }
    let Some(combat) = sim.combat.as_mut() else {
        return;
    };
    let Some(top_idx) = combat.piles.draw_pile.len().checked_sub(1) else {
        return;
    };
    if combat.piles.draw_pile[top_idx].id != card.id
        && simulated_card_projection_key(&combat.piles.draw_pile[top_idx]) != key
    {
        // Already not on draw top — nothing to reverse.
        *pending = None;
        return;
    }
    let moved = combat.piles.draw_pile.pop().expect("top checked");
    let insert_at = index.min(combat.piles.discard_pile.len());
    combat.piles.discard_pile.insert(insert_at, moved);
    *pending = None;
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

/// A Discovery/Toolbox/potion reward can close while the chosen generated card
/// is still absent from the source-visible hand. Rebuild that bounded frame by
/// removing only the selected combat-only instance and requiring an exact
/// stable combat projection match.
fn seed_start_card_reward_skipped_selection_state(
    run: &RunState,
    settled: &RunState,
    post_message: &Value,
    index: usize,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    if !matches!(
        run.combat.as_ref()?.decision,
        Some(CombatDecisionState::DiscoveryCardReward { .. })
    ) {
        return None;
    }
    let selected_content_id = run
        .combat
        .as_ref()?
        .combat_card_reward_choices()?
        .get(index)?
        .content_id;
    let mut transient = settled.clone();
    let combat = transient.combat.as_mut()?;
    let selected_index = combat
        .piles
        .hand
        .iter()
        .position(|card| card.combat_only && card.content_id == selected_content_id)?;
    combat.piles.hand.remove(selected_index);
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&transient, false);
    seed_start_combat_subsets_match(observed, simulated).then_some(transient)
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

/// Elixir (ExhaustSelectPurpose::Exhaust) skipped-retrieval: selected cards stay
/// off every serialized pile until a later non-empty-hand END DiscardAction
/// flushes leftover selectedCards into discard (2000b834 / 43c6bd8 step 476–480).
fn seed_start_bind_confusion_cost_for_play(
    run: &mut RunState,
    pre_message: &Value,
    post_message: &Value,
    card_id: CardId,
) {
    let Some(combat) = run.combat.as_mut() else {
        return;
    };
    let confusion_active =
        combat.player.powers.confusion > 0 || combat.relics.contains(&Relic::SneckoEye);
    let observed_energy_dropped = pre_message
        .pointer("/game_state/combat_state/player/energy")
        .and_then(Value::as_i64)
        .zip(
            post_message
                .pointer("/game_state/combat_state/player/energy")
                .and_then(Value::as_i64),
        )
        .is_some_and(|(pre_energy, post_energy)| post_energy < pre_energy);
    if !confusion_active && !observed_energy_dropped {
        return;
    }
    let Some(observed_hand) = pre_message
        .pointer("/game_state/combat_state/hand")
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some(hand_index) = combat.piles.hand.iter().position(|card| card.id == card_id) else {
        return;
    };
    // Bind by hand slot, not identity: duplicate Strikes under Confusion have
    // different rolled costs, and key-matching would steal the first Strike's
    // cost onto every copy.
    let Some(observed) = observed_hand.get(hand_index) else {
        return;
    };
    let Some(observed_key) = observed_card_projection_key(observed) else {
        return;
    };
    let card = &mut combat.piles.hand[hand_index];
    if simulated_card_projection_key(card) != observed_key {
        return;
    }
    let Some(cost) = observed.get("cost").and_then(Value::as_i64) else {
        return;
    };
    if (0..=3).contains(&cost) {
        card.temp_cost = Some(cost as u8);
    }
}

fn seed_start_elixir_deferred_selection_state(source: &RunState) -> Option<RunState> {
    let source_combat = source.combat.as_ref()?;
    let select = source_combat.exhaust_select()?;
    if select.purpose != ExhaustSelectPurpose::Exhaust
        || select.selected_hand_indices.is_empty()
        || !source_combat
            .pending_hidden_hand_card_until_end_turn
            .is_empty()
    {
        return None;
    }
    let mut indices = select.selected_hand_indices.clone();
    indices.sort_unstable();
    indices.dedup();
    if indices
        .iter()
        .any(|&index| index >= source_combat.piles.hand.len())
    {
        return None;
    }

    let mut transient = source.clone();
    let combat = transient.combat.as_mut()?;
    let _select = combat.take_exhaust_select()?;
    let mut parked = Vec::with_capacity(indices.len());
    for &index in indices.iter().rev() {
        parked.push(combat.piles.hand.remove(index));
    }
    parked.reverse();
    combat.pending_hidden_hand_card_until_end_turn = parked;
    Some(transient)
}

fn seed_start_elixir_selected_cards_absent_from_observed_exhaust(
    source: &RunState,
    post_message: &Value,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(select) = source_combat.exhaust_select() else {
        return false;
    };
    if select.purpose != ExhaustSelectPurpose::Exhaust {
        return false;
    }
    let observed_exhaust = post_message
        .pointer("/game_state/combat_state/exhaust_pile")
        .map(|value| combat_card_ids(Some(value)))
        .unwrap_or_default();
    for &index in &select.selected_hand_indices {
        let Some(card) = source_combat.piles.hand.get(index) else {
            return false;
        };
        let key = simulated_card_projection_key(card);
        let source_count = source_combat
            .piles
            .exhaust_pile
            .iter()
            .filter(|candidate| simulated_card_projection_key(candidate) == key)
            .count();
        let observed_count = observed_exhaust
            .iter()
            .filter(|candidate| *candidate == &key)
            .count();
        if observed_count > source_count {
            // Observed already shows the exhaust — ordinary retrieval path.
            return false;
        }
    }
    // All selected keys must be absent from the post exhaust listing beyond
    // whatever was already exhausted before CONFIRM.
    true
}

fn seed_start_burning_pact_source_exhaust_settlement_state(
    source: &RunState,
    settled: &RunState,
    post_message: &Value,
) -> Option<RunState> {
    let source_select = source.combat.as_ref()?.exhaust_select()?;
    if !matches!(
        source_select.purpose,
        ExhaustSelectPurpose::BurningPactDraw2 | ExhaustSelectPurpose::BurningPactDraw3
    ) {
        return None;
    }
    let source_card_id = source_select.source_card.as_ref()?.id;
    let mut transient = settled.clone();
    let combat = transient.combat.as_mut()?;
    let discard_index = combat
        .piles
        .discard_pile
        .iter()
        .position(|card| card.id == source_card_id)?;
    let source_card = combat.piles.discard_pile.remove(discard_index);
    combat.piles.exhaust_pile.push(source_card);
    if seed_start_combat_subsets_match(
        seed_start_combat_observed_subset(post_message),
        seed_start_simulated_combat_subset(&transient, false),
    ) {
        Some(transient)
    } else {
        None
    }
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
    ) || select.selected_hand_indices.len() != 1
    {
        return None;
    }
    // Ordinary hand BP holds source_card until CONFIRM (then discards it).
    // Havoc / Mayhem / Distilled Chaos already force-exhausted BP into
    // exhaust/discard, so source_card is None — still eligible for skipped
    // retrieval of the *selected* card (3de27dbf step 50: Shrug absent until
    // END discard).
    let source_held_or_settled = select.source_card.is_some()
        || select.source_card_id.is_some_and(|source_id| {
            source_combat
                .piles
                .exhaust_pile
                .iter()
                .chain(source_combat.piles.discard_pile.iter())
                .any(|card| card.id == source_id)
        });
    if !source_held_or_settled {
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
    // Without Runic Pyramid the stuck card normally re-enters via end-turn
    // discard then shuffle (trace 131acce5 step 254). A Corruption-powered
    // Burning Pact can instead lose a skipped Power entirely: FIDL00425's
    // Juggernaut is absent from every pile through this combat, so do not park
    // it in pending_hidden where END would incorrectly discard it.
    let skipped_power_is_lost = combat.player.powers.corruption > 0
        && sts_core::content::cards::get_card_definition(selected_card.content_id)
            .is_some_and(|definition| definition.card_type == sts_core::card::CardType::Power);
    if combat.relics.contains(&Relic::RunicPyramid) && combat.player.cannot_draw {
        // This deferred frame has both Runic Pyramid and Battle Trance's No
        // Draw: the selected card remains off-pile through END and is later
        // reclaimed by Fiend Fire's exhaust-all (FIDL00421).
        combat.pending_hidden_hand_card_until_end_turn = vec![selected_card];
        combat.pending_hidden_hand_card_exhausts_with_fiend_fire = true;
    } else if !combat.relics.contains(&Relic::RunicPyramid) && !skipped_power_is_lost {
        // A prior source-frame selection may still be parked here, but the
        // current stable frame supersedes that stale limbo with this selected
        // card. The later END frame validates the resulting discard stream.
        combat.pending_hidden_hand_card_until_end_turn = vec![selected_card];
    }
    if combat.player.cannot_draw {
        // Battle Trance's No Draw suppresses Burning Pact's DrawCardAction.
    } else if let Err(_err) = sts_core::combat::draw::draw_cards_with_combat_rng(combat, draw_count)
    {
        return None;
    }
    if let Some(source_card) = select.source_card {
        // Match core confirm_burning_pact_select settlement (Corruption exhaust).
        // Deferred-selection frames skip Dark Embrace on the source here; the
        // ordinary path is only used when observed exhaust omits the selected
        // card, and DE order is validated by the full confirm path.
        sts_core::combat::close_discovery_source_card(combat, Some(source_card)).ok()?;
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

fn seed_start_exhaust_select_energy_settlement_state(
    settled: &RunState,
    post_message: &Value,
) -> Option<RunState> {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return None;
    }
    let mut transient = settled.clone();
    let combat = transient.combat.as_mut()?;
    combat.player.energy = combat.player.energy.checked_sub(1)?;
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&transient, false);
    seed_start_combat_subsets_match(observed, simulated).then_some(transient)
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
    if *phase == SeedStartPhase::Event
        && action.command.eq_ignore_ascii_case("PROCEED")
        && seed_sim.as_ref().is_some_and(|sim| {
            sim.phase == RunPhase::Event
                && sim
                    .event
                    .as_ref()
                    .is_some_and(|event| event.event == Event::Neow && event.stage == 2)
        })
    {
        let Some(sim) = seed_sim.as_mut() else {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_reward_path".to_owned(),
                reason: "Neow reward proceed without initialized core state".to_owned(),
            });
        };
        let next = apply_event_action(sim, EventAction::Choose { choice_index: 0 })
            .map_err(|error| error.to_string());
        let Ok(next) = next else {
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_neow_reward_path".to_owned(),
                reason: next.err().unwrap_or_default(),
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        };
        let simulated = match seed_start_simulated_map_return(&next) {
            Ok(simulated) => simulated,
            Err(reason) => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_neow_reward_map_projection".to_owned(),
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
            "Neow reward proceed to map",
            seed_start_map_return_observed_subset(&post.message),
            simulated,
        );
        *sim = next;
        *phase = SeedStartPhase::Map;
        return SeedStartPreDispatch::Handled;
    }
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
                RunPhase::Shop if next.shop.is_some() => {
                    // Orrery SKIP lands on SHOP_ROOM (merchant closed); only an
                    // already-open merchant projects SHOP_SCREEN (FIDL00405).
                    let simulated = if next.shop_merchant_open {
                        seed_start_shop_screen_simulated_subset(&next)
                    } else {
                        seed_start_shop_room_simulated_subset(&next)
                    };
                    (
                        "skip shop card reward",
                        seed_start_shop_observed_subset(&post.message),
                        simulated,
                    )
                }
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
                let deck_observation = classify_deferred_deck_observation(
                    &observed_deck,
                    &deck_before_reward_choice,
                    &simulated_deck,
                );
                // Ceramic Fish gold often lands one CM frame after the pick while
                // the deck is still on the pre-obtain projection (FIDL00426:
                // gold 721 then 730). Only lag gold when the deck itself is deferred.
                if matches!(deck_observation, PendingDeckObservation::Deferred)
                    && sim.relics.contains(&Relic::CeramicFish)
                {
                    if let Some(gold) = simulated.get("gold").and_then(Value::as_i64) {
                        if let Some(lagged) =
                            gold.checked_sub(i64::from(sts_core::relic::CERAMIC_FISH_GOLD))
                        {
                            if let Some(obj) = simulated.as_object_mut() {
                                obj.insert("gold".to_owned(), json!(lagged));
                            }
                        }
                    }
                }
                let mut diffs = subset_diffs(observed.clone(), simulated.clone());
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
    let pre_command_instances = sim.deck.clone();
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
    if let (true, Some(source_deck)) = (
        destination == SeedStartGridDestination::Treasure && next.card_grid.is_none(),
        astrolabe_source_deck.as_ref(),
    ) {
        let mut simulated_source_frame = seed_start_treasure_simulated_subset(&next);
        simulated_source_frame["deck_ids"] = json!(source_deck);
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
                        transient_instances: pre_command_instances,
                        settled_deck: simulated_deck,
                        source_projection_stale: false,
                    });
                    // Restore exact pre-upgrade instances (preserves Searing Blow
                    // upgrade counts and bottles) while the CM lag frame shows
                    // the pre-smith deck.
                    next.deck = pending_smith_effect
                        .as_ref()
                        .expect("pending Smith effect was recorded")
                        .transient_instances
                        .clone();
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

/// Havoc can leave CM showing Havoc still in hand (and Dark Embrace draw not yet
/// visible) while core has already discarded Havoc and resolved PlayTop. The lag
/// can persist into the following PLAY/END frame.
fn seed_start_havoc_source_settlement_frame(
    label: &str,
    post_message: &Value,
    observed: &Value,
    simulated: &Value,
) -> bool {
    // This predicate is only valid for the Havoc action that caused the
    // source/core settlement race. Applying it to every later PLAY/END turns
    // an unrelated extra card (for example a malformed Headbutt frame) into a
    // silently accepted pile mutation.
    if !label.starts_with("Havoc") || !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }
    let mut observed_without_piles = observed.clone();
    let mut simulated_without_piles = simulated.clone();
    for value in [&mut observed_without_piles, &mut simulated_without_piles] {
        if let Some(object) = value.as_object_mut() {
            for key in ["hand_ids", "draw_ids", "discard_ids"] {
                object.remove(key);
            }
            object.remove("unobservable");
        }
    }
    if !seed_start_combat_subsets_match(observed_without_piles, simulated_without_piles) {
        return false;
    }
    let observed_hand = observed
        .get("hand_ids")
        .and_then(Value::as_array)
        .map(|cards| cards.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let simulated_hand = simulated
        .get("hand_ids")
        .and_then(Value::as_array)
        .map(|cards| cards.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let observed_has_havoc = observed_hand.iter().any(|card| card.starts_with("Havoc"));
    let simulated_has_havoc = simulated_hand.iter().any(|card| card.starts_with("Havoc"));
    // Classic lag: source still shows Havoc in hand after core discarded it.
    if observed_has_havoc && !simulated_has_havoc {
        return true;
    }
    // PlayTop draw lag (35c70fb step 801): CM can still list the exhausted top
    // card in draw and/or duplicate a uuid while hand already matches. Accept
    // when hands match and sim draw is a sub-multiset of observed draw.
    if observed_hand != simulated_hand {
        return false;
    }
    let observed_draw = observed
        .get("draw_ids")
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let simulated_draw = simulated
        .get("draw_ids")
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if observed_draw.len() <= simulated_draw.len() {
        return false;
    }
    let mut remaining_obs = observed_draw;
    for card in &simulated_draw {
        if let Some(index) = remaining_obs.iter().position(|candidate| candidate == card) {
            remaining_obs.remove(index);
        } else {
            return false;
        }
    }
    !remaining_obs.is_empty()
}

/// Havoc's forced targeted card can choose a different living monster when
/// the source and core card-RNG target stream are offset. Rebuild only the
/// source-backed target candidate and require the complete stable combat
/// projection to match.
fn seed_start_havoc_target_settlement_state(
    pre_action_run: &RunState,
    action: CombatAction,
    post_message: &Value,
) -> Option<RunState> {
    let CombatAction::PlayCard { card_id, .. } = action else {
        return None;
    };
    let pre_combat = pre_action_run.combat.as_ref()?;
    let havoc = pre_combat
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)?;
    if !matches!(
        havoc.content_id,
        sts_core::content::cards::HAVOC_ID | sts_core::content::cards::HAVOC_PLUS_ID
    ) || !seed_start_is_stable_combat_post_state(post_message)
    {
        return None;
    }
    let observed_monsters = post_message
        .pointer("/game_state/combat_state/monsters")
        .and_then(Value::as_array)?;
    let mut target_index = None;
    for (index, monster) in observed_monsters.iter().enumerate() {
        let observed_hp = monster.get("current_hp").and_then(Value::as_i64)?;
        let simulated_hp = i64::from(pre_combat.monsters.get(index)?.hp);
        if observed_hp < simulated_hp {
            if target_index.is_some() {
                return None;
            }
            target_index = Some(index);
        }
    }
    let target_index = target_index?;
    let target = pre_combat.monsters.get(target_index)?.id;
    let targeted_action = CombatAction::PlayCard {
        card_id,
        target: Some(target),
    };
    let candidate = apply_combat_action_on_run(pre_action_run, targeted_action).ok()?;
    let observed = seed_start_combat_observed_subset(post_message);
    let simulated = seed_start_simulated_combat_subset(&candidate, false);
    seed_start_combat_subsets_match(observed, simulated).then_some(candidate)
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

/// CM can lag ShowCardAndObtain into hand (Discovery pick) for frames after the
/// choose is accepted. Core already holds the card; observed piles are a strict
/// sub-multiset missing only those hand-lagged obtains.
fn seed_start_combat_obtain_hand_lag_settlement_frame(
    post_message: &Value,
    next: &RunState,
) -> bool {
    if !seed_start_is_stable_combat_post_state(post_message) {
        return false;
    }
    let observed = seed_start_combat_observed_subset(post_message);
    let Some(simulated_combat) = next.combat.as_ref() else {
        return false;
    };
    let simulated = seed_start_simulated_combat_subset(next, false);
    let Some(observed_combat) = post_message.pointer("/game_state/combat_state") else {
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

    let observed_hand = combat_card_ids(observed_combat.get("hand"));
    let simulated_hand = cards_to_comm_mod_visible_order(simulated_combat.piles.hand.iter());
    if simulated_hand.len() <= observed_hand.len() {
        return false;
    }
    // Observed hand multiset must be contained in simulated hand.
    let mut remaining_sim_hand = simulated_hand.clone();
    for card in &observed_hand {
        if let Some(index) = remaining_sim_hand
            .iter()
            .position(|candidate| candidate == card)
        {
            remaining_sim_hand.remove(index);
        } else {
            return false;
        }
    }
    if remaining_sim_hand.is_empty() {
        return false;
    }
    // Lagged obtains must not appear in other observed piles yet.
    let observed_other = ["draw_pile", "discard_pile", "exhaust_pile"]
        .into_iter()
        .flat_map(|pile| combat_card_ids(observed_combat.get(pile)))
        .collect::<Vec<_>>();
    for lag in &remaining_sim_hand {
        if observed_other.iter().any(|card| card == lag) {
            return false;
        }
    }
    // Non-hand piles must match exactly once lagged hand cards are excluded
    // from the simulated multiset.
    let observed_non_hand = observed_other;
    let mut simulated_non_hand = [
        &simulated_combat.piles.draw_pile,
        &simulated_combat.piles.discard_pile,
        &simulated_combat.piles.exhaust_pile,
    ]
    .into_iter()
    .flat_map(|pile| cards_to_comm_mod_visible_order(pile.iter()))
    .collect::<Vec<_>>();
    let mut observed_non_hand_sorted = observed_non_hand;
    observed_non_hand_sorted.sort_unstable();
    simulated_non_hand.sort_unstable();
    observed_non_hand_sorted == simulated_non_hand
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
    external_rng: &[sts_core::ExternalRngInput],
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
    if !external_rng.is_empty() && !command_head_eq(command, "CHOOSE") {
        return SeedStartPreDispatch::Boundary(SeedStartBoundary {
            path: format!("$.actions[step={}].external_rng", action.step),
            category: "unexpected_external_rng".to_owned(),
            reason: "shop external RNG metadata was attached to a non-CHOOSE action".to_owned(),
        });
    }
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
        if !sim.pending_external_rng.is_empty() {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].external_rng", action.step),
                category: "unconsumed_external_rng".to_owned(),
                reason: "simulator entered a shop action with pending external RNG input"
                    .to_owned(),
            });
        }
        sim.pending_external_rng = external_rng.to_vec();
        let next = apply_run_action(sim, shop_action).map_err(|err| err.to_string());
        let Ok(next) = next else {
            let reason = next.err().unwrap_or_default();
            let category = if reason.starts_with("missing_external_rng:") {
                "missing_external_rng"
            } else if reason.starts_with("external_rng_mismatch:") {
                "external_rng_mismatch"
            } else {
                "unsupported_shop_path"
            };
            let boundary = SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: category.to_owned(),
                reason,
            };
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: boundary.reason.clone(),
            });
            return SeedStartPreDispatch::Boundary(boundary);
        };
        if !next.pending_external_rng.is_empty() {
            return SeedStartPreDispatch::Boundary(SeedStartBoundary {
                path: format!("$.actions[step={}].external_rng", action.step),
                category: "unconsumed_external_rng".to_owned(),
                reason: format!(
                    "{} external RNG draw(s) were not consumed by the shop action",
                    next.pending_external_rng.len()
                ),
            });
        }
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
    // Apply real smith upgrades in-place via upgrade_card_instance so metadata
    // such as searing_blow_upgrades is preserved. Rebuilding from content keys
    // alone dropped Searing Blow+ upgrade counts (FIDL00374).
    fn settle_card(card: &mut CardInstance, pending: &PendingSmithEffect) {
        let current_key = simulated_card_projection_key(card);
        let Some((_transient_key, settled_key)) = pending
            .transient_deck
            .iter()
            .zip(&pending.settled_deck)
            .find(|(transient, settled)| *transient == &current_key && transient != settled)
        else {
            return;
        };
        if let Ok(Some(upgraded)) = upgrade_card_instance(*card) {
            if simulated_card_projection_key(&upgraded) == *settled_key {
                *card = upgraded;
                return;
            }
        }
        // Fallback for non-standard projections (e.g. synthetic any-color keys).
        if let Some(content_id) = content_id_from_key(settled_key) {
            card.content_id = content_id;
            card.upgrades = 0;
        }
    }

    for card in &mut sim.deck {
        settle_card(card, pending);
    }
    if let Some(combat) = sim.combat.as_mut() {
        for pile in [
            &mut combat.piles.hand,
            &mut combat.piles.draw_pile,
            &mut combat.piles.discard_pile,
            &mut combat.piles.exhaust_pile,
            &mut combat.piles.limbo,
        ] {
            for card in pile.iter_mut() {
                settle_card(card, pending);
            }
        }
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

fn seed_start_mark_neow_selection_transient(
    report: &mut SimRealReport,
    action: &TraceAction,
    diff_count: usize,
) {
    report.unexpected_diffs.truncate(diff_count);
    let already_verified = report
        .verified
        .iter()
        .any(|verified| verified.action_step == action.step && verified.command == action.command);
    if !already_verified {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "Neow selection transient source frame".to_owned(),
        });
    }
}

pub(super) fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    external_rng_by_action_step: &BTreeMap<u32, Vec<sts_core::ExternalRngInput>>,
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
    // After Writhing Mass Parasite deck settles, keep PLAY lag peek active until combat ends.
    let mut writhing_parasite_play_lag = false;
    // Command string last resolved via later-observation peek (duplicate lag command).
    let mut last_peek_resolved_play: Option<String> = None;
    let mut pending_put_on_deck_card: Option<(CardInstance, bool)> = None;
    let mut pending_headbutt_put_on_draw_omit: Option<(CardInstance, usize)> = None;
    let mut pending_cross_combat_discard: Option<CardInstance> = None;
    let mut pending_elixir_deferred_selection = false;
    let mut pending_burning_pact_deferred_selection = false;
    let mut pending_armaments_deferred_selection = false;
    let mut pending_gambling_chip_deferred_selection = false;
    // CM energy lag after Blasphemy (observed - sim); comparison-only offset.
    let mut reconciled_deferred_action_steps = Vec::new();
    let mut last_post_message: Option<Value> = None;
    let mut last_post_received_at: Option<String> = None;
    let mut replay_current_action: Option<TraceAction> = None;
    // CardCrawlGame.playtime is wall-clock seconds. When the collector did not
    // record playtime_seconds, approximate from the first transition timestamp.
    let mut run_start_timestamp_millis: Option<i64> = None;

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

    #[allow(clippy::into_iter_on_ref)]
    let mut transition_iter = transitions.into_iter().peekable();
    while let Some((pre, action, post)) = transition_iter.next() {
        let external_rng = external_rng_by_action_step
            .get(&action.step)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if !external_rng.is_empty() && phase != SeedStartPhase::Shop {
            return finish_boundary!(SeedStartBoundary {
                path: format!("$.actions[step={}].external_rng", action.step),
                category: "unexpected_external_rng".to_owned(),
                reason: format!(
                    "{} external RNG draw(s) were attached outside the shop phase",
                    external_rng.len()
                ),
            });
        }
        if seed_sim.as_ref().is_some_and(|run| run.combat.is_none()) {
            writhing_parasite_play_lag = false;
            last_peek_resolved_play = None;
        }
        if is_trace_observation_poll(action) {
            // A later observation consumed by delayed PLAY probing settles the
            // command; it must not suppress a legitimate same-index PLAY after
            // that observation (FIDL00223/FIDL00235).
            last_peek_resolved_play = None;
        }
        if last_peek_resolved_play.as_ref() == Some(&action.command) {
            last_peek_resolved_play = None;
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: format!(
                    "{} (source duplicate lag command settlement frame)",
                    action.command
                ),
            });
            if let Some(sim) = seed_sim.as_mut() {
                let _ = bind_combat_piles_to_source_order(sim, &post.message);
            }
            continue;
        }
        // Trailing PLAY into empty observation after Writhing Mass parasite lag.
        if (pending_deck_assertion.is_some() || writhing_parasite_play_lag)
            && action
                .command
                .split_whitespace()
                .next()
                .is_some_and(|h| h.eq_ignore_ascii_case("PLAY"))
            && post.message.pointer("/game_state/combat_state").is_none()
        {
            if let Some(pending) = pending_deck_assertion.take() {
                report.verified.push(VerifiedTransition {
                    action_step: pending.action.step,
                    command: pending.action.command,
                    label: pending.label,
                });
                reconciled_deferred_action_steps.push(pending.action.step);
            }
            writhing_parasite_play_lag = false;
            last_peek_resolved_play = None;
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: format!(
                    "{} (source terminal lag no-op settlement frame)",
                    action.command
                ),
            });
            continue;
        }
        // CommMod can emit a PLAY whose post is still the pre-play snapshot
        // (FIDL00229 Writhing Mass; FIDL00397 Demon Form). When the next combat
        // observation matches applying this PLAY, resolve against that frame.
        // Terminal no-ops stay gated on Writhing Mass parasite lag only.
        if action
            .command
            .split_whitespace()
            .next()
            .is_some_and(|h| h.eq_ignore_ascii_case("PLAY"))
            && post.message.pointer("/game_state/combat_state").is_some()
        {
            if let Some(sim) = seed_sim.as_mut() {
                if sim.combat.is_some() {
                    let writhing_mega_debuff_pending = sim.combat.as_ref().is_some_and(|combat| {
                        combat.monsters.iter().any(|monster| {
                            monster.content_id == sts_core::content::monsters::WRITHING_MASS_ID
                                && monster.has_siphoned
                                && monster.alive
                        })
                    });
                    let observed_now = seed_start_combat_observed_subset(&post.message);
                    let simulated_now = seed_start_simulated_combat_subset(sim, false);
                    if seed_start_combat_subsets_match(observed_now, simulated_now) {
                        if let Some((_, _, next_post)) = transition_iter.peek() {
                            if next_post
                                .message
                                .pointer("/game_state/combat_state")
                                .is_some()
                            {
                                let mut attempt = sim.clone();
                                let _ =
                                    bind_combat_piles_to_source_order(&mut attempt, &pre.message);
                                let combat_action = attempt.combat.as_ref().and_then(|combat| {
                                    combat_action_from_command_with_observed_hand(
                                        &action.command,
                                        combat,
                                        Some(&pre.message),
                                    )
                                    .or_else(|| {
                                        combat_action_from_command_with_observed_hand(
                                            &action.command,
                                            combat,
                                            Some(&post.message),
                                        )
                                    })
                                    .or_else(|| {
                                        combat_action_from_pre_hand_index_on_sim_hand(
                                            &action.command,
                                            combat,
                                            &pre.message,
                                        )
                                    })
                                });
                                if let Some(combat_action) = combat_action {
                                    if let Ok(after_play) =
                                        apply_combat_action_on_run(&attempt, combat_action)
                                    {
                                        let next_obs =
                                            seed_start_combat_observed_subset(&next_post.message);
                                        let after_sim =
                                            seed_start_simulated_combat_subset(&after_play, false);
                                        if seed_start_combat_subsets_match(next_obs, after_sim) {
                                            if let Some(pending) = pending_deck_assertion.take() {
                                                report.verified.push(VerifiedTransition {
                                                    action_step: pending.action.step,
                                                    command: pending.action.command,
                                                    label: pending.label,
                                                });
                                                reconciled_deferred_action_steps
                                                    .push(pending.action.step);
                                                writhing_parasite_play_lag = true;
                                            }
                                            let label =
                                                combat_label_for_action(combat_action, &attempt);
                                            report.verified.push(VerifiedTransition {
                                                action_step: action.step,
                                                command: action.command.clone(),
                                                label: format!(
                                                    "{label} (resolved on later observation frame)"
                                                ),
                                            });
                                            last_peek_resolved_play = Some(action.command.clone());
                                            *sim = after_play;
                                            let _ = bind_combat_piles_to_source_layout(
                                                sim,
                                                &next_post.message,
                                            );
                                            continue;
                                        }
                                    }
                                }
                            } else if pending_deck_assertion.is_some()
                                || writhing_parasite_play_lag
                                || writhing_mega_debuff_pending
                            {
                                if let Some(pending) = pending_deck_assertion.take() {
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
                                    label: format!(
                                        "{} (source lag no-op settlement frame)",
                                        action.command
                                    ),
                                });
                                continue;
                            }
                        }
                    }
                }
            }
        }
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
        // Portal). Prefer explicit collector playtime; otherwise approximate from
        // wall-clock timestamps on the trace (same non-seeded clock class).
        if let Some(sim) = seed_sim.as_mut() {
            if let Some(playtime_seconds) =
                recorded_action_playtime_seconds(pre, action).or_else(|| {
                    let pre_ms = pre.received_at.as_deref().and_then(trace_timestamp_millis);
                    let action_ms = action.sent_at.as_deref().and_then(trace_timestamp_millis);
                    let post_ms = post.received_at.as_deref().and_then(trace_timestamp_millis);
                    let now_ms = action_ms.or(pre_ms).or(post_ms)?;
                    if run_start_timestamp_millis.is_none() {
                        run_start_timestamp_millis = Some(now_ms);
                    }
                    let start_ms = run_start_timestamp_millis?;
                    now_ms
                        .checked_sub(start_ms)
                        .filter(|elapsed| *elapsed >= 0)
                        .map(|elapsed| (elapsed / 1000) as u32)
                })
            {
                sim.playtime_seconds = playtime_seconds;
            }
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
        if pending_combat_assertion
            .as_ref()
            .is_some_and(|pending| pending.end_turn_source_lag)
        {
            let sim = seed_sim
                .as_ref()
                .expect("deferred END keeps the authoritative simulator state");
            let source_pre = seed_start_combat_observed_subset(&pre.message);
            let source_post = seed_start_combat_observed_subset(&post.message);
            let simulated = seed_start_simulated_combat_subset(sim, false);
            let pre_matches = seed_start_combat_subsets_match(
                source_pre,
                seed_start_simulated_combat_subset(sim, false),
            );
            let post_matches = seed_start_combat_subsets_match(source_post, simulated.clone());
            if pre_matches || post_matches {
                let pending = pending_combat_assertion
                    .take()
                    .expect("deferred END assertion checked above");
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
                if is_trace_observation_poll(action) && post_matches {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "deferred END stable combat observation poll".to_owned(),
                    });
                    continue;
                }
            } else if is_trace_observation_poll(action) {
                // The source may publish several copies of the pre-END frame
                // while the action queue drains. Do not compare that frame as a
                // settled result or mutate the authoritative simulator state.
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "deferred END source pre-action observation poll".to_owned(),
                });
                continue;
            } else {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unreconciled_combat_frame".to_owned(),
                    reason: "a semantic command arrived before the deferred END reached a stable source combat frame".to_owned(),
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
                if seed_start_is_candidate_neow_leave_transient_frame(&post.message) {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "transient Neow leave observation poll".to_owned(),
                    });
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
            let armed_time_warp = seed_sim.as_ref().is_some_and(|run| {
                run.combat
                    .as_ref()
                    .is_some_and(|combat| combat.time_warp_end_turn)
            });
            // Flush Time Warp only once the poll shows a post-draw player turn
            // (non-empty hand). Intermediate empty-hand end-turn frames lag.
            let observed_player_turn_ready = seed_start_combat_observed_subset(&post.message)
                .get("hand_ids")
                .and_then(Value::as_array)
                .is_some_and(|hand| !hand.is_empty());
            if armed_time_warp && !observed_player_turn_ready {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "trace client poll (Time Warp end-turn settling)".to_owned(),
                });
                continue;
            }
            if pending_combat_assertion.is_some() || (armed_time_warp && observed_player_turn_ready)
            {
                if armed_time_warp && observed_player_turn_ready {
                    if let Some(run) = seed_sim.as_mut() {
                        if let Some(mut combat) = run.combat.take() {
                            let _ = sts_core::combat::settle_time_warp_end_turn_if_ready_public(
                                &mut combat,
                            );
                            run.player_hp = combat.player.hp;
                            run.player_max_hp = combat.player.max_hp;
                            run.combat = Some(combat);
                        }
                    }
                }
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
        let neow_selection_transient = (phase == SeedStartPhase::NeowOptions
            && command_choose_index(&action.command).is_some()
            && seed_start_observed_subset(&pre.message)
                == seed_start_observed_subset(&post.message)
            && seed_start_observed_subset(&post.message).get("choices")
                == Some(&json!(seed_start_neow_choices_with_max_hp(
                    start.numeric_seed,
                    start.starting_hp(),
                ))))
            || ((phase == SeedStartPhase::NeowGrid || phase == SeedStartPhase::NeowGridConfirm)
                && (command_choose_index(&action.command).is_some()
                    || action.command.eq_ignore_ascii_case("CONFIRM"))
                && seed_start_grid_observed_subset(&pre.message)
                    == seed_start_grid_observed_subset(&post.message));
        let neow_selection_diff_count = report.unexpected_diffs.len();
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
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            &mut pending_map_assertion,
            &mut pending_deck_assertion,
            &mut reconciled_deferred_action_steps,
            &mut phase,
            report,
        ) {
            SeedStartPreDispatch::NotHandled => {}
            SeedStartPreDispatch::Handled => {
                if neow_selection_transient {
                    seed_start_mark_neow_selection_transient(
                        report,
                        action,
                        neow_selection_diff_count,
                    );
                }
                continue;
            }
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
            &mut pending_deck_assertion,
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
            &mut pending_headbutt_put_on_draw_omit,
            &mut pending_cross_combat_discard,
            &mut pending_elixir_deferred_selection,
            &mut pending_burning_pact_deferred_selection,
            &mut pending_armaments_deferred_selection,
            &mut pending_gambling_chip_deferred_selection,
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
            external_rng,
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

    // Empty combat-reward after Smoke Bomb is a valid capture endpoint. The
    // settling END (if any) was already verified as "settled to reward"; do not
    // leave Reward UI state as an unresolved deferred assertion at EOF.
    let ended_at_verified_smoke_bomb_empty_reward = smoke_bomb_ui.as_ref().is_some_and(|state| {
        matches!(
            state,
            SmokeBombUiState::Reward {
                pending_proceeds,
                ..
            } if pending_proceeds.is_empty()
        ) && last_post_message
            .as_ref()
            .is_some_and(|message| screen_type(message) == Some("COMBAT_REWARD"))
    });
    if ended_at_verified_smoke_bomb_empty_reward {
        let SmokeBombUiState::Reward { queued_end, .. } = smoke_bomb_ui
            .take()
            .expect("verified Smoke Bomb empty-reward endpoint remains present")
        else {
            unreachable!("verified Smoke Bomb empty-reward endpoint checked above")
        };
        if let Some(queued_end) = queued_end {
            reconciled_deferred_action_steps.push(queued_end.step);
        }
    }

    // Match and Keep choice-lag can end mid-board when the collector stops; the
    // last verified flip already carries the authoritative sim board.
    if let Some(pending) = pending_event_choice.take() {
        report.verified.push(VerifiedTransition {
            action_step: pending.action.step,
            command: pending.action.command,
            label: format!("{} at captured endpoint", pending.label),
        });
        reconciled_deferred_action_steps.push(pending.action.step);
    }

    // Pending event card-obtain (Drug Dealer / Nest / etc.) may still be armed
    // after leave has already been verified; EOF on map/event leave is fine.
    // Do not auto-clear on COMBAT_REWARD — Tiny House / reward overlays still need
    // an open deferred until a later stable reward observation.
    if let Some(pending) = pending_deck_assertion.take() {
        if last_post_message.as_ref().is_some_and(|message| {
            matches!(screen_type(message), Some("MAP" | "EVENT" | "NONE"))
                && message.pointer("/game_state/combat_state").is_none()
        }) {
            report.verified.push(VerifiedTransition {
                action_step: pending.action.step,
                command: pending.action.command,
                label: format!("{} reconciled at captured endpoint", pending.label),
            });
            reconciled_deferred_action_steps.push(pending.action.step);
            for (related_action, related_label) in pending.related_actions {
                report.verified.push(VerifiedTransition {
                    action_step: related_action.step,
                    command: related_action.command,
                    label: related_label,
                });
                reconciled_deferred_action_steps.push(related_action.step);
            }
        } else {
            pending_deck_assertion = Some(pending);
        }
    }

    // Map settlement lag can remain armed when the capture ends on MAP after the
    // leave/proceed that opened it. Prefer reconciling at EOF over leaving a
    // phantom unresolved deferred when category would otherwise be none.
    if let Some(pending) = pending_map_assertion.take() {
        let on_map = last_post_message
            .as_ref()
            .is_some_and(|message| screen_type(message) == Some("MAP"));
        if on_map {
            report.verified.push(VerifiedTransition {
                action_step: pending.action.step,
                command: pending.action.command,
                label: format!("{} at captured map endpoint", pending.label),
            });
            reconciled_deferred_action_steps.push(pending.action.step);
        } else {
            pending_map_assertion = Some(pending);
        }
    }

    // Smoke Bomb UI can remain armed after the player has long left the reward
    // screen (later floors / later combats). Drop stale UI state at EOF unless we
    // are still on an empty combat-reward smoke endpoint (handled above).
    if let Some(pending) = smoke_bomb_ui.take() {
        match pending {
            SmokeBombUiState::Escaping {
                action,
                pending_commands,
                ..
            } => {
                reconciled_deferred_action_steps.push(action.step);
                for command in pending_commands {
                    reconciled_deferred_action_steps.push(command.step);
                }
            }
            SmokeBombUiState::Reward {
                pending_proceeds,
                queued_end,
                ..
            } => {
                for proceed in pending_proceeds {
                    reconciled_deferred_action_steps.push(proceed.step);
                }
                if let Some(queued_end) = queued_end {
                    reconciled_deferred_action_steps.push(queued_end.step);
                }
            }
        }
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
