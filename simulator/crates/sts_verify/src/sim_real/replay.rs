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
    run.note_card_content_id = content_id_from_key(&profile.note_card)
        .ok_or_else(|| format!("unknown Note card {:?}", profile.note_card))?;
    run.note_card_upgrades = profile.note_upgrades;
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
    let (observed, simulated) = if run.card_grid.is_some() {
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
    compare_subset(report, action, label, observed, simulated);
    Ok(())
}

/// A new HandCardSelectScreen calls `prep()` before opening and clears the
/// previous screen's `selectedCards`. If a prior skipped-retrieval candidate
/// still parks those cards, replace that stale screen-owned selection before
/// rebuilding the newly interrupted action.
fn clear_superseded_selection_screen_pending(run: &mut RunState) {
    if let Some(combat) = run.combat.as_mut() {
        combat.pending_hidden_hand_card_until_end_turn.clear();
        combat.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
    }
}

fn skipped_put_on_deck_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    if !matches!(
        decision,
        RunDecisionAction::Run(RunAction::ConfirmHandSelect)
    ) {
        return Ok(None);
    }
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(hand_select) = combat.hand_select() else {
        return Ok(None);
    };
    if !matches!(
        hand_select.purpose,
        HandSelectPurpose::WarcryPutOnDraw
            | HandSelectPurpose::ThinkingAheadPutOnDraw
            | HandSelectPurpose::ForethoughtPutOnDraw
    ) {
        return Ok(None);
    }

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let (mut candidate, selected) =
        sts_core::run::apply_hand_select_confirm_skipped_put_on_deck_retrieval(&source)
            .map_err(|error| error.to_string())?;
    let combat = candidate
        .combat
        .as_mut()
        .ok_or_else(|| "skipped put-on-deck candidate lost combat state".to_owned())?;
    combat
        .pending_hidden_hand_card_until_end_turn
        .push(selected);
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_burning_pact_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    if !matches!(
        decision,
        RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)
    ) {
        return Ok(None);
    }
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(exhaust_select) = combat.exhaust_select() else {
        return Ok(None);
    };
    if !matches!(
        exhaust_select.purpose,
        ExhaustSelectPurpose::BurningPactDraw2 | ExhaustSelectPurpose::BurningPactDraw3
    ) {
        return Ok(None);
    }
    if exhaust_select.source_card.is_none() || exhaust_select.selected_hand_indices.len() != 1 {
        return Ok(None);
    }
    // Runic Pyramid keeps the retained hand cards out of the next shuffle;
    // leave this selected card fully untracked for that source-backed window.
    let retained_by_runic_pyramid = combat.relics.contains(&Relic::RunicPyramid);

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let (mut candidate, selected) =
        sts_core::run::apply_exhaust_select_confirm_skipped_burning_pact_retrieval(&source)
            .map_err(|error| error.to_string())?;
    let combat = candidate
        .combat
        .as_mut()
        .ok_or_else(|| "skipped Burning Pact candidate lost combat state".to_owned())?;
    if !retained_by_runic_pyramid {
        combat
            .pending_hidden_hand_card_until_end_turn
            .push(selected);
    }
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_exhume_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseExhaustSelect { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(exhaust_select) = combat.exhaust_select() else {
        return Ok(None);
    };
    if exhaust_select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand
        || exhaust_select.source_card.is_none()
        || !exhaust_select.source_card_force_exhaust
        || !exhaust_select.selected_hand_indices.is_empty()
    {
        return Ok(None);
    }

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let candidate = sts_core::run::apply_exhaust_select_choice_skipped_exhume(&source, index)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn stable_exhume_skipped_post(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    message.get("ready_for_command").and_then(Value::as_bool) == Some(true)
        && message.get("boundary_kind").and_then(Value::as_str) == Some("quiescent")
        && message.get("actions_queued").and_then(Value::as_u64) == Some(0)
        && game.get("screen_type").and_then(Value::as_str) == Some("NONE")
        && game.get("action_phase").and_then(Value::as_str) == Some("WAITING_ON_USER")
}

fn skipped_gambling_chip_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    if !matches!(
        decision,
        RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)
    ) {
        return Ok(None);
    }
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(exhaust_select) = combat.exhaust_select() else {
        return Ok(None);
    };
    if exhaust_select.purpose != ExhaustSelectPurpose::GamblingChip {
        return Ok(None);
    }
    // Empty selections are identical on the normal path; only non-empty selects
    // can produce the skipped-retrieval combat projection.
    if exhaust_select.selected_hand_indices.is_empty() {
        return Ok(None);
    }
    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let candidate =
        sts_core::run::apply_exhaust_select_confirm_skipped_gambling_chip_retrieval(&source)
            .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_burning_pact_selected_card_is_absent_from_observed_exhaust(
    source: &RunState,
    post: &TraceState,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(exhaust_select) = source_combat.exhaust_select() else {
        return false;
    };
    let Some(selected_index) = exhaust_select.selected_hand_indices.first().copied() else {
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
    let observed_exhaust = post
        .message
        .pointer("/game_state/combat_state/exhaust_pile");
    if !observed_exhaust.is_some_and(Value::is_array) {
        return false;
    }
    let observed_exhaust_count = combat_card_ids(observed_exhaust)
        .into_iter()
        .filter(|card| card == &selected_key)
        .count();
    observed_exhaust_count <= source_exhaust_count
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
    if let Some(decision) = seed_start_active_combat_decision(run)? {
        return seed_start_bind_combat_decision_command(decision, command)
            .map(|(action, label)| (RunDecisionAction::Run(action), label.to_owned()));
    }
    if let Some(potion_use) = parse_potion_use(command) {
        return Ok((
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: potion_use.slot,
                target: seed_start_potion_command_target(run, &potion_use),
            }),
            "combat potion use".to_owned(),
        ));
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
            && current.event.as_ref().is_some_and(|event| {
                event.event == sts_core::Event::SpireHeart
                    && event.stage == 4
                    && event.choices.is_empty()
            })
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
            if !external_rng.is_empty() {
                Some(boundary(
                    action,
                    "unconsumed_external_rng",
                    "observation command cannot consume external RNG",
                ))
            } else {
                compare_direct_run(
                    report,
                    action,
                    post,
                    "direct simulator observation",
                    current,
                )
                .err()
                .map(|reason| boundary(action, "invalid_direct_projection", reason))
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
                            Some(boundary(
                                action,
                                "unsupported_direct_transition",
                                reason,
                            ))
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
                            // PutOnDeckAction has a source-backed skipped-retrieval
                            // frame: rebuild from the pre-CONFIRM state, then use it
                            // only when the complete observed combat projection
                            // matches. This keeps an unrelated pile mismatch on the
                            // normal transition fail-closed.
                            let skipped_candidate = skipped_put_on_deck_candidate(&source, decision)
                                .ok()
                                .flatten()
                                .filter(|candidate| candidate.pending_external_rng.is_empty())
                                .filter(|candidate| {
                                    subset_diffs(
                                        seed_start_combat_observed_subset(&post.message),
                                        seed_start_simulated_combat_subset(candidate),
                                    )
                                    .is_empty()
                                })
                                .or_else(|| {
                                    skipped_burning_pact_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            skipped_burning_pact_selected_card_is_absent_from_observed_exhaust(
                                                &source, post,
                                            )
                                        })
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                })
                                .or_else(|| {
                                    skipped_gambling_chip_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                })
                                .or_else(|| {
                                    skipped_exhume_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_exhume_skipped_post(&post.message))
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                });
                            if let Some(candidate) = skipped_candidate {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: label.clone(),
                                });
                                state.seed_sim = Some(candidate);
                                None
                            } else if let Err(reason) =
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
    use std::collections::VecDeque;

    #[test]
    fn skipped_put_on_deck_candidate_parks_selected_card_until_end_turn() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        let selected_card_id = combat.piles.hand[1].id;
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: sts_core::combat::HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id,
                selected_hand_index: Some(1),
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: VecDeque::new(),
        });

        let candidate = skipped_put_on_deck_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ConfirmHandSelect),
        )
        .expect("candidate construction")
        .expect("Warcry candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat.pending_hidden_hand_card_until_end_turn[0].id,
            selected_card_id
        );
        assert!(combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .chain(combat.piles.limbo.iter())
            .all(|card| card.id != selected_card_id));
        assert!(combat
            .piles
            .hand
            .iter()
            .all(|card| card.id != selected_card_id));
    }

    #[test]
    fn skipped_burning_pact_candidate_replaces_superseded_screen_selection() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let superseded_card = combat.piles.hand.remove(1);
        let superseded_card_id = superseded_card.id;
        combat.pending_hidden_hand_card_until_end_turn = vec![superseded_card];
        let mut source_card = combat.piles.hand.remove(0);
        source_card.content_id = sts_core::content::cards::BURNING_PACT_ID;
        let source_card_id = source_card.id;
        let selected_card_id = combat.piles.hand[0].id;
        combat.decision = Some(CombatDecisionState::ExhaustSelect {
            state: sts_core::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::BurningPactDraw2,
                source_card_id: Some(source_card_id),
                source_card: Some(source_card),
                source_card_force_exhaust: false,
                selected_hand_indices: vec![0],
                interrupted_by_cultist_potion: false,
                pending_actions: VecDeque::new(),
            },
        });

        let candidate = skipped_burning_pact_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ConfirmExhaustSelect),
        )
        .expect("candidate construction")
        .expect("Burning Pact candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat
                .pending_hidden_hand_card_until_end_turn
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![selected_card_id]
        );
        assert!(combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .chain(combat.piles.limbo.iter())
            .chain(combat.pending_hidden_hand_card_until_end_turn.iter())
            .all(|card| card.id != superseded_card_id));
        assert!(combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .chain(combat.piles.limbo.iter())
            .all(|card| card.id != selected_card_id));
        assert!(combat.piles.discard_pile.iter().any(|card| {
            card.id == source_card_id
                && card.content_id == sts_core::content::cards::BURNING_PACT_ID
        }));
    }

    #[test]
    fn skipped_exhume_candidate_keeps_selected_card_in_exhaust() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(1000),
            sts_core::content::cards::DEFEND_R_ID,
        )];
        let selected_id = CardId::new(1001);
        combat.piles.exhaust_pile = vec![CardInstance::new(
            selected_id,
            sts_core::content::cards::DAZED_ID,
        )];
        let source_id = CardId::new(1002);
        combat.decision = Some(CombatDecisionState::ExhaustSelect {
            state: sts_core::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::ExhumeReturnToHand,
                source_card_id: Some(source_id),
                source_card: Some(CardInstance::new(
                    source_id,
                    sts_core::content::cards::EXHUME_ID,
                )),
                source_card_force_exhaust: true,
                selected_hand_indices: Vec::new(),
                interrupted_by_cultist_potion: false,
                pending_actions: VecDeque::new(),
            },
        });

        let candidate = skipped_exhume_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ChooseExhaustSelect { index: 0 }),
        )
        .expect("candidate construction")
        .expect("force-play Exhume candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert!(combat.piles.hand.iter().all(|card| card.id != selected_id));
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == selected_id));
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == sts_core::content::cards::EXHUME_ID));
        assert!(combat.exhaust_select().is_none());
    }

    #[test]
    fn skipped_gambling_chip_candidate_parks_selected_cards_until_end_turn() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let selected_indices = vec![1, 0, 2];
        let selected_ids = selected_indices
            .iter()
            .map(|index| combat.piles.hand[*index].id)
            .collect::<Vec<_>>();
        let hand_before = combat.piles.hand.len();
        let draw_before = combat.piles.draw_pile.len();
        combat.decision = Some(CombatDecisionState::ExhaustSelect {
            state: sts_core::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::GamblingChip,
                source_card_id: None,
                source_card: None,
                source_card_force_exhaust: false,
                selected_hand_indices: selected_indices,
                interrupted_by_cultist_potion: false,
                pending_actions: VecDeque::new(),
            },
        });

        let candidate = skipped_gambling_chip_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ConfirmExhaustSelect),
        )
        .expect("candidate construction")
        .expect("Gambling Chip candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat
                .pending_hidden_hand_card_until_end_turn
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            selected_ids
        );
        assert_eq!(combat.piles.hand.len(), hand_before - 3);
        assert_eq!(combat.piles.draw_pile.len(), draw_before);
        assert!(combat.piles.discard_pile.is_empty());
        assert!(combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .chain(combat.piles.limbo.iter())
            .all(|card| !selected_ids.contains(&card.id)));
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
            note_card: "Strike".to_owned(),
            note_upgrades: 0,
        };
        let action = TraceAction {
            step: 2,
            command: "PROCEED".to_owned(),
            sent_at: None,
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
