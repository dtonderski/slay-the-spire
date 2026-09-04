use super::*;

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
    if !start.character.eq_ignore_ascii_case("IRONCLAD") {
        return Err(format!("unsupported character {:?}", start.character));
    }
    let mut run = RunState::try_seeded_ironclad_with_boss_unlocks(
        start.numeric_seed as u64,
        start.ascension,
        boss_unlocks,
    )
    .map_err(|error| error.to_string())?;
    run.hp = start.verification_starting_hp;
    run.max_hp = start.verification_starting_hp;
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
            json!({ "in_game": false }),
            json!({ "in_game": run.phase != RunPhase::Complete }),
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
    compare_subset(report, action, observed, simulated);
    Ok(())
}

fn combat_decision(run: &RunState, command: &str) -> Result<RunDecisionAction, String> {
    if run
        .combat
        .as_ref()
        .is_some_and(|combat| combat.phase == sts_core::adapter_internals::CombatPhase::Lost)
        && command_head_eq(command, "PROCEED")
    {
        return Ok(RunDecisionAction::Run(RunAction::Proceed));
    }
    // Potion use remains legal while a hand/discard/exhaust selection is
    // open. Decode it before the active-selection command binding so an
    // interrupting POTION USE is not misread as an invalid selection action.
    if let Some(potion_use) = parse_potion_use(command) {
        return Ok(RunDecisionAction::Run(RunAction::UsePotion {
            slot: potion_use.slot,
            target: seed_start_potion_command_target(run, &potion_use),
        }));
    }
    if let Some(decision) = seed_start_active_combat_decision(run)? {
        return seed_start_bind_combat_decision_command(decision, command)
            .map(|(action, _)| RunDecisionAction::Run(action));
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
    Ok(RunDecisionAction::Combat(action))
}

pub(super) fn direct_decision(run: &RunState, command: &str) -> Result<RunDecisionAction, String> {
    if run.phase == RunPhase::Combat {
        return combat_decision(run, command);
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
            .is_some_and(sts_core::adapter_internals::RewardScreen::card_reward_is_active);
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
    selected.ok_or_else(|| {
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
    external_rng: &[sts_core::adapter_internals::ExternalRngInput],
    report: &mut SimRealReport,
    inputs: SeedStartReplayInputs<'_>,
) -> Option<SeedStartBoundary> {
    let SeedStartReplayInputs {
        start,
        boss_unlocks,
        profile,
        source_playtime_seconds,
    } = inputs;
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
                    if let Err(reason) = compare_direct_run(report, action, post, &run) {
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
            return Some(boundary(
                action,
                "missing_start_state",
                "command arrived before START",
            ));
        };
        if current.phase == RunPhase::Complete
            && (current.current_act == 4
                || current.event.as_ref().is_some_and(|event| {
                    event.event == sts_core::adapter_internals::Event::SpireHeart
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
            } else if let Err(reason) = compare_direct_run(report, action, post, current) {
                Some(boundary(action, "invalid_direct_projection", reason))
            } else {
                None
            }
        } else {
            match direct_decision(current, &action.command) {
                Err(reason) => Some(boundary(action, "unsupported_command", reason)),
                Ok(decision) => {
                    let mut source = current.clone();
                    // Action metadata is authoritative when present. Older
                    // traces fall back to the immediately preceding source
                    // observation until collection records action timing again.
                    if let Some(playtime_seconds) =
                        action.playtime_seconds.or(source_playtime_seconds)
                    {
                        source.playtime_seconds = playtime_seconds;
                    }
                    source
                        .pending_external_rng
                        .extend(external_rng.iter().copied());
                    match apply_run_decision_action(&source, decision) {
                        Err(error) => Some(boundary(
                            action,
                            "transition_error",
                            error.to_string(),
                        )),
                        Ok(next) if !next.pending_external_rng.is_empty() => Some(boundary(
                            action,
                            "unconsumed_external_rng",
                            format!(
                                "{} typed external RNG input(s) were not consumed by the direct transition",
                                next.pending_external_rng.len()
                            ),
                        )),
                        Ok(next) => {
                            if let Err(reason) = compare_direct_run(report, action, post, &next)
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

    boundary.or_else(|| seed_start_take_first_diff_boundary(report))
}
