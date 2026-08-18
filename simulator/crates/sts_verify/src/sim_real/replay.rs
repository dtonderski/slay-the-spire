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

fn pending_event_obtain_simulated_subset(run: &RunState) -> Option<Value> {
    if run.phase != RunPhase::Event || run.pending_obtain_cards.is_empty() {
        return None;
    }
    let canonical = seed_start_event_simulated_subset(run);
    let has_leave = canonical
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| choices.iter().any(|choice| choice == "leave"));
    if !has_leave {
        return None;
    }
    let mut deck = run.deck.clone();
    for (index, content_id) in run.pending_obtain_cards.iter().copied().enumerate() {
        deck.push(CardInstance::new(
            CardId::new((index + 1) as u64),
            content_id,
        ));
    }
    Some(seed_start_event_simulated_subset_with_deck(
        run,
        deck_content_keys(&deck),
    ))
}

fn immediate_combat_obtain_simulated_subset(next: &RunState) -> Option<Value> {
    if next.phase != RunPhase::Combat || next.pending_combat_obtain_cards.is_empty() {
        return None;
    }

    // Combat AddCardToDeckAction publication is mixed in the target bridge:
    // some END frames expose the newly created curse before the next
    // combat-owned transition, while the canonical simulator queue settles on
    // that next transition. Compare the eager publication as a projection only.
    let mut published = next.clone();
    let pending = published.pending_combat_obtain_cards.clone();
    for content_id in pending {
        // Reuse normal obtain hooks (Ceramic Fish, card-add relics, and
        // max-HP effects) in the temporary projection so this timing
        // allowance does not hide accompanying gameplay state.
        published.gain_deck_card(content_id).ok()?;
    }
    Some(seed_start_simulated_combat_subset(&published))
}

/// A duplicate END can publish a queued combat obtain without starting a
/// second player/monster turn. CommunicationMod accepts the button command
/// while `endTurnQueued` is still settling, and the following frame can expose
/// only the typed AddCardToDeckAction (FIDL01595 / FIDL01515 Parasite +
/// Ceramic Fish). Flush the authoritative pending queue, then require the
/// complete observed combat projection to match; never hydrate the card or
/// its UUID from the observation.
///
/// Do not execute the queued `EndTurnAction` here. SuperFastMode can still
/// show the live hand (FIDL01595 END 1119, FIDL01515 END 915). The leftover
/// EndTurn discards and runs Combust on a later STATE, or a following END
/// finishes the turn from this published combat.
fn duplicate_end_combat_obtain_publication_candidate(
    source: &RunState,
    post: &TraceState,
    command: &str,
) -> Option<RunState> {
    if !command_head_eq(command, "END")
        || source.phase != RunPhase::Combat
        || source.pending_combat_obtain_cards.is_empty()
        || source
            .combat
            .as_ref()
            .is_none_or(|combat| combat.decision.is_some())
    {
        return None;
    }

    let source_combat = source.combat.as_ref()?;
    let mut published = source.clone();
    published.flush_pending_combat_obtain_cards().ok()?;
    if published.combat.as_ref()? != source_combat {
        return None;
    }
    if !subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&published),
    )
    .is_empty()
    {
        return None;
    }
    published.validate().ok()?;
    Some(published)
}

fn deferred_pending_combat_obtain_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if source.pending_combat_obtain_cards.is_empty()
        || matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn))
    {
        return None;
    }
    let mut deferred = source.clone();
    deferred.defer_pending_combat_obtain_settlement = true;
    let candidate = apply_run_decision_action(&deferred, decision).ok()?;
    if !candidate.pending_external_rng.is_empty() {
        return None;
    }
    if subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    {
        Some(candidate)
    } else {
        None
    }
}

fn headbutt_play_from_hand(source: &RunState, decision: RunDecisionAction) -> bool {
    let RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) = decision else {
        return false;
    };
    source.combat.as_ref().is_some_and(|combat| {
        combat.piles.hand.iter().any(|card| {
            card.id == card_id
                && matches!(
                    card.content_id,
                    sts_core::content::cards::HEADBUTT_ID
                        | sts_core::content::cards::HEADBUTT_PLUS_ID
                )
        })
    })
}

fn apply_headbutt_draw_alias(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
    alias: CardInstance,
    combat_only: bool,
) -> Option<RunState> {
    let mut candidate = apply_run_decision_action(source, decision).ok()?;
    let combat = candidate.combat.as_mut()?;
    let verifier_id = alias
        .id
        .get()
        .checked_add(sts_core::HEADBUTT_SKIPPED_RETRIEVAL_ALIAS_ID_OFFSET)?;
    let mut aliased = alias;
    aliased.id = sts_core::CardId::new(verifier_id);
    if combat_only {
        aliased.combat_only = true;
    }
    combat.piles.draw_pile.push(aliased);
    candidate.pending_headbutt_alias = None;
    if subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    {
        candidate.validate().ok()?;
        Some(candidate)
    } else {
        None
    }
}

fn deferred_headbutt_alias_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !headbutt_play_from_hand(source, decision) {
        return None;
    }
    if let Some(alias) = source.pending_headbutt_alias.or_else(|| {
        source
            .combat
            .as_ref()?
            .pending_hidden_hand_card_until_end_turn
            .first()
            .cloned()
    }) {
        if let Some(candidate) = apply_headbutt_draw_alias(source, decision, post, alias, false) {
            return Some(candidate);
        }
    }
    // Singleton PutOnDeckAction can leave the same AbstractCard in exhaust
    // and on top of draw (FIDL01246 Feed, FIDL01834 Reaper+). Remint a
    // combat-only view from simulator exhaust; do not copy observed names.
    let combat = source.combat.as_ref()?;
    if combat.piles.discard_pile.len() != 1 {
        return None;
    }
    for alias in combat.piles.exhaust_pile.iter().rev().copied() {
        if let Some(candidate) = apply_headbutt_draw_alias(source, decision, post, alias, true) {
            return Some(candidate);
        }
    }
    // The same auto-put can also republish the previous top of draw with the
    // same UUID after the discard card is added (FIDL01787 Strike). Remint
    // that simulator top; do not copy observed identities.
    for alias in combat.piles.draw_pile.iter().rev().copied() {
        if let Some(candidate) = apply_headbutt_draw_alias(source, decision, post, alias, true) {
            return Some(candidate);
        }
        if let Some(candidate) = apply_headbutt_draw_alias(source, decision, post, alias, false) {
            return Some(candidate);
        }
    }
    None
}

fn deferred_colosseum_opening_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    let RunDecisionAction::Event(EventAction::Choose { choice_index: 1 }) = decision else {
        return None;
    };
    if !source
        .event
        .as_ref()
        .is_some_and(|screen| screen.event == Event::Colosseum && screen.stage == 2)
    {
        return None;
    }
    let candidate = sts_core::apply_event_action_with_deferred_colosseum_opening(
        source,
        EventAction::Choose { choice_index: 1 },
    )
    .ok()?;
    if candidate.phase != RunPhase::Combat || !candidate.pending_external_rng.is_empty() {
        return None;
    }
    if subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    {
        Some(candidate)
    } else {
        None
    }
}

fn deferred_cursed_key_chest_simulated_subset(
    source: &RunState,
    decision: RunDecisionAction,
    next: &RunState,
) -> Option<Value> {
    if !matches!(decision, RunDecisionAction::Run(RunAction::OpenChest))
        || source.phase != RunPhase::Treasure
        || next.phase != RunPhase::Reward
        || !source.relics.contains(&Relic::CursedKey)
        || next.deck.len() != source.deck.len() + 1
    {
        return None;
    }

    // Cursed Key's ShowCardAndObtainEffect can publish its curse on the next
    // reward poll rather than on the initial chest-open frame. Keep the core's
    // eager obtain authoritative, but compare the source-backed old-deck
    // publication when every other reward field agrees.
    let source_ids: std::collections::HashSet<_> = source.deck.iter().map(|card| card.id).collect();
    let mut deferred = next.clone();
    let added_index = deferred
        .deck
        .iter()
        .position(|card| !source_ids.contains(&card.id))?;
    deferred.deck.remove(added_index);
    // The eager core obtain also runs card-added relic hooks (notably Ceramic
    // Fish). The target's old-deck publication predates those hooks, so project
    // the source gold alongside the source deck while retaining the generated
    // reward offers and RNG reservations.
    deferred.gold = source.gold;
    Some(seed_start_reward_simulated_subset(&deferred))
}

fn pending_reward_obtain_publication_candidate(
    run: &RunState,
    post: &TraceState,
    command: &str,
) -> Option<RunState> {
    if choose_index(command).is_none()
        || run.phase != RunPhase::Reward
        || run.card_grid.is_some()
        || run.pending_obtain_cards.is_empty()
    {
        return None;
    }
    let mut published = run.clone();
    published.flush_pending_obtain_cards().ok()?;
    let diffs = subset_diffs(
        seed_start_reward_observed_subset(&post.message),
        seed_start_reward_simulated_subset(&published),
    );
    diffs.is_empty().then_some(published)
}

fn deferred_event_leave_simulated_subset(source: &RunState, next: &RunState) -> Option<Value> {
    if source.phase != RunPhase::Event
        || next.phase != RunPhase::Idle
        || !source.event.as_ref().is_some_and(|event| {
            event
                .choices
                .iter()
                .any(|choice| choice.label.eq_ignore_ascii_case("leave"))
        })
    {
        return None;
    }

    // CommunicationMod can return one command-ready Leave frame before the
    // room closes. Project that frame from the pre-action event and the
    // canonical post-action state; keep `next` authoritative for the next
    // STATE/map command.
    let mut transient = next.clone();
    transient.phase = RunPhase::Event;
    transient.event = source.event.clone();
    Some(seed_start_event_simulated_subset(&transient))
}

fn deferred_scry_keep_simulated_subset(
    source: &RunState,
    decision: RunDecisionAction,
    next: &RunState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Run(RunAction::ConfirmDrawSelect)
    ) {
        return None;
    }
    let source_combat = source.combat.as_ref()?;
    let draw_select = source_combat.draw_select()?;
    if draw_select.purpose != sts_core::combat::DrawSelectPurpose::Scry {
        return None;
    }
    let selected_index = draw_select.selected_draw_index?;
    let selected_card = source_combat.piles.draw_pile.get(selected_index).copied()?;
    let mut deferred = next.clone();
    let combat = deferred.combat.as_mut()?;
    let discard_index = combat
        .piles
        .discard_pile
        .iter()
        .position(|card| card.id == selected_card.id)?;
    combat.piles.discard_pile.remove(discard_index);
    combat.piles.draw_pile.insert(selected_index, selected_card);
    Some(deferred)
}

fn skipped_event_room_entry_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    next: &RunState,
    post: &TraceState,
) -> Option<RunState> {
    let RunDecisionAction::Map(action) = decision else {
        return None;
    };
    if source.phase != RunPhase::Idle || next.phase != RunPhase::Event {
        return None;
    }
    let node_id = match action {
        sts_core::MapAction::ChooseNode { node_id } => node_id,
    };
    let room_kind = source.map.as_ref()?.map.node(node_id)?.room_kind;
    if room_kind != sts_core::RoomKind::Event {
        return None;
    }
    // EventRoom.onPlayerEntry already rolled the room and removed the event
    // from the act list. SuperFastMode published MAP before the event screen
    // was command-ready, so keep that RNG/seen-list and drop the unopened
    // screen (FIDL01297: beggar consumed at floor 19, Mausoleum at 21).
    let mut published = next.clone();
    published.phase = RunPhase::Idle;
    published.event = None;
    let observed = seed_start_map_return_observed_subset(&post.message);
    if observed.get("screen_type").and_then(Value::as_str) != Some("MAP") {
        return None;
    }
    let simulated = seed_start_simulated_map_return(&published).ok()?;
    subset_diffs(observed, simulated)
        .is_empty()
        .then_some(published)
}

fn deferred_event_obtain_map_simulated_subset(source: &RunState, next: &RunState) -> Option<Value> {
    if source.phase != RunPhase::Event
        || next.phase != RunPhase::Idle
        || source.pending_obtain_cards.is_empty()
        || !source.event.as_ref().is_some_and(|event| {
            event
                .choices
                .iter()
                .any(|choice| choice.label.eq_ignore_ascii_case("leave"))
        })
    {
        return None;
    }

    // CommunicationMod can publish a completed event's Leave -> MAP frame
    // before the event's ShowCardAndObtainEffect deck mutation is visible. Keep
    // the simulator's canonical flushed state, but construct the source-backed
    // stale-publication projection without mutating it.
    let mut deferred = next.clone();
    for content_id in source.pending_obtain_cards.iter().rev() {
        let index = deferred
            .deck
            .iter()
            .rposition(|card| card.content_id == *content_id)?;
        deferred.deck.remove(index);
    }
    seed_start_simulated_map_return(&deferred).ok()
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
            RunPhase::Event => {
                let observed = seed_start_event_observed_subset(&post.message);
                let simulated = pending_event_obtain_simulated_subset(run)
                    .filter(|published| {
                        subset_diffs(observed.clone(), published.clone()).is_empty()
                    })
                    .unwrap_or_else(|| seed_start_event_simulated_subset(run));
                (observed, simulated)
            }
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

/// A new card-selection screen calls `prep()` before opening and clears the
/// previous screen's `selectedCards`. If a prior skipped-retrieval candidate
/// still parks those cards, replace that stale screen-owned selection before
/// rebuilding the newly interrupted action.
fn clear_superseded_selection_screen_pending(run: &mut RunState) {
    if let Some(combat) = run.combat.as_mut() {
        combat.pending_hidden_hand_card_until_end_turn.clear();
        combat.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
    }
}

fn opened_new_selection_screen(source: &RunState, next: &RunState) -> bool {
    let before = source
        .combat
        .as_ref()
        .and_then(|combat| combat.decision.as_ref());
    let after = next
        .combat
        .as_ref()
        .and_then(|combat| combat.decision.as_ref());
    let kind = |decision: &sts_core::combat::CombatDecisionState| match decision {
        sts_core::combat::CombatDecisionState::HandSelect { .. } => Some(0_u8),
        sts_core::combat::CombatDecisionState::DrawSelect { .. } => Some(1),
        sts_core::combat::CombatDecisionState::DiscardSelect { .. } => Some(2),
        sts_core::combat::CombatDecisionState::ExhaustSelect { .. } => Some(3),
        _ => None,
    };
    let (Some(before_kind), Some(after_kind)) = (before.and_then(kind), after.and_then(kind))
    else {
        return before.and_then(kind).is_none() && after.and_then(kind).is_some();
    };
    if before_kind != after_kind {
        return true;
    }
    match (before, after) {
        (
            Some(sts_core::combat::CombatDecisionState::HandSelect { state: before, .. }),
            Some(sts_core::combat::CombatDecisionState::HandSelect { state: after, .. }),
        ) => before.source_card_id != after.source_card_id || before.purpose != after.purpose,
        (
            Some(sts_core::combat::CombatDecisionState::DrawSelect { state: before }),
            Some(sts_core::combat::CombatDecisionState::DrawSelect { state: after }),
        ) => before.source_card_id != after.source_card_id || before.purpose != after.purpose,
        (
            Some(sts_core::combat::CombatDecisionState::DiscardSelect { state: before }),
            Some(sts_core::combat::CombatDecisionState::DiscardSelect { state: after }),
        ) => before.source_card_id != after.source_card_id || before.purpose != after.purpose,
        (
            Some(sts_core::combat::CombatDecisionState::ExhaustSelect { state: before }),
            Some(sts_core::combat::CombatDecisionState::ExhaustSelect { state: after }),
        ) => before.source_card_id != after.source_card_id || before.purpose != after.purpose,
        _ => false,
    }
}

/// `HandCardSelectScreen.prep()` clears `selectedCards`. Grid screens
/// (Secret Weapon, Headbutt, Scry) use `GridCardSelectScreen` and leave that
/// list alone, so a prior combat's skipped-retrieval residual can still
/// publish on a later END.
fn opened_hand_card_select_screen(next: &RunState) -> bool {
    matches!(
        next.combat
            .as_ref()
            .and_then(|combat| combat.decision.as_ref()),
        Some(sts_core::combat::CombatDecisionState::HandSelect { .. })
            | Some(sts_core::combat::CombatDecisionState::ExhaustSelect { .. })
    )
}

fn should_drop_cross_combat_hand_select_residual(source: &RunState, next: &RunState) -> bool {
    opened_new_selection_screen(source, next) && opened_hand_card_select_screen(next)
}

/// Keep the residual while this combat still parks it, and across a
/// combat-ending END (the leftover `selectedCards` is not a master-deck card).
/// Same-combat non-empty settlement and a successful next-combat inject leave
/// `pending_hidden` empty while combat continues.
fn end_turn_consumed_cross_combat_hand_select_residual(next: &RunState) -> bool {
    next.combat
        .as_ref()
        .is_some_and(|combat| combat.pending_hidden_hand_card_until_end_turn.is_empty())
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
            | HandSelectPurpose::ForethoughtPutAnyOnDraw
    ) {
        return Ok(None);
    }

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    if hand_select.purpose == HandSelectPurpose::ForethoughtPutAnyOnDraw {
        let combat = source
            .combat
            .as_mut()
            .ok_or_else(|| "skipped Forethought+ candidate lost combat state".to_owned())?;
        sts_core::combat::transition::confirm_forethought_multi_select_skipped_retrieval(combat)
            .map_err(|error| error.to_string())?;
        source.validate().map_err(|error| error.to_string())?;
        return Ok(Some(source));
    }

    let retained_by_runic_pyramid = combat.relics.contains(&Relic::RunicPyramid);
    let (mut candidate, selected) =
        sts_core::run::apply_hand_select_confirm_skipped_put_on_deck_retrieval(&source)
            .map_err(|error| error.to_string())?;
    if !retained_by_runic_pyramid {
        let combat = candidate
            .combat
            .as_mut()
            .ok_or_else(|| "skipped put-on-deck candidate lost combat state".to_owned())?;
        combat
            .pending_hidden_hand_card_until_end_turn
            .push(selected);
    }
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_skipped_put_on_deck_candidate(
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
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }

    let retained_by_runic_pyramid = combat.relics.contains(&Relic::RunicPyramid);
    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let (mut candidate, selected) =
        sts_core::run::apply_hand_select_confirm_skipped_put_on_deck_retrieval_without_time_warp_end(
            &source,
        )
        .map_err(|error| error.to_string())?;
    if !retained_by_runic_pyramid {
        let combat = candidate.combat.as_mut().ok_or_else(|| {
            "deferred Time Warp skipped put-on-deck candidate lost combat state".to_owned()
        })?;
        combat
            .pending_hidden_hand_card_until_end_turn
            .push(selected);
    }
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn put_on_deck_return_to_hand_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Result<Option<RunState>, String> {
    if !matches!(
        decision,
        RunDecisionAction::Run(RunAction::ConfirmHandSelect)
    ) {
        return Ok(None);
    }
    let Some(combat) = source.combat.as_ref() else {
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
    let selected_index = hand_select
        .selected_hand_index
        .ok_or_else(|| "put-on-deck return candidate has no selected card".to_owned())?;
    let selected_id = combat
        .piles
        .hand
        .get(selected_index)
        .ok_or_else(|| "put-on-deck return candidate index is out of range".to_owned())?
        .id;
    let mut candidate =
        apply_run_decision_action(source, decision).map_err(|error| error.to_string())?;
    let candidate_combat = candidate
        .combat
        .as_mut()
        .ok_or_else(|| "put-on-deck return candidate lost combat state".to_owned())?;
    if let Some(draw_index) = candidate_combat
        .piles
        .draw_pile
        .iter()
        .position(|card| card.id == selected_id)
    {
        let selected = candidate_combat.piles.draw_pile.remove(draw_index);
        candidate_combat.piles.hand.push(selected);
    } else if !candidate_combat
        .piles
        .hand
        .iter()
        .any(|card| card.id == selected_id)
    {
        return Err("put-on-deck return candidate selected card missing".to_owned());
    }
    candidate.validate().map_err(|error| error.to_string())?;
    let observed = seed_start_combat_observed_subset(&post.message);
    let simulated = seed_start_simulated_combat_subset(&candidate);
    Ok(subset_diffs(observed, simulated)
        .is_empty()
        .then_some(candidate))
}

fn skipped_warcry_auto_place_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    if combat.decision.is_some() {
        return Ok(None);
    }
    let Some(card) = combat.piles.hand.iter().find(|card| card.id == card_id) else {
        return Ok(None);
    };
    if !matches!(card.content_id, WARCRY_ID | WARCRY_PLUS_ID) {
        return Ok(None);
    }
    if combat.piles.hand.iter().any(|other| other.id != card_id) {
        return Ok(None);
    }

    let mut source = run.clone();
    if let Some(combat) = source.combat.as_mut() {
        combat.skip_put_on_deck_auto_place = true;
    }
    let candidate =
        apply_run_decision_action(&source, decision).map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_hand_select_candidate(
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
            | HandSelectPurpose::ForethoughtPutAnyOnDraw
    ) {
        return Ok(None);
    }
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }

    let candidate = sts_core::run::apply_hand_select_confirm_without_time_warp_end(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_hand_select_metallicize_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    if deferred_time_warp_hand_select_candidate(run, decision)?.is_none() {
        return Ok(None);
    }
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    if combat.player.powers.metallicize <= 0 {
        return Ok(None);
    }
    let candidate = sts_core::run::apply_hand_select_confirm_time_warp_metallicize_lag(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn time_warp_status_lag_hand_select_candidate(
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
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }
    let Some(selected_index) = hand_select.selected_hand_index else {
        return Ok(None);
    };
    let Some(selected) = combat.piles.hand.get(selected_index) else {
        return Ok(None);
    };
    if !matches!(
        selected.content_id,
        sts_core::content::cards::BURN_ID
            | sts_core::content::cards::DECAY_ID
            | sts_core::content::cards::REGRET_ID
            | sts_core::content::cards::DOUBT_ID
            | sts_core::content::cards::SHAME_ID
    ) {
        return Ok(None);
    }

    let candidate = sts_core::run::apply_hand_select_confirm_time_warp_status_lag(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn time_warp_remaining_status_lag_hand_select_candidate(
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
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }
    let Some(selected_index) = hand_select.selected_hand_index else {
        return Ok(None);
    };
    let Some(selected) = combat.piles.hand.get(selected_index) else {
        return Ok(None);
    };
    let is_end_turn_autoplay = |content_id| {
        matches!(
            content_id,
            sts_core::content::cards::BURN_ID
                | sts_core::content::cards::DECAY_ID
                | sts_core::content::cards::REGRET_ID
                | sts_core::content::cards::DOUBT_ID
                | sts_core::content::cards::SHAME_ID
        )
    };
    if is_end_turn_autoplay(selected.content_id) {
        return Ok(None);
    }
    let leftover_autoplay = combat.piles.hand.iter().enumerate().any(|(index, card)| {
        index != selected_index
            && card.id != hand_select.source_card_id
            && is_end_turn_autoplay(card.content_id)
    });
    if !leftover_autoplay {
        return Ok(None);
    }

    let candidate = sts_core::run::apply_hand_select_confirm_time_warp_remaining_status_lag(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_armaments_candidate(
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
    if hand_select.purpose != HandSelectPurpose::ArmamentsUpgrade
        || hand_select.selected_hand_index.is_none()
        || !(combat.play_top_force_exhaust_active
            || combat
                .piles
                .exhaust_pile
                .iter()
                .chain(combat.piles.discard_pile.iter())
                .any(|card| card.id == hand_select.source_card_id))
    {
        return Ok(None);
    }

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let candidate = sts_core::combat::confirm_hand_select_skipped_armaments_retrieval(
        source
            .combat
            .as_mut()
            .ok_or_else(|| "skipped Armaments candidate lost combat state".to_owned())?,
    )
    .map(|_| source)
    .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_dual_wield_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    skipped_dual_wield_candidate_with_restore(run, decision, true)
}

fn skipped_dual_wield_without_restore_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    skipped_dual_wield_candidate_with_restore(run, decision, false)
}

fn skipped_dual_wield_candidate_with_restore(
    run: &RunState,
    decision: RunDecisionAction,
    restore_dropped: bool,
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
    if hand_select.purpose != HandSelectPurpose::DualWieldCopy
        || hand_select.selected_hand_index.is_none()
    {
        return Ok(None);
    }

    let mut candidate = run.clone();
    clear_superseded_selection_screen_pending(&mut candidate);
    let combat = candidate
        .combat
        .as_mut()
        .ok_or_else(|| "skipped Dual Wield candidate lost combat state".to_owned())?;
    if restore_dropped {
        sts_core::combat::confirm_dual_wield_select_skipped_retrieval(combat)
            .map_err(|error| error.to_string())?;
    } else {
        sts_core::combat::confirm_dual_wield_select_skipped_retrieval_without_restore(combat)
            .map_err(|error| error.to_string())?;
    }
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_exhaust_select_candidate(
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
        ExhaustSelectPurpose::BurningPactDraw2
            | ExhaustSelectPurpose::BurningPactDraw3
            | ExhaustSelectPurpose::TrueGritExhaustOne
            | ExhaustSelectPurpose::PurityExhaustUpTo3
            | ExhaustSelectPurpose::ExhumeReturnToHand
    ) {
        return Ok(None);
    }
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }

    let candidate = sts_core::run::apply_exhaust_select_confirm_without_time_warp_end(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_exhaust_select_metallicize_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    if deferred_time_warp_exhaust_select_candidate(run, decision)?.is_none() {
        return Ok(None);
    }
    let candidate = sts_core::run::apply_exhaust_select_confirm_time_warp_metallicize_lag(run)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

/// CommunicationMod can still emit PLAY after a Time Warp exhaust/hand select
/// CONFIRM parked the forced END. That leftover command must flush the end-turn
/// instead of resolving the card (FIDL01566 PLAY after Burning Pact).
fn deferred_time_warp_end_instead_of_play_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. })
    ) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if combat.decision.is_some() {
        return None;
    }
    if !(combat.time_warp_end_turn
        || combat.time_warp_end_turn_pre_discard_settled
        || combat.time_warp_end_powers_applied)
    {
        return None;
    }
    let observed = seed_start_combat_observed_subset(&post.message);
    flush_deferred_time_warp_end_for_leftover_play(source, &observed, false)
        .or_else(|| flush_deferred_time_warp_end_for_leftover_play(source, &observed, true))
        .or_else(|| flush_deferred_time_warp_powers_and_discard(source, &observed))
}

fn flush_deferred_time_warp_powers_and_discard(
    source: &RunState,
    observed: &serde_json::Value,
) -> Option<RunState> {
    // Leftover PLAY after a 12th-card CONFIRM can publish Combust + discard
    // before DrawCardAction (FIDL01666 Hemokinesis STATE: empty hand, -1 HP,
    // Time Eater -7). Full end_player_turn would already have drawn.
    let mut candidate = source.clone();
    let combat = candidate.combat.as_mut()?;
    sts_core::combat::settle_leftover_end_turn_player_powers_and_discard(combat).ok()?;
    candidate.player_hp = combat.player.hp;
    candidate.player_max_hp = combat.player.max_hp;
    if !candidate.pending_external_rng.is_empty() || candidate.validate().is_err() {
        return None;
    }
    subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

fn flush_deferred_time_warp_end_for_leftover_play(
    source: &RunState,
    observed: &serde_json::Value,
    resume_pre_discard: bool,
) -> Option<RunState> {
    let mut candidate = source.clone();
    let mut combat = candidate.combat.take()?;
    if resume_pre_discard {
        if !combat.time_warp_end_turn_pre_discard_settled {
            return None;
        }
        combat.time_warp_duplicate_monster_queue = false;
        combat.time_warp_end_turn = false;
    }
    let finished = sts_core::combat::end_player_turn(&combat).ok()?;
    candidate.player_hp = finished.player.hp;
    candidate.player_max_hp = finished.player.max_hp;
    candidate.combat = Some(finished);
    if !candidate.pending_external_rng.is_empty() || candidate.validate().is_err() {
        return None;
    }
    subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

fn skipped_purity_candidate(
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
    if exhaust_select.purpose != ExhaustSelectPurpose::PurityExhaustUpTo3
        || exhaust_select.source_card_id.is_none()
        || exhaust_select.selected_hand_indices.is_empty()
    {
        return Ok(None);
    }

    let mut candidate = run.clone();
    clear_superseded_selection_screen_pending(&mut candidate);
    sts_core::combat::confirm_purity_select_skipped_retrieval(
        candidate
            .combat
            .as_mut()
            .ok_or_else(|| "skipped Purity candidate lost combat state".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_true_grit_candidate(
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
    if exhaust_select.purpose != ExhaustSelectPurpose::TrueGritExhaustOne
        || exhaust_select.source_card.is_none()
        || exhaust_select.selected_hand_indices.len() != 1
    {
        return Ok(None);
    }

    let mut candidate = run.clone();
    clear_superseded_selection_screen_pending(&mut candidate);
    sts_core::combat::confirm_true_grit_select_skipped_retrieval(
        candidate
            .combat
            .as_mut()
            .ok_or_else(|| "skipped True Grit candidate lost combat state".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
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
    if exhaust_select.selected_hand_indices.len() != 1 {
        return Ok(None);
    }
    let force_exhausted_source_ready = exhaust_select.source_card_force_exhaust
        && exhaust_select.source_card_id.is_some_and(|source_id| {
            combat
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.id == source_id)
        });
    if exhaust_select.source_card.is_none() && !force_exhausted_source_ready {
        return Ok(None);
    }
    // Runic Pyramid keeps the retained hand cards out of the next shuffle;
    // leave this selected card fully untracked for that source-backed window.
    let retained_by_runic_pyramid = combat.relics.contains(&Relic::RunicPyramid);

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let (mut candidate, selected) =
        sts_core::run::apply_exhaust_select_confirm_skipped_burning_pact_retrieval_without_time_warp_end(
            &source,
        )
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

fn deferred_shop_pending_obtain_candidate(
    source: &RunState,
    next: &RunState,
    post: &TraceState,
) -> Option<RunState> {
    if source.phase != RunPhase::Shop || source.pending_obtain_cards.is_empty() {
        return None;
    }
    let mut candidate = next.clone();
    candidate.flush_pending_obtain_cards().ok()?;
    let observed = seed_start_shop_observed_subset(&post.message);
    let simulated = if candidate.shop_merchant_open {
        seed_start_shop_screen_simulated_subset(&candidate)
    } else {
        seed_start_shop_room_simulated_subset(&candidate)
    };
    subset_diffs(observed, simulated)
        .is_empty()
        .then_some(candidate)
}

fn cross_shop_dolly_candidate(
    source: &RunState,
    next: &RunState,
    pending_card: Option<CardInstance>,
    post: &TraceState,
) -> Option<RunState> {
    if source.phase != RunPhase::Shop || pending_card.is_none() {
        return None;
    }
    let mut candidate = next.clone();
    let mut card = pending_card?;
    card.id = sts_core::CardId::new(candidate.next_card_instance_id().ok()?);
    candidate.add_deck_card(card).ok()?;
    let observed = seed_start_shop_observed_subset(&post.message);
    let simulated = if candidate.shop_merchant_open {
        seed_start_shop_screen_simulated_subset(&candidate)
    } else {
        seed_start_shop_room_simulated_subset(&candidate)
    };
    subset_diffs(observed, simulated)
        .is_empty()
        .then_some(candidate)
}

fn cross_shop_dolly_before_action_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    pending_card: Option<CardInstance>,
    post: &TraceState,
) -> Option<RunState> {
    if source.phase != RunPhase::Shop || pending_card.is_none() {
        return None;
    }
    let mut pre_action = source.clone();
    let mut card = pending_card?;
    card.id = sts_core::CardId::new(pre_action.next_card_instance_id().ok()?);
    pre_action.add_deck_card(card).ok()?;
    let candidate = sts_core::run::apply_run_decision_action(&pre_action, decision).ok()?;
    let observed = seed_start_shop_observed_subset(&post.message);
    let simulated = if candidate.shop_merchant_open {
        seed_start_shop_screen_simulated_subset(&candidate)
    } else {
        seed_start_shop_room_simulated_subset(&candidate)
    };
    subset_diffs(observed, simulated)
        .is_empty()
        .then_some(candidate)
}

fn deferred_monster_death_gremlin_horn_candidate(
    source: &RunState,
    next: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return None;
    }
    let source_combat = source.combat.as_ref()?;
    let next_combat = next.combat.as_ref()?;
    if !source_combat
        .relics
        .contains(&sts_core::relic::Relic::GremlinHorn)
        || source_combat
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .count()
            <= next_combat
                .monsters
                .iter()
                .filter(|monster| monster.alive)
                .count()
        || next_combat.piles.hand.len() < 5
    {
        return None;
    }
    let mut candidate = next.clone();
    let combat = candidate.combat.as_mut()?;
    let card = combat.piles.draw_pile.pop()?;
    combat.piles.hand.push(card);
    candidate.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

/// Extra leftover EndTurn can open another Codex while the hand is still held
/// (FIDL01486 END 475: Clash/Twin/Immolate, Sword Boomerang stays). Ordinary
/// stage-2 END would discard. Keep stage 1 so the next CHOOSE is another
/// first-offer pause.
fn deferred_nilrys_keep_hand_extra_offer_on_end_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if !combat.resume_end_turn_after_nilrys_codex
        || combat.decision.is_some()
        || combat.piles.hand.is_empty()
        || !matches!(combat.nilrys_codex_end_turn_stage, 1 | 2)
    {
        return None;
    }
    let mut candidate = source.clone();
    let combat = candidate.combat.as_mut()?;
    sts_core::relic::open_nilrys_codex_card_reward(combat).ok()?;
    combat.resume_end_turn_after_nilrys_codex = true;
    combat.nilrys_codex_end_turn_stage = 1;
    candidate.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

fn deferred_nilrys_second_offer_on_end_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if !combat.resume_end_turn_after_nilrys_codex
        || combat.nilrys_codex_end_turn_stage != 1
        || combat.decision.is_some()
    {
        return None;
    }
    let mut candidate = source.clone();
    let combat = candidate.combat.as_mut()?;
    combat.nilrys_codex_end_turn_stage = 2;
    let next = sts_core::combat::end_player_turn(combat).ok()?;
    candidate.player_hp = next.player.hp;
    candidate.player_max_hp = next.player.max_hp;
    candidate.combat = Some(next);
    candidate.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

fn deferred_nilrys_first_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 1
        || !matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::NilrysCodexCardReward { .. })
        )
    {
        return None;
    }
    let park = |index: Option<usize>, apply_end_turn_block: bool| -> Option<RunState> {
        let mut candidate = source.clone();
        let combat = candidate.combat.as_mut()?;
        match index {
            Some(index) => {
                sts_core::relic::nilrys_codex_park_choice_without_insert(combat, index).ok()?;
            }
            None => {
                combat.decision = None;
            }
        }
        // First-offer close continues callEndOfTurnActions card autoplays
        // (Regret/Burn) while the chosen Codex card stays unpublished
        // (FIDL01597 CHOOSE 822: Regret discards, Rampage is not inserted).
        sts_core::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(combat).ok()?;
        candidate.player_hp = combat.player.hp;
        candidate.player_max_hp = combat.player.max_hp;
        if apply_end_turn_block {
            sts_core::relic::nilrys_codex_apply_paused_end_turn_block_powers(combat).ok()?;
        }
        combat.nilrys_codex_end_turn_stage = 2;
        candidate.validate().ok()?;
        subset_diffs(
            seed_start_combat_observed_subset(&post.message),
            seed_start_simulated_combat_subset(&candidate),
        )
        .is_empty()
        .then_some(candidate)
    };
    match decision {
        RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            let insert_and_plated = || -> Option<RunState> {
                let mut candidate = apply_run_decision_action(source, decision).ok()?;
                let combat = candidate.combat.as_mut()?;
                sts_core::relic::nilrys_codex_apply_paused_end_turn_block_powers(combat).ok()?;
                combat.nilrys_codex_end_turn_stage = 2;
                candidate.validate().ok()?;
                subset_diffs(
                    seed_start_combat_observed_subset(&post.message),
                    seed_start_simulated_combat_subset(&candidate),
                )
                .is_empty()
                .then_some(candidate)
            };
            park(Some(index), false)
                .or_else(|| park(Some(index), true))
                .or_else(insert_and_plated)
        }
        RunDecisionAction::Run(RunAction::SkipCombatCardReward) => {
            park(None, false).or_else(|| park(None, true))
        }
        _ => None,
    }
}

/// Book.takeTurn reads live `stabCount` after the first queued multi-stab.
/// The default two-step keeps captured N+N (FIDL01727 step 880). Accept the
/// live N+(N+1) packet count only when that frame matches (step 887).
fn deferred_nilrys_book_live_second_stab_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let book_multi = combat.monsters.iter().any(|monster| {
        monster.alive
            && monster.content_id == sts_core::content::monsters::BOOK_OF_STABBING_ID
            && matches!(
                monster.intent,
                sts_core::MonsterIntent::AttackMultiple { .. }
            )
    });
    if !book_multi {
        return None;
    }
    let mut candidate = source.clone();
    candidate
        .combat
        .as_mut()?
        .nilrys_book_second_stab_uses_live_count = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// Two leftover EndTurn actions usually duplicate MonsterQueueItem
/// (FIDL01727). SuperFastMode can still publish a single takeTurn + rollMove
/// (FIDL01486 Byrd Grow / Chosen Hex, no extra player HP loss).
fn deferred_nilrys_single_monster_queue_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let mut candidate = source.clone();
    candidate.combat.as_mut()?.nilrys_duplicate_monster_queue = false;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can publish after the first leftover RollMoveAction
/// (FIDL01727 Collector Mega Debuff instead of the second Fireball roll).
fn deferred_nilrys_single_post_queue_roll_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let mut candidate = source.clone();
    candidate.combat.as_mut()?.nilrys_single_post_queue_roll = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can publish leftover takeTurns and the next draw before
/// leftover RollMoveActions (FIDL01727 CHOOSE 1059: Collector stays Fireball).
fn deferred_nilrys_skip_post_queue_rolls_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let mut candidate = source.clone();
    candidate.combat.as_mut()?.nilrys_skip_post_queue_rolls = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can snapshot after duplicate takeTurns but before
/// StrengthSelf RollMoveActions apply (FIDL01486 Byrd remains Caw while Chosen
/// consumes both leftover rolls into Drain). The leftover Byrd rolls still
/// burn `monster_rng` so later Caw rolls stay on-stream.
fn deferred_nilrys_hold_strength_self_rolls_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let holds_buff = combat.monsters.iter().any(|monster| {
        monster.alive
            && matches!(
                monster.intent,
                sts_core::MonsterIntent::StrengthSelf { amount } if amount != 0
            )
    });
    if !holds_buff {
        return None;
    }
    let mut candidate = source.clone();
    candidate.combat.as_mut()?.nilrys_hold_strength_self_rolls = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can snapshot after StrengthSelf's first leftover roll and
/// before other monsters' RollMoveActions (FIDL01486 SKIP 468: Byrd Peck,
/// Chosen still Drain).
fn deferred_nilrys_one_strength_self_roll_hold_others_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let holds_buff = combat.monsters.iter().any(|monster| {
        monster.alive
            && matches!(
                monster.intent,
                sts_core::MonsterIntent::StrengthSelf { amount } if amount != 0
            )
    });
    if !holds_buff {
        return None;
    }
    let mut candidate = source.clone();
    candidate
        .combat
        .as_mut()?
        .nilrys_one_strength_self_roll_hold_others = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// Leftover RollMoveActions can interleave actors (Byrd, Chosen, Byrd, Chosen)
/// instead of both rolls per actor (FIDL01486 CHOOSE 478).
fn deferred_nilrys_interleave_post_queue_rolls_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let mut candidate = source.clone();
    candidate
        .combat
        .as_mut()?
        .nilrys_interleave_post_queue_rolls = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can keep Peck displayed while Chosen consumes the leftover
/// `monster_rng` draws (FIDL01486 SKIP 491: Drain, not Swoop / Debilitate).
fn deferred_nilrys_hold_attack_multiple_rolls_on_second_choice_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    match decision {
        RunDecisionAction::Run(
            RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward,
        ) => {}
        _ => return None,
    }
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3 || !combat.nilrys_duplicate_monster_queue {
        return None;
    }
    let holds_multi = combat.monsters.iter().any(|monster| {
        monster.alive
            && matches!(
                monster.intent,
                sts_core::MonsterIntent::AttackMultiple { .. }
            )
    });
    if !holds_multi {
        return None;
    }
    let mut candidate = source.clone();
    candidate.combat.as_mut()?.nilrys_hold_attack_multiple_rolls = true;
    let next = apply_run_decision_action(&candidate, decision).ok()?;
    next.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&next),
    )
    .is_empty()
    .then_some(next)
}

/// SuperFastMode can resolve a legal PLAY and the leftover EndTurn in one
/// frame (FIDL01486 PLAY 515: Strike hits Book for 9, then a 2-hit stab
/// through the parked plated block). Skipping the PLAY leaves Book HP and
/// the discard pile short.
fn deferred_nilrys_play_then_leftover_end_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. })
    ) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if !combat.resume_end_turn_after_nilrys_codex
        || combat.decision.is_some()
        || (combat.nilrys_codex_end_turn_stage != 1 && combat.nilrys_codex_end_turn_stage != 2)
    {
        return None;
    }
    let played = apply_run_decision_action(source, decision).ok()?;
    if !played.pending_external_rng.is_empty() {
        return None;
    }
    let try_finish = |dup: bool, pending_powers: bool, live_book: bool| -> Option<RunState> {
        let mut candidate = played.clone();
        let combat = candidate.combat.as_mut()?;
        sts_core::combat::transition::settle_leftover_end_turn_hand_discard(combat).ok()?;
        combat.decision = None;
        combat.nilrys_codex_end_turn_stage = 3;
        combat.resume_end_turn_after_nilrys_codex = true;
        combat.nilrys_duplicate_monster_queue = dup;
        combat.nilrys_end_powers_pending = pending_powers;
        combat.nilrys_book_second_stab_uses_live_count = live_book;
        let finished = sts_core::combat::end_player_turn(combat).ok()?;
        candidate.player_hp = finished.player.hp;
        candidate.player_max_hp = finished.player.max_hp;
        candidate.card_random_rng_counter = finished.rng.card_random_rng.counter();
        candidate.combat = Some(finished);
        candidate.validate().ok()?;
        subset_diffs(
            seed_start_combat_observed_subset(&post.message),
            seed_start_simulated_combat_subset(&candidate),
        )
        .is_empty()
        .then_some(candidate)
    };
    try_finish(false, false, false)
        .or_else(|| try_finish(true, false, false))
        .or_else(|| try_finish(false, true, false))
        .or_else(|| try_finish(true, true, false))
        .or_else(|| try_finish(true, false, true))
        .or_else(|| try_finish(true, true, true))
}

/// After a first-offer CHOOSE parks at stage 2, leftover EndTurn can skip the
/// second Codex, finish the monster turn, draw, and immediately open the next
/// turn's first Codex (FIDL01486 END 601: Book's captured 14-hit through 8
/// block, then Offering / Pummel / Clothesline with the new hand held).
fn deferred_nilrys_leftover_end_skip_second_offer_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if !combat.resume_end_turn_after_nilrys_codex
        || combat.decision.is_some()
        || combat.piles.hand.is_empty()
        || (combat.nilrys_codex_end_turn_stage != 1 && combat.nilrys_codex_end_turn_stage != 2)
    {
        return None;
    }
    let try_finish =
        |dup: bool, pending_powers: bool, live_book: bool, open_next: bool| -> Option<RunState> {
            let mut candidate = source.clone();
            let combat = candidate.combat.as_mut()?;
            sts_core::combat::transition::settle_leftover_end_turn_hand_discard(combat).ok()?;
            combat.decision = None;
            combat.nilrys_codex_end_turn_stage = 3;
            combat.resume_end_turn_after_nilrys_codex = true;
            combat.nilrys_duplicate_monster_queue = dup;
            combat.nilrys_end_powers_pending = pending_powers;
            combat.nilrys_book_second_stab_uses_live_count = live_book;
            let finished = sts_core::combat::end_player_turn(combat).ok()?;
            candidate.player_hp = finished.player.hp;
            candidate.player_max_hp = finished.player.max_hp;
            candidate.card_random_rng_counter = finished.rng.card_random_rng.counter();
            candidate.combat = Some(finished);
            if open_next
                && candidate
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.monsters.iter().any(|monster| monster.alive))
            {
                candidate = apply_run_decision_action(
                    &candidate,
                    RunDecisionAction::Combat(CombatAction::EndTurn),
                )
                .ok()?;
            }
            candidate.validate().ok()?;
            subset_diffs(
                seed_start_combat_observed_subset(&post.message),
                seed_start_simulated_combat_subset(&candidate),
            )
            .is_empty()
            .then_some(candidate)
        };
    try_finish(false, false, false, true)
        .or_else(|| try_finish(true, false, false, true))
        .or_else(|| try_finish(false, true, false, true))
        .or_else(|| try_finish(true, true, false, true))
        .or_else(|| try_finish(true, false, true, true))
        .or_else(|| try_finish(true, true, true, true))
        .or_else(|| try_finish(false, false, false, false))
        .or_else(|| try_finish(true, false, false, false))
        .or_else(|| try_finish(false, true, false, false))
        .or_else(|| try_finish(true, true, false, false))
}

/// Two-step second CHOOSE can close the offer without inserting when the first
/// pick already entered the draw pile (FIDL01486 CHOOSE 610: Flame Barrier is
/// the only new card; Disarm is not shuffled in). Default apply inserts then
/// resumes the monster turn.
fn deferred_nilrys_second_choice_without_insert_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    let index = match decision {
        RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index }) => Some(index),
        RunDecisionAction::Run(RunAction::SkipCombatCardReward) => None,
        _ => return None,
    };
    let combat = source.combat.as_ref()?;
    if combat.nilrys_codex_end_turn_stage != 3
        || !matches!(
            combat.decision.as_ref(),
            Some(CombatDecisionState::NilrysCodexCardReward { .. })
        )
    {
        return None;
    }
    let try_close = |set: fn(&mut sts_core::combat::CombatState)| -> Option<RunState> {
        let mut candidate = source.clone();
        let combat = candidate.combat.as_mut()?;
        match index {
            Some(index) => {
                sts_core::relic::nilrys_codex_park_choice_without_insert(combat, index).ok()?;
            }
            None => {
                combat.decision = None;
            }
        }
        set(combat);
        let finished = sts_core::combat::end_player_turn(combat).ok()?;
        candidate.player_hp = finished.player.hp;
        candidate.player_max_hp = finished.player.max_hp;
        candidate.card_random_rng_counter = finished.rng.card_random_rng.counter();
        candidate.combat = Some(finished);
        candidate.validate().ok()?;
        subset_diffs(
            seed_start_combat_observed_subset(&post.message),
            seed_start_simulated_combat_subset(&candidate),
        )
        .is_empty()
        .then_some(candidate)
    };
    try_close(|_| {})
        .or_else(|| try_close(|c| c.nilrys_duplicate_monster_queue = false))
        .or_else(|| try_close(|c| c.nilrys_end_powers_pending = false))
        .or_else(|| {
            try_close(|c| {
                c.nilrys_duplicate_monster_queue = false;
                c.nilrys_end_powers_pending = false;
            })
        })
        .or_else(|| try_close(|c| c.nilrys_book_second_stab_uses_live_count = true))
        .or_else(|| try_close(|c| c.nilrys_hold_attack_multiple_rolls = true))
        .or_else(|| try_close(|c| c.nilrys_single_post_queue_roll = true))
        .or_else(|| try_close(|c| c.nilrys_skip_post_queue_rolls = true))
        .or_else(|| try_close(|c| c.nilrys_interleave_post_queue_rolls = true))
}

/// First-offer SKIP leaves EndTurn queued. SuperFastMode can discard that
/// hand (swallowing the next PLAY) and publish the next turn in one frame
/// (FIDL01772 step 614). Ordinary apply would spend the leftover hand.
/// After a first-offer CHOOSE that parked at stage 2, the same leftover
/// EndTurn can swallow PLAY and finish the monster turn (FIDL01486 PLAY 515).
fn deferred_nilrys_leftover_end_instead_of_play_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. })
    ) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if !combat.resume_end_turn_after_nilrys_codex || combat.decision.is_some() {
        return None;
    }
    if combat.nilrys_codex_end_turn_stage != 1 && combat.nilrys_codex_end_turn_stage != 2 {
        return None;
    }
    let mut candidate = source.clone();
    let from_stage_two = {
        let combat = candidate.combat.as_mut()?;
        combat.nilrys_codex_end_turn_stage == 2
    };
    let try_finish = |source: &RunState| -> Option<RunState> {
        let mut candidate = source.clone();
        let combat = candidate.combat.as_mut()?;
        let finished = sts_core::combat::end_player_turn(combat).ok()?;
        candidate.player_hp = finished.player.hp;
        candidate.player_max_hp = finished.player.max_hp;
        candidate.card_random_rng_counter = finished.rng.card_random_rng.counter();
        candidate.combat = Some(finished);
        candidate.validate().ok()?;
        subset_diffs(
            seed_start_combat_observed_subset(&post.message),
            seed_start_simulated_combat_subset(&candidate),
        )
        .is_empty()
        .then_some(candidate)
    };
    if !from_stage_two {
        return try_finish(&candidate);
    }
    let discard_then_monsters = |set: fn(&mut sts_core::combat::CombatState)| -> Option<RunState> {
        let mut flagged = candidate.clone();
        {
            let combat = flagged.combat.as_mut()?;
            sts_core::combat::transition::settle_leftover_end_turn_hand_discard(combat).ok()?;
            combat.decision = None;
            combat.nilrys_codex_end_turn_stage = 3;
            combat.resume_end_turn_after_nilrys_codex = true;
            combat.nilrys_duplicate_monster_queue = true;
            combat.nilrys_end_powers_pending = true;
            set(combat);
        }
        try_finish(&flagged)
    };
    discard_then_monsters(|_| {})
        .or_else(|| discard_then_monsters(|c| c.nilrys_interleave_post_queue_rolls = true))
        .or_else(|| discard_then_monsters(|c| c.nilrys_hold_attack_multiple_rolls = true))
        .or_else(|| discard_then_monsters(|c| c.nilrys_single_post_queue_roll = true))
        .or_else(|| discard_then_monsters(|c| c.nilrys_skip_post_queue_rolls = true))
        .or_else(|| discard_then_monsters(|c| c.nilrys_book_second_stab_uses_live_count = true))
        .or_else(|| discard_then_monsters(|c| c.nilrys_duplicate_monster_queue = false))
        .or_else(|| {
            let combat = candidate.combat.as_mut()?;
            combat.nilrys_codex_end_turn_stage = 3;
            combat.nilrys_duplicate_monster_queue = true;
            combat.nilrys_end_powers_pending = true;
            try_finish(&candidate)
        })
}

/// After leftover EndTurn already finished the previous turn, SuperFastMode can
/// publish the next EndTurn's first Codex on a PLAY (FIDL01486 PLAY 621:
/// Combust stays in hand, Flex / Shockwave / Evolve). Ordinary apply spends
/// the card.
fn deferred_nilrys_play_opens_next_first_codex_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. })
    ) {
        return None;
    }
    let combat = source.combat.as_ref()?;
    if combat.decision.is_some()
        || combat.resume_end_turn_after_nilrys_codex
        || combat.piles.hand.is_empty()
        || !combat.relics.contains(&sts_core::relic::Relic::NilrysCodex)
    {
        return None;
    }
    let candidate =
        apply_run_decision_action(source, RunDecisionAction::Combat(CombatAction::EndTurn)).ok()?;
    candidate.validate().ok()?;
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&candidate),
    )
    .is_empty()
    .then_some(candidate)
}

/// Writhing Mass queues `AddCardToDeckAction(Parasite)` after Mega Debuff.
/// SuperFastMode can publish that obtain on a PLAY that Java rejects, so the
/// captured frame shows the new deck/gold and an unchanged combat hand
/// (FIDL01726 PLAY 1230, FIDL01782 PLAY 1090, FIDL01572 PLAY 1091).
/// After Parasite publishes on a rejected PLAY, Java still has that play on
/// the action queue. The next captured command (END or another PLAY) can
/// resolve the leftover play instead of the recorded command (FIDL01782 END
/// 1091 Pommel Strike; FIDL01572 PLAY 1092 Defend).
fn deferred_leftover_rejected_play_candidate(
    source: &RunState,
    leftover: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        leftover,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. } | CombatAction::EndTurn)
    ) {
        return None;
    }
    if source.phase != RunPhase::Combat
        || source
            .combat
            .as_ref()
            .is_some_and(|combat| combat.decision.is_some())
    {
        return None;
    }
    let observed = seed_start_combat_observed_subset(&post.message);
    let candidate = apply_run_decision_action(source, leftover).ok()?;
    if candidate.pending_external_rng.is_empty()
        && subset_diffs(
            observed.clone(),
            seed_start_simulated_combat_subset(&candidate),
        )
        .is_empty()
    {
        return Some(candidate);
    }
    if leftover != RunDecisionAction::Combat(CombatAction::EndTurn) {
        return None;
    }
    // SuperFastMode can publish after leftover EndTurn discards the hand and
    // before the monster turn (FIDL01782 STATE 1093).
    let mut discarded = source.clone();
    let combat = discarded.combat.as_mut()?;
    sts_core::combat::settle_leftover_end_turn_hand_discard(combat).ok()?;
    discarded.player_hp = combat.player.hp;
    discarded.player_max_hp = combat.player.max_hp;
    discarded.validate().ok()?;
    if subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(&discarded),
    )
    .is_empty()
    {
        return Some(discarded);
    }
    // Duplicate obtain END can publish the live hand while the first
    // MonsterQueueItem is already settling. The following END continues after
    // that first action (FIDL01595 END 1120).
    let mut advanced = source.clone();
    let combat = advanced.combat.as_mut()?;
    sts_core::combat::turn::run_first_monster_action_without_cleanup(combat).ok()?;
    advanced.player_hp = combat.player.hp;
    advanced.player_max_hp = combat.player.max_hp;
    let continued = apply_run_decision_action(&advanced, leftover).ok()?;
    if continued.pending_external_rng.is_empty()
        && continued.validate().is_ok()
        && subset_diffs(observed, seed_start_simulated_combat_subset(&continued)).is_empty()
    {
        return Some(continued);
    }
    None
}

/// SuperFastMode can execute the mirrored 1-based hand index (FIDL01727
/// PLAY 1191: Ghostly Armor is PLAY 5 in display order, PLAY 1 in the command).
fn deferred_reversed_play_index_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    default_next: &RunState,
    post: &TraceState,
) -> Option<RunState> {
    let (card_id, target) = match decision {
        RunDecisionAction::Combat(CombatAction::PlayCard { card_id, target }) => (card_id, target),
        _ => return None,
    };
    let observed = seed_start_combat_observed_subset(&post.message);
    if subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(default_next),
    )
    .is_empty()
    {
        return None;
    }
    let combat = source.combat.as_ref()?;
    let index = combat
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)?;
    let mirrored = combat.piles.hand.len().checked_sub(index + 1)?;
    let mirrored_id = combat.piles.hand.get(mirrored)?.id;
    if mirrored_id == card_id {
        return None;
    }
    let applied = apply_run_decision_action(
        source,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: mirrored_id,
            target,
        }),
    )
    .ok()?;
    if !applied.pending_external_rng.is_empty() || applied.validate().is_err() {
        return None;
    }
    subset_diffs(observed, seed_start_simulated_combat_subset(&applied))
        .is_empty()
        .then_some(applied)
}

/// When the bound PLAY index and its mirror both miss, SuperFastMode may have
/// executed the unique legal play that matches the observed combat subset
/// (FIDL01727 PLAY 1192: Offering, not Strike).
fn deferred_alternate_play_card_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    default_next: &RunState,
    post: &TraceState,
) -> Option<RunState> {
    let bound = match decision {
        RunDecisionAction::Combat(action @ CombatAction::PlayCard { .. }) => action,
        _ => return None,
    };
    let observed = seed_start_combat_observed_subset(&post.message);
    if subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(default_next),
    )
    .is_empty()
    {
        return None;
    }
    let combat = source.combat.as_ref()?;
    let mut matched = None;
    for action in sts_core::combat::legal_combat_actions(combat).ok()? {
        if !matches!(action, CombatAction::PlayCard { .. }) || action == bound {
            continue;
        }
        let applied = apply_run_decision_action(source, RunDecisionAction::Combat(action)).ok()?;
        if !applied.pending_external_rng.is_empty() || applied.validate().is_err() {
            continue;
        }
        if subset_diffs(
            observed.clone(),
            seed_start_simulated_combat_subset(&applied),
        )
        .is_empty()
        {
            if matched.is_some() {
                return None;
            }
            matched = Some(applied);
        }
    }
    matched
}

/// Bind the unused command after a leftover play using the post-leftover hand.
/// Pre-leftover PLAY 2 on [Dropkick, Heavy Blade+, True Grit] is Heavy Blade+;
/// after leftover Dropkick it is True Grit (FIDL01617).
fn rebind_leftover_pending_command(
    candidate: &RunState,
    command: &str,
) -> Option<RunDecisionAction> {
    match direct_decision(candidate, command) {
        Ok((
            rebound @ RunDecisionAction::Combat(
                CombatAction::PlayCard { .. } | CombatAction::EndTurn,
            ),
            _,
        )) => Some(rebound),
        _ => None,
    }
}

fn leftover_end_state_is_eligible(source: &RunState, leftover: Option<&RunDecisionAction>) -> bool {
    if leftover == Some(&RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return true;
    }
    // Time Warp's leftover EndTurnAction can reject a PLAY and still publish
    // Combust / discard / loseBlock / takeTurn on later STATE polls
    // (FIDL01645 / FIDL01666 / FIDL01691).
    source.combat.as_ref().is_some_and(|combat| {
        combat.time_warp_end_turn
            || combat.time_warp_end_turn_pre_discard_settled
            || combat.time_warp_end_powers_applied
            || combat.leftover_end_turn_draw_remaining > 0
    })
}

fn leftover_end_completed_next_player_turn(source: &RunState, candidate: &RunState) -> bool {
    let source_turns = source
        .combat
        .as_ref()
        .map(|combat| combat.relic_counters.player_turns_started)
        .unwrap_or(0);
    let candidate_turns = candidate
        .combat
        .as_ref()
        .map(|combat| combat.relic_counters.player_turns_started)
        .unwrap_or(0);
    candidate_turns > source_turns
}

fn leftover_end_state_publication_candidate(
    source: &RunState,
    leftover: Option<RunDecisionAction>,
    post: &TraceState,
) -> Option<RunState> {
    if !leftover_end_state_is_eligible(source, leftover.as_ref()) {
        return None;
    }
    if source.phase != RunPhase::Combat {
        return None;
    }
    let observed = seed_start_combat_observed_subset(&post.message);
    if source
        .combat
        .as_ref()
        .is_some_and(|combat| combat.leftover_end_turn_draw_remaining > 0)
    {
        return leftover_end_state_continue_draw(source, observed);
    }
    // CombustPower.atEndOfTurn runs before DiscardAtEndOfTurnAction. A rejected
    // leftover PLAY can drain discard first (FIDL01358 empty-hand STATE) while
    // Combust is still queued (FIDL01666 HP/enemy damage with an already-empty
    // hand). Try the power pulse whenever end-of-turn powers have not run.
    if source
        .combat
        .as_ref()
        .is_some_and(|combat| !combat.time_warp_end_powers_applied)
    {
        let mut discarded = source.clone();
        if let Some(combat) = discarded.combat.as_mut() {
            if sts_core::combat::settle_leftover_end_turn_player_powers_and_discard(combat).is_ok()
            {
                discarded.player_hp = combat.player.hp;
                discarded.player_max_hp = combat.player.max_hp;
                if discarded.validate().is_ok()
                    && subset_diffs(
                        observed.clone(),
                        seed_start_simulated_combat_subset(&discarded),
                    )
                    .is_empty()
                {
                    return Some(discarded);
                }
            }
        }
    }
    let mut lose_block = source.clone();
    if let Some(combat) = lose_block.combat.as_mut() {
        sts_core::combat::settle_leftover_end_turn_monster_lose_block(combat);
        lose_block.player_hp = combat.player.hp;
        if subset_diffs(
            observed.clone(),
            seed_start_simulated_combat_subset(&lose_block),
        )
        .is_empty()
        {
            return Some(lose_block);
        }
    }
    let mut finished = source.clone();
    let combat = finished.combat.as_mut()?;
    let skip_post_queue_rolls = combat.nilrys_skip_post_queue_rolls;
    sts_core::combat::settle_leftover_end_turn_monster_and_draw(combat).ok()?;
    finished.player_hp = combat.player.hp;
    finished.player_max_hp = combat.player.max_hp;
    finished.validate().ok()?;
    if subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(&finished),
    )
    .is_empty()
    {
        return Some(finished);
    }
    if !skip_post_queue_rolls {
        let mut skipped = source.clone();
        if let Some(combat) = skipped.combat.as_mut() {
            combat.nilrys_skip_post_queue_rolls = true;
            if sts_core::combat::settle_leftover_end_turn_monster_and_draw(combat).is_ok() {
                skipped.player_hp = combat.player.hp;
                skipped.player_max_hp = combat.player.max_hp;
                if skipped.validate().is_ok()
                    && subset_diffs(
                        observed.clone(),
                        seed_start_simulated_combat_subset(&skipped),
                    )
                    .is_empty()
                {
                    return Some(skipped);
                }
            }
        }
    }
    leftover_end_state_mid_draw_from_finished(finished, observed.clone())
        .or_else(|| leftover_end_state_monster_and_draw_skipping_post_draw(source, observed))
}

fn leftover_end_state_monster_and_draw_skipping_post_draw(
    source: &RunState,
    observed: Value,
) -> Option<RunState> {
    let mut finished = source.clone();
    let combat = finished.combat.as_mut()?;
    sts_core::combat::settle_leftover_end_turn_monster_and_draw_skipping_post_draw_relics(combat)
        .ok()?;
    finished.player_hp = combat.player.hp;
    finished.player_max_hp = combat.player.max_hp;
    finished.validate().ok()?;
    if subset_diffs(
        observed.clone(),
        seed_start_simulated_combat_subset(&finished),
    )
    .is_empty()
    {
        return Some(finished);
    }
    leftover_end_state_mid_draw_from_finished(finished, observed)
}

fn leftover_end_state_continue_draw(source: &RunState, observed: Value) -> Option<RunState> {
    let remaining = source.combat.as_ref()?.leftover_end_turn_draw_remaining as usize;
    for extra in 1..=remaining {
        let mut next = source.clone();
        let draw_finished = {
            let combat = next.combat.as_mut()?;
            sts_core::combat::draw::draw_cards_with_combat_rng(combat, extra).ok()?;
            combat.leftover_end_turn_draw_remaining = remaining.saturating_sub(extra) as u8;
            next.player_hp = combat.player.hp;
            next.player_max_hp = combat.player.max_hp;
            combat.leftover_end_turn_draw_remaining == 0
        };
        if next.validate().is_err() {
            continue;
        }
        if subset_diffs(observed.clone(), seed_start_simulated_combat_subset(&next)).is_empty() {
            return Some(next);
        }
        if draw_finished {
            // Mid-draw leftover STATE peels before atTurnStartPostDraw. The
            // completing poll draws the last card then Warped Tongs (FIDL01807
            // STATE 853 Ghostly Armor+).
            {
                let combat = next.combat.as_mut()?;
                sts_core::relic::apply_start_of_player_turn_post_draw_relics(combat).ok()?;
                next.player_hp = combat.player.hp;
                next.player_max_hp = combat.player.max_hp;
            }
            if next.validate().is_err() {
                continue;
            }
            if subset_diffs(observed.clone(), seed_start_simulated_combat_subset(&next)).is_empty()
            {
                return Some(next);
            }
        }
    }
    None
}

fn leftover_end_state_mid_draw_from_finished(
    finished: RunState,
    observed: Value,
) -> Option<RunState> {
    let drawn = finished.combat.as_ref()?.piles.hand.len();
    for peel in 1..=drawn {
        let mut mid = finished.clone();
        let combat = mid.combat.as_mut()?;
        for _ in 0..peel {
            let card = combat.piles.hand.pop()?;
            combat.piles.draw_pile.push(card);
        }
        combat.leftover_end_turn_draw_remaining = peel as u8;
        mid.player_hp = combat.player.hp;
        mid.player_max_hp = combat.player.max_hp;
        if mid.validate().is_err() {
            continue;
        }
        if subset_diffs(observed.clone(), seed_start_simulated_combat_subset(&mid)).is_empty() {
            return Some(mid);
        }
    }
    None
}

fn leftover_end_finished_player_turn(source: &RunState, candidate: &RunState) -> bool {
    let source_empty = source
        .combat
        .as_ref()
        .is_some_and(|combat| combat.piles.hand.is_empty());
    let candidate_ready = candidate
        .combat
        .as_ref()
        .is_some_and(|combat| !combat.piles.hand.is_empty() && combat.decision.is_none());
    source_empty && candidate_ready
}

fn deferred_combat_obtain_instead_of_play_candidate(
    source: &RunState,
    decision: RunDecisionAction,
    post: &TraceState,
) -> Option<RunState> {
    if !matches!(
        decision,
        RunDecisionAction::Combat(CombatAction::PlayCard { .. })
    ) {
        return None;
    }
    if source.phase != RunPhase::Combat || source.pending_combat_obtain_cards.is_empty() {
        return None;
    }
    if source
        .combat
        .as_ref()
        .is_some_and(|combat| combat.decision.is_some())
    {
        return None;
    }
    let source_combat = source.combat.as_ref()?;
    let mut published = source.clone();
    published.flush_pending_combat_obtain_cards().ok()?;
    if published.combat.as_ref()? != source_combat {
        return None;
    }
    subset_diffs(
        seed_start_combat_observed_subset(&post.message),
        seed_start_simulated_combat_subset(&published),
    )
    .is_empty()
    .then_some(published)
}

fn skipped_shop_remove_grid_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::GridSelect { index } = decision else {
        return Ok(None);
    };
    let Some(grid) = run.card_grid.as_ref() else {
        return Ok(None);
    };
    let mut candidate = sts_core::run::apply_run_decision_action(run, decision)
        .map_err(|error| error.to_string())?;
    match grid.purpose {
        sts_core::run::grid::GridPurpose::DollysMirror => {
            if candidate.deck.len() != run.deck.len() + 1 {
                return Ok(None);
            }
            candidate.deck.pop();
        }
        sts_core::run::grid::GridPurpose::ShopRemove => {
            let selected = grid
                .cards
                .get(index)
                .copied()
                .ok_or_else(|| "shop-remove grid selection is out of range".to_owned())?;
            let source_position = run
                .deck
                .iter()
                .position(|card| card.id == selected.id)
                .ok_or_else(|| "shop-remove grid card is not source-owned".to_owned())?;
            if candidate.deck.iter().any(|card| card.id == selected.id) {
                return Ok(None);
            }
            candidate.deck.insert(source_position, selected);
        }
        _ => return Ok(None),
    }
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_headbutt_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseDiscardSelect { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(discard_select) = combat.discard_select() else {
        return Ok(None);
    };
    if discard_select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw
        || (discard_select.source_card.is_none() && !discard_select.source_card_force_exhaust)
    {
        return Ok(None);
    }
    let selected = combat
        .piles
        .discard_pile
        .get(index)
        .cloned()
        .ok_or_else(|| "Headbutt skipped candidate selected index is out of range".to_owned())?;

    let mut candidate = sts_core::run::apply_discard_select_choice_skipped_retrieval(run, index)
        .map_err(|error| error.to_string())?;
    candidate.pending_headbutt_alias = Some(selected);
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn deferred_time_warp_skipped_headbutt_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseDiscardSelect { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    let Some(discard_select) = combat.discard_select() else {
        return Ok(None);
    };
    if discard_select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw
        || (discard_select.source_card.is_none() && !discard_select.source_card_force_exhaust)
    {
        return Ok(None);
    }
    if !combat.monsters.iter().any(|monster| {
        monster.alive && monster.content_id == sts_core::content::monsters::TIME_EATER_ID
    }) {
        return Ok(None);
    }
    let selected = combat
        .piles
        .discard_pile
        .get(index)
        .cloned()
        .ok_or_else(|| {
            "Time Warp skipped Headbutt candidate selected index is out of range".to_owned()
        })?;

    let mut candidate =
        sts_core::run::apply_discard_select_choice_skipped_retrieval_without_time_warp_end(
            run, index,
        )
        .map_err(|error| error.to_string())?;
    candidate.pending_headbutt_alias = Some(selected);
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_draw_select_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseDrawSelect { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    if combat.draw_select().is_none() {
        return Ok(None);
    }

    let candidate = sts_core::run::apply_draw_select_choice_skipped_retrieval(run, index)
        .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_discovery_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    if !matches!(
        combat.decision.as_ref(),
        Some(CombatDecisionState::DiscoveryCardReward { .. })
    ) {
        return Ok(None);
    }

    let candidate =
        sts_core::run::apply_combat_card_reward_choice_skipped_discovery_retrieval(run, index)
            .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn skipped_toolbox_candidate(
    run: &RunState,
    decision: RunDecisionAction,
) -> Result<Option<RunState>, String> {
    let RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index }) = decision else {
        return Ok(None);
    };
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    if !matches!(
        combat.decision.as_ref(),
        Some(CombatDecisionState::ToolboxCardReward { .. })
    ) {
        return Ok(None);
    }

    let candidate =
        sts_core::run::apply_combat_card_reward_choice_skipped_toolbox_retrieval(run, index)
            .map_err(|error| error.to_string())?;
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

fn stable_quiescent_combat_post(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    message.get("ready_for_command").and_then(Value::as_bool) == Some(true)
        && message.get("boundary_kind").and_then(Value::as_str) == Some("quiescent")
        && message.get("actions_queued").and_then(Value::as_u64) == Some(0)
        && game.get("screen_type").and_then(Value::as_str) == Some("NONE")
        && game.get("action_phase").and_then(Value::as_str) == Some("WAITING_ON_USER")
}

fn park_cross_combat_residual_and_end_turn(
    run: &RunState,
    decision: RunDecisionAction,
    pending_card: Option<CardInstance>,
) -> Result<Option<RunState>, String> {
    park_cross_combat_residuals_and_end_turn(run, decision, pending_card.as_slice())
}

fn park_cross_combat_residuals_and_end_turn(
    run: &RunState,
    decision: RunDecisionAction,
    pending_cards: &[CardInstance],
) -> Result<Option<RunState>, String> {
    if !matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return Ok(None);
    }
    if pending_cards.is_empty() || run.combat.is_none() {
        return Ok(None);
    }
    let mut parked = run.clone();
    let combat = parked.combat.as_mut().expect("validated combat");
    // The target can publish leftover selectedCards beside the next combat's
    // live copies with the same observed UUIDs. Remint so validation stays
    // unique, then park in pending_hidden so end-turn settlement places them
    // after the discarded hand and before monster-turn status cards.
    for card in pending_cards {
        let mut residual = *card;
        residual.id = CardId::new(
            combat
                .next_card_instance_id()
                .map_err(|error| error.to_string())?,
        );
        combat
            .pending_hidden_hand_card_until_end_turn
            .push(residual);
    }
    let candidate =
        apply_run_decision_action(&parked, RunDecisionAction::Combat(CombatAction::EndTurn))
            .map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
}

fn cross_combat_burning_pact_candidate(
    run: &RunState,
    decision: RunDecisionAction,
    pending_card: Option<CardInstance>,
) -> Result<Option<RunState>, String> {
    park_cross_combat_residual_and_end_turn(run, decision, pending_card)
}

fn cross_combat_put_on_deck_candidate(
    run: &RunState,
    decision: RunDecisionAction,
    pending_card: Option<CardInstance>,
) -> Result<Option<RunState>, String> {
    park_cross_combat_residual_and_end_turn(run, decision, pending_card)
}

fn skipped_recycle_candidate(
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
    if exhaust_select.purpose != ExhaustSelectPurpose::RecycleExhaustOne
        || exhaust_select.source_card.is_none()
        || exhaust_select.source_card_force_exhaust
        || exhaust_select.interrupted_by_cultist_potion
        || exhaust_select.selected_hand_indices.len() != 1
    {
        return Ok(None);
    }

    let mut source = run.clone();
    clear_superseded_selection_screen_pending(&mut source);
    let (mut candidate, selected) =
        sts_core::run::apply_exhaust_select_confirm_skipped_recycle_retrieval(&source)
            .map_err(|error| error.to_string())?;
    candidate
        .combat
        .as_mut()
        .ok_or_else(|| "skipped Recycle candidate lost combat state".to_owned())?
        .pending_hidden_hand_card_until_end_turn
        .push(selected);
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(Some(candidate))
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

fn skipped_purity_selected_cards_are_absent_from_observed_exhaust(
    source: &RunState,
    post: &TraceState,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(exhaust_select) = source_combat.exhaust_select() else {
        return false;
    };
    if exhaust_select.purpose != ExhaustSelectPurpose::PurityExhaustUpTo3
        || exhaust_select.selected_hand_indices.is_empty()
    {
        return false;
    }
    let observed_exhaust = post
        .message
        .pointer("/game_state/combat_state/exhaust_pile");
    if !observed_exhaust.is_some_and(Value::is_array) {
        return false;
    }
    let observed_ids = combat_card_ids(observed_exhaust);
    exhaust_select.selected_hand_indices.iter().all(|index| {
        let Some(selected_card) = source_combat.piles.hand.get(*index) else {
            return false;
        };
        let selected_key = simulated_card_projection_key(selected_card);
        let source_count = source_combat
            .piles
            .exhaust_pile
            .iter()
            .filter(|card| simulated_card_projection_key(card) == selected_key)
            .count();
        let observed_count = observed_ids
            .iter()
            .filter(|card| *card == &selected_key)
            .count();
        observed_count <= source_count
    })
}

fn skipped_true_grit_selected_card_is_absent_from_observed_exhaust(
    source: &RunState,
    post: &TraceState,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(exhaust_select) = source_combat.exhaust_select() else {
        return false;
    };
    if exhaust_select.purpose != ExhaustSelectPurpose::TrueGritExhaustOne
        || exhaust_select.selected_hand_indices.len() != 1
    {
        return false;
    }
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

fn skipped_recycle_selected_card_is_absent_from_observed_exhaust(
    source: &RunState,
    post: &TraceState,
) -> bool {
    let Some(source_combat) = source.combat.as_ref() else {
        return false;
    };
    let Some(exhaust_select) = source_combat.exhaust_select() else {
        return false;
    };
    if exhaust_select.purpose != ExhaustSelectPurpose::RecycleExhaustOne
        || exhaust_select.selected_hand_indices.len() != 1
    {
        return false;
    }
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
    pending_cross_combat_put_on_deck_card: Option<CardInstance>,
    pending_cross_combat_burning_pact_card: Option<CardInstance>,
    pending_cross_combat_hidden_cards: Vec<CardInstance>,
    pending_shop_dolly_card: Option<CardInstance>,
    pending_cursed_key_reward_publication: bool,
    pending_rejected_combat_play: Option<RunDecisionAction>,
}

impl StreamingSeedStartReplay {
    pub(super) fn settle_time_warp_after_rejected_command(&mut self) -> Result<(), String> {
        let leftover_end = matches!(
            self.pending_rejected_combat_play,
            Some(RunDecisionAction::Combat(CombatAction::EndTurn))
        );
        let Some(run) = self.seed_sim.as_mut() else {
            return Ok(());
        };
        let Some(combat) = run.combat.as_mut() else {
            return Ok(());
        };
        if leftover_end {
            // Time Warp already queued discard. A Parasite duplicate END only
            // published AddCardToDeckAction; Combust is still pending and must
            // wait for the leftover STATE (FIDL01515 PLAY 916).
            let time_warp = combat.time_warp_end_turn
                || combat.time_warp_end_turn_pre_discard_settled
                || combat.time_warp_end_powers_applied;
            if time_warp {
                sts_core::combat::settle_leftover_end_turn_hand_discard(combat)
                    .map_err(|error| error.to_string())?;
                run.player_hp = combat.player.hp;
                run.player_max_hp = combat.player.max_hp;
            }
            return Ok(());
        }
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
            } else if let Some(candidate) = leftover_end_state_publication_candidate(
                current,
                state.pending_rejected_combat_play,
                post,
            ) {
                if leftover_end_finished_player_turn(current, &candidate) {
                    state.pending_rejected_combat_play = None;
                }
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "deferred leftover end-turn publication".to_owned(),
                });
                state.seed_sim = Some(candidate);
                None
            } else {
                // A forced Time Warp END can be published in two stable
                // observations: first after the hand is discarded, then after
                // the queued monster turn runs. Try the latter only against
                // the observed projection; never hydrate from the observation.
                if current
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.pending_end_turn_feel_no_pain_block > 0)
                {
                    let tick = current
                        .combat
                        .as_ref()
                        .map(|combat| {
                            combat
                                .player
                                .powers
                                .feel_no_pain
                                .max(0)
                                .min(combat.pending_end_turn_feel_no_pain_block)
                        })
                        .unwrap_or(0);
                    let grants = [
                        tick,
                        current
                            .combat
                            .as_ref()
                            .map(|c| c.pending_end_turn_feel_no_pain_block)
                            .unwrap_or(0),
                    ];
                    let mut accepted = None;
                    for grant in grants {
                        if grant <= 0 {
                            continue;
                        }
                        let mut with_fnp = current.clone();
                        if let Some(combat) = with_fnp.combat.as_mut() {
                            combat.player.block = combat.player.block.saturating_add(grant);
                            combat.pending_end_turn_feel_no_pain_block = combat
                                .pending_end_turn_feel_no_pain_block
                                .saturating_sub(grant);
                        }
                        let mut probe = SimRealReport {
                            total_actions: 0,
                            action_dispositions: Vec::new(),
                            action_integrity: None,
                            verified: Vec::new(),
                            unsupported: Vec::new(),
                            unexpected_diffs: Vec::new(),
                            seed_start: None,
                        };
                        let probe_result = compare_direct_run(
                            &mut probe,
                            action,
                            post,
                            "deferred leftover Feel No Pain block",
                            &with_fnp,
                        );
                        if probe_result.is_ok() && probe.unexpected_diffs.is_empty() {
                            accepted = Some(with_fnp);
                            break;
                        }
                    }
                    if let Some(with_fnp) = accepted {
                        state.seed_sim = Some(with_fnp);
                        compare_direct_run(
                            report,
                            action,
                            post,
                            "deferred leftover Feel No Pain block",
                            state.seed_sim.as_ref().expect("fnp state"),
                        )
                        .expect("validated leftover Feel No Pain projection");
                        return None;
                    }
                }
                if current
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.time_warp_end_turn_pre_discard_settled)
                {
                    let mut resumed = current.clone();
                    let resumed_result = resumed.combat.take().map(|mut combat| {
                        // The deferred publication has already exposed the
                        // first queued monster item; do not replay the stale
                        // duplicate Time Warp queue while resuming it.
                        combat.time_warp_duplicate_monster_queue = false;
                        combat.time_warp_end_turn = false;
                        sts_core::combat::end_player_turn(&combat)
                    });
                    if let Some(Ok(combat)) = resumed_result {
                        resumed.player_hp = combat.player.hp;
                        resumed.player_max_hp = combat.player.max_hp;
                        resumed.combat = Some(combat);
                        let mut probe = SimRealReport {
                            total_actions: 0,
                            action_dispositions: Vec::new(),
                            action_integrity: None,
                            verified: Vec::new(),
                            unsupported: Vec::new(),
                            unexpected_diffs: Vec::new(),
                            seed_start: None,
                        };
                        let probe_result = compare_direct_run(
                            &mut probe,
                            action,
                            post,
                            "deferred Time Warp monster turn",
                            &resumed,
                        );
                        if probe_result.is_ok() && probe.unexpected_diffs.is_empty() {
                            state.seed_sim = Some(resumed);
                            compare_direct_run(
                                report,
                                action,
                                post,
                                "deferred Time Warp monster turn",
                                state.seed_sim.as_ref().expect("resumed state"),
                            )
                            .expect("validated deferred Time Warp projection");
                            return None;
                        }
                    }
                }
                if current.combat.as_ref().is_some_and(|combat| {
                    combat.time_warp_end_turn || combat.time_warp_end_powers_applied
                }) {
                    let mut finished = current.clone();
                    let finished_result = finished
                        .combat
                        .take()
                        .map(|combat| sts_core::combat::end_player_turn(&combat));
                    if let Some(Ok(combat)) = finished_result {
                        finished.player_hp = combat.player.hp;
                        finished.player_max_hp = combat.player.max_hp;
                        finished.combat = Some(combat);
                        let mut probe = SimRealReport {
                            total_actions: 0,
                            action_dispositions: Vec::new(),
                            action_integrity: None,
                            verified: Vec::new(),
                            unsupported: Vec::new(),
                            unexpected_diffs: Vec::new(),
                            seed_start: None,
                        };
                        let probe_result = compare_direct_run(
                            &mut probe,
                            action,
                            post,
                            "deferred Time Warp end after Metallicize lag",
                            &finished,
                        );
                        if probe_result.is_ok() && probe.unexpected_diffs.is_empty() {
                            state.seed_sim = Some(finished);
                            compare_direct_run(
                                report,
                                action,
                                post,
                                "deferred Time Warp end after Metallicize lag",
                                state.seed_sim.as_ref().expect("finished tw state"),
                            )
                            .expect("validated Time Warp Metallicize lag completion");
                            return None;
                        }
                    }
                }
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
            if state.pending_cursed_key_reward_publication
                && choose_index(&action.command).is_some()
                && current.phase == RunPhase::Reward
                && current.card_grid.is_none()
                && subset_diffs(
                    seed_start_reward_observed_subset(&post.message),
                    seed_start_reward_simulated_subset(current),
                )
                .is_empty()
            {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "deferred Cursed Key reward publication".to_owned(),
                });
                // The publication still shows the stale ordinary `[gold,
                // relic]` list, while the next command is bound against the
                // settled chest owner. The target presents the relic first
                // during this publication window, then leaves gold for the
                // following CHOOSE.
                if let Some(run) = state.seed_sim.as_mut() {
                    if let Some(treasure) = run.treasure_room.as_mut() {
                        treasure.relic_before_gold = true;
                    }
                }
                state.pending_cursed_key_reward_publication = false;
                return None;
            }
            if let Some(candidate) =
                pending_reward_obtain_publication_candidate(current, post, &action.command)
            {
                report.verified.push(VerifiedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    label: "deferred reward card publication".to_owned(),
                });
                state.seed_sim = Some(candidate);
                return None;
            }
            if external_rng.is_empty() {
                if let Some(published) = duplicate_end_combat_obtain_publication_candidate(
                    current,
                    post,
                    &action.command,
                ) {
                    state.pending_rejected_combat_play =
                        Some(RunDecisionAction::Combat(CombatAction::EndTurn));
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "duplicate END deferred combat obtain publication".to_owned(),
                    });
                    state.seed_sim = Some(published);
                    return None;
                }
            }
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
                    let leftover_play = state.pending_rejected_combat_play.take();
                    if let Some(leftover) = leftover_play {
                        if let Some(candidate) =
                            deferred_leftover_rejected_play_candidate(&source, leftover, post)
                        {
                            let skip_end_rebind = leftover
                                == RunDecisionAction::Combat(CombatAction::EndTurn)
                                && command_head_eq(&action.command, "END")
                                && leftover_end_completed_next_player_turn(&source, &candidate);
                            if !skip_end_rebind {
                                if let Some(rebound) =
                                    rebind_leftover_pending_command(&candidate, &action.command)
                                {
                                    state.pending_rejected_combat_play = Some(rebound);
                                }
                            }
                            report.verified.push(VerifiedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: "deferred leftover rejected play".to_owned(),
                            });
                            state.seed_sim = Some(candidate);
                            return None;
                        }
                    }
                    if let Some(candidate) =
                        deferred_headbutt_alias_candidate(&source, decision, post)
                    {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "deferred Headbutt skipped-retrieval alias".to_owned(),
                        });
                        state.seed_sim = Some(candidate);
                        return None;
                    }
                    if let Some(candidate) =
                        deferred_combat_obtain_instead_of_play_candidate(&source, decision, post)
                    {
                        if matches!(
                            decision,
                            RunDecisionAction::Combat(CombatAction::PlayCard { .. })
                        ) {
                            state.pending_rejected_combat_play = Some(decision);
                        }
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "deferred combat obtain instead of rejected play".to_owned(),
                        });
                        state.seed_sim = Some(candidate);
                        return None;
                    }
                    if let Some(candidate) =
                        deferred_pending_combat_obtain_candidate(&source, decision, post)
                    {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "deferred combat obtain publication".to_owned(),
                        });
                        state.seed_sim = Some(candidate);
                        return None;
                    }
                    if external_rng.is_empty() {
                        if let Some(candidate) =
                            deferred_colosseum_opening_candidate(&source, decision, post)
                        {
                            report.verified.push(VerifiedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: "deferred Colosseum opening publication".to_owned(),
                            });
                            state.seed_sim = Some(candidate);
                            return None;
                        }
                    }
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
                        Ok(mut next) => {
                            // HandCardSelectScreen.prep clears the previous
                            // screen's selectedCards before a new screen opens.
                            // Mirror that lifecycle even when the new screen's
                            // normal transition is target-authoritative and no
                            // skipped candidate is accepted.
                            if opened_new_selection_screen(&source, &next) {
                                clear_superseded_selection_screen_pending(&mut next);
                                if should_drop_cross_combat_hand_select_residual(&source, &next) {
                                    state.pending_cross_combat_put_on_deck_card = None;
                                    state.pending_cross_combat_burning_pact_card = None;
                                    state.pending_cross_combat_hidden_cards.clear();
                                }
                            }
                            // PutOnDeckAction has a source-backed skipped-retrieval
                            // frame: rebuild from the pre-CONFIRM state, then use it
                            // only when the complete observed combat projection
                            // matches. This keeps an unrelated pile mismatch on the
                            // normal transition fail-closed.
                            let skipped_candidate = deferred_monster_death_gremlin_horn_candidate(
                                &source,
                                &next,
                                decision,
                                post,
                            )
                            .or_else(|| {
                                deferred_nilrys_keep_hand_extra_offer_on_end_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_second_offer_on_end_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_leftover_end_skip_second_offer_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_first_choice_candidate(&source, decision, post)
                            })
                            .or_else(|| {
                                deferred_nilrys_second_choice_without_insert_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_hold_strength_self_rolls_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_one_strength_self_roll_hold_others_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_interleave_post_queue_rolls_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_hold_attack_multiple_rolls_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_single_post_queue_roll_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_skip_post_queue_rolls_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_single_monster_queue_on_second_choice_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_book_live_second_stab_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_play_then_leftover_end_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_leftover_end_instead_of_play_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| {
                                deferred_time_warp_end_instead_of_play_candidate(
                                    &source, decision, post,
                                )
                                .filter(|candidate| candidate.pending_external_rng.is_empty())
                                .filter(|_| {
                                    !subset_diffs(
                                        seed_start_combat_observed_subset(&post.message),
                                        seed_start_simulated_combat_subset(&next),
                                    )
                                    .is_empty()
                                })
                            })
                            .or_else(|| {
                                deferred_reversed_play_index_candidate(
                                    &source, decision, &next, post,
                                )
                            })
                            .or_else(|| {
                                deferred_alternate_play_card_candidate(
                                    &source, decision, &next, post,
                                )
                            })
                            .or_else(|| {
                                deferred_nilrys_play_opens_next_first_codex_candidate(
                                    &source, decision, post,
                                )
                            })
                            .or_else(|| skipped_shop_remove_grid_candidate(&source, decision)
                                .ok()
                                .flatten()
                                .filter(|candidate| candidate.pending_external_rng.is_empty())
                                .filter(|candidate| {
                                    let observed = seed_start_shop_observed_subset(&post.message);
                                    let simulated = if candidate.shop_merchant_open {
                                        seed_start_shop_screen_simulated_subset(candidate)
                                    } else {
                                        seed_start_shop_room_simulated_subset(candidate)
                                    };
                                    subset_diffs(observed, simulated).is_empty()
                                }))
                                .or_else(|| {
                                    deferred_shop_pending_obtain_candidate(&source, &next, post)
                                })
                                .or_else(|| {
                                    cross_shop_dolly_candidate(
                                        &source,
                                        &next,
                                        state.pending_shop_dolly_card,
                                        post,
                                    )
                                })
                                .or_else(|| {
                                    cross_shop_dolly_before_action_candidate(
                                        &source,
                                        decision,
                                        state.pending_shop_dolly_card,
                                        post,
                                    )
                                })
                                .or_else(|| skipped_put_on_deck_candidate(&source, decision)
                                .ok()
                                .flatten()
                                .filter(|candidate| candidate.pending_external_rng.is_empty())
                                .filter(|candidate| {
                                    subset_diffs(
                                        seed_start_combat_observed_subset(&post.message),
                                        seed_start_simulated_combat_subset(candidate),
                                    )
                                    .is_empty()
                                }))
                                .or_else(|| {
                                    put_on_deck_return_to_hand_candidate(&source, decision, post)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                })
                                .or_else(|| {
                                    skipped_warcry_auto_place_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    deferred_time_warp_hand_select_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    deferred_time_warp_hand_select_metallicize_candidate(
                                        &source, decision,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    deferred_time_warp_skipped_put_on_deck_candidate(
                                        &source, decision,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    time_warp_status_lag_hand_select_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    time_warp_remaining_status_lag_hand_select_candidate(
                                        &source, decision,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    skipped_armaments_candidate(&source, decision)
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
                                    skipped_dual_wield_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                })
                                .or_else(|| {
                                    skipped_dual_wield_without_restore_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                })
                                .or_else(|| {
                                    deferred_time_warp_exhaust_select_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    deferred_time_warp_exhaust_select_metallicize_candidate(
                                        &source, decision,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    skipped_purity_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            skipped_purity_selected_cards_are_absent_from_observed_exhaust(
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
                                    skipped_true_grit_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            skipped_true_grit_selected_card_is_absent_from_observed_exhaust(
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
                                    skipped_recycle_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
                                        })
                                        .filter(|_| {
                                            skipped_recycle_selected_card_is_absent_from_observed_exhaust(
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
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|candidate| {
                                            subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(candidate),
                                            )
                                            .is_empty()
                                        })
                                })
                                .or_else(|| {
                                    skipped_headbutt_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    deferred_time_warp_skipped_headbutt_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    skipped_draw_select_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    skipped_discovery_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    skipped_toolbox_candidate(&source, decision)
                                        .ok()
                                        .flatten()
                                        .filter(|candidate| candidate.pending_external_rng.is_empty())
                                        .filter(|_| stable_quiescent_combat_post(&post.message))
                                        .filter(|_| {
                                            !subset_diffs(
                                                seed_start_combat_observed_subset(&post.message),
                                                seed_start_simulated_combat_subset(&next),
                                            )
                                            .is_empty()
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
                                    cross_combat_put_on_deck_candidate(
                                        &source,
                                        decision,
                                        state.pending_cross_combat_put_on_deck_card,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| stable_quiescent_combat_post(&post.message))
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    cross_combat_burning_pact_candidate(
                                        &source,
                                        decision,
                                        state.pending_cross_combat_burning_pact_card,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| stable_quiescent_combat_post(&post.message))
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
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
                                    park_cross_combat_residuals_and_end_turn(
                                        &source,
                                        decision,
                                        &state.pending_cross_combat_hidden_cards,
                                    )
                                    .ok()
                                    .flatten()
                                    .filter(|candidate| candidate.pending_external_rng.is_empty())
                                    .filter(|_| stable_quiescent_combat_post(&post.message))
                                    .filter(|_| {
                                        !subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(&next),
                                        )
                                        .is_empty()
                                    })
                                    .filter(|candidate| {
                                        subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(candidate),
                                        )
                                        .is_empty()
                                    })
                                });
                            let immediate_combat_obtain =
                                immediate_combat_obtain_simulated_subset(&next).filter(|candidate| {
                                    subset_diffs(
                                        seed_start_combat_observed_subset(&post.message),
                                        candidate.clone(),
                                    )
                                    .is_empty()
                                });
                            let deferred_cursed_key_chest =
                                deferred_cursed_key_chest_simulated_subset(&source, decision, &next)
                                    .filter(|candidate| {
                                        subset_diffs(
                                            seed_start_reward_observed_subset(&post.message),
                                            candidate.clone(),
                                        )
                                        .is_empty()
                                    });
                            let deferred_reward_obtain =
                                pending_reward_obtain_publication_candidate(&next, post, &action.command);
                            let deferred_event_leave = deferred_event_leave_simulated_subset(&source, &next)
                                .filter(|candidate| {
                                    subset_diffs(
                                        seed_start_event_observed_subset(&post.message),
                                        candidate.clone(),
                                    )
                                    .is_empty()
                                });
                            let deferred_event_publication =
                                deferred_event_obtain_map_simulated_subset(&source, &next)
                                    .filter(|candidate| {
                                        subset_diffs(
                                            seed_start_map_return_observed_subset(&post.message),
                                            candidate.clone(),
                                        )
                                        .is_empty()
                                    });
                            let deferred_scry_keep =
                                deferred_scry_keep_simulated_subset(&source, decision, &next)
                                    .filter(|candidate| {
                                        subset_diffs(
                                            seed_start_combat_observed_subset(&post.message),
                                            seed_start_simulated_combat_subset(candidate),
                                        )
                                        .is_empty()
                                    });
                            if let Some(candidate) = skipped_candidate {
                                if matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn))
                                    && end_turn_consumed_cross_combat_hand_select_residual(&candidate)
                                {
                                    state.pending_cross_combat_put_on_deck_card = None;
                                    state.pending_cross_combat_burning_pact_card = None;
                                    state.pending_cross_combat_hidden_cards.clear();
                                }
                                if matches!(
                                    decision,
                                    RunDecisionAction::Run(RunAction::ConfirmHandSelect)
                                ) && source.combat.as_ref().is_some_and(|combat| {
                                    combat.hand_select().is_some_and(|select| {
                                        matches!(
                                            select.purpose,
                                            HandSelectPurpose::WarcryPutOnDraw
                                                | HandSelectPurpose::ThinkingAheadPutOnDraw
                                                | HandSelectPurpose::ForethoughtPutOnDraw
                                        )
                                    })
                                }) {
                                    state.pending_cross_combat_put_on_deck_card = candidate
                                        .combat
                                        .as_ref()
                                        .and_then(|combat| {
                                            combat
                                                .pending_hidden_hand_card_until_end_turn
                                                .last()
                                                .copied()
                                        });
                                }
                                if matches!(decision, RunDecisionAction::GridSelect { .. })
                                    && source.card_grid.as_ref().is_some_and(|grid| {
                                        grid.purpose == sts_core::run::grid::GridPurpose::DollysMirror
                                    })
                                {
                                    if let RunDecisionAction::GridSelect { index } = decision {
                                        state.pending_shop_dolly_card = source
                                            .card_grid
                                            .as_ref()
                                            .and_then(|grid| grid.cards.get(index).copied());
                                    }
                                }
                                if matches!(
                                    decision,
                                    RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)
                                ) && source.combat.as_ref().is_some_and(|combat| {
                                    combat.exhaust_select().is_some_and(|select| {
                                        matches!(
                                            select.purpose,
                                            ExhaustSelectPurpose::BurningPactDraw2
                                                | ExhaustSelectPurpose::BurningPactDraw3
                                        )
                                    })
                                }) {
                                    state.pending_cross_combat_burning_pact_card = candidate
                                        .combat
                                        .as_ref()
                                        .and_then(|combat| {
                                            combat
                                                .pending_hidden_hand_card_until_end_turn
                                                .last()
                                                .copied()
                                        });
                                }
                                if matches!(
                                    decision,
                                    RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)
                                ) && source.combat.as_ref().is_some_and(|combat| {
                                    combat.exhaust_select().is_some_and(|select| {
                                        select.purpose == ExhaustSelectPurpose::PurityExhaustUpTo3
                                    })
                                }) {
                                    state.pending_cross_combat_hidden_cards = candidate
                                        .combat
                                        .as_ref()
                                        .map(|combat| {
                                            combat.pending_hidden_hand_card_until_end_turn.clone()
                                        })
                                        .unwrap_or_default();
                                }
                                if state.pending_shop_dolly_card.is_some()
                                    && (cross_shop_dolly_candidate(
                                        &source,
                                        &next,
                                        state.pending_shop_dolly_card,
                                        post,
                                    )
                                    .is_some()
                                        || cross_shop_dolly_before_action_candidate(
                                            &source,
                                            decision,
                                            state.pending_shop_dolly_card,
                                            post,
                                        )
                                        .is_some())
                                {
                                    state.pending_shop_dolly_card = None;
                                }
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: label.clone(),
                                });
                                state.seed_sim = Some(candidate);
                                None
                            } else if let Some(candidate) = deferred_scry_keep {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: label.clone(),
                                });
                                state.seed_sim = Some(candidate);
                                None
                            } else if immediate_combat_obtain.is_some()
                                || deferred_cursed_key_chest.is_some()
                                || deferred_reward_obtain.is_some()
                                || deferred_event_leave.is_some()
                                || deferred_event_publication.is_some()
                            {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: label.clone(),
                                });
                                state.seed_sim = Some(deferred_reward_obtain.unwrap_or(next));
                                state.pending_cursed_key_reward_publication =
                                    deferred_cursed_key_chest.is_some();
                                None
                            } else if let Some(candidate) =
                                skipped_event_room_entry_candidate(&source, decision, &next, post)
                            {
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
                                if matches!(decision, RunDecisionAction::Combat(CombatAction::EndTurn))
                                    && end_turn_consumed_cross_combat_hand_select_residual(&next)
                                {
                                    state.pending_cross_combat_put_on_deck_card = None;
                                    state.pending_cross_combat_burning_pact_card = None;
                                    state.pending_cross_combat_hidden_cards.clear();
                                }
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
    fn skipped_event_room_entry_requires_idle_map_choose_into_event() {
        let run = RunState::combat_fixture();
        let post = TraceState {
            step: 275,
            received_at: None,
            message: json!({
                "game_state": {
                    "screen_type": "MAP",
                    "floor": 19,
                    "choice_list": ["x=2", "x=3"],
                    "screen_state": {
                        "first_node_chosen": true,
                        "current_node": {"symbol": "?", "x": 3, "y": 1},
                        "next_nodes": [
                            {"symbol": "?", "x": 2, "y": 2},
                            {"symbol": "M", "x": 3, "y": 2}
                        ]
                    }
                }
            }),
        };
        assert!(skipped_event_room_entry_candidate(
            &run,
            RunDecisionAction::Run(RunAction::Proceed),
            &run,
            &post,
        )
        .is_none());
    }

    #[test]
    fn leftover_play_rebinds_next_command_to_post_leftover_hand() {
        use sts_core::content::cards::{DROPKICK_ID, HEAVY_BLADE_PLUS_ID, TRUE_GRIT_ID};
        use sts_core::{CardId, CardInstance};

        let mut run = RunState::combat_fixture();
        let target = {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 3;
            combat.piles.hand = vec![
                CardInstance::new(CardId::new(1), DROPKICK_ID),
                CardInstance::new(CardId::new(2), HEAVY_BLADE_PLUS_ID),
                CardInstance::new(CardId::new(3), TRUE_GRIT_ID),
            ];
            combat.monsters.iter().find(|m| m.alive).map(|m| m.id)
        };
        let before = direct_decision(&run, "PLAY 2 0").expect("pre-leftover PLAY 2");
        let RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) = before.0 else {
            panic!("expected play");
        };
        assert_eq!(
            card_id,
            CardId::new(2),
            "pre-leftover PLAY 2 is Heavy Blade+"
        );

        let leftover = RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(1),
            target,
        });
        let after_dropkick = apply_run_decision_action(&run, leftover).expect("leftover Dropkick");
        let rebound = rebind_leftover_pending_command(&after_dropkick, "PLAY 2 0")
            .expect("rebind PLAY 2 after leftover");
        let RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) = rebound else {
            panic!("expected rebound play");
        };
        assert_eq!(card_id, CardId::new(3), "post-leftover PLAY 2 is True Grit");
    }

    #[test]
    fn leftover_end_finished_player_turn_requires_drawn_hand_without_decision() {
        let fixture = RunState::combat_fixture();
        let mut source = fixture.clone();
        source.combat.as_mut().expect("combat").piles.hand.clear();

        // One drawn card is enough: Time Warp settlement publishes the leftover
        // frame before the hand has refilled.
        let mut partial = fixture.clone();
        partial
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .hand
            .truncate(1);
        assert!(leftover_end_finished_player_turn(&source, &partial));
        assert!(leftover_end_finished_player_turn(&source, &fixture));

        assert!(
            !leftover_end_finished_player_turn(&source, &source),
            "an undrawn candidate hand has not finished the turn"
        );
        assert!(
            !leftover_end_finished_player_turn(&partial, &fixture),
            "a source that still holds cards is not a leftover END frame"
        );
        assert!(
            !leftover_end_completed_next_player_turn(&fixture, &fixture),
            "the same player-turn counter is not a completed leftover END"
        );
        let mut advanced = fixture.clone();
        advanced
            .combat
            .as_mut()
            .expect("combat")
            .relic_counters
            .player_turns_started += 1;
        assert!(leftover_end_completed_next_player_turn(&fixture, &advanced));

        let mut pending_decision = fixture.clone();
        let combat = pending_decision.combat.as_mut().expect("combat");
        let source_card_id = combat.piles.hand[0].id;
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: sts_core::combat::HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: VecDeque::new(),
        });
        assert!(
            !leftover_end_finished_player_turn(&source, &pending_decision),
            "an open selection means the turn has not settled"
        );
    }

    #[test]
    fn leftover_end_state_requires_end_or_time_warp_flags() {
        let mut run = RunState::combat_fixture();
        assert!(
            !leftover_end_state_is_eligible(&run, None),
            "ordinary leftover STATE is not a Time Warp loseBlock frame"
        );
        assert!(leftover_end_state_is_eligible(
            &run,
            Some(&RunDecisionAction::Combat(CombatAction::EndTurn)),
        ));
        run.combat.as_mut().expect("combat").time_warp_end_turn = true;
        assert!(
            leftover_end_state_is_eligible(&run, None),
            "rejected PLAY after Time Warp 12 still publishes leftover loseBlock"
        );
        run.combat.as_mut().expect("combat").time_warp_end_turn = false;
        run.combat
            .as_mut()
            .expect("combat")
            .time_warp_end_turn_pre_discard_settled = true;
        assert!(leftover_end_state_is_eligible(&run, None));
        run.combat
            .as_mut()
            .expect("combat")
            .time_warp_end_turn_pre_discard_settled = false;
        run.combat
            .as_mut()
            .expect("combat")
            .leftover_end_turn_draw_remaining = 2;
        assert!(leftover_end_state_is_eligible(&run, None));
    }

    #[test]
    fn leftover_end_state_after_time_warp_play_applies_monster_lose_block() {
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.piles.hand.clear();
            combat.time_warp_end_turn = true;
            combat.time_warp_end_turn_pre_discard_settled = true;
            combat.monsters[0].block = 20;
        }
        let mut observed = seed_start_simulated_combat_subset(&run);
        observed["monsters"][0]["block"] = json!(0);
        let post = comm_mod_combat_post_from_subset(&observed);
        let candidate = leftover_end_state_publication_candidate(&run, None, &post)
            .expect("Time Warp leftover STATE applies loseBlock");
        assert_eq!(
            candidate.combat.as_ref().expect("combat").monsters[0].block,
            0
        );
        assert_eq!(
            candidate.player_hp, run.player_hp,
            "loseBlock does not take the monster turn"
        );
    }

    fn comm_mod_combat_post_from_subset(subset: &Value) -> TraceState {
        let monsters = subset
            .get("monsters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|monster| {
                json!({
                    "name": monster.get("name").cloned().unwrap_or(json!("")),
                    "current_hp": monster.get("current_hp").cloned().unwrap_or(json!(0)),
                    "max_hp": monster.get("max_hp").cloned().unwrap_or(json!(0)),
                    "block": monster.get("block").cloned().unwrap_or(json!(0)),
                    "intent": monster.get("intent").cloned().unwrap_or(json!("")),
                    "move_id": monster.get("move_id").cloned().unwrap_or(json!(0)),
                    "is_gone": false,
                    "powers": [
                        {"id": "Strength", "amount": monster.get("strength").cloned().unwrap_or(json!(0))},
                        {"id": "Ritual", "amount": monster.get("ritual").cloned().unwrap_or(json!(0))},
                        {"id": "Vulnerable", "amount": monster.get("vulnerable").cloned().unwrap_or(json!(0))},
                    ],
                })
            })
            .collect::<Vec<_>>();
        let ids_to_cards = |key: &str| {
            subset
                .get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| {
                    let name = id.as_str().unwrap_or("");
                    let upgraded = name.ends_with('+');
                    json!({
                        "id": name.trim_end_matches('+'),
                        "upgrades": if upgraded { 1 } else { 0 },
                    })
                })
                .collect::<Vec<_>>()
        };
        let named = |key: &str| {
            subset
                .get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|id| json!({"id": id, "name": id}))
                .collect::<Vec<_>>()
        };
        TraceState {
            step: 1774,
            received_at: None,
            message: json!({
                "game_state": {
                    "screen_type": subset.get("screen_type").cloned().unwrap_or(json!("NONE")),
                    "ascension_level": subset.get("ascension").cloned().unwrap_or(json!(0)),
                    "floor": subset.get("floor").cloned().unwrap_or(json!(1)),
                    "gold": subset.get("gold").cloned().unwrap_or(json!(0)),
                    "current_hp": subset.get("current_hp").cloned().unwrap_or(json!(0)),
                    "max_hp": subset.get("max_hp").cloned().unwrap_or(json!(0)),
                    "deck": ids_to_cards("deck_ids"),
                    "relics": named("relic_ids"),
                    "potions": named("potion_ids"),
                    "combat_state": {
                        "player": {
                            "current_hp": subset.get("combat_player_hp").cloned().unwrap_or(json!(0)),
                            "block": subset.get("combat_player_block").cloned().unwrap_or(json!(0)),
                            "energy": subset.get("combat_player_energy").cloned().unwrap_or(json!(0)),
                        },
                        "hand": ids_to_cards("hand_ids"),
                        "draw_pile": ids_to_cards("draw_ids"),
                        "discard_pile": ids_to_cards("discard_ids"),
                        "monsters": monsters,
                    }
                }
            }),
        }
    }

    #[test]
    fn skipped_put_on_deck_candidate_parks_selected_card_until_end_turn() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        let selected_card_id = combat.piles.hand[1].id;
        combat.piles.hand[1].temp_cost = Some(0);
        combat.piles.hand[1].temp_cost_turn_only = true;
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
        assert_eq!(
            combat.pending_hidden_hand_card_until_end_turn[0].temp_cost, None,
            "skipped retrieval restores the card's printed cost"
        );
        assert!(!combat.pending_hidden_hand_card_until_end_turn[0].temp_cost_turn_only);
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
    fn skipped_warcry_auto_place_candidate_keeps_drawn_card_in_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_id = combat.piles.hand[0].id;
        combat.piles.hand = vec![CardInstance::new(
            source_id,
            sts_core::content::cards::WARCRY_ID,
        )];
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), sts_core::content::cards::DEFEND_R_ID),
            CardInstance::new(CardId::new(3), sts_core::content::cards::STRIKE_R_ID),
        ];
        combat.piles.discard_pile.clear();
        combat.piles.exhaust_pile.clear();
        combat.player.energy = 0;

        let candidate = skipped_warcry_auto_place_candidate(
            &run,
            RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: source_id,
                target: None,
            }),
        )
        .expect("candidate construction")
        .expect("lone Warcry candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![sts_core::content::cards::STRIKE_R_ID]
        );
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == sts_core::content::cards::WARCRY_ID));
        assert!(!combat.skip_put_on_deck_auto_place);
    }

    #[test]
    fn skipped_put_on_deck_under_runic_pyramid_does_not_park_card() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.relics.push(Relic::RunicPyramid);
        let source_card_id = combat.piles.hand[0].id;
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
        .expect("Runic Pyramid candidate should be eligible");
        assert!(candidate
            .combat
            .as_ref()
            .expect("candidate combat")
            .pending_hidden_hand_card_until_end_turn
            .is_empty());
    }

    #[test]
    fn normal_new_hand_select_clears_superseded_screen_selection() {
        let mut source = RunState::combat_fixture();
        let combat = source.combat.as_mut().expect("combat fixture");
        let stale = combat.piles.hand.remove(1);
        combat.pending_hidden_hand_card_until_end_turn = vec![stale];
        let source_card_id = combat.piles.hand[0].id;
        let mut next = source.clone();
        next.combat.as_mut().expect("next combat").decision =
            Some(CombatDecisionState::HandSelect {
                state: sts_core::combat::HandSelectState {
                    purpose: HandSelectPurpose::WarcryPutOnDraw,
                    source_card_id,
                    selected_hand_index: None,
                    selected_hand_indices: Vec::new(),
                    dual_wield_restore_on_confirm: Vec::new(),
                    dual_wield_force_exhaust: false,
                },
                pending_actions: VecDeque::new(),
            });

        assert!(opened_new_selection_screen(&source, &next));
        clear_superseded_selection_screen_pending(&mut next);
        assert!(next
            .combat
            .as_ref()
            .expect("next combat")
            .pending_hidden_hand_card_until_end_turn
            .is_empty());
    }

    #[test]
    fn secret_weapon_grid_does_not_drop_cross_combat_burning_pact_residual() {
        let source = RunState::combat_fixture();
        let mut next = source.clone();
        let combat = next.combat.as_mut().expect("combat fixture");
        let source_id = combat.piles.hand[0].id;
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: sts_core::combat::DrawSelectState {
                purpose: sts_core::combat::DrawSelectPurpose::SecretWeaponAttackToHand,
                source_card_id: source_id,
                selectable_card_ids: combat.piles.draw_pile.iter().map(|card| card.id).collect(),
                selected_draw_index: None,
                pending_actions: VecDeque::new(),
            },
        });
        assert!(
            opened_new_selection_screen(&source, &next),
            "Secret Weapon still opens a new selection screen"
        );
        assert!(
            !should_drop_cross_combat_hand_select_residual(&source, &next),
            "GridCardSelectScreen must not prep() HandCardSelectScreen.selectedCards"
        );
    }

    #[test]
    fn new_exhaust_select_drops_cross_combat_burning_pact_residual() {
        let source = RunState::combat_fixture();
        let mut next = source.clone();
        let combat = next.combat.as_mut().expect("combat fixture");
        let mut source_card = combat.piles.hand.remove(0);
        source_card.content_id = sts_core::content::cards::BURNING_PACT_ID;
        let source_card_id = source_card.id;
        combat.decision = Some(CombatDecisionState::ExhaustSelect {
            state: sts_core::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::BurningPactDraw2,
                source_card_id: Some(source_card_id),
                source_card: Some(source_card),
                source_card_force_exhaust: false,
                selected_hand_indices: Vec::new(),
                interrupted_by_cultist_potion: false,
                pending_actions: VecDeque::new(),
            },
        });
        assert!(should_drop_cross_combat_hand_select_residual(
            &source, &next
        ));
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
    }

    #[test]
    fn skipped_discovery_candidate_closes_without_adding_the_chosen_card() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_hand = combat.piles.hand.clone();
        let choice_content = combat.piles.hand[0].content_id;
        combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
            choices: vec![CardInstance::new(CardId::new(10_000), choice_content)],
            source_card: Some(CardInstance::new(
                CardId::new(10_001),
                sts_core::content::cards::DISCOVERY_ID,
            )),
            source_card_force_exhaust: false,
            source_card_play_top: false,
            pending_actions: VecDeque::new(),
        });

        let candidate = skipped_discovery_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index: 0 }),
        )
        .expect("candidate construction")
        .expect("Discovery candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_hand.iter().map(|card| card.id).collect::<Vec<_>>()
        );
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == sts_core::content::cards::DISCOVERY_ID));
        assert!(combat.exhaust_select().is_none());
    }

    #[test]
    fn headbutt_plus_is_eligible_for_the_draw_alias_candidate() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            sts_core::content::cards::HEADBUTT_PLUS_ID,
        )];
        assert!(headbutt_play_from_hand(
            &run,
            RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(sts_core::MonsterId::new(1)),
            }),
        ));
        assert!(!headbutt_play_from_hand(
            &run,
            RunDecisionAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: Some(sts_core::MonsterId::new(1)),
            }),
        ));
    }

    #[test]
    fn skipped_headbutt_candidate_closes_without_retrieving_the_discard_card() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source = CardInstance::new(CardId::new(300), sts_core::content::cards::HEADBUTT_ID);
        let first = CardInstance::new(CardId::new(301), sts_core::content::cards::THUNDERCLAP_ID);
        let second = CardInstance::new(CardId::new(302), sts_core::content::cards::HAVOC_ID);
        combat.piles.hand.clear();
        combat.piles.draw_pile.clear();
        combat.piles.discard_pile = vec![first, second];
        combat.piles.exhaust_pile.clear();
        combat.play_top_force_exhaust_active = true;
        combat.decision = Some(CombatDecisionState::DiscardSelect {
            state: sts_core::combat::DiscardSelectState {
                purpose: DiscardSelectPurpose::HeadbuttPutOnDraw,
                source_card_id: Some(source.id),
                source_card: Some(source),
                source_card_force_exhaust: true,
                selected_discard_indices: Vec::new(),
                max_choices: 1,
                selected_discard_index: None,
                pending_actions: VecDeque::new(),
            },
        });

        let candidate = skipped_headbutt_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ChooseDiscardSelect { index: 1 }),
        )
        .expect("candidate construction")
        .expect("Headbutt candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert!(combat.decision.is_none());
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == sts_core::content::cards::HEADBUTT_ID));
        assert!(combat.piles.draw_pile.is_empty());
        assert_eq!(
            combat
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![
                sts_core::content::cards::THUNDERCLAP_ID,
                sts_core::content::cards::HAVOC_ID,
            ]
        );
    }

    #[test]
    fn skipped_purity_selected_cards_are_absent_from_observed_exhaust_when_not_exhausted() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let selected = combat.piles.hand[0];
        let selected_key = simulated_card_projection_key(&selected);
        combat.decision = Some(CombatDecisionState::ExhaustSelect {
            state: sts_core::combat::ExhaustSelectState {
                purpose: ExhaustSelectPurpose::PurityExhaustUpTo3,
                source_card_id: Some(selected.id),
                source_card: None,
                source_card_force_exhaust: false,
                selected_hand_indices: vec![0],
                interrupted_by_cultist_potion: false,
                pending_actions: VecDeque::new(),
            },
        });
        let post = TraceState {
            step: 1,
            received_at: None,
            message: json!({
                "game_state": {
                    "combat_state": {
                        "exhaust_pile": []
                    }
                }
            }),
        };
        assert!(skipped_purity_selected_cards_are_absent_from_observed_exhaust(&run, &post));
        let exhausted_post = TraceState {
            step: 1,
            received_at: None,
            message: json!({
                "game_state": {
                    "combat_state": {
                        "exhaust_pile": [{ "id": selected_key, "upgrades": 0 }]
                    }
                }
            }),
        };
        assert!(
            !skipped_purity_selected_cards_are_absent_from_observed_exhaust(&run, &exhausted_post)
        );
    }

    #[test]
    fn deferred_time_warp_hand_select_candidate_keeps_current_hand() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        combat.monsters[0].content_id = sts_core::content::monsters::TIME_EATER_ID;
        combat.monsters[0].powers.time_warp = 11;
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
        let original_hand_len = combat.piles.hand.len();
        let candidate = deferred_time_warp_hand_select_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ConfirmHandSelect),
        )
        .expect("candidate construction")
        .expect("Time Warp Warcry candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert!(combat.decision.is_none());
        assert_eq!(combat.piles.hand.len(), original_hand_len.saturating_sub(2));
    }

    #[test]
    fn deferred_time_warp_end_instead_of_play_requires_pending_end() {
        let run = RunState::combat_fixture();
        let card_id = run.combat.as_ref().expect("combat").piles.hand[0].id;
        let decision = RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id,
            target: None,
        });
        let post = TraceState {
            step: 1,
            received_at: None,
            message: json!({}),
        };
        assert!(
            deferred_time_warp_end_instead_of_play_candidate(&run, decision, &post).is_none(),
            "ordinary PLAY must not flush a turn that Time Warp has not ended"
        );
    }

    #[test]
    fn deferred_time_warp_skipped_put_on_deck_keeps_hand_and_hp() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_card_id = combat.piles.hand[0].id;
        let hp_before = combat.player.hp;
        combat.monsters[0].content_id = sts_core::content::monsters::TIME_EATER_ID;
        combat.monsters[0].powers.time_warp = 11;
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: sts_core::combat::HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id,
                selected_hand_index: Some(1),
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: VecDeque::from([
                sts_core::action::InternalAction::ApplyDeferredTimeWarpCardPlay,
            ]),
        });
        let original_hand_len = combat.piles.hand.len();
        let candidate = deferred_time_warp_skipped_put_on_deck_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ConfirmHandSelect),
        )
        .expect("candidate construction")
        .expect("Time Warp skipped Warcry candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert!(combat.decision.is_none());
        assert_eq!(combat.player.hp, hp_before);
        assert_eq!(combat.piles.hand.len(), original_hand_len.saturating_sub(2));
        assert_eq!(combat.pending_hidden_hand_card_until_end_turn.len(), 1);
    }

    #[test]
    fn skipped_draw_select_candidate_closes_without_retrieving_the_skill() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let source_id = CardId::new(200);
        let selected_id = CardId::new(201);
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            sts_core::content::cards::STRIKE_R_ID,
        )];
        combat.piles.draw_pile = vec![CardInstance::new(
            selected_id,
            sts_core::content::cards::DEFEND_R_ID,
        )];
        combat.piles.limbo = vec![CardInstance::new(
            source_id,
            sts_core::content::cards::SECRET_TECHNIQUE_ID,
        )];
        combat.piles.exhaust_pile.clear();
        combat.decision = Some(CombatDecisionState::DrawSelect {
            state: sts_core::combat::DrawSelectState {
                purpose: sts_core::combat::DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id: source_id,
                selectable_card_ids: vec![selected_id],
                selected_draw_index: None,
                pending_actions: VecDeque::new(),
            },
        });

        let candidate = skipped_draw_select_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ChooseDrawSelect { index: 0 }),
        )
        .expect("candidate construction")
        .expect("Secret Technique candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(combat.piles.hand.len(), 1);
        assert_eq!(combat.piles.draw_pile[0].id, selected_id);
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == sts_core::content::cards::SECRET_TECHNIQUE_ID));
    }

    #[test]
    fn skipped_toolbox_candidate_closes_without_adding_the_chosen_card() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let original_hand = combat.piles.hand.clone();
        combat.decision = Some(CombatDecisionState::ToolboxCardReward {
            choices: vec![CardInstance::new(
                CardId::new(10_000),
                sts_core::content::cards::SECRET_TECHNIQUE_ID,
            )],
        });

        let candidate = skipped_toolbox_candidate(
            &run,
            RunDecisionAction::Run(RunAction::ChooseCombatCardReward { index: 0 }),
        )
        .expect("candidate construction")
        .expect("Toolbox candidate should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            original_hand.iter().map(|card| card.id).collect::<Vec<_>>()
        );
        assert!(combat.toolbox_card_reward_choices().is_none());
    }

    #[test]
    fn park_cross_combat_residuals_remints_each_card() {
        let run = RunState::combat_fixture();
        let cards = [
            CardInstance::new(CardId::new(1000), sts_core::content::cards::STRIKE_R_ID),
            CardInstance::new(CardId::new(1001), sts_core::content::cards::DEFEND_R_ID),
            CardInstance::new(CardId::new(1002), sts_core::content::cards::DROPKICK_ID),
        ];
        let candidate = park_cross_combat_residuals_and_end_turn(
            &run,
            RunDecisionAction::Combat(CombatAction::EndTurn),
            &cards,
        )
        .expect("candidate construction")
        .expect("multi-card residual should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        for card in &cards {
            let residual = combat
                .piles
                .hand
                .iter()
                .chain(combat.piles.draw_pile.iter())
                .chain(combat.piles.discard_pile.iter())
                .chain(combat.pending_hidden_hand_card_until_end_turn.iter())
                .find(|existing| existing.content_id == card.content_id)
                .expect("residual content present after end-turn");
            assert_ne!(residual.id, card.id);
        }
    }

    #[test]
    fn cross_combat_put_on_deck_candidate_uses_transient_card_id() {
        let mut run = RunState::combat_fixture();
        let carried = CardInstance::new(CardId::new(1000), sts_core::content::cards::WOUND_ID);
        run.combat
            .as_mut()
            .expect("combat fixture")
            .piles
            .discard_pile
            .push(carried);
        let candidate = cross_combat_put_on_deck_candidate(
            &run,
            RunDecisionAction::Combat(CombatAction::EndTurn),
            Some(carried),
        )
        .expect("candidate construction")
        .expect("typed cross-combat residual should be eligible beside the live copy");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        let residual = combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.pending_hidden_hand_card_until_end_turn.iter())
            .find(|card| card.content_id == carried.content_id && card.id != carried.id)
            .expect("reminted residual beside the live copy");
        assert_eq!(residual.content_id, carried.content_id);
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .chain(combat.piles.draw_pile.iter())
                .chain(combat.piles.discard_pile.iter())
                .filter(|card| card.content_id == carried.content_id)
                .count(),
            2
        );
    }

    #[test]
    fn empty_hand_end_keeps_cross_combat_hand_select_residual() {
        let mut run = RunState::combat_fixture();
        run.combat
            .as_mut()
            .expect("combat fixture")
            .pending_hidden_hand_card_until_end_turn = vec![CardInstance::new(
            CardId::new(1000),
            sts_core::content::cards::STRIKE_R_ID,
        )];
        assert!(!end_turn_consumed_cross_combat_hand_select_residual(&run));
        run.combat
            .as_mut()
            .expect("combat fixture")
            .pending_hidden_hand_card_until_end_turn
            .clear();
        assert!(end_turn_consumed_cross_combat_hand_select_residual(&run));
        run.combat = None;
        assert!(!end_turn_consumed_cross_combat_hand_select_residual(&run));
    }

    #[test]
    fn cross_combat_burning_pact_candidate_uses_transient_card_id() {
        let run = RunState::combat_fixture();
        let carried = CardInstance::new(CardId::new(1000), sts_core::content::cards::REGRET_ID);
        let candidate = cross_combat_burning_pact_candidate(
            &run,
            RunDecisionAction::Combat(CombatAction::EndTurn),
            Some(carried),
        )
        .expect("candidate construction")
        .expect("typed Burning Pact residual should be eligible");
        let combat = candidate.combat.as_ref().expect("candidate combat");
        let residual = combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.pending_hidden_hand_card_until_end_turn.iter())
            .find(|card| card.content_id == carried.content_id)
            .expect("residual after end-turn settlement");
        assert_eq!(residual.content_id, carried.content_id);
        assert_ne!(residual.id, carried.id);
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
    fn skipped_gambling_chip_empty_hand_draws_with_unceasing_top() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.relics.push(Relic::UnceasingTop);
        let selected_indices = (0..combat.piles.hand.len()).collect::<Vec<_>>();
        let top_draw = combat.piles.draw_pile.last().copied();
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
            combat.piles.hand.last().map(|card| card.id),
            top_draw.map(|card| card.id),
            "Unceasing Top draws the top card into the emptied hand"
        );
        assert!(combat.piles.discard_pile.is_empty());
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
            pending_cross_combat_put_on_deck_card: None,
            pending_cross_combat_burning_pact_card: None,
            pending_cross_combat_hidden_cards: Vec::new(),
            pending_shop_dolly_card: None,
            pending_cursed_key_reward_publication: false,
            pending_rejected_combat_play: None,
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
