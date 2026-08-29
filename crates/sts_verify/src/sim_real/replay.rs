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

fn boundary(action: &TraceAction, category: &str, reason: impl Into<String>) -> SeedStartBoundary {
    SeedStartBoundary {
        path: format!("$.actions[step={}].command", action.step),
        category: category.to_owned(),
        reason: reason.into(),
    }
}

fn initialize_run(
    start: &StartRunCommand,
    boss_unlocks: BossUnlockState,
    profile: &TraceProfile,
) -> Result<RunState, String> {
    let mut run = RunState::try_seeded_ironclad_with_boss_unlocks(
        start.numeric_seed as u64,
        start.ascension,
        boss_unlocks,
    )
    .map_err(|error| error.to_string())?;
    if let Some(starting_hp) = start.verification_starting_hp {
        run.player_hp = starting_hp;
        run.player_max_hp = starting_hp;
    }
    run.note_card_content_id = profile
        .note_card
        .as_deref()
        .map(|note_card| {
            content_id_from_key(note_card).ok_or_else(|| format!("unknown Note card {note_card:?}"))
        })
        .transpose()?;
    run.note_card_upgrades = profile.note_upgrades;
    run.set_final_act_available(profile.final_act_available)
        .map_err(|error| error.to_string())?;
    run.validate().map_err(|error| error.to_string())?;
    Ok(run)
}

fn compare_direct_run(
    report: &mut SimRealReport,
    action: &TraceAction,
    post: &TraceState,
    label: &str,
    run: &RunState,
) -> Result<(), String> {
    // A collector can terminate the CommunicationMod process immediately after
    // a valid action. With no game_state there is no authoritative room snapshot
    // to compare; the protocol's only meaningful postcondition is in_game=false.
    // Keep the direct transition validated, but do not invent a next map from an
    // absent publication.
    if post.message.get("game_state").is_none()
        && post.message.get("in_game").and_then(Value::as_bool) == Some(false)
        && post
            .message
            .get("available_commands")
            .and_then(Value::as_array)
            .is_some_and(|commands| commands.iter().any(|command| command == "start"))
    {
        compare_subset(
            report,
            action,
            label,
            json!({ "in_game": false }),
            json!({ "in_game": false }),
        );
        return Ok(());
    }
    let (mut observed, mut simulated) = if run.card_grid.is_some() {
        (
            seed_start_grid_observed_subset(&post.message),
            seed_start_grid_simulated_subset(run),
        )
    } else {
        match run.phase {
            RunPhase::Combat => (
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(run),
            ),
            RunPhase::Reward
                if run
                    .reward
                    .as_ref()
                    .is_some_and(|reward| !reward.boss_relic_choices.is_empty()) =>
            {
                (
                    seed_start_boss_reward_observed_subset(&post.message),
                    seed_start_boss_reward_simulated_subset(run),
                )
            }
            RunPhase::Reward => (
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(run),
            ),
            // A queued obtain is not in the master deck yet: the leave screen
            // still shows the pre-obtain deck and the card lands on the exit
            // transition. The canonical deck already excludes pending obtains.
            RunPhase::Event => (
                seed_start_event_observed_subset(&post.message),
                seed_start_event_simulated_subset(run),
            ),
            RunPhase::Idle => (
                seed_start_map_return_observed_subset(&post.message),
                seed_start_simulated_map_return(run)?,
            ),
            RunPhase::Victory => (
                seed_start_victory_observed_subset(&post.message),
                seed_start_victory_simulated_subset(run),
            ),
            RunPhase::Rest => (
                seed_start_rest_observed_subset(&post.message),
                seed_start_rest_simulated_subset(run),
            ),
            // Reserved boss-relic pool entries exist before the chest is opened.
            // Project BOSS_REWARD only while the boss-relic reward screen is active.
            RunPhase::Treasure
                if run
                    .reward
                    .as_ref()
                    .is_some_and(|reward| !reward.boss_relic_choices.is_empty()) =>
            {
                (
                    seed_start_boss_reward_observed_subset(&post.message),
                    seed_start_boss_reward_simulated_subset(run),
                )
            }
            RunPhase::Treasure => (
                seed_start_treasure_observed_subset(&post.message),
                seed_start_treasure_simulated_subset(run),
            ),
            RunPhase::Shop => (
                seed_start_shop_observed_subset(&post.message),
                if run.shop_merchant_open {
                    seed_start_shop_screen_simulated_subset(run)
                } else {
                    seed_start_shop_room_simulated_subset(run)
                },
            ),
            RunPhase::Complete => (
                seed_start_victory_observed_subset(&post.message),
                seed_start_complete_simulated_subset(run),
            ),
        }
    };
    if let Some(observed_keys) = post
        .message
        .get("game_state")
        .and_then(|game| game.get("keys"))
    {
        if let (Some(observed), Some(simulated)) =
            (observed.as_object_mut(), simulated.as_object_mut())
        {
            observed.insert("keys".to_owned(), observed_keys.clone());
            simulated.insert(
                "keys".to_owned(),
                json!({
                    "emerald": run.has_emerald_key,
                    "ruby": run.has_ruby_key,
                    "sapphire": run.has_sapphire_key,
                }),
            );
        }
    }
    compare_subset(report, action, label, observed, simulated);
    Ok(())
}

fn combat_decision(run: &RunState, command: &str) -> Result<(RunDecisionAction, String), String> {
    if run
        .combat
        .as_ref()
        .is_some_and(|combat| combat.phase == sts_core::CombatPhase::Lost)
        && command_head_eq(command, "PROCEED")
    {
        return Ok((
            RunDecisionAction::Run(RunAction::Proceed),
            "combat loss proceed".to_owned(),
        ));
    }
    // Potion use remains legal while a hand/discard/exhaust selection is
    // open. Decode it before the active-selection command binding so an
    // interrupting POTION USE is not misread as an invalid selection action.
    if let Some(potion_use) = parse_potion_use(command) {
        return Ok((
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: potion_use.slot,
                target: seed_start_potion_command_target(run, &potion_use),
            }),
            "combat potion use".to_owned(),
        ));
    }
    if let Some(decision) = seed_start_active_combat_decision(run)? {
        return seed_start_bind_combat_decision_command(decision, command)
            .map(|(action, label)| (RunDecisionAction::Run(action), label.to_owned()));
    }
    let combat = run
        .combat
        .as_ref()
        .ok_or_else(|| "combat phase has no combat state".to_owned())?;
    let action = combat_action_from_command(command, combat)
        .ok_or_else(|| format!("could not decode combat command {command:?}"))?;
    if let Some(reason) = unsupported_seed_start_combat_command(combat, command) {
        return Err(reason);
    }
    Ok((
        RunDecisionAction::Combat(action),
        combat_label_for_action(action, run),
    ))
}

pub(super) fn direct_decision(
    run: &RunState,
    command: &str,
) -> Result<(RunDecisionAction, String), String> {
    if run.phase == RunPhase::Combat {
        let (action, label) = combat_decision(run, command)?;
        return Ok((action, label));
    }
    let legal = legal_run_decision_actions(run).map_err(|error| error.to_string())?;
    let selected = if let Some(index) = choose_index(command) {
        if run.phase == RunPhase::Reward && run.card_grid.is_none() {
            // Reward CHOOSE indices follow CommunicationMod choice_list order,
            // not the denser legal-action vector (which also includes Proceed/Skip).
            Some(RunDecisionAction::Run(
                seed_start_bind_reward_choose_action(run, index)?,
            ))
        } else {
            // A relic pickup can open a card grid without leaving Reward phase
            // (for example Bottled Flame/Lightning/Tornado). CommunicationMod
            // CHOOSE then addresses the active grid, not the outer reward list.
            legal.get(index).copied()
        }
    } else if command_head_eq(command, "CONFIRM") {
        legal.iter().copied().find(|action| {
            matches!(
                action,
                RunDecisionAction::GridConfirm
                    | RunDecisionAction::Run(RunAction::ConfirmHandSelect)
                    | RunDecisionAction::Run(RunAction::ConfirmDrawSelect)
                    | RunDecisionAction::Run(RunAction::ConfirmDiscardSelect)
                    | RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)
            )
        })
    } else if command_head_eq(command, "CANCEL") {
        legal
            .iter()
            .copied()
            .find(|action| matches!(action, RunDecisionAction::GridCancel))
    } else if command_head_eq(command, "PROCEED") {
        legal.iter().copied().find(|action| {
            matches!(
                action,
                RunDecisionAction::Run(RunAction::Proceed)
                    | RunDecisionAction::Rest(RestAction::Proceed)
            )
        })
    } else if command_head_eq(command, "LEAVE") {
        legal
            .iter()
            .copied()
            .find(|action| matches!(action, RunDecisionAction::Run(RunAction::LeaveShop)))
    } else if command_head_eq(command, "SKIP") {
        // CommunicationMod SKIP on an open CardRewardScreen closes back to the
        // outer combat-reward list (card item remains). Prefer CloseCardReward
        // over SkipReward, which would abandon the whole reward overlay.
        let card_reward_open = run
            .reward
            .as_ref()
            .is_some_and(sts_core::RewardScreen::card_reward_is_active);
        legal.iter().copied().find(|action| {
            if card_reward_open {
                matches!(action, RunDecisionAction::Run(RunAction::CloseCardReward))
            } else {
                matches!(
                    action,
                    RunDecisionAction::Run(
                        RunAction::SkipReward
                            | RunAction::SkipPotionReward
                            | RunAction::SkipCombatCardReward
                            | RunAction::CloseCardReward
                    )
                )
            }
        })
    } else {
        parse_potion_use(command).map(|potion_use| {
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: potion_use.slot,
                target: seed_start_potion_command_target(run, &potion_use),
            })
        })
    };
    selected
        .map(|action| (action, format!("direct {:?} transition", run.phase)))
        .ok_or_else(|| {
            format!(
                "command {command:?} does not identify one of {} simulator-owned legal actions in {:?}",
                legal.len(),
                run.phase
            )
        })
}

#[derive(Default)]
pub(super) struct StreamingSeedStartReplay {
    seed_sim: Option<RunState>,
    replay_action: Option<TraceAction>,
}

impl StreamingSeedStartReplay {
    /// A rejected command can still let a queued end-turn drain in the target
    /// between the command error and the next observed state. This advances
    /// from simulator state only; it never reads the observation.
    pub(super) fn settle_time_warp_after_rejected_command(&mut self) -> Result<(), String> {
        let Some(run) = self.seed_sim.as_mut() else {
            return Ok(());
        };
        let Some(combat) = run.combat.as_mut() else {
            return Ok(());
        };
        sts_core::combat::settle_queued_end_turn_discard_after_rejected_command(combat)
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn finish_streaming_boundary(
    state: &mut StreamingSeedStartReplay,
    boundary: SeedStartBoundary,
    replay_capture: &mut Option<&mut ReplayCapture>,
) -> SeedStartBoundary {
    record_replay_checkpoint(
        replay_capture,
        state.replay_action.take(),
        state.seed_sim.as_ref(),
    );
    boundary
}

pub(super) struct SeedStartReplayInputs<'a> {
    pub(super) start: &'a StartRunCommand,
    pub(super) boss_unlocks: BossUnlockState,
    pub(super) profile: &'a TraceProfile,
    pub(super) source_playtime_seconds: Option<u32>,
}

pub(super) fn verify_seed_start_transition(
    state: &mut StreamingSeedStartReplay,
    action: &TraceAction,
    post: &TraceState,
    external_rng: &[sts_core::ExternalRngInput],
    report: &mut SimRealReport,
    inputs: SeedStartReplayInputs<'_>,
    replay_capture: &mut Option<&mut ReplayCapture>,
) -> Option<SeedStartBoundary> {
    let SeedStartReplayInputs {
        start,
        boss_unlocks,
        profile,
        source_playtime_seconds,
    } = inputs;
    record_replay_checkpoint(
        replay_capture,
        state.replay_action.take(),
        state.seed_sim.as_ref(),
    );
    state.replay_action = Some(action.clone());
    sts_core::set_rng_trace_context(sts_core::RngTraceContext {
        action_step: Some(action.step),
        command: Some(action.command.clone()),
    });

    let boundary = if start.matches_command(&action.command) {
        if state.seed_sim.is_some() {
            Some(boundary(
                action,
                "invalid_start_command",
                "START appeared after initialization",
            ))
        } else {
            match initialize_run(start, boss_unlocks, profile) {
                Ok(run) => {
                    if let Err(reason) =
                        compare_direct_run(report, action, post, "direct START transition", &run)
                    {
                        Some(boundary(action, "invalid_direct_projection", reason))
                    } else {
                        state.seed_sim = Some(run);
                        None
                    }
                }
                Err(reason) => Some(boundary(action, "invalid_start_state", reason)),
            }
        }
    } else {
        let Some(current) = state.seed_sim.as_ref() else {
            return Some(finish_streaming_boundary(
                state,
                boundary(
                    action,
                    "missing_start_state",
                    "command arrived before START",
                ),
                replay_capture,
            ));
        };
        if current.phase == RunPhase::Complete
            && (current.current_act == 4
                || current.event.as_ref().is_some_and(|event| {
                    event.event == sts_core::Event::SpireHeart
                        && event.stage == 4
                        && event.choices.is_empty()
                }))
            && action.command.trim().eq_ignore_ascii_case("PROCEED")
        {
            if !external_rng.is_empty() {
                Some(boundary(
                    action,
                    "unconsumed_external_rng",
                    "terminal presentation exit cannot consume external RNG",
                ))
            } else {
                compare_subset(
                    report,
                    action,
                    "leave completed run",
                    json!({
                        "in_game": post.message.get("in_game").and_then(Value::as_bool),
                    }),
                    json!({ "in_game": false }),
                );
                seed_start_take_first_diff_boundary(report)
            }
        } else if command_head_eq(&action.command, "STATE")
            || command_head_eq(&action.command, "WAIT")
        {
            // Observation commands do not advance the simulator. The recorded
            // frame is expected output only.
            if !external_rng.is_empty() {
                Some(boundary(
                    action,
                    "unconsumed_external_rng",
                    "observation command cannot consume external RNG",
                ))
            } else if let Err(reason) = compare_direct_run(
                report,
                action,
                post,
                "direct simulator observation",
                current,
            ) {
                Some(boundary(action, "invalid_direct_projection", reason))
            } else {
                None
            }
        } else {
            match direct_decision(current, &action.command) {
                Err(reason) => {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: reason.clone(),
                    });
                    Some(boundary(action, "unsupported_direct_command", reason))
                }
                Ok((decision, label)) => {
                    let mut source = current.clone();
                    // Secret Portal eligibility is wall-clock state, not seeded
                    // gameplay RNG. Feed the last observed source-state timer
                    // before event selection; do not infer it from floor number.
                    if let Some(playtime_seconds) =
                        action.playtime_seconds.or(source_playtime_seconds)
                    {
                        source.playtime_seconds = playtime_seconds;
                    }
                    source
                        .pending_external_rng
                        .extend(external_rng.iter().copied());
                    match apply_run_decision_action(&source, decision) {
                        Err(error) => {
                            let reason = error.to_string();
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: reason.clone(),
                            });
                            Some(boundary(action, "unsupported_direct_transition", reason))
                        }
                        Ok(next) if !next.pending_external_rng.is_empty() => Some(boundary(
                            action,
                            "unconsumed_external_rng",
                            format!(
                                "{} typed external RNG input(s) were not consumed by the direct transition",
                                next.pending_external_rng.len()
                            ),
                        )),
                        Ok(next) => {
                            if let Err(reason) =
                                compare_direct_run(report, action, post, &label, &next)
                            {
                                Some(boundary(action, "invalid_direct_projection", reason))
                            } else {
                                state.seed_sim = Some(next);
                                None
                            }
                        }
                    }
                }
            }
        }
    };

    let boundary = boundary.or_else(|| seed_start_take_first_diff_boundary(report));
    boundary.map(|boundary| finish_streaming_boundary(state, boundary, replay_capture))
}

pub(super) fn finish_streaming_seed_start_replay(
    state: &mut StreamingSeedStartReplay,
    replay_capture: &mut Option<&mut ReplayCapture>,
) -> Option<RunState> {
    record_replay_checkpoint(
        replay_capture,
        state.replay_action.take(),
        state.seed_sim.as_ref(),
    );
    state.seed_sim.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialize_run_preserves_optional_profile_inputs() {
        let start = StartRunCommand {
            action_step: 1,
            character: "IRONCLAD".to_owned(),
            ascension: 0,
            external_seed: "1".to_owned(),
            numeric_seed: 1,
            verification_starting_hp: None,
        };
        let profile = TraceProfile {
            note_card: None,
            note_upgrades: 0,
            final_act_available: Some(true),
        };

        let run = initialize_run(&start, BossUnlockState::default(), &profile)
            .expect("optional profile inputs initialize the run");

        assert_eq!(run.note_card_content_id, None);
        assert_eq!(run.final_act_available, Some(true));
    }

    #[test]
    fn completed_spire_heart_proceed_is_terminal_and_accounted() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.current_act = 3;
        run.current_floor = 51;
        run.current_room_override = Some(RoomKind::Victory);
        run.phase = RunPhase::Complete;
        let mut heart = sts_core::event_screen(Event::SpireHeart);
        heart.stage = 4;
        heart.choices.clear();
        run.event = Some(heart);
        run.validate().expect("completed Heart state validates");

        let expected_state = run.clone();
        let mut replay = StreamingSeedStartReplay {
            seed_sim: Some(run),
            replay_action: None,
        };
        let start = StartRunCommand {
            action_step: 1,
            character: "IRONCLAD".to_owned(),
            ascension: 0,
            external_seed: "1".to_owned(),
            numeric_seed: 1,
            verification_starting_hp: None,
        };
        let profile = TraceProfile {
            note_card: Some("Strike".to_owned()),
            note_upgrades: 0,
            final_act_available: Some(false),
        };
        let action = TraceAction {
            step: 2,
            command: "PROCEED".to_owned(),
            sent_at: None,
            command_meta: None,
            playtime_seconds: None,
        };
        let post = TraceState {
            step: 2,
            received_at: None,
            message: json!({"in_game": false}),
        };
        let mut report = SimRealReport {
            total_actions: 1,
            action_dispositions: Vec::new(),
            action_integrity: None,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            seed_start: None,
        };
        let mut replay_capture: Option<&mut ReplayCapture> = None;

        let boundary = verify_seed_start_transition(
            &mut replay,
            &action,
            &post,
            &[],
            &mut report,
            SeedStartReplayInputs {
                start: &start,
                boss_unlocks: BossUnlockState::default(),
                profile: &profile,
                source_playtime_seconds: None,
            },
            &mut replay_capture,
        );

        assert!(boundary.is_none());
        assert_eq!(report.unexpected_diffs, Vec::new());
        assert_eq!(report.unsupported, Vec::new());
        assert_eq!(report.verified.len(), 1);
        assert_eq!(report.verified[0].action_step, 2);
        assert_eq!(report.verified[0].label, "leave completed run");
        assert_eq!(replay.seed_sim, Some(expected_state));
    }
}
