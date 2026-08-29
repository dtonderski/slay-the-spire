use super::card_effects;
mod card_actions;
mod damage_actions;
mod decision_actions;
mod defense_actions;
mod pile_actions;
mod player_actions;
use crate::{
    action::{CardPile, CombatAction, HpLossSource, InternalAction},
    card::{CardType, TargetRequirement},
    combat::{
        apply_burning_blood,
        cost::{effective_card_cost, printed_card_cost},
        damage::{
            deal_damage_info_to_monster_with_result, deal_unmodified_damage_to_monster,
            reflect_spikes_to_player, DamageInfo, DamageSource,
        },
        validate_combat_action, CombatDecisionState, CombatPhase, DiscardSelectPurpose,
        DrawSelectPurpose, HandSelectPurpose,
    },
    content::cards::{
        card_instance_is_upgradeable, get_card_definition, required_upgrade_content_id,
        upgrade_card_instance, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID, CLASH_ID,
        CLASH_PLUS_ID, CORRUPTION_ID, CORRUPTION_PLUS_ID, DAZED_ID, DUAL_WIELD_ID,
        DUAL_WIELD_PLUS_ID, EXHUME_ID, EXHUME_PLUS_ID, NECRONOMICURSE_ID, NORMALITY_ID, PAIN_ID,
        PURITY_PLUS_ID, SENTINEL_ID, SENTINEL_PLUS_ID, WHIRLWIND_ID, WHIRLWIND_PLUS_ID,
    },
    content::monsters::{
        apply_collector_death_escape, apply_gremlin_leader_death_escape,
        apply_reptomancer_death_escape, awakened_one_is_half_dead, check_slime_boss_split,
        get_monster_definition, guardian_accumulate_hp_damage, release_stasis_card_on_death,
        wake_lagavulin_on_damage, AWAKENED_ONE_ID, GIANT_HEAD_ID, GUARDIAN_ID,
    },
    content::shop_pool::{colorless_discovery_pool, ironclad_combat_discovery_pool},
    ids::{CardId, ContentId, MonsterId},
    power::{
        apply_monster_vulnerable, apply_monster_weak, apply_player_vulnerable, calculate_block,
    },
    relic::Relic,
    rng::JavaRng,
    CardInstance, CombatState, MonsterState, SimError, SimResult,
};
use std::collections::VecDeque;

pub use super::card_effects::top_draw_card_definition;

const MAX_HAND_SIZE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTransition {
    pub state: CombatState,
    pub event_log: Vec<InternalAction>,
}

/// Death callbacks that the target queues until an end-turn power action has
/// settled and the visible hand discard has completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeferredMonsterDeath {
    pub(crate) stasis_card: Option<CardInstance>,
    pub(crate) gremlin_horn: bool,
}

pub fn apply_combat_action(state: &CombatState, action: CombatAction) -> SimResult<CombatState> {
    Ok(apply_combat_action_with_events(state, action)?.state)
}

pub fn apply_combat_action_with_events(
    state: &CombatState,
    action: CombatAction,
) -> SimResult<CombatTransition> {
    validate_combat_action(state, action)?;

    let mut transition = match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => apply_end_turn(state),
    }?;
    if matches!(action, CombatAction::PlayCard { .. })
        && transition.state.opening_end_turn_pending
        && transition.state.decision.is_none()
        && transition.state.phase == CombatPhase::WaitingForPlayer
    {
        // Fight-two opening END already published after DrawCardAction. The
        // leftover EndTurnAction passed callEndOfTurnActions before that
        // ready frame; remaining work is discard + monster turn + next draw.
        // A second atEndOfTurn would expire Flex played on this frame
        // (FIDL01576).
        transition.state.opening_end_turn_pending = false;
        transition.state.preserve_temp_strength_on_next_start = true;
        crate::combat::hand::resolve_end_of_turn_hand(&mut transition.state)?;
        crate::combat::hand::discard_end_of_turn_hand(&mut transition.state)?;
        crate::combat::turn::settle_opening_end_turn_monster_and_draw(&mut transition.state)?;
    }
    transition.state.validate()?;
    Ok(transition)
}

fn apply_end_turn(state: &CombatState) -> SimResult<CombatTransition> {
    if state.opening_turn_pending {
        let mut next = state.clone();
        let opening_intents = std::mem::take(&mut next.pending_opening_monster_intents);
        if opening_intents.len() != next.monsters.len() {
            return Err(SimError::InvalidState(
                "opening combat intent queue does not match monsters",
            ));
        }
        for (monster, intent) in next.monsters.iter_mut().zip(opening_intents) {
            monster.intent = intent;
        }
        next.opening_turn_pending = false;
        let relics = next.relics.clone();
        crate::relic::apply_start_of_combat_relics(&mut next, &relics)?;
        crate::combat::turn::start_player_turn_after_opening_combat_relics(&mut next)?;
        next.opening_end_turn_pending = true;
        return Ok(CombatTransition {
            state: next,
            event_log: Vec::new(),
        });
    }
    let ethereal_ids = end_turn_ethereal_hand_card_ids(state);
    let mut prepared = state.clone();
    if prepared.time_warp_end_turn {
        // Explicit END after a lagged Time Warp CONFIRM is a second
        // EndTurnAction on top of callEndTurnEarlySequence (FIDL01425 /
        // FIDL01601 two Reverberates). Leftover PLAY still flushes once.
        prepared.time_warp_duplicate_monster_queue = true;
    }
    let next = crate::combat::end_player_turn(&prepared)?;
    let event_log = ethereal_ids
        .into_iter()
        .filter(|card_id| {
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.id == *card_id)
        })
        .map(|card_id| InternalAction::CardExhausted { card_id })
        .collect();

    Ok(CombatTransition {
        state: next,
        event_log,
    })
}

fn end_turn_ethereal_hand_card_ids(state: &CombatState) -> Vec<CardId> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect()
}

pub fn apply_play_top_draw_card_action(
    state: &CombatState,
    target: Option<MonsterId>,
) -> SimResult<CombatState> {
    Ok(process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card: false,
            random_living_target: false,
        }]),
    )?
    .state)
}

pub fn apply_play_top_draw_card_to_state(
    state: &mut CombatState,
    target: Option<MonsterId>,
) -> SimResult<()> {
    let transition = process_internal_queue(
        state,
        VecDeque::from([InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card: false,
            random_living_target: false,
        }]),
    )?;
    *state = transition.state;
    Ok(())
}

/// MayhemPower queues one PlayTopCardAction per stack before any of those
/// cards' `use()` follow-ups (`addToBot`). Deep Breath therefore shuffles
/// after the remaining PlayTops have already taken their cards off the pile
/// (FIDL01709: Mayhem 2 plays the pre-shuffle Dramatic Entrance).
pub fn apply_mayhem_play_top_cards(
    state: &mut CombatState,
    targets: &[Option<MonsterId>],
) -> SimResult<()> {
    let resolves = pop_mayhem_play_top_cards(state, targets)?;
    if resolves.is_empty() {
        return Ok(());
    }
    let transition = process_internal_queue(state, resolves)?;
    *state = transition.state;
    Ok(())
}

/// PlayTopCardAction only removes the top into limbo / cardQueue. The card
/// is not `use()`d until `GameActionManager` drains the action queue
/// (including start-of-turn Fire Breathing) and then services cardQueue.
pub(crate) fn pop_mayhem_play_top_cards(
    state: &mut CombatState,
    targets: &[Option<MonsterId>],
) -> SimResult<VecDeque<InternalAction>> {
    let mut resolves = VecDeque::new();
    for target in targets.iter().copied() {
        let follow_ups = apply_play_top_draw_card(state, target, false, false)?;
        resolves.extend(follow_ups);
    }
    Ok(resolves)
}

fn apply_play_card(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<CombatTransition> {
    let mut prepared = state.clone();
    // A prior force-play can leave this marker set if its select closed without
    // consuming it. Ordinary hand play must not inherit exhaustOnUseOnce, but
    // Havoc / Mayhem / Distilled Chaos still set the marker on nested PlayTop
    // and must keep it if that PlayTop opens a select.
    prepared.play_top_force_exhaust_active = false;
    let (mut queued_state, queue) = card_effects::play_card_queue(&prepared, card_id, target)?;
    // STS AbstractPlayer.cardInUse: while this card resolves (including Pain
    // LoseHP queued from triggerOnOtherCardPlayed), tookDamage must not see it.
    queued_state.card_in_use = Some(card_id);
    let mut transition = process_internal_queue(&queued_state, queue)?;
    transition.state.card_in_use = None;
    // Pen Nib doubles only the attack that crossed its threshold. A normal
    // card transition ends that scope after all nested effects settle; retain
    // it only while a hand/exhaust selection still owns the card-in-use
    // continuation.
    if transition.state.decision.is_none() {
        transition.state.pen_nib_double_active = false;
    }
    Ok(transition)
}

pub(crate) fn process_internal_queue(
    state: &CombatState,
    mut queue: VecDeque<InternalAction>,
) -> SimResult<CombatTransition> {
    let mut next = state.clone();
    let mut event_log = Vec::new();

    while let Some(internal_action) = queue.pop_front() {
        if let InternalAction::PlayCardCopy { card_id } = internal_action {
            if copied_card_cannot_use(&next, card_id)? {
                event_log.push(internal_action);
                while let Some(skipped_action) = queue.pop_front() {
                    event_log.push(skipped_action);
                    if matches!(skipped_action, InternalAction::EndCopiedCardEffects) {
                        break;
                    }
                }
                next.pen_nib_double_active = false;
                continue;
            }
        }
        if let InternalAction::SkipCopiedCardEffectsIfTargetDead { target } = internal_action {
            event_log.push(internal_action);
            if !living_monster_alive(&next, target) {
                while let Some(skipped_action) = queue.pop_front() {
                    event_log.push(skipped_action);
                    if matches!(skipped_action, InternalAction::EndCopiedCardEffects) {
                        break;
                    }
                }
            }
            continue;
        }
        if matches!(
            internal_action,
            InternalAction::SkipCopiedCardEffectsIfCombatDone
        ) {
            event_log.push(internal_action);
            if next.time_warp_end_turn
                || next
                    .monsters
                    .iter()
                    .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
            {
                while let Some(skipped_action) = queue.pop_front() {
                    event_log.push(skipped_action);
                    if matches!(skipped_action, InternalAction::EndCopiedCardEffects) {
                        break;
                    }
                }
            }
            continue;
        }
        if matches!(internal_action, InternalAction::EndCopiedCardEffects) {
            next.pen_nib_double_active = false;
            event_log.push(internal_action);
            continue;
        }
        // A queued MakeTempCardInDrawPileAction is abandoned when the preceding
        // damage has ended combat. The target does not publish the generated
        // status and, importantly, does not consume cardRandomRng for a random
        // insertion that can no longer be observed. Keep the gate generic for
        // all random-spot generated cards; surviving and revival paths retain
        // the normal insertion below.
        let combat_is_ending = next.player.hp <= 0
            || next
                .monsters
                .iter()
                .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster));
        if combat_is_ending
            && matches!(
                internal_action,
                InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
                    | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
            )
        {
            event_log.push(internal_action);
            continue;
        }
        let had_hand_select = matches!(next.decision, Some(CombatDecisionState::HandSelect { .. }));
        let pain_before_reaper = matches!(
            internal_action,
            InternalAction::PlayCard { card_id }
                if next.piles.hand.iter().any(|card| {
                    card.id == card_id
                        && matches!(
                            card.content_id,
                            crate::content::cards::REAPER_ID
                                | crate::content::cards::REAPER_PLUS_ID
                        )
                })
        );
        // Havoc/Mayhem PlayTopCardAction removes the top card then queues the
        // forced play with addToTop. Hex's MakeTempCardInDrawPile is addToBot
        // after Havoc.use, so it can land after the top card is removed but
        // before Pommel (etc.) draws — size n-1 after remove, not n-1-draws
        // (FIDL00381). Drain trailing Hex inserts before nested card resolve.
        let exhaust_follow_up = matches!(
            internal_action,
            InternalAction::CardExhausted { .. } | InternalAction::HandCardExhausted { .. }
        );
        let defer_time_warp_card_play = matches!(internal_action, InternalAction::PlayCard { .. })
            && queue.iter().any(is_player_selection_action)
            && next.monsters.iter().any(|monster| {
                monster.alive && monster.content_id == crate::content::monsters::TIME_EATER_ID
            });
        let follow_ups = if let InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card,
            random_living_target,
        } = internal_action
        {
            let mid_hex: Vec<InternalAction> = {
                let mut drained = Vec::new();
                while let Some(front) = queue.front() {
                    if matches!(
                        front,
                        InternalAction::AddGeneratedCardToDrawPileRandomSpot { content_id }
                            if *content_id == DAZED_ID
                    ) || matches!(
                        front,
                        InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
                            content_id,
                            ..
                        } if *content_id == DAZED_ID
                    ) {
                        drained.push(queue.pop_front().expect("front exists"));
                    } else {
                        break;
                    }
                }
                drained
            };
            apply_play_top_with_mid_hex(
                &mut next,
                target,
                exhaust_played_card,
                random_living_target,
                mid_hex,
            )?
        } else {
            apply_internal_action_with_defer(&mut next, internal_action, defer_time_warp_card_play)?
        };
        event_log.push(internal_action);
        if matches!(
            internal_action,
            InternalAction::ResolveStormOfSteel { .. }
                | InternalAction::ResolveSteamBarrier { .. }
                | InternalAction::ResolveFollowUpEnergy { .. }
        ) {
            // These source actions compute from live card/hand history at their
            // action-manager boundary. Drain their newly queued actions before
            // a later copied CardQueueItem re-evaluates the same card.
            for action in follow_ups.into_iter().rev() {
                queue.push_front(action);
            }
            continue;
        }
        if matches!(internal_action, InternalAction::ResolveTopDrawCard { .. }) {
            // ResolveTopDrawCard expands one card-queue item into its ordered
            // action sequence. Insert that sequence as a lane ahead of existing
            // card-queue siblings without treating its actions as callbacks.
            let index = queue
                .iter()
                .position(|action| matches!(action, InternalAction::ResolveTopDrawCard { .. }))
                .unwrap_or(queue.len());
            for (offset, action) in follow_ups.into_iter().enumerate() {
                queue.insert(index + offset, action);
            }
            continue;
        }
        if matches!(internal_action, InternalAction::DealPreparedDamage { .. }) {
            if let Some(index) = queue
                .iter()
                .position(|action| matches!(action, InternalAction::EndCopiedCardEffects))
            {
                // EndCopiedCardEffects is a simulator card-queue marker, not a
                // target action. Keep the prepared hit's direct reactions in
                // their originating copy before a later copy checks its target
                // or runs on-use triggers.
                for (offset, action) in follow_ups.into_iter().enumerate() {
                    queue.insert(index + offset, action);
                }
                continue;
            }
        }
        let gremlin_horn_expansion =
            matches!(internal_action, InternalAction::ApplyGremlinHornOnDeath);
        let mut gremlin_horn_insert_index = 0;
        for follow_up in follow_ups {
            if matches!(
                internal_action,
                InternalAction::DealDamageAndGainBlockUnblocked { .. }
            ) && matches!(follow_up, InternalAction::GainPrecomputedCardBlock { .. })
            {
                // WallopAction addToTop's its GainBlockAction after damage.
                queue.push_front(follow_up);
            } else if gremlin_horn_expansion {
                // Gremlin Horn queues GainEnergy then Draw at this exact death.
                // Put both ahead of later deaths already on the queue; follow-ups
                // created by Draw remain addToBot behind those later deaths.
                queue.insert(gremlin_horn_insert_index, follow_up);
                gremlin_horn_insert_index += 1;
            } else if pain_before_reaper
                && matches!(follow_up, InternalAction::LoseHp { .. })
                && queue.iter().any(|action| {
                    matches!(action, InternalAction::DealDamageAllAndHealUnblocked { .. })
                })
            {
                let index = queue
                    .iter()
                    .position(|action| {
                        matches!(action, InternalAction::DealDamageAllAndHealUnblocked { .. })
                    })
                    .expect("Reaper healing action remains queued");
                queue.insert(index, follow_up);
            } else if exhaust_follow_up {
                // ExhaustAll runs before UseCardAction settlement. Feel No
                // Pain's addToBot GainBlock therefore precedes Beat of Death's
                // onAfterUseCard damage, but remains behind Sharp Hide damage
                // already queued from onUseCard.
                let exhausted_is_card_in_use = match internal_action {
                    InternalAction::CardExhausted { card_id }
                    | InternalAction::HandCardExhausted { card_id } => {
                        next.card_in_use == Some(card_id)
                    }
                    _ => false,
                };
                if !exhausted_is_card_in_use
                    && matches!(follow_up, InternalAction::GainBlockFromExhaust { .. })
                {
                    if let Some(index) = queue.iter().position(|action| {
                        matches!(action, InternalAction::DealThornsDamageToPlayer { .. })
                    }) {
                        queue.insert(index, follow_up);
                        continue;
                    }
                }
                // The target's onExhaust callbacks are addToBot actions on the
                // original UseCardAction. A Double Tap/Necronomicon copy is a
                // later card-use boundary, so these callbacks must drain before
                // its PlayCardCopy marker. This is observable when Dark Embrace
                // draws between the two Wild Strike Wounds in FIDL01320.
                if let Some(index) = queue
                    .iter()
                    .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
                {
                    queue.insert(index, follow_up);
                } else {
                    push_follow_up(&mut queue, follow_up, card_in_use_is_whirlwind(&next));
                }
            } else {
                push_follow_up(&mut queue, follow_up, card_in_use_is_whirlwind(&next));
            }
        }
        if !queue.is_empty() {
            match next.decision.as_mut() {
                Some(CombatDecisionState::HandSelect { .. }) => {
                    let just_opened = !had_hand_select;
                    // Armaments' own Hex is queued after AwaitHandSelect and must
                    // wait for CONFIRM. Havoc Hex is a parent follow-up after
                    // nested PlayTop already opened the select — real still runs
                    // that bot action under the open screen (15ab4cc step 769).
                    let flush_free_hex = !just_opened
                        || matches!(internal_action, InternalAction::PlayTopDrawCard { .. });
                    if just_opened && !flush_free_hex {
                        if let Some(CombatDecisionState::HandSelect {
                            pending_actions, ..
                        }) = next.decision.as_mut()
                        {
                            pending_actions.extend(queue.drain(..));
                        }
                        break;
                    }
                    let mut deferred = VecDeque::new();
                    while let Some(queued) = queue.pop_front() {
                        if matches!(
                            queued,
                            InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
                                | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
                        ) {
                            let free_follow_ups = apply_internal_action(&mut next, queued)?;
                            event_log.push(queued);
                            for free_follow_up in free_follow_ups {
                                push_follow_up(&mut queue, free_follow_up, false);
                            }
                        } else {
                            deferred.push_back(queued);
                        }
                    }
                    if let Some(CombatDecisionState::HandSelect {
                        pending_actions, ..
                    }) = next.decision.as_mut()
                    {
                        pending_actions.extend(deferred);
                    }
                    break;
                }
                Some(CombatDecisionState::DiscardSelect { state }) => {
                    state.pending_actions.extend(queue.drain(..));
                    break;
                }
                Some(CombatDecisionState::ExhaustSelect { state }) => {
                    state.pending_actions.extend(queue.drain(..));
                    break;
                }
                Some(CombatDecisionState::DrawSelect { state }) => {
                    // Draw-selection screens are also opened before queued
                    // on-use follow-ups (Hex/Dazed, etc.) resolve. Keep those
                    // actions behind the grid so CONFIRM settles the selected
                    // card before mutating the remaining draw pile.
                    state.pending_actions.extend(queue.drain(..));
                    break;
                }
                Some(CombatDecisionState::DiscoveryCardReward {
                    pending_actions, ..
                }) => {
                    // FIDL00233: Hex onUseCard Dazed must wait until Discovery
                    // reward CHOOSE closes, not land while CARD_REWARD is open.
                    pending_actions.extend(queue.drain(..));
                    break;
                }
                _ => {}
            }
        }
    }

    if !matches!(
        next.decision,
        Some(CombatDecisionState::DiscardSelect { .. })
    ) {
        flush_pending_player_spikes_damage_if_ready(&mut next)?;
        flush_pending_monster_death_relics_if_ready(&mut next)?;
    }

    // Darkling Life Link: when card damage leaves every Darkling half-dead,
    // permanently kill them so combat can end (source Darkling.damage allDead).
    if !matches!(
        next.decision,
        Some(CombatDecisionState::DiscardSelect { .. })
    ) {
        let _ = crate::combat::damage::resolve_darkling_life_link(&mut next.monsters);
    }

    // Byrd's Grounded action is queued behind the complete card action. A
    // copied or multi-hit card therefore keeps Flight's reduction for every
    // hit, even when an earlier hit reduced Flight to zero.
    if !matches!(
        next.decision,
        Some(CombatDecisionState::DiscardSelect { .. })
    ) {
        for monster in &mut next.monsters {
            if monster.powers.flight_grounding_pending {
                monster.powers.flight_grounding_pending = false;
                if monster.alive {
                    monster.intent = crate::MonsterIntent::Stun;
                }
            }
        }
    }

    // Resolve any damage-triggered monster reactions that remain at the end
    // of the card action. Copied-card boundaries settle the same hook earlier.
    if !matches!(
        next.decision,
        Some(CombatDecisionState::DiscardSelect { .. })
    ) {
        crate::content::monsters::resolve_deferred_monster_reactions(&mut next.monsters);
    }

    if next.time_warp_end_turn
        && !next.defer_time_warp_end_turn
        && next.player.hp > 0
        && next.decision.is_none()
    {
        next.time_warp_end_turn = false;
        if next.monsters.iter().any(|monster| monster.alive) {
            next = crate::combat::end_player_turn(&next)?;
        } else {
            // TimeWarpPower.onAfterUseCard still callEndTurnEarlySequence after
            // a lethal 12th card. Burns in hand autoplay before DeathScreen
            // (FIDL01371: Anger+ kills Time Eater, Burn deals 2).
            crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(&mut next)?;
        }
    }

    if next.player.hp <= 0
        || next
            .monsters
            .iter()
            .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        settle_combat_end_from_current_hp(&mut next)?;
    } else {
        next.phase = CombatPhase::WaitingForPlayer;
    }

    Ok(CombatTransition {
        state: next,
        event_log,
    })
}

fn copied_card_cannot_use(state: &CombatState, card_id: CardId) -> SimResult<bool> {
    let definition = card_content_definition(state, card_id)?;
    let normality_blocks = state
        .piles
        .hand
        .iter()
        .any(|card| card.content_id == NORMALITY_ID)
        && state.relic_counters.cards_played_this_turn >= 3;
    let entangled_blocks =
        state.player.powers.entangled > 0 && definition.card_type == CardType::Attack;
    let clash_blocks = matches!(definition.id, CLASH_ID | CLASH_PLUS_ID)
        && state.piles.hand.iter().any(|card| {
            get_card_definition(card.content_id)
                .is_none_or(|candidate| candidate.card_type != CardType::Attack)
        });
    Ok(normality_blocks
        || entangled_blocks
        || clash_blocks
        || !crate::relic::can_play_card_with_relics(state))
}

fn card_in_use_is_whirlwind(state: &CombatState) -> bool {
    let Some(card_id) = state.card_in_use else {
        return false;
    };
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.limbo.iter())
        .chain(state.piles.discard_pile.iter())
        .any(|card| {
            card.id == card_id && matches!(card.content_id, WHIRLWIND_ID | WHIRLWIND_PLUS_ID)
        })
}

fn push_follow_up(
    queue: &mut VecDeque<InternalAction>,
    follow_up: InternalAction,
    whirlwind_in_use: bool,
) {
    // ResolveTopDrawCard represents a parked card-queue item. The target action
    // manager drains every action already queued by the outer card—including
    // exhaust callbacks—before servicing that card queue. Keep this as a lane
    // rule rather than enumerating individual Feel No Pain, Dead Branch, or
    // draw follow-ups.
    if !matches!(follow_up, InternalAction::ResolveTopDrawCard { .. }) {
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::ResolveTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if whirlwind_in_use && matches!(follow_up, InternalAction::GainBlockDirect { .. }) {
        // Whirlwind.use() only addToBots WhirlwindAction. UseCardAction then
        // addToBots Ornamental Fan / Rage GainBlock before that wrapper
        // addToBots DamageAllEnemiesAction, so Fan block is up for Spiker
        // thorns (FIDL01552). Thunderclap/Cleave queue DamageAll in use()
        // itself, so Fan stays after those hits.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DealDamageAll { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::GainBlockDirect { .. }) {
        // Rage and Ornamental Fan addToBot their GainBlockAction during
        // UseCardAction, before that action queues source-card settlement.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::MoveCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::DealPreparedDamage { .. }) {
        // AbstractCard calculates Hemokinesis damage before use() queues LoseHP.
        // Hemokinesis.use then addToBots LoseHP and Damage before UseCardAction
        // invokes Rage/Ornamental Fan, so the prepared hit (and Spiker thorns)
        // must resolve before attack-triggered block. A copied play likewise
        // resolves it before EndCopiedCardEffects and any later copy.
        if let Some(index) = queue.iter().position(|action| {
            matches!(
                action,
                InternalAction::GainBlockDirect { .. }
                    | InternalAction::MoveCard { .. }
                    | InternalAction::EndCopiedCardEffects
            )
        }) {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::DealSharpHideDamageToPlayer { .. }
    ) {
        // SharpHidePower.onUseCard addToBots its DamageAction after card.use(),
        // but before UseCardAction moves the source out of limbo. HP-loss
        // callbacks such as Runic Cube therefore draw before source settlement.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::MoveCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::ApplyVulnerable { .. }) {
        // Hand Drill's onBlockBroken ApplyPowerAction is addToBot from the
        // original DamageAction. The action queue drains it before the card
        // queue services a Double Tap/Necronomicon copy, so that later card
        // sees Vulnerable. Hits already queued by one multi-hit card remain
        // ahead of this follow-up and do not see it.
        if let Some(index) = queue.iter().position(|action| {
            matches!(
                action,
                InternalAction::SkipCopiedCardEffectsIfTargetDead { .. }
                    | InternalAction::SkipCopiedCardEffectsIfCombatDone
                    | InternalAction::PlayCardCopy { .. }
            )
        }) {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::GainBlockDirect { .. }) {
        // Rage / Fan onUseCard is addToBot after card.use() and before a
        // Double Tap copy. Body Slam's copy must read that block (FIDL01618).
        // Do not insert inside the copy-skip window: a lethal original would
        // skip Rage and Juggernaut (FIDL01768 Anger).
        if let Some(index) = queue.iter().position(|action| {
            matches!(
                action,
                InternalAction::SkipCopiedCardEffectsIfTargetDead { .. }
                    | InternalAction::SkipCopiedCardEffectsIfCombatDone
                    | InternalAction::PlayCardCopy { .. }
            )
        }) {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::CardExhausted { .. } | InternalAction::HandCardExhausted { .. }
    ) {
        // Exhaust settlement is an addToBot action from the played card. It
        // must run before attack-triggered monster block (Malleable/Curl Up),
        // so effects such as Charon's Ashes see the pre-block state.
        queue.push_front(follow_up);
        return;
    }

    if matches!(
        follow_up,
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
            | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
    ) {
        // HexPower.onUseCard queues MakeTempCardInDrawPileAction with addToBot
        // after card.use() and before UseCardAction. That insert must land
        // before the source MoveCard settlement so Dark Embrace / other
        // on-exhaust draws still see the pre-exhaust draw pile (and consume
        // cardRandomRng against the correct size).
        //
        // Havoc.use addToBot's PlayTop first, then Hex onUseCard. Hex must not
        // land before PlayTop removes the forced card (insert size n vs n-1).
        //
        // Exception: when MoveCard is followed by AwaitHandSelect (force-exhaust
        // Armaments via Havoc / Mayhem / Distilled Chaos), card.use() has opened
        // a select that must close first. Real Hex then lands after CONFIRM
        // (15ab4cc step 769–771: second Dazed only on Armaments CONFIRM). Insert
        // after the select so it becomes hand-select pending_actions.
        //
        // Keep it ahead of an Ink Bottle draw already returned by card-play
        // relic hooks as well.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DrawCardsFromInkBottle { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        if let Some(index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
        {
            queue.insert(index + 1, follow_up);
            return;
        }
        // Burning Pact's ExhaustSelect is the same pause as Armaments' hand
        // select: Hex on the forced card must wait for CONFIRM (FIDL01694).
        if let Some(index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::AwaitExhaustSelect { .. }))
        {
            queue.insert(index + 1, follow_up);
            return;
        }
        if let Some(move_index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::MoveCard { .. }))
        {
            if let Some(select_index) = queue.iter().enumerate().find_map(|(index, action)| {
                (index > move_index && matches!(action, InternalAction::AwaitHandSelect { .. }))
                    .then_some(index)
            }) {
                queue.insert(select_index + 1, follow_up);
                return;
            }
            queue.insert(move_index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::DrawCardsFromInkBottle { .. }) {
        // AbstractPlayer.useCard calls card.use() first (Havoc addToBot's
        // PlayTopCardAction), then constructs UseCardAction. Ink Bottle's
        // onUseCard runs in that constructor and addToBot's DrawCardAction,
        // then UseCardAction itself is addToBottom'd. Net order:
        //   PlayTop (from use) → Ink draw (from onUseCard) → settle (UseCardAction)
        // Prefer after any queued PlayTop so Ink cannot steal the forced top
        // card (session-16 Havoc+ / Whirlwind / Uppercut+).
        if let Some(index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
        {
            queue.insert(index + 1, follow_up);
            return;
        }
        // Transmutation.use addToBot's TransmutationAction; that action later
        // addToBot's MakeTempCardInHand *after* InkBottle.onUseCard queued its
        // draw. Infernal Blade addToBot's MakeTempCardInHand directly from
        // use(), so its generate stays ahead of Ink.
        if let Some(index) = queue.iter().position(|action| {
            matches!(
                action,
                InternalAction::AddRandomColorlessCardsToHandWhileSourceInLimbo { .. }
                    | InternalAction::AddRandomColorlessCardToHand { .. }
            )
        }) {
            queue.insert(index, follow_up);
            return;
        }
        // Cards without a forced top-play still settle via MoveCard after
        // onUseCard; keep the draw before that limbo→discard/exhaust move.
        if let Some(index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::MoveCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::LoseHp {
            source: HpLossSource::Card(_),
            ..
        }
    ) {
        // UseCardAction's constructor queues Pain with addToTop after
        // Havoc.use has queued PlayTopCardAction. Pain therefore preempts top
        // extraction; Runic Cube's resulting draw does too.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        // Pain.triggerOnOtherCardPlayed uses addToTop. If the played card opens
        // a player-selection screen, its LoseHPAction must settle before that
        // screen becomes observable (Warcry/Burning Pact with Pain). Keep the
        // existing card-effect ordering when no selection boundary is queued;
        // card HP-loss hooks such as Rupture still resolve in their established
        // position relative to damage and powers (FIDL00409).
        if let Some(index) = queue.iter().position(is_player_selection_action) {
            queue.insert(index, follow_up);
            return;
        }
        // PainPower's LoseHPAction is queued by UseCardAction. When the played
        // card is Rupture, its ApplyPowerAction is already in the card queue,
        // but the loss must resolve before the newly applied Rupture can react
        // to that same loss (FIDL00409: playing Rupture with Pain in hand).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::GainRupture { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        // Battle Trance queues Draw + NoDraw in use() before Pain addToTop.
        // Puzzle then addToTop's its draws, so they run before No Draw
        // (FIDL01716). Do not lift Pain ahead of already-queued attack damage:
        // Rupture from that same loss is addToBot and must not buff the hit
        // (bash_damage_uses_strength_before_pain_rupture_from_same_play).
        if queue
            .iter()
            .any(|action| matches!(action, InternalAction::SetCannotDraw))
        {
            queue.push_front(follow_up);
            return;
        }
        // Pain.triggerOnOtherCardPlayed is addToTop on the action queue.
        // Double Tap's copy is a later card-queue item, so Pain/Rupture from
        // the original settle before the copy hits (FIDL02397 Pummel+).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        // UseCardAction constructor addToTops Pain after card.use() queued
        // MakeTempCardInHand / MakeTempCardInDrawPile, so LoseHP / Runic Cube
        // draw run first (FIDL02215 Infernal Blade hand; FIDL02191 Wild Strike
        // Wound insert sees the post-Cube pile).
        if let Some(index) = queue.iter().position(|action| {
            matches!(
                action,
                InternalAction::AddGeneratedCardToPile {
                    to: CardPile::Hand,
                    ..
                } | InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo { .. }
                    | InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
                    | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
            )
        }) {
            queue.insert(index, follow_up);
            return;
        }
        // UseCardAction applies Pain before it settles the played card.
        // Runic Cube's wasHPLost DrawCardAction therefore shuffles/draws
        // before the source card enters discard (FIDL02215 Bash).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::MoveCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::GainMonsterBlock { .. }
            | InternalAction::RerollWrithingMassAfterAttack { .. }
    ) {
        // STS CurlUpPower / MalleablePower / ReactivePower onAttacked use
        // addToBot. Remaining same-card hits and other already-queued bot
        // actions (notably Juggernaut's DamageRandomEnemyAction from an earlier
        // GainBlock in the same card) therefore resolve before the monster
        // gains block. Compulsive's RollMoveAction is also addToBot, so a
        // Headbutt PutOnDeck GRID pauses before the visible intent changes
        // (FIDL01747).
        //
        // Double Tap / Echo Form card-queue copies are modeled as PlayCardCopy
        // and must stay behind Malleable/Curl Up so the copy encounters the
        // block (design_curl_up_action_order.md).
        //
        // Havoc PlayTop defers Malleable until after the outer skill's bot work.
        // Letter Opener (DealUnmodifiedDamage) must resolve first (FIDL00428);
        // without LO, keep Malleable before the outer MoveCard/UseCard settle so
        // Dead Branch / exhaust RNG stays aligned (trace-2026-07-06 Havoc).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        // After Letter Opener (and similar unmodified blasts) already on the bot
        // queue from the outer Havoc skill (FIDL00428). Otherwise push_back so
        // Juggernaut thorns from earlier GainBlock stay ahead of Malleable.
        if let Some(index) = queue
            .iter()
            .rposition(|action| matches!(action, InternalAction::DealUnmodifiedDamage { .. }))
        {
            queue.insert(index + 1, follow_up);
            return;
        }
        // PressEndTurnButtonAction only flags the end; the action manager still
        // drains addToBot Malleable / Reactive before the enemy phase. Block
        // granted after SettleForcedEndTurn survives loseBlock (FIDL02294).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::SettleForcedEndTurn))
        {
            queue.insert(index, follow_up);
            return;
        }
        queue.push_back(follow_up);
        return;
    }

    if matches!(follow_up, InternalAction::HandCardExhausted { .. }) {
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DrawCardsFromInkBottle { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::AddGeneratedHandCardBeforePendingDraw { .. }
    ) {
        // Dead Branch onExhaust is addToBot(MakeTempCardInHand). PlayTop parks
        // the forced card on the card queue (`ResolveTopDrawCard`); the action
        // queue must still drain that MakeTempCard before the parked card is
        // serviced (FIDL01582 nested Havoc → Strike: Feel No Pain then Sword
        // Boomerang, not the reverse).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::ResolveTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::DrawCardsFromInkBottle { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::GainStrength { .. } | InternalAction::GainDexterity { .. }
    ) {
        // Shuriken / Kunai onUseCard is addToBot ApplyPower. Double Tap's copy
        // is a later card-queue item, so the copy sees the new Strength/Dex.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayCardCopy { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(
        follow_up,
        InternalAction::AddGeneratedCardToPile {
            to: CardPile::Hand,
            ..
        }
    ) {
        // Dead Branch onExhaust is addToBot(MakeTempCardInHand). When Havoc
        // self-exhaust still sits ahead of its PlayTop (Corruption settle-before-
        // PlayTop), keep the DB hand-add before that PlayTop so forced-card RNG
        // cannot run first (FIDL00441 Dual Wield → Power Through).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::ResolveTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::DrawCards { .. }) {
        // Runic Cube's DrawCardAction is addToTop from Pain's pre-PlayTop HP
        // loss, so it also preempts extraction. Other outer action-queue draws
        // cannot be serviced behind the later parked card queue.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::PlayTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
        // PlayTopCardAction parks the selected card on the card queue, then the
        // action queue continues through UseCardAction. Dark Embrace's addToBot
        // DrawCardAction therefore consumes leftover draw before the parked card
        // is serviced. Otherwise a nested empty-draw Havoc PlayTops the leftover
        // Defend after popping the discarded parent (FIDL01677).
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::ResolveTopDrawCard { .. }))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    if matches!(follow_up, InternalAction::ApplyGremlinHornOnDeath) {
        // Conclude addToBots DamageAll then PressEndTurn; UseCardAction is
        // already queued. Horn addToBots after the hit, so it drains after the
        // source MoveCard and before the simulator's SettleForcedEndTurn.
        if let Some(index) = queue
            .iter()
            .position(|action| matches!(action, InternalAction::SettleForcedEndTurn))
        {
            queue.insert(index, follow_up);
            return;
        }
    }

    queue.push_back(follow_up);
}

pub fn flush_pending_player_spikes_damage_if_ready(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    if next.pending_player_spikes_damage <= 0 || next.decision.is_some() {
        return Ok(());
    }
    let damage = std::mem::take(&mut next.pending_player_spikes_damage);
    let hp_loss = reflect_spikes_to_player(&mut next.player, &next.relics, damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(&mut next, hp_loss)?;
    *state = next;
    Ok(())
}

pub fn flush_pending_monster_death_relics_if_ready(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    if next.pending_monster_death_relic_triggers == 0 || next.decision.is_some() {
        return Ok(());
    }
    let triggers = std::mem::take(&mut next.pending_monster_death_relic_triggers);
    let queue = (0..triggers)
        .map(|_| InternalAction::ApplyGremlinHornOnDeath)
        .collect::<VecDeque<_>>();
    let transition = process_internal_queue(&next, queue)?;
    *state = transition.state;
    Ok(())
}

fn is_player_selection_action(action: &InternalAction) -> bool {
    matches!(
        action,
        InternalAction::AwaitHandSelect { .. }
            | InternalAction::AwaitDrawSelect { .. }
            | InternalAction::AwaitDiscardSelect { .. }
            | InternalAction::AwaitCopiedDiscardSelect { .. }
            | InternalAction::AwaitExhaustSelect { .. }
            | InternalAction::OpenDiscoveryCardReward { .. }
    )
}

fn checked_combat_sum(value: i32, amount: i32) -> SimResult<i32> {
    value.checked_add(amount).ok_or(SimError::InvalidState(
        "combat integer addition overflows i32",
    ))
}

fn checked_add_combat_value(value: &mut i32, amount: i32) -> SimResult<()> {
    *value = checked_combat_sum(*value, amount)?;
    Ok(())
}

fn apply_internal_action(
    state: &mut CombatState,
    action: InternalAction,
) -> SimResult<Vec<InternalAction>> {
    apply_internal_action_with_defer(state, action, false)
}

fn apply_internal_action_with_defer(
    state: &mut CombatState,
    action: InternalAction,
    defer_time_warp_card_play: bool,
) -> SimResult<Vec<InternalAction>> {
    match action {
        InternalAction::ConsumeDuplicationPotion => card_actions::consume_duplication_potion(state),
        InternalAction::ConsumeDoubleTap => card_actions::consume_double_tap(state),
        InternalAction::ConsumeNecronomicon => card_actions::consume_necronomicon(state),
        InternalAction::ConsumeVigor => card_actions::consume_vigor(state),
        InternalAction::PlayCard { card_id } => {
            card_actions::play_card(state, card_id, defer_time_warp_card_play)
        }
        InternalAction::ApplyDeferredTimeWarpCardPlay => apply_deferred_time_warp_card_play(state),
        InternalAction::PlayCardCopy { card_id } => card_actions::play_card_copy(state, card_id),
        InternalAction::SkipCopiedCardEffectsIfTargetDead { .. }
        | InternalAction::SkipCopiedCardEffectsIfCombatDone => Ok(Vec::new()),
        InternalAction::ResolvePendingMonsterReactions => {
            crate::content::monsters::resolve_deferred_monster_reactions(&mut state.monsters);
            Ok(Vec::new())
        }
        InternalAction::EndCopiedCardEffects => {
            state.pen_nib_double_active = false;
            Ok(Vec::new())
        }
        InternalAction::SpendEnergy { amount } => card_actions::spend_energy(state, amount),
        InternalAction::SpendCardEnergy { card_id } => {
            card_actions::spend_card_energy(state, card_id)
        }
        InternalAction::SetHandCardCostForTurn { card_id, cost } => {
            card_actions::set_hand_card_cost_for_turn(state, card_id, cost)
        }
        InternalAction::SetHandCardCostForCombat { card_id, cost } => {
            card_actions::set_hand_card_cost_for_combat(state, card_id, cost)
        }
        InternalAction::ReduceHandCardCostForCombat { card_id, amount } => {
            card_actions::reduce_hand_card_cost_for_combat(state, card_id, amount)
        }
        InternalAction::DealDamage { info } => damage_actions::deal_damage(state, info),
        InternalAction::PrepareCardDamage { info } => {
            damage_actions::prepare_card_damage(state, info)
        }
        InternalAction::DealPreparedDamage { info } => {
            damage_actions::deal_prepared_damage(state, info)
        }
        InternalAction::DealBaneDamageIfPoisoned { info } => {
            damage_actions::deal_bane_damage_if_poisoned(state, info)
        }
        InternalAction::DealBodySlamDamage { source, target } => {
            damage_actions::deal_body_slam_damage(state, source, target)
        }
        InternalAction::DealHandOfGreedDamage { info, gold } => {
            damage_actions::deal_hand_of_greed_damage(state, info, gold)
        }
        InternalAction::DealDamageRandomEnemy { source, amount } => {
            damage_actions::deal_damage_random_enemy(state, source, amount)
        }
        InternalAction::DealDamageAndHealUnblocked { info } => {
            damage_actions::deal_damage_and_heal_unblocked(state, info)
        }
        InternalAction::DealFeedDamage { info, max_hp_gain } => {
            damage_actions::deal_feed_damage(state, info, max_hp_gain)
        }
        InternalAction::DealRitualDaggerDamage { info, growth } => {
            damage_actions::deal_ritual_dagger_damage(state, info, growth)
        }
        InternalAction::DealDamageAll { source, amount } => {
            damage_actions::deal_damage_all(state, source, amount)
        }
        InternalAction::FireBreathingDamage { amount } => {
            crate::combat::draw::apply_fire_breathing_damage(state, amount)?;
            Ok(Vec::new())
        }
        InternalAction::DealDamageAllRepeated {
            source,
            amount,
            times,
        } => damage_actions::deal_damage_all_repeated(state, source, amount, times),
        InternalAction::DealDamageAllAndHealUnblocked { source, amount } => {
            damage_actions::deal_damage_all_and_heal_unblocked(state, source, amount)
        }
        InternalAction::DealDamageAndGainBlockUnblocked { info } => {
            damage_actions::deal_damage_and_gain_block_unblocked(state, info)
        }
        InternalAction::DealSharpHideDamageToPlayer { amount }
        | InternalAction::DealThornsDamageToPlayer { amount } => {
            let hp_loss = reflect_spikes_to_player(&mut state.player, &state.relics, amount);
            crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
            Ok(Vec::new())
        }
        InternalAction::HealPlayer { amount } => defense_actions::heal_player(state, amount),
        InternalAction::GainBlock { amount } => defense_actions::gain_player_block(state, amount),
        InternalAction::GainPrecomputedCardBlock { amount } => {
            defense_actions::gain_precomputed_player_card_block(state, amount)
        }
        InternalAction::GainBlockDirect { amount } => {
            defense_actions::gain_player_block_direct(state, amount)
        }
        InternalAction::GainBlockFromExhaust { amount } => {
            defense_actions::gain_player_block_from_exhaust(state, amount)
        }
        InternalAction::GainMonsterBlock { target, amount } => {
            defense_actions::gain_monster_block(state, target, amount)
        }
        InternalAction::RerollWrithingMassAfterAttack { target } => {
            crate::combat::turn::reroll_writhing_mass_after_attack(state, target);
            Ok(Vec::new())
        }
        InternalAction::PreventBlockGain { turns } => {
            defense_actions::prevent_block_gain(state, turns)
        }
        InternalAction::GainTemporaryThorns { amount } => {
            defense_actions::gain_temporary_thorns(state, amount)
        }
        InternalAction::DoublePlayerBlock => defense_actions::double_player_block(state),
        InternalAction::ApplyVulnerable { target, amount } => {
            defense_actions::apply_monster_vulnerable(state, target, amount)
        }
        InternalAction::ApplyMark { target, amount } => {
            let applied = {
                let monster = living_monster_mut(state, target)?;
                crate::power::apply_monster_mark(&mut monster.powers, amount)?
            };
            Ok(
                sadistic_nature_follow_up_after_monster_debuff(state, target, applied)
                    .into_iter()
                    .collect(),
            )
        }
        InternalAction::ApplyPlayerVulnerable { amount } => {
            defense_actions::apply_player_vulnerable(state, amount)
        }
        InternalAction::ApplyWeak { target, amount } => {
            defense_actions::apply_weak(state, target, amount)
        }
        InternalAction::ApplyWeakIfTargetAttacking { target, amount } => {
            let attacking = state
                .monsters
                .iter()
                .find(|monster| monster.id == target && monster.alive)
                .is_some_and(|monster| card_effects::monster_intent_is_attack(monster.intent));
            if attacking {
                defense_actions::apply_weak(state, target, amount)
            } else {
                Ok(Vec::new())
            }
        }
        InternalAction::ApplyPoison { target, amount } => {
            defense_actions::apply_poison(state, target, amount)
        }
        InternalAction::TriggerMarks => damage_actions::trigger_marks(state),
        InternalAction::LoseMonsterHp { target, amount } => {
            damage_actions::lose_monster_hp(state, target, amount)
        }
        InternalAction::ReduceMonsterStrength { target, amount } => {
            defense_actions::reduce_strength(state, target, amount)
        }
        InternalAction::ReduceMonsterStrengthThisTurn { target, amount } => {
            defense_actions::reduce_strength_this_turn(state, target, amount)
        }
        InternalAction::MoveCard { card_id, from, to } => {
            // Pen Nib bonus ends when the doubled attack card leaves play.
            if state.card_in_use == Some(card_id) {
                state.pen_nib_double_active = false;
            }
            let to = apply_deferred_played_card_strange_spoon(state, card_id, to);
            pile_actions::move_card_between_piles(state, card_id, from, to)
        }
        InternalAction::ManualDiscardCard { card_id } => {
            pile_actions::manual_discard_card(state, card_id)
        }
        InternalAction::ReturnExhaustCardToHand { card_id } => {
            pile_actions::return_exhaust_card_to_hand(state, card_id)
        }
        InternalAction::ForethoughtAutoMove {
            source_card_id,
            card_id,
        } => pile_actions::forethought_auto_move(state, source_card_id, card_id),
        InternalAction::ExhaustAllNonAttackCards { excluded_card_id } => {
            pile_actions::exhaust_all_non_attack_cards(state, excluded_card_id)
        }
        InternalAction::ExhaustRandomHandCardExcept { excluded_card_id } => {
            let mut follow_ups =
                pile_actions::exhaust_random_hand_card_except(state, excluded_card_id)?;
            // A skipped hand-selection can leave source selectedCards outside
            // the visible hand. Random exhaust actions consume that residual
            // in the same batch (after the visible card, before the source
            // True Grit settles), rather than leaving it to a later END.
            let pending_hidden = std::mem::take(&mut state.pending_hidden_hand_card_until_end_turn);
            state.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
            for card in pending_hidden {
                let card_id = card.id;
                state.piles.exhaust_pile.push(card);
                apply_on_exhaust_effects_except_bot_queued_powers(state, card_id)?;
                follow_ups.extend(feel_no_pain_block_follow_up(state));
                follow_ups.extend(dead_branch_follow_up(state));
                follow_ups.extend(dark_embrace_then_necronomicurse_follow_ups(state, card_id)?);
            }
            Ok(follow_ups)
        }
        InternalAction::ResolveStormOfSteel {
            source_card_id,
            upgraded,
        } => pile_actions::resolve_storm_of_steel(state, source_card_id, upgraded),
        InternalAction::ResolveFiendFire {
            source_card_id,
            target,
            amount,
        } => damage_actions::resolve_fiend_fire(state, source_card_id, target, amount),
        InternalAction::RemoveCard { card_id, from } => {
            pile_actions::remove_card(state, card_id, from)
        }
        InternalAction::AddCardToPile { content_id, to } => {
            pile_actions::add_card(state, content_id, to)
        }
        InternalAction::AddGeneratedCardToPile {
            content_id,
            to,
            temp_cost,
            temp_cost_turn_only,
        } => {
            pile_actions::add_generated_card(state, content_id, to, temp_cost, temp_cost_turn_only)
        }
        InternalAction::AddGeneratedUpgradedCardToPile { content_id, to } => {
            pile_actions::add_generated_upgraded_card(state, content_id, to)
        }
        InternalAction::AddGeneratedCardsToHandWhileSourceInLimbo {
            content_id,
            source_card_id,
            count,
            temp_cost,
            temp_cost_turn_only,
        } => pile_actions::add_generated_cards_to_hand_while_source_in_limbo(
            state,
            content_id,
            source_card_id,
            count,
            temp_cost,
            temp_cost_turn_only,
        ),
        InternalAction::AddGeneratedHandCardBeforePendingDraw {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => pile_actions::add_generated_hand_card_before_pending_draw(
            state,
            content_id,
            temp_cost,
            temp_cost_turn_only,
        ),
        InternalAction::AddStatEquivalentCopyToPile { card, to } => {
            pile_actions::add_stat_equivalent_copy(state, card, to)
        }
        InternalAction::AddCardInstanceToHandOrDiscard { card } => {
            pile_actions::add_card_instance_to_hand_or_discard(state, card)
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { content_id } => {
            pile_actions::add_generated_card_to_random_draw_spot(state, content_id, None, false)
        }
        InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        } => pile_actions::add_generated_card_to_random_draw_spot(
            state,
            content_id,
            temp_cost,
            temp_cost_turn_only,
        ),
        InternalAction::AddRandomColorlessCardToHand { temp_cost, upgrade } => {
            pile_actions::add_random_colorless_card_to_hand(state, temp_cost, upgrade)
        }
        InternalAction::AddRandomColorlessCardsToHandWhileSourceInLimbo {
            source_card_id,
            count,
            temp_cost,
            upgrade,
        } => pile_actions::add_random_colorless_cards_to_hand_while_source_in_limbo(
            state,
            source_card_id,
            count,
            temp_cost,
            upgrade,
        ),
        InternalAction::DrawCards { count } => pile_actions::draw_cards(state, count),
        InternalAction::DrawCardsWithoutEvolve { count } => {
            pile_actions::draw_cards_without_evolve(state, count)
        }
        InternalAction::DrawCardsWhilePlayedCardIsInLimbo { card_id, count } => {
            pile_actions::draw_cards_while_played_card_is_in_limbo(state, card_id, count)
        }
        InternalAction::DrawCardsWhilePlayedCardIsInLimboWithoutEvolve { card_id, count } => {
            pile_actions::draw_cards_while_played_card_is_in_limbo_without_evolve(
                state, card_id, count,
            )
        }
        InternalAction::DrawCardsFromInkBottle { count } => pile_actions::draw_cards(state, count),
        InternalAction::ShuffleDiscardIntoDraw => pile_actions::shuffle_discard_into_draw(state),
        InternalAction::DeepBreathShuffleDiscardIntoDraw => {
            pile_actions::deep_breath_shuffle_discard_into_draw(state)
        }
        InternalAction::DrawCardsIfNoAttacksInHand { count } => {
            pile_actions::draw_cards_if_no_attacks_in_hand(state, count)
        }
        InternalAction::DrawThenScrapeDiscard { count } => {
            // Scrape's DrawCardAction runs while UseCardAction still holds the
            // source in cardInUse, same as Violence (FIDL01255).
            with_card_in_use_out_of_hand(state, |next| {
                pile_actions::draw_then_scrape_discard(next, count)
            })
        }
        InternalAction::DrawRandomAttacksFromDrawPile { count } => {
            // ViolenceAction runs while UseCardAction still holds the source in
            // cardInUse. Hand capacity must not count that card, or the third
            // attack overflows to discard (FIDL01255 Headbutt).
            with_card_in_use_out_of_hand(state, |next| {
                pile_actions::draw_random_attacks(next, count)
            })
        }
        InternalAction::GainEnergy { amount } => player_actions::gain_energy(state, amount),
        InternalAction::LoseEnergy { amount } => player_actions::lose_energy(state, amount),
        InternalAction::LoseHp { amount, source } => player_actions::lose_hp(state, amount, source),
        InternalAction::SetCannotDraw => player_actions::set_cannot_draw(state),
        InternalAction::ClearPlayerDebuffs => {
            state.player.remove_debuffs()?;
            Ok(Vec::new())
        }
        InternalAction::GainRage { amount } => player_actions::gain_rage(state, amount),
        InternalAction::SetRandomHandCardCostForCombat {
            amount,
            excluded_card_id,
        } => player_actions::set_random_hand_card_cost(state, amount, excluded_card_id),
        InternalAction::UpgradeHandCardsExcept { card_id } => {
            player_actions::upgrade_hand_cards_other_than(state, card_id)
        }
        InternalAction::UpgradeHandCard { card_id } => {
            player_actions::upgrade_one_hand_card(state, card_id)
        }
        InternalAction::IncreaseRampageDamage { card_id, amount } => {
            player_actions::increase_rampage_damage(state, card_id, amount)
        }
        InternalAction::ResolveSteamBarrier { card_id } => {
            player_actions::resolve_steam_barrier(state, card_id)
        }
        InternalAction::ResolveFollowUpEnergy { should_gain } => {
            player_actions::resolve_follow_up_energy(should_gain)
        }
        InternalAction::GainFeelNoPain { amount } => {
            player_actions::gain_feel_no_pain(state, amount)
        }
        InternalAction::GainDarkEmbrace { amount } => {
            player_actions::gain_dark_embrace(state, amount)
        }
        InternalAction::GainBarricade { amount } => player_actions::gain_barricade(state, amount),
        InternalAction::GainEvolve { amount } => player_actions::gain_evolve(state, amount),
        InternalAction::GainBerserk { amount } => player_actions::gain_berserk(state, amount),
        InternalAction::GainFasting { amount } => player_actions::gain_fasting(state, amount),
        InternalAction::GainLikeWater { amount } => player_actions::gain_like_water(state, amount),
        InternalAction::GainRupture { amount } => player_actions::gain_rupture(state, amount),
        InternalAction::GainJuggernaut { amount } => player_actions::gain_juggernaut(state, amount),
        InternalAction::GainBrutality { amount } => player_actions::gain_brutality(state, amount),
        InternalAction::GainMayhem { amount } => player_actions::gain_mayhem(state, amount),
        InternalAction::GainPanache { amount } => player_actions::gain_panache(state, amount),
        InternalAction::GainCombust { amount } => player_actions::gain_combust(state, amount),
        InternalAction::GainDoubleTap { amount } => player_actions::gain_double_tap(state, amount),
        InternalAction::GainFireBreathing { amount } => {
            player_actions::gain_fire_breathing(state, amount)
        }
        InternalAction::GainCorruption { amount } => player_actions::gain_corruption(state, amount),
        InternalAction::EnterDivinity => player_actions::enter_divinity(state),
        InternalAction::ApplyEndTurnDeath => player_actions::apply_end_turn_death(state),
        InternalAction::GainSadisticNature { amount } => {
            player_actions::gain_sadistic_nature(state, amount)
        }
        InternalAction::GainMagnetism { amount } => player_actions::gain_magnetism(state, amount),
        InternalAction::GainCreativeAI { amount } => {
            player_actions::gain_creative_ai(state, amount)
        }
        InternalAction::GainStorm { amount } => player_actions::gain_storm(state, amount),
        InternalAction::GainAfterImage { amount } => {
            player_actions::gain_after_image(state, amount)
        }
        InternalAction::GainThorns { amount } => player_actions::gain_thorns(state, amount),
        InternalAction::IncreaseMaxOrbs { amount } => {
            player_actions::increase_max_orbs(state, amount)
        }
        InternalAction::RecurseRightmostOrb => player_actions::recurse_rightmost_orb(state),
        InternalAction::ChannelLightning => player_actions::channel_lightning(state),
        InternalAction::ChannelFrost => player_actions::channel_frost(state),
        InternalAction::ChannelDark => player_actions::channel_dark(state),
        InternalAction::DarkImpulse => player_actions::dark_impulse(state),
        InternalAction::ForceEndTurn => {
            state.time_warp_end_turn = true;
            Ok(Vec::new())
        }
        InternalAction::SettleForcedEndTurn => {
            settle_time_warp_end_turn_if_ready(state)?;
            Ok(Vec::new())
        }
        InternalAction::ApplyGremlinHornOnDeath => {
            Ok(crate::relic::monster_death_relic_actions(state))
        }
        InternalAction::ExecuteJudgement { target, threshold } => {
            execute_judgement(state, target, threshold)
        }
        InternalAction::GainStaticDischarge { amount } => {
            player_actions::gain_static_discharge(state, amount)
        }
        InternalAction::LightningOrbPassive => player_actions::lightning_orb_passive(state),
        InternalAction::ArmTheBomb { turns, damage } => {
            player_actions::arm_the_bomb(state, turns, damage)
        }
        InternalAction::DealUnmodifiedDamage { target, amount } => {
            if state.pending_letter_opener_blasts > 0 {
                state.pending_letter_opener_blasts -= 1;
            }
            deal_unmodified_damage_to_living_monster(state, target, amount)?;
            Ok(Vec::new())
        }
        InternalAction::DealUnmodifiedDamageRandom { amount } => {
            apply_juggernaut_random_damage(state, amount)?;
            Ok(Vec::new())
        }
        InternalAction::GainMetallicize { amount } => {
            player_actions::gain_metallicize(state, amount)
        }
        InternalAction::GainStrength { amount } => player_actions::gain_strength(state, amount),
        InternalAction::GainMantra { amount } => player_actions::gain_mantra(state, amount),
        InternalAction::EnterCalm => player_actions::enter_calm(state),
        InternalAction::EnterWrath => player_actions::enter_wrath(state),
        InternalAction::ExitCalm => player_actions::enter_neutral(state),
        InternalAction::DiscardToHand { card_id } => pile_actions::discard_to_hand(state, card_id),
        InternalAction::GainDexterity { amount } => player_actions::gain_dexterity(state, amount),
        InternalAction::GainTempStrength { amount } => {
            player_actions::gain_temp_strength(state, amount)
        }
        InternalAction::GainIntangible { amount } => player_actions::gain_intangible(state, amount),
        InternalAction::GainRitual { amount } => player_actions::gain_ritual(state, amount),
        InternalAction::GainArtifact { amount } => player_actions::gain_artifact(state, amount),
        InternalAction::UpgradeCombatCards => player_actions::upgrade_all_combat_cards(state),
        InternalAction::UnceasingTopDraw => {
            if state.piles.hand.is_empty() {
                player_draw_cards(state, crate::relic::UNCEASING_TOP_DRAW)?;
            }
            Ok(Vec::new())
        }
        InternalAction::CardExhausted { card_id } => {
            // Feel No Pain and Dark Embrace both queue via addToBot on exhaust
            // (after remaining card.use / onUseCard / UseCardAction settlement).
            // Immediate DE draws would reshuffle before the played card reaches
            // discard (Sever Soul + empty draw), desyncing hand/draw order.
            // Havoc/Mayhem PlayTop exhausts the forced card after use(). Horn
            // onDeath is addToBot during that use(), before onExhaust Dead
            // Branch (FIDL01520 Bite: Madness then Perfected Strike). Do not
            // flush on other CardExhausted (Fiend Fire's own MoveCard exhaust
            // still has Horn pending from its hits — FIDL01434).
            if state.play_top_force_exhaust_active && state.card_in_use == Some(card_id) {
                flush_pending_monster_death_relics_if_ready(state)?;
            }
            apply_on_exhaust_effects_except_bot_queued_powers(state, card_id)?;
            let mut follow_ups = feel_no_pain_block_follow_up(state);
            follow_ups.extend(dead_branch_follow_up(state));
            follow_ups.extend(dark_embrace_then_necronomicurse_follow_ups(state, card_id)?);
            Ok(follow_ups)
        }
        InternalAction::HandCardExhausted { card_id } => {
            if state.play_top_force_exhaust_active && state.card_in_use == Some(card_id) {
                flush_pending_monster_death_relics_if_ready(state)?;
            }
            apply_on_exhaust_effects_except_bot_queued_powers(state, card_id)?;
            let mut follow_ups = feel_no_pain_block_follow_up(state);
            follow_ups.extend(dead_branch_follow_up_before_pending_draw(state));
            follow_ups.extend(dark_embrace_then_necronomicurse_follow_ups(state, card_id)?);
            Ok(follow_ups)
        }
        InternalAction::PlayTopDrawCard {
            target,
            exhaust_played_card,
            random_living_target,
        } => apply_play_top_draw_card(state, target, exhaust_played_card, random_living_target),
        InternalAction::ResolveTopDrawCard {
            card_id,
            target,
            exhaust_played_card,
        } => resolve_top_draw_card(state, card_id, target, exhaust_played_card),
        InternalAction::EndPlayTopCardResolution {
            card_id,
            deferred_destination,
            previous_card_in_use,
            previous_force_exhaust,
        } => end_play_top_card_resolution(
            state,
            card_id,
            deferred_destination,
            previous_card_in_use,
            previous_force_exhaust,
        ),
        InternalAction::PutHandCardOnTopOfDraw { card_id } => {
            let card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
            state.piles.draw_pile.insert(0, card);
            Ok(Vec::new())
        }
        InternalAction::CopyHandCardToHand { card_id } => {
            let card = find_hand_card(state, card_id)?;
            let next_id = CardId::new(state.next_card_instance_id()?);
            state
                .piles
                .draw_pile
                .push(CardInstance::new(next_id, card.content_id));
            Ok(Vec::new())
        }
        InternalAction::AwaitHandSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_hand_select(state, source_card_id, purpose),
        InternalAction::AwaitDrawSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_draw_select(state, source_card_id, purpose),
        InternalAction::AwaitDiscardSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_discard_select(state, source_card_id, purpose),
        InternalAction::AwaitCopiedDiscardSelect { purpose } => {
            decision_actions::await_copied_discard_select(state, purpose)
        }
        InternalAction::AwaitExhaustSelect {
            source_card_id,
            purpose,
        } => decision_actions::await_exhaust_select(state, source_card_id, purpose),
        InternalAction::OpenDiscoveryCardReward { source_card_id } => {
            decision_actions::open_discovery_card_reward(state, source_card_id)
        }
    }
}

fn apply_mummified_hand_on_power_play(
    state: &mut CombatState,
    played_card_id: CardId,
    card_type: CardType,
) -> SimResult<()> {
    if card_type != CardType::Power || !state.relics.contains(&Relic::MummifiedHand) {
        return Ok(());
    }

    // Mummified Hand's on-use hook observes the hand after a newly played
    // Power's cost-changing effect has taken effect. In particular, playing
    // Corruption leaves only non-Skills eligible for the relic's random pick.
    let corruption_active = state.player.powers.corruption > 0
        || state
            .piles
            .hand
            .iter()
            .find(|card| card.id == played_card_id)
            .is_some_and(|card| matches!(card.content_id, CORRUPTION_ID | CORRUPTION_PLUS_ID));
    let candidates = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            if card.id == played_card_id {
                return None;
            }
            let definition = get_card_definition(card.content_id)?;
            let cost_for_turn = effective_card_cost(card).ok()?;
            let corruption_zeroed = corruption_active && definition.card_type == CardType::Skill;
            // A Forethought card marked freeToPlayOnce is not an eligible
            // Mummified Hand target even while its displayed costForTurn is
            // still positive. The target leaves that hidden flag intact until
            // the card is played (FIDL01748), so do not spend the random pick
            // on a card that is already free.
            // STS otherwise filters on costForTurn > 0 (not base cost), so
            // X-cost cards with a positive turn cost remain eligible.
            (cost_for_turn > 0 && !card.free_to_play_once && !corruption_zeroed).then_some(index)
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Ok(());
    }

    let pick = state
        .rng
        .card_random_rng
        .random_int_range(0, (candidates.len() - 1) as i32) as usize;
    let card = &mut state.piles.hand[candidates[pick]];
    crate::combat::cost::set_card_cost_for_turn(card, 0)
}

fn apply_deferred_time_warp_card_play(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    let mut triggered = false;
    for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
        if monster.content_id == crate::content::monsters::TIME_EATER_ID {
            checked_add_combat_value(&mut monster.powers.time_warp, 1)?;
            if monster.powers.time_warp >= 12 {
                monster.powers.time_warp = 0;
                triggered = true;
            }
        }
    }
    if triggered {
        state.time_warp_end_turn = true;
        for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
            checked_add_combat_value(&mut monster.powers.strength, 2)?;
        }
    }
    Ok(Vec::new())
}

fn apply_on_card_play_powers(
    state: &mut CombatState,
    card_type: CardType,
    defer_time_warp: bool,
) -> SimResult<Vec<InternalAction>> {
    let mut follow_ups = Vec::new();

    let mut time_warp_triggered = false;
    for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
        if monster.content_id == GIANT_HEAD_ID || monster.powers.slow > 0 {
            checked_add_combat_value(&mut monster.powers.slow, 1)?;
        }
        if !defer_time_warp && monster.content_id == crate::content::monsters::TIME_EATER_ID {
            checked_add_combat_value(&mut monster.powers.time_warp, 1)?;
            if monster.powers.time_warp >= 12 {
                monster.powers.time_warp = 0;
                time_warp_triggered = true;
            }
        }
        if card_type == CardType::Power
            && monster.content_id == AWAKENED_ONE_ID
            && monster.mode_shift == 0
        {
            let curiosity = if state.ascension >= 19 { 2 } else { 1 };
            checked_add_combat_value(&mut monster.powers.strength, curiosity)?;
        }
    }
    if defer_time_warp {
        follow_ups.push(InternalAction::ApplyDeferredTimeWarpCardPlay);
    }
    if time_warp_triggered {
        state.time_warp_end_turn = true;
        for monster in state.monsters.iter_mut().filter(|monster| monster.alive) {
            // Time Warp's source power applies +2 Strength and calls the early
            // end-turn sequence; it does not grant monster block.
            checked_add_combat_value(&mut monster.powers.strength, 2)?;
        }
    }

    if card_type == CardType::Attack {
        // SharpHidePower.onUseCard addToBot's thorns DamageAction after card.use().
        // Queue it as a PlayCard follow-up (push_back) so exhaust/Feel No Pain
        // addToBot block from the same card resolves after Sharp Hide.
        let sharp_hide_damage: i32 = state
            .monsters
            .iter()
            .filter(|monster| {
                monster.alive && monster.content_id == GUARDIAN_ID && monster.powers.spikes > 0
            })
            .map(|monster| monster.powers.spikes)
            .try_fold(0, checked_combat_sum)?;
        if sharp_hide_damage > 0 {
            follow_ups.push(InternalAction::DealSharpHideDamageToPlayer {
                amount: sharp_hide_damage,
            });
        }
    }

    if state.player.powers.after_image > 0 {
        // AfterImagePower.onUseCard addToBot GainBlockAction from useCard,
        // before UseCardAction. Beat of Death is onAfterUseCard at the start
        // of UseCardAction, so the block is already queued (FIDL02358 Bash+
        // absorbs 1 of 2 Beat of Death).
        follow_ups.push(InternalAction::GainBlockDirect {
            amount: state.player.powers.after_image,
        });
    }

    // BeatOfDeathPower.onAfterUseCard addToBot's THORNS DamageAction at the
    // start of UseCardAction, before that action exhausts the card. Feel No
    // Pain's onExhaust GainBlock is therefore still behind this hit.
    let beat_of_death: i32 = state
        .monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && monster.content_id == crate::content::monsters::CORRUPT_HEART_ID
                && monster.powers.beat_of_death > 0
        })
        .map(|monster| monster.powers.beat_of_death)
        .try_fold(0, checked_combat_sum)?;
    if beat_of_death > 0 {
        follow_ups.push(InternalAction::DealThornsDamageToPlayer {
            amount: beat_of_death,
        });
    }

    if card_type == CardType::Power && state.player.powers.storm > 0 {
        for _ in 0..state.player.powers.storm {
            follow_ups.push(InternalAction::ChannelLightning);
        }
    }

    if state.player.powers.hex > 0 && card_type != CardType::Attack {
        for _ in 0..state.player.powers.hex {
            follow_ups.push(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: DAZED_ID,
            });
        }
    }

    if state.player.powers.panache <= 0 {
        return Ok(follow_ups);
    }
    checked_add_combat_value(&mut state.player.powers.panache_cards_played, 1)?;
    if state.player.powers.panache_cards_played < 5 {
        return Ok(follow_ups);
    }

    state.player.powers.panache_cards_played = 0;
    let amount = state.player.powers.panache;
    follow_ups.extend(
        state
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| InternalAction::DealUnmodifiedDamage {
                target: monster.id,
                amount,
            }),
    );
    Ok(follow_ups)
}

fn apply_hand_card_play_triggers(
    state: &CombatState,
    played_card_id: CardId,
) -> Vec<InternalAction> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != played_card_id && card.content_id == PAIN_ID)
        .map(|card| InternalAction::LoseHp {
            amount: 1,
            source: HpLossSource::Card(card.id),
        })
        .collect()
}

fn apply_copied_card_play_triggers(state: &CombatState) -> Vec<InternalAction> {
    state
        .piles
        .hand
        .iter()
        .filter(|card| card.content_id == PAIN_ID)
        .map(|card| InternalAction::LoseHp {
            amount: 1,
            source: HpLossSource::Card(card.id),
        })
        .collect()
}

fn deal_attack_damage_to_all_living(
    state: &mut CombatState,
    source: CardId,
    amount: i32,
) -> SimResult<(i32, Vec<InternalAction>)> {
    let player_powers = state.player.powers;
    let temp_strength = state.player.temp_strength;
    let relics = state.relics.clone();
    let targets: Vec<(MonsterId, ContentId, i32)> = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| (monster.id, monster.content_id, monster.powers.spikes))
        .collect();
    let mut total_hp_damage = 0;
    let mut follow_ups = Vec::new();
    let mut defeated_targets = Vec::new();

    for (target, monster_content_id, spikes) in targets {
        // DamageAllEnemiesAction completes its indexed damage loop before the
        // queued SuicideActions from Reptomancer.die run. Preserve the living
        // target snapshot so later Daggers are still hit by the same action.
        let Some(monster) = living_monster_mut_opt(state, target) else {
            continue;
        };
        let (hp_damage, still_alive, hand_drill_applies, curl_up_block, malleable_block) = {
            let damage = deal_damage_info_to_monster_with_result(
                monster,
                DamageInfo {
                    source: DamageSource::Card(source),
                    target,
                    amount,
                },
                player_powers,
                temp_strength,
                &relics,
            );
            wake_lagavulin_on_damage(monster, damage.hp_damage);
            guardian_accumulate_hp_damage(monster, damage.hp_damage);
            (
                damage.hp_damage,
                monster.alive,
                relics.contains(&crate::Relic::HandDrill) && damage.broke_block,
                damage.curl_up_block,
                damage.malleable_block,
            )
        };
        push_attack_block_follow_ups(
            state,
            &mut follow_ups,
            target,
            monster_content_id,
            still_alive,
            curl_up_block,
            malleable_block,
        );
        if still_alive && hand_drill_applies {
            follow_ups.extend(apply_player_vulnerable_debuff(
                state,
                target,
                crate::relic::HAND_DRILL_VULNERABLE,
                true,
            )?);
        }
        total_hp_damage += hp_damage;
        check_slime_boss_split(state, target);
        if !still_alive {
            defeated_targets.push(target);
        }
        apply_or_queue_spikes_to_player(state, monster_content_id, spikes)?;
    }

    for target in defeated_targets {
        // Death callbacks are queued after the complete all-enemy damage loop.
        // Gremlin Horn still resolves after the card's queued MoveCard action,
        // letting an empty-draw-pile shuffle include the lethal source card.
        follow_ups.extend(queue_monster_death_hooks(state, target)?);
    }

    Ok((total_hp_damage, follow_ups))
}

fn apply_or_queue_spikes_to_player(
    state: &mut CombatState,
    monster_content_id: ContentId,
    spikes: i32,
) -> SimResult<()> {
    if spikes <= 0 {
        return Ok(());
    }
    if monster_content_id == GUARDIAN_ID {
        return Ok(());
    }
    let hp_loss = reflect_spikes_to_player(&mut state.player, &state.relics, spikes);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)
}

fn push_attack_block_follow_ups(
    state: &mut CombatState,
    follow_ups: &mut Vec<InternalAction>,
    target: MonsterId,
    monster_content_id: ContentId,
    still_alive: bool,
    curl_up_block: Option<i32>,
    malleable_block: Option<i32>,
) {
    if still_alive
        && malleable_block.is_some()
        && monster_content_id == crate::content::monsters::WRITHING_MASS_ID
    {
        follow_ups.push(InternalAction::RerollWrithingMassAfterAttack { target });
    }
    if still_alive {
        // Only delay Malleable/Curl Up when Letter Opener blasts are already on
        // the parent queue (FIDL00428). Plain Havoc PlayTop keeps immediate
        // Malleable so multi-hit / Dead Branch ordering stays stable.
        let defer_monster_block =
            state.play_top_resolving_depth > 0 && state.pending_letter_opener_blasts > 0;
        if let Some(amount) = curl_up_block {
            if defer_monster_block {
                state
                    .deferred_play_top_monster_blocks
                    .push((target, amount));
            } else {
                follow_ups.push(InternalAction::GainMonsterBlock { target, amount });
            }
        }
        if let Some(amount) = malleable_block {
            if defer_monster_block {
                state
                    .deferred_play_top_monster_blocks
                    .push((target, amount));
            } else {
                follow_ups.push(InternalAction::GainMonsterBlock { target, amount });
            }
        }
    }
}

fn deal_unmodified_damage_to_living_monster(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<()> {
    // Card-play powers such as Panache queue one damage action per target
    // before the played card's own damage resolves. A lethal card hit can
    // therefore make one of those queued targets disappear before its action
    // runs; the target game's all-enemy action simply skips that dead monster.
    if !state
        .monsters
        .iter()
        .any(|monster| monster.id == target && monster.alive)
    {
        return Ok(());
    }
    let still_alive = {
        let monster = living_monster_mut(state, target)?;
        let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
        wake_lagavulin_on_damage(monster, hp_damage);
        monster.alive
    };
    check_slime_boss_split(state, target);
    if !still_alive {
        apply_monster_death_hooks(state, target)?;
    }
    Ok(())
}

/// Combat continues after a death when another monster is alive or Awakened One
/// is in the half-dead phase (FIDL00378 Gremlin Horn on first form kill).
fn combat_continues_after_monster_death(state: &CombatState) -> bool {
    state
        .monsters
        .iter()
        .any(|monster| monster.alive || awakened_one_is_half_dead(monster))
}

fn apply_monster_death_hooks_deferred_relics(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let mut next = state.clone();
    apply_monster_death_non_relic_hooks(&mut next, monster_id)?;
    if combat_continues_after_monster_death(&next) && next.relics.contains(&Relic::GremlinHorn) {
        next.pending_monster_death_relic_triggers = next
            .pending_monster_death_relic_triggers
            .checked_add(1)
            .ok_or(SimError::InvalidState(
                "pending monster-death relic triggers overflow u32",
            ))?;
    }
    *state = next;
    Ok(())
}

pub(crate) fn apply_monster_death_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let mut next = state.clone();
    apply_monster_death_non_relic_hooks(&mut next, monster_id)?;
    if combat_continues_after_monster_death(&next) {
        crate::relic::apply_monster_death_relics(&mut next)?;
    }
    *state = next;
    Ok(())
}

pub(crate) fn queue_monster_death_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<Vec<InternalAction>> {
    let stasis_card = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
        .and_then(|monster| monster.stasis_card.take());
    apply_monster_death_non_stasis_hooks(state, monster_id)?;
    let mut follow_ups = stasis_card
        .into_iter()
        .map(|card| InternalAction::AddCardInstanceToHandOrDiscard { card })
        .collect::<Vec<_>>();
    // GremlinHorn.onMonsterDeath addToBots GainEnergy + Draw at the death
    // inside DamageAll / the lethal hit, after the already-queued UseCardAction
    // and before a simulator SettleForcedEndTurn artifact.
    if combat_continues_after_monster_death(state) && state.relics.contains(&Relic::GremlinHorn) {
        follow_ups.push(InternalAction::ApplyGremlinHornOnDeath);
    }
    Ok(follow_ups)
}

/// Queue one end-turn-power death in the same order as the target action
/// manager. End-turn damage resolves before DiscardAtEndOfTurnAction, but the
/// Stasis release and Gremlin Horn callbacks run after that discard. Keeping
/// the pair together is important when multiple monsters die in one damage
/// action: the callbacks interleave as Stasis, Horn, Stasis, Horn.
pub(crate) fn queue_end_turn_monster_death(
    state: &mut CombatState,
    monster_id: MonsterId,
    deferred: &mut Vec<DeferredMonsterDeath>,
) -> SimResult<()> {
    let stasis_card = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
        .and_then(|monster| monster.stasis_card.take());
    apply_monster_death_non_stasis_hooks(state, monster_id)?;
    let gremlin_horn =
        combat_continues_after_monster_death(state) && state.relics.contains(&Relic::GremlinHorn);
    if combat_continues_after_monster_death(state) {
        deferred.push(DeferredMonsterDeath {
            stasis_card,
            gremlin_horn,
        });
    } else if let Some(card) = stasis_card {
        release_deferred_stasis_card(state, card);
    }
    Ok(())
}

fn release_deferred_stasis_card(state: &mut CombatState, card: CardInstance) {
    if state.piles.hand.len() < MAX_HAND_SIZE {
        state.piles.hand.push(card);
    } else {
        state.piles.discard_pile.push(card);
    }
}

pub(crate) fn resolve_deferred_end_turn_monster_deaths(
    state: &mut CombatState,
    deferred: Vec<DeferredMonsterDeath>,
) -> SimResult<()> {
    let mut queue = VecDeque::new();
    for death in deferred {
        if let Some(card) = death.stasis_card {
            queue.push_back(InternalAction::AddCardInstanceToHandOrDiscard { card });
        }
        if death.gremlin_horn {
            queue.push_back(InternalAction::ApplyGremlinHornOnDeath);
        }
    }
    if queue.is_empty() {
        return Ok(());
    }
    let transition = process_internal_queue(state, queue)?;
    *state = transition.state;
    Ok(())
}

fn apply_monster_death_non_relic_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
    {
        release_stasis_card_on_death(monster, &mut state.piles);
    }
    apply_monster_death_non_stasis_hooks(state, monster_id)
}

fn apply_monster_death_non_stasis_hooks(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let dead_monster_content_id = state
        .monsters
        .iter()
        .find(|monster| monster.id == monster_id)
        .map(|monster| monster.content_id);
    if dead_monster_content_id == Some(crate::content::monsters::MUGGER_ID) {
        // Mugger.playDeathSfx consumes one inclusive aiRng.random(2) draw.
        // Looter uses process-global MathUtils for its death voice instead.
        let _ = state.rng.monster_rng.random_int(2);
    }
    let ended_surrounded = dead_monster_content_id.is_some_and(|content_id| {
        matches!(
            content_id,
            crate::content::monsters::SPIRE_SHIELD_ID | crate::content::monsters::SPIRE_SPEAR_ID
        )
    });
    if ended_surrounded {
        // AbstractMonster.die removes Surrounded from the player and
        // BackAttackPower from the surviving Act 4 elite.
        for monster in &mut state.monsters {
            monster.back_attack = false;
        }
    }
    apply_gremlin_leader_death_escape(&mut state.monsters, monster_id);
    apply_reptomancer_death_escape(&mut state.monsters, monster_id);
    apply_collector_death_escape(&mut state.monsters, monster_id);
    apply_spore_cloud_on_monster_death(state, monster_id)
}

fn apply_spore_cloud_on_monster_death(
    state: &mut CombatState,
    monster_id: MonsterId,
) -> SimResult<()> {
    let amount = state
        .monsters
        .iter()
        .find(|monster| monster.id == monster_id)
        .map_or(0, |monster| monster.powers.spore_cloud);
    if amount <= 0 || !state.monsters.iter().any(|monster| monster.alive) {
        return Ok(());
    }

    // A Fungi Beast killed by reactive thorns during the monster phase queues
    // its Spore Cloud Vulnerable after the attack's damage. It therefore must
    // survive this phase's cleanup even when the thorns are persistent
    // (Bronze Scales), not only temporary (Flame Barrier). Player-turn kills
    // and stacked Vulnerable applications retain their ordinary duration.
    let had_no_vulnerable = state.player.powers.vulnerable == 0;
    let applied = apply_player_vulnerable(&mut state.player.powers, amount)?;
    if applied
        && had_no_vulnerable
        && state.phase == crate::combat::state::CombatPhase::MonsterTurn
        && (state.player.temp_thorns > 0 || state.player.powers.thorns > 0)
    {
        state.player.vulnerable_just_applied = true;
    }
    Ok(())
}

/// Sadistic NaturePower.onApplyPower addToBot's DamageAction, so it must land
/// behind same-card Malleable/Curl Up bot block (FIDL00242 Bash → Malleable then
/// Sadistic eats the block).
pub(super) fn sadistic_nature_follow_up_after_monster_debuff(
    state: &CombatState,
    target: MonsterId,
    applied: bool,
) -> Option<InternalAction> {
    if !applied || state.player.powers.sadistic_nature <= 0 {
        return None;
    }
    Some(InternalAction::DealUnmodifiedDamage {
        target,
        amount: state.player.powers.sadistic_nature,
    })
}

pub(super) fn apply_player_vulnerable_debuff(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
    preserve_vulnerable_through_end_turn: bool,
) -> SimResult<Vec<InternalAction>> {
    let applies_champion_belt = state.relics.contains(&crate::Relic::ChampionBelt);
    let mut vulnerable_applied = false;
    let mut champion_belt_weak_applied = false;
    if let Some(monster) = living_monster_mut_opt(state, target) {
        let mut next_powers = monster.powers;
        vulnerable_applied = apply_monster_vulnerable(&mut next_powers, amount)?;
        if vulnerable_applied && applies_champion_belt {
            champion_belt_weak_applied =
                apply_monster_weak(&mut next_powers, crate::relic::CHAMPION_BELT_WEAK)?;
        }
        monster.vulnerable_just_applied =
            preserve_vulnerable_through_end_turn && vulnerable_applied;
        monster.powers = next_powers;
    }

    let mut follow_ups = Vec::new();
    if let Some(action) =
        sadistic_nature_follow_up_after_monster_debuff(state, target, vulnerable_applied)
    {
        follow_ups.push(action);
    }
    if let Some(action) =
        sadistic_nature_follow_up_after_monster_debuff(state, target, champion_belt_weak_applied)
    {
        follow_ups.push(action);
    }
    Ok(follow_ups)
}

fn juggernaut_follow_up_for_positive_block_gain(
    state: &mut CombatState,
    gained: i32,
) -> Vec<InternalAction> {
    if gained <= 0 || state.player.powers.juggernaut <= 0 {
        return Vec::new();
    }
    vec![InternalAction::DealUnmodifiedDamageRandom {
        amount: state.player.powers.juggernaut,
    }]
}

pub(crate) fn apply_juggernaut_after_direct_block_gain(
    state: &mut CombatState,
    gained: i32,
) -> SimResult<()> {
    if let Some(InternalAction::DealUnmodifiedDamageRandom { amount }) =
        juggernaut_follow_up_for_positive_block_gain(state, gained)
            .into_iter()
            .next()
    {
        apply_juggernaut_random_damage(state, amount)?;
    }
    Ok(())
}

fn execute_judgement(
    state: &mut CombatState,
    target: MonsterId,
    threshold: i32,
) -> SimResult<Vec<InternalAction>> {
    let Some(monster) = living_monster_mut_opt(state, target) else {
        return Ok(Vec::new());
    };
    if monster.hp > threshold {
        return Ok(Vec::new());
    }
    // InstantKillAction: currentHealth = 0, then damage(HP_LOSS, 0).
    // Darkling.damage still enters the first-death COUNT / Life Link pose.
    monster.hp = 0;
    crate::combat::damage::deal_hp_loss_damage_to_monster(monster, 0);
    apply_monster_death_hooks(state, target)?;
    Ok(Vec::new())
}

pub(crate) fn apply_orb_end_of_turn_passives(state: &mut CombatState) -> SimResult<()> {
    player_actions::apply_orb_end_of_turn_passives(state)
}

/// StaticDischargePower.onAttacked: Channel Lightning `amount` times when the
/// player takes unblocked attack damage (not Thorns / HP_LOSS).
pub(crate) fn apply_static_discharge_on_attacked(
    state: &mut CombatState,
    hp_damage: i32,
) -> SimResult<()> {
    if hp_damage <= 0 || state.player.powers.static_discharge <= 0 {
        return Ok(());
    }
    for _ in 0..state.player.powers.static_discharge {
        player_actions::channel_lightning(state)?;
    }
    Ok(())
}

pub(crate) fn apply_juggernaut_random_damage(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    let Some(target) = random_living_monster_id(state) else {
        return Ok(());
    };
    let hand_drill = state.relics.contains(&crate::Relic::HandDrill);
    let (still_alive, broke_block) = {
        let monster = living_monster_mut(state, target)?;
        let block_before = monster.block;
        let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
        wake_lagavulin_on_damage(monster, hp_damage);
        (monster.alive, block_before > 0 && monster.block == 0)
    };
    if still_alive && hand_drill && broke_block {
        let relics = state.relics.clone();
        if let Some(monster) = living_monster_mut_opt(state, target) {
            let mut powers = monster.powers;
            crate::relic::apply_monster_vulnerable_with_relics(
                &mut powers,
                &relics,
                crate::relic::HAND_DRILL_VULNERABLE,
            )?;
            monster.vulnerable_just_applied = true;
            monster.powers = powers;
        }
    }
    check_slime_boss_split(state, target);
    if !still_alive {
        apply_monster_death_hooks(state, target)?;
    }
    Ok(())
}

fn apply_player_card_block_gain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if state.player.no_block_turns > 0 {
        return Ok(Vec::new());
    }
    let gained = calculate_block(amount, state.player.powers);
    checked_add_combat_value(&mut state.player.block, gained)?;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, gained))
}

pub(crate) fn apply_player_direct_block_gain_without_juggernaut(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    // Direct callbacks such as Rage and Captain's Wheel bypass No Block; the
    // ordinary card GainBlock path remains suppressed by that power.
    // The target runtime uses signed 32-bit arithmetic. Authoritative combat
    // transitions validate that block remains nonnegative before returning.
    state.player.block = state.player.block.wrapping_add(amount);
    Ok(())
}

pub(crate) fn apply_player_direct_block_gain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    apply_player_direct_block_gain_without_juggernaut(state, amount)?;
    apply_juggernaut_after_direct_block_gain(state, amount)
}

/// End-of-turn Metallicize/Plated Armor gains resolve from their own power
/// callbacks before No Block Power expires. Unlike card and relic gains, these
/// automatic callbacks are not suppressed by No Block.
pub(crate) fn apply_player_end_turn_automatic_block_gain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<()> {
    // Metallicize and Plated Armor call Player.gainBlock directly from their
    // power hooks; Frail modifies card block, not these automatic callbacks.
    let gained = amount.max(0);
    checked_add_combat_value(&mut state.player.block, gained)?;
    apply_juggernaut_after_direct_block_gain(state, gained)
}

fn random_living_monster_id(state: &mut CombatState) -> Option<MonsterId> {
    let living: Vec<_> = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect();
    if living.is_empty() {
        return None;
    }
    let index = state
        .rng
        .card_random_rng
        .random_int((living.len() - 1) as i32) as usize;
    living.get(index).copied()
}

fn random_hand_card_id_except(state: &mut CombatState, excluded_card_id: CardId) -> Option<CardId> {
    let candidates = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != excluded_card_id)
        .map(|card| card.id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let index = state
        .rng
        .card_random_rng
        .random_int((candidates.len() - 1) as i32) as usize;
    candidates.get(index).copied()
}

pub(crate) fn reserve_dead_branch_card_content(state: &mut CombatState) -> Option<ContentId> {
    if !state.relics.contains(&Relic::DeadBranch) || !combat_continues_after_monster_death(state) {
        return None;
    }

    let pool = dead_branch_card_pool();
    let index = state
        .rng
        .card_random_rng
        .random_int((pool.len() - 1) as i32) as usize;
    Some(pool[index])
}

fn dead_branch_follow_up(state: &mut CombatState) -> Option<InternalAction> {
    let content_id = reserve_dead_branch_card_content(state)?;
    Some(InternalAction::AddGeneratedCardToPile {
        content_id,
        to: CardPile::Hand,
        temp_cost: None,
        temp_cost_turn_only: false,
    })
}

fn dead_branch_follow_up_before_pending_draw(state: &mut CombatState) -> Option<InternalAction> {
    match dead_branch_follow_up(state) {
        Some(InternalAction::AddGeneratedCardToPile {
            content_id,
            to: CardPile::Hand,
            temp_cost,
            temp_cost_turn_only,
        }) => Some(InternalAction::AddGeneratedHandCardBeforePendingDraw {
            content_id,
            temp_cost,
            temp_cost_turn_only,
        }),
        other => other,
    }
}

fn dead_branch_card_pool() -> Vec<ContentId> {
    // returnTrulyRandomCardInCombat filters HEALING-tagged cards (Feed/Reaper).
    ironclad_combat_discovery_pool().to_vec()
}

pub(crate) fn apply_on_exhaust_effects(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    apply_on_exhaust_effects_inner(state, card_id, true, true, false).map(|_| ())
}

pub(crate) fn apply_on_exhaust_effects_for_end_turn(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Option<i32>> {
    apply_on_exhaust_effects_inner(state, card_id, false, true, true)
}

fn apply_on_exhaust_effects_except_bot_queued_powers(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<()> {
    // Skip Feel No Pain and Dark Embrace: both use addToBot in the target
    // (`GainBlockAction` / `DrawCardAction`) and are emitted as follow-ups.
    apply_on_exhaust_effects_inner(state, card_id, false, false, false).map(|_| ())
}

/// STS FeelNoPainPower.onExhaust addToBot's GainBlockAction.
fn feel_no_pain_block_follow_up(state: &CombatState) -> Vec<InternalAction> {
    if state.player.powers.feel_no_pain > 0 {
        vec![InternalAction::GainBlockFromExhaust {
            amount: state.player.powers.feel_no_pain,
        }]
    } else {
        Vec::new()
    }
}

/// STS DarkEmbracePower.onExhaust addToBot's DrawCardAction.
fn dark_embrace_draw_follow_up(state: &CombatState) -> Vec<InternalAction> {
    if state.player.powers.dark_embrace > 0 {
        vec![InternalAction::DrawCards {
            count: state.player.powers.dark_embrace.max(0) as usize,
        }]
    } else {
        Vec::new()
    }
}

/// STS Necronomicurse.triggerOnExhaust queues MakeTempCardInHandAction after
/// relic/power `onExhaust` bot actions (Dark Embrace DrawCardAction first).
pub(crate) fn necronomicurse_replacement_follow_up(
    state: &CombatState,
    card_id: CardId,
) -> Vec<InternalAction> {
    if exhausted_card_content_id(state, card_id) != Some(NECRONOMICURSE_ID) {
        return Vec::new();
    }
    vec![InternalAction::AddGeneratedCardToPile {
        content_id: NECRONOMICURSE_ID,
        to: CardPile::Hand,
        temp_cost: None,
        temp_cost_turn_only: false,
    }]
}

fn apply_necronomicurse_replacement(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    if exhausted_card_content_id(state, card_id) != Some(NECRONOMICURSE_ID) {
        return Ok(());
    }
    add_generated_card_to_pile(state, NECRONOMICURSE_ID, CardPile::Hand, None, false)
}

/// Dark Embrace `DrawCardAction` before Necronomicurse `MakeTempCardInHand`.
/// With no Dark Embrace, apply the replacement immediately so ExhaustAll
/// batches (Sever Soul) still see it before later queue actions (FIDL01511
/// PLAY 1190).
pub(crate) fn dark_embrace_then_necronomicurse_follow_ups(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let draw = dark_embrace_draw_follow_up(state);
    if draw.is_empty() {
        apply_necronomicurse_replacement(state, card_id)?;
        Ok(Vec::new())
    } else {
        let mut follow_ups = draw;
        follow_ups.extend(necronomicurse_replacement_follow_up(state, card_id));
        Ok(follow_ups)
    }
}

fn queued_dark_embrace_then_necronomicurse_follow_ups(
    state: &CombatState,
    card_id: CardId,
) -> Vec<InternalAction> {
    let mut follow_ups = dark_embrace_draw_follow_up(state);
    follow_ups.extend(necronomicurse_replacement_follow_up(state, card_id));
    follow_ups
}

fn apply_on_exhaust_effects_inner(
    state: &mut CombatState,
    card_id: CardId,
    draw_with_dark_embrace: bool,
    apply_feel_no_pain: bool,
    defer_juggernaut: bool,
) -> SimResult<Option<i32>> {
    match exhausted_card_content_id(state, card_id) {
        // Energy is nonnegative in every valid combat state, so signed target
        // overflow is rejected by the authoritative transition validation.
        Some(SENTINEL_PLUS_ID) => state.player.energy = state.player.energy.wrapping_add(3),
        Some(SENTINEL_ID) => state.player.energy = state.player.energy.wrapping_add(2),
        _ => {}
    }
    let deferred_juggernaut = if apply_feel_no_pain && state.player.powers.feel_no_pain > 0 {
        let gained = state.player.powers.feel_no_pain;
        if defer_juggernaut {
            apply_player_direct_block_gain_without_juggernaut(state, gained)?;
            // FNP GainBlockAction ignores No Block; Juggernaut still sees
            // that gain (FIDL01702 Ghostly Armor + Panic Button).
            (gained > 0 && state.player.powers.juggernaut > 0)
                .then_some(state.player.powers.juggernaut)
        } else {
            apply_player_direct_block_gain(state, gained)?;
            None
        }
    } else {
        None
    };
    if draw_with_dark_embrace && state.player.powers.dark_embrace > 0 {
        player_draw_cards(state, state.player.powers.dark_embrace as usize)?;
    }
    if (apply_feel_no_pain || draw_with_dark_embrace)
        && exhausted_card_content_id(state, card_id) == Some(NECRONOMICURSE_ID)
    {
        // Full onExhaust path: power DrawCardAction then card triggerOnExhaust.
        let replacement = make_generated_card(state, NECRONOMICURSE_ID)?;
        if state.piles.hand.len() < MAX_HAND_SIZE {
            state.piles.hand.push(replacement);
        } else {
            state.piles.discard_pile.push(replacement);
        }
    }
    if state.relics.contains(&Relic::CharonsAshes) {
        // Charon's Ashes queues DamageAllEnemiesAction THORNS. That hits
        // block and AbstractCreature.brokeBlock notifies Hand Drill
        // (FIDL01673 Ghostly Armor exhaust vs Malleable block).
        let hand_drill = state.relics.contains(&Relic::HandDrill);
        let targets = state
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| monster.id)
            .collect::<Vec<_>>();
        for target in targets {
            let (still_alive, broke_block) = {
                let Ok(monster) = living_monster_mut(state, target) else {
                    // A prior Charon's Ashes hit can resolve a death hook that
                    // removes or replaces another collected target. The target
                    // snapshot is authoritative for this trigger, but the
                    // live state must be checked again before applying damage.
                    continue;
                };
                let block_before = monster.block;
                let hp_damage =
                    deal_unmodified_damage_to_monster(monster, crate::relic::CHARONS_ASHES_DAMAGE);
                wake_lagavulin_on_damage(monster, hp_damage);
                (monster.alive, block_before > 0 && monster.block == 0)
            };
            if still_alive && hand_drill && broke_block {
                let relics = state.relics.clone();
                if let Some(monster) = living_monster_mut_opt(state, target) {
                    let mut powers = monster.powers;
                    crate::relic::apply_monster_vulnerable_with_relics(
                        &mut powers,
                        &relics,
                        crate::relic::HAND_DRILL_VULNERABLE,
                    )?;
                    monster.vulnerable_just_applied = true;
                    monster.powers = powers;
                }
            }
            check_slime_boss_split(state, target);
            if !still_alive {
                if defer_juggernaut {
                    apply_monster_death_hooks_deferred_relics(state, target)?;
                } else {
                    apply_monster_death_hooks(state, target)?;
                }
            }
        }
    }
    Ok(deferred_juggernaut)
}

fn exhausted_card_content_id(state: &CombatState, card_id: CardId) -> Option<ContentId> {
    state
        .piles
        .exhaust_pile
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| card.content_id)
}

fn with_card_in_use_out_of_hand<T>(
    state: &mut CombatState,
    operation: impl FnOnce(&mut CombatState) -> SimResult<T>,
) -> SimResult<T> {
    let mut next = state.clone();
    let staged = next.card_in_use.and_then(|card_id| {
        next.piles
            .hand
            .iter()
            .position(|card| card.id == card_id)
            .map(|index| (index, next.piles.hand.remove(index)))
    });
    let result = operation(&mut next);
    if let Some((index, card)) = staged {
        next.piles
            .hand
            .insert(index.min(next.piles.hand.len()), card);
    }
    let result = result?;
    *state = next;
    Ok(result)
}

pub(crate) fn player_draw_cards(state: &mut CombatState, count: usize) -> SimResult<()> {
    if state.player.cannot_draw {
        return Ok(());
    }
    with_card_in_use_out_of_hand(state, |next| {
        crate::combat::draw::draw_cards_with_combat_rng(next, count)
    })
}

pub(crate) fn player_draw_cards_with_deferred_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    if state.player.cannot_draw {
        return Ok(Vec::new());
    }
    with_card_in_use_out_of_hand(state, |next| {
        crate::combat::draw::draw_cards_with_combat_rng_deferred_evolve(next, count)
    })
}

pub(crate) fn player_draw_cards_from_hp_loss_with_deferred_evolve_policy(
    state: &mut CombatState,
    count: usize,
    bypass_no_draw: bool,
) -> SimResult<Vec<InternalAction>> {
    if state.player.cannot_draw && !bypass_no_draw {
        return Ok(Vec::new());
    }
    with_card_in_use_out_of_hand(state, |next| {
        crate::combat::draw::draw_cards_with_combat_rng_deferred_evolve(next, count)
    })
}

pub(crate) fn resolve_deferred_draw_follow_ups(
    state: &mut CombatState,
    follow_ups: Vec<InternalAction>,
) -> SimResult<()> {
    let mut pending = VecDeque::from(follow_ups);
    while let Some(follow_up) = pending.pop_front() {
        let nested = match follow_up {
            InternalAction::DrawCards { count } => {
                player_draw_cards_with_deferred_evolve(state, count)?
            }
            InternalAction::FireBreathingDamage { amount } => {
                crate::combat::draw::apply_fire_breathing_damage(state, amount)?;
                Vec::new()
            }
            InternalAction::GainBlockDirect { amount } => {
                apply_player_direct_block_gain(state, amount)?;
                Vec::new()
            }
            InternalAction::GainEnergy { amount } => {
                player_actions::gain_energy(state, amount)?;
                Vec::new()
            }
            InternalAction::LoseEnergy { amount } => {
                player_actions::lose_energy(state, amount)?;
                Vec::new()
            }
            _ => {
                return Err(SimError::InvalidState(
                    "unexpected non-draw follow-up in deferred draw queue",
                ));
            }
        };
        pending.extend(nested);
    }
    Ok(())
}

pub(crate) fn player_draw_cards_without_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<()> {
    if state.player.cannot_draw {
        return Ok(());
    }
    with_card_in_use_out_of_hand(state, |next| {
        crate::combat::draw::draw_cards_with_combat_rng_without_evolve(next, count)
    })
}

pub(crate) fn player_shuffle_discard_into_draw(state: &mut CombatState) -> SimResult<()> {
    let follow_ups = crate::combat::draw::shuffle_discard_into_draw_with_combat_rng(state)?;
    apply_shuffle_relic_follow_ups(state, follow_ups)
}

fn apply_shuffle_relic_follow_ups(
    state: &mut CombatState,
    follow_ups: Vec<InternalAction>,
) -> SimResult<()> {
    for follow_up in follow_ups {
        match follow_up {
            InternalAction::GainBlockDirect { amount } => {
                apply_player_direct_block_gain(state, amount)?;
            }
            InternalAction::GainEnergy { amount } => {
                player_actions::gain_energy(state, amount)?;
            }
            _ => {
                return Err(SimError::InvalidState("unexpected shuffle relic follow-up"));
            }
        }
    }
    Ok(())
}

fn hand_contains_attack(state: &CombatState) -> bool {
    state.piles.hand.iter().any(|card| {
        get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    })
}

fn draw_random_attacks_from_draw_pile(state: &mut CombatState, count: usize) {
    let mut attack_ids = Vec::new();
    for card in &state.piles.draw_pile {
        let is_attack = get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack);
        if !is_attack {
            continue;
        }

        if attack_ids.is_empty() {
            attack_ids.push(card.id);
        } else {
            // CardGroup::addToRandomSpot asks cardRandomRng for an inclusive
            // index in 0..=(size - 1). It never appends to the end once the
            // temporary group is non-empty.
            let index = state
                .rng
                .card_random_rng
                .random_int((attack_ids.len() - 1) as i32) as usize;
            attack_ids.insert(index, card.id);
        }
    }

    for _ in 0..count {
        if attack_ids.is_empty() {
            return;
        }

        let shuffle_seed = state.rng.shuffle_rng.random_long();
        JavaRng::new(shuffle_seed).collections_shuffle(&mut attack_ids);

        let selected_id = attack_ids.remove(0);
        let Some(draw_index) = state
            .piles
            .draw_pile
            .iter()
            .position(|card| card.id == selected_id)
        else {
            continue;
        };
        let card = state.piles.draw_pile.remove(draw_index);
        if state.piles.hand.len() >= 10 {
            state.piles.discard_pile.push(card);
        } else {
            state.piles.hand.push(card);
        }
    }
}

fn apply_unceasing_top_after_hand_emptied(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&Relic::UnceasingTop) {
        player_draw_cards(state, crate::relic::UNCEASING_TOP_DRAW)?;
    }
    Ok(())
}

fn add_card_to_pile(state: &mut CombatState, content_id: ContentId, to: CardPile) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let card = CardInstance::new(next_id, content_id);
    push_card_to_pile(state, card, to);
    Ok(())
}

fn add_generated_card_to_pile(
    state: &mut CombatState,
    content_id: ContentId,
    to: CardPile,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<()> {
    let mut card = make_generated_card(state, content_id)?;
    card.temp_cost = temp_cost;
    card.temp_cost_turn_only = temp_cost_turn_only;
    let destination = if to == CardPile::Hand && state.piles.hand.len() >= MAX_HAND_SIZE {
        CardPile::DiscardPile
    } else {
        to
    };
    if destination == CardPile::Hand {
        apply_corruption_cost_to_generated_hand_card(state, &mut card);
    }
    push_card_to_pile(state, card, destination);
    Ok(())
}

pub(crate) fn make_generated_card(
    state: &mut CombatState,
    content_id: ContentId,
) -> SimResult<CardInstance> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    apply_generated_card_metadata(state, &mut card);
    Ok(card)
}

pub(crate) fn dead_branch_card_for_end_turn(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Option<CardInstance>> {
    reserve_dead_branch_card_content(state)
        .map(|content_id| {
            let mut card = CardInstance::new(card_id, content_id);
            card.combat_only = true;
            apply_generated_card_metadata(state, &mut card);
            Ok(card)
        })
        .transpose()
}

fn add_stat_equivalent_copy_to_pile(
    state: &mut CombatState,
    source: CardInstance,
    to: CardPile,
) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let mut copy = source;
    copy.id = next_id;
    copy.bottled = false;
    // AbstractCard.makeStatEquivalentCopy() starts from makeCopy() and does
    // not copy purgeOnUse. Anger copies therefore keep cycling through combat
    // piles after play even when their source was temporary.
    copy.combat_only = false;
    let destination = if to == CardPile::Hand && state.piles.hand.len() >= MAX_HAND_SIZE {
        CardPile::DiscardPile
    } else {
        to
    };
    push_card_to_pile(state, copy, destination);
    Ok(())
}

/// Public wrapper for Nilry's Codex and similar combat-generated draw inserts.
pub fn add_generated_card_to_draw_pile_random_spot_public(
    state: &mut CombatState,
    content_id: ContentId,
) -> SimResult<()> {
    add_generated_card_to_draw_pile_random_spot(state, content_id, None, false)
}

fn add_generated_card_to_draw_pile_random_spot(
    state: &mut CombatState,
    content_id: ContentId,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<()> {
    let next_id = CardId::new(state.next_card_instance_id()?);
    let mut card = CardInstance {
        combat_only: true,
        ..CardInstance::new(next_id, content_id)
    };
    apply_generated_card_metadata(state, &mut card);
    card.temp_cost = temp_cost;
    card.temp_cost_turn_only = temp_cost_turn_only;
    if state.piles.draw_pile.is_empty() {
        state.piles.draw_pile.push(card);
        return Ok(());
    }
    // CardGroup.addToRandomSpot selects an existing position; it does not
    // append the generated card after the current last entry. The target
    // effect constructor performs exactly one insertion and one card RNG draw.
    let bound = (state.piles.draw_pile.len() - 1) as i32;
    let index = state.rng.card_random_rng.random_int(bound) as usize;
    state.piles.draw_pile.insert(index, card);
    Ok(())
}

/// PlayTop removes the selected card and applies optional Hex inserts before
/// the current action queue continues. The selected card stays in limbo until
/// `ResolveTopDrawCard` runs after that queue, matching PlayTopCardAction's
/// card-queue handoff and the parent UseCardAction settlement.
fn apply_play_top_with_mid_hex(
    state: &mut CombatState,
    target: Option<crate::ids::MonsterId>,
    exhaust_played_card: bool,
    random_living_target: bool,
    mid_hex: Vec<InternalAction>,
) -> SimResult<Vec<InternalAction>> {
    let actions =
        apply_play_top_draw_card(state, target, exhaust_played_card, random_living_target)?;
    // Top card is already removed. Land Havoc Hex Dazed against this size
    // before the forced card is resolved (FIDL00381).
    for hex_action in mid_hex {
        let _ = apply_internal_action(state, hex_action)?;
    }
    Ok(actions)
}

fn apply_generated_card_metadata(state: &CombatState, card: &mut CardInstance) {
    if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
        card.blood_for_blood_cost_reduction = state.player.damage_events_this_combat;
    }
}

/// ShowCardAndAddToHandEffect setCostForTurn(-9) when Corruption is active.
pub(crate) fn apply_corruption_cost_to_generated_hand_card(
    state: &CombatState,
    card: &mut CardInstance,
) {
    if state.player.powers.corruption <= 0 {
        return;
    }
    let Some(definition) = get_card_definition(card.content_id) else {
        return;
    };
    if definition.card_type == CardType::Skill && definition.cost >= 0 {
        if !card.temp_cost_turn_only {
            card.combat_cost_under_turn_override = card.temp_cost;
        }
        card.temp_cost = Some(0);
        card.temp_cost_turn_only = true;
    }
}

fn random_colorless_card(state: &mut CombatState, upgrade: bool) -> SimResult<ContentId> {
    let pool = colorless_discovery_pool()
        .into_iter()
        .map(|content_id| {
            if upgrade {
                required_upgrade_content_id(content_id)
            } else {
                Ok(content_id)
            }
        })
        .collect::<SimResult<Vec<_>>>()?;
    let max_index = pool
        .len()
        .checked_sub(1)
        .and_then(|index| i32::try_from(index).ok())
        .ok_or(SimError::InvalidState(
            "colorless discovery pool has no representable random bound",
        ))?;
    let idx = state.rng.card_random_rng.random_int(max_index) as usize;
    Ok(pool[idx])
}

fn push_card_to_pile(state: &mut CombatState, card: CardInstance, to: CardPile) {
    match to {
        CardPile::DiscardPile => state.piles.discard_pile.push(card),
        CardPile::DrawPile => state.piles.draw_pile.push(card),
        CardPile::Hand => state.piles.hand.push(card),
        CardPile::ExhaustPile => state.piles.exhaust_pile.push(card),
    }
}

fn living_monster_mut(state: &mut CombatState, target: MonsterId) -> SimResult<&mut MonsterState> {
    living_monster_mut_opt(state, target)
        .ok_or(SimError::IllegalAction("target is not a living monster"))
}

fn living_monster_alive(state: &CombatState, target: MonsterId) -> bool {
    state
        .monsters
        .iter()
        .any(|monster| monster.id == target && monster.alive)
}

fn living_monster_mut_opt(state: &mut CombatState, target: MonsterId) -> Option<&mut MonsterState> {
    state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
}

fn apply_enrage_on_card_type(state: &mut CombatState, card_type: CardType) -> SimResult<()> {
    if card_type != CardType::Skill {
        return Ok(());
    }

    for monster in &mut state.monsters {
        if !monster.alive {
            continue;
        }
        if get_monster_definition(monster.content_id).is_some_and(|definition| {
            definition.enrage_weak_on_skill > 0 && monster.powers.anger > 0
        }) {
            checked_add_combat_value(&mut monster.powers.strength, monster.powers.anger)?;
        }
    }
    Ok(())
}

/// Rage grants block after the attack's `use()` damage actions resolve (STS
/// queues GainBlockAction from onUseCard after card.use()). Returning a
/// follow-up so Body Slam still reads pre-Rage block when computing damage.
/// Whirlwind is the exception: `use()` only queues `WhirlwindAction`, and that
/// wrapper addToBots the hits after Rage, so the card queue owns the block.
fn apply_rage_on_card_type(
    state: &CombatState,
    card_type: CardType,
    content_id: ContentId,
) -> SimResult<Vec<InternalAction>> {
    if matches!(content_id, WHIRLWIND_ID | WHIRLWIND_PLUS_ID) {
        return Ok(Vec::new());
    }
    if card_type == CardType::Attack && state.player.temp_rage_block > 0 {
        return Ok(vec![InternalAction::GainBlockDirect {
            amount: state.player.temp_rage_block,
        }]);
    }
    Ok(Vec::new())
}

fn set_random_hand_card_cost_for_combat(
    state: &mut CombatState,
    amount: u8,
    excluded_card_id: CardId,
) -> SimResult<()> {
    // useCard.removeCard runs before MadnessAction, so getRandomCard sees the
    // remaining hand only. Sample that group with random(size-1) and retry
    // when the pick fails the costForTurn / printed-cost gate. Keeping the
    // source in the live group uses random(n) and desyncs later Madness
    // (FIDL01474 unaffordable; FIDL01461 first-divs earlier).
    let remaining: Vec<usize> = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter_map(|(index, card)| (card.id != excluded_card_id).then_some(index))
        .collect();
    if remaining.is_empty() {
        return Ok(());
    }

    let better_possible = remaining.iter().try_fold(false, |found, index| {
        Ok(found || effective_card_cost(&state.piles.hand[*index])? > 0)
    })?;
    let possible = remaining.iter().try_fold(false, |found, index| {
        Ok(found || printed_card_cost(&state.piles.hand[*index])? > 0)
    })?;
    if !better_possible && !possible {
        return Ok(());
    }

    let chosen = random_madness_remaining_index(state, &remaining, better_possible)?;
    let card = &mut state.piles.hand[chosen];
    card.temp_cost = Some(amount);
    card.combat_cost_under_turn_override = None;
    card.temp_cost_turn_only = false;
    Ok(())
}

fn random_madness_remaining_index(
    state: &mut CombatState,
    remaining: &[usize],
    better_possible: bool,
) -> SimResult<usize> {
    loop {
        let last = remaining
            .len()
            .checked_sub(1)
            .ok_or(SimError::InvalidState(
                "MadnessAction remaining hand emptied",
            ))?;
        let pick = state.rng.card_random_rng.random_int(last as i32) as usize;
        let index = remaining[pick];
        if madness_card_matches(&state.piles.hand[index], better_possible)? {
            return Ok(index);
        }
    }
}

fn madness_card_matches(card: &CardInstance, better_possible: bool) -> SimResult<bool> {
    if better_possible {
        Ok(effective_card_cost(card)? > 0)
    } else {
        Ok(printed_card_cost(card)? > 0)
    }
}

fn upgrade_hand_cards_except(state: &mut CombatState, excluded_card_id: CardId) -> SimResult<()> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != excluded_card_id)
        .map(|card| Ok((card.id, upgrade_card_instance(*card)?)))
        .collect::<SimResult<Vec<_>>>()?;
    for (card_id, upgraded) in upgrades {
        if let Some(upgraded) = upgraded {
            *find_hand_card_mut(state, card_id)? = upgraded;
        }
    }
    Ok(())
}

fn upgrade_hand_card(state: &mut CombatState, card_id: CardId) -> SimResult<()> {
    let card = find_hand_card_mut(state, card_id)?;
    *card = upgrade_card_instance(*card)?.ok_or(SimError::IllegalAction("card cannot upgrade"))?;
    Ok(())
}

fn is_play_top_draw_pile_insert(action: &InternalAction) -> bool {
    matches!(
        action,
        InternalAction::AddGeneratedCardToDrawPileRandomSpot { .. }
            | InternalAction::AddGeneratedCardToDrawPileRandomSpotWithCost { .. }
    )
}

pub(crate) fn flush_deferred_mayhem_play_top_draw_inserts(
    state: &mut CombatState,
) -> SimResult<()> {
    let mut next = state.clone();
    next.defer_mayhem_play_top_draw_inserts = false;
    next.defer_mayhem_play_top_settlement = false;
    let inserts = std::mem::take(&mut next.deferred_mayhem_play_top_draw_inserts);
    if !inserts.is_empty() && next.player.hp > 0 {
        let transition = process_internal_queue(&next, VecDeque::from(inserts))?;
        next = transition.state;
    }

    let mut settlement_ids = std::collections::HashSet::new();
    for (card, destination) in &next.deferred_mayhem_play_top_settlements {
        if !settlement_ids.insert(card.id)
            || next
                .piles
                .hand
                .iter()
                .chain(next.piles.draw_pile.iter())
                .chain(next.piles.discard_pile.iter())
                .chain(next.piles.exhaust_pile.iter())
                .chain(next.piles.limbo.iter())
                .any(|candidate| candidate.id == card.id)
        {
            return Err(SimError::InvalidState(
                "deferred Mayhem settlement has duplicate card ownership",
            ));
        }
        if !matches!(destination, CardPile::DiscardPile | CardPile::ExhaustPile) {
            return Err(SimError::InvalidState(
                "deferred Mayhem settlement has invalid destination",
            ));
        }
    }
    if next.card_in_use.is_some() && !next.deferred_mayhem_play_top_settlements.is_empty() {
        return Err(SimError::InvalidState(
            "deferred Mayhem settlement would replace cardInUse",
        ));
    }

    while !next.deferred_mayhem_play_top_settlements.is_empty() {
        // Keep later parked sources authoritative while this settlement runs so
        // Dead Branch and other generators cannot reuse one of their IDs.
        let (card, destination) = next.deferred_mayhem_play_top_settlements.remove(0);
        let card_id = card.id;
        // Recreate the ordinary UseCardAction source shape only after residual
        // inserts have drained. MoveCard then owns Spoon, Power removal, exhaust
        // callbacks, Pen Nib cleanup, and Unceasing Top exactly as a normal play.
        next.piles.hand.push(card);
        next.card_in_use = Some(card_id);
        if destination == CardPile::ExhaustPile
            && next.relics.contains(&crate::Relic::StrangeSpoon)
            && !next
                .defer_strange_spoon_until_source_move
                .contains(&card_id)
        {
            next.defer_strange_spoon_until_source_move.push(card_id);
        }
        let movement = InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: destination,
        };
        let transition = process_internal_queue(&next, VecDeque::from([movement]))?;
        next = transition.state;
        if next.card_in_use != Some(card_id) {
            return Err(SimError::InvalidState(
                "deferred Mayhem settlement lost cardInUse ownership",
            ));
        }
        next.card_in_use = None;
    }
    *state = next;
    Ok(())
}

fn is_play_top_deferred_power_gain(action: &InternalAction) -> bool {
    matches!(
        action,
        InternalAction::GainFeelNoPain { .. }
            | InternalAction::GainDarkEmbrace { .. }
            | InternalAction::GainBarricade { .. }
            | InternalAction::GainEvolve { .. }
            | InternalAction::GainBerserk { .. }
            | InternalAction::GainFasting { .. }
            | InternalAction::GainLikeWater { .. }
            | InternalAction::GainRupture { .. }
            | InternalAction::GainJuggernaut { .. }
            | InternalAction::GainBrutality { .. }
            | InternalAction::GainMayhem { .. }
            | InternalAction::GainPanache { .. }
            | InternalAction::GainCombust { .. }
            | InternalAction::GainDoubleTap { .. }
            | InternalAction::GainFireBreathing { .. }
            | InternalAction::GainCorruption { .. }
            | InternalAction::GainSadisticNature { .. }
            | InternalAction::GainMagnetism { .. }
            | InternalAction::GainCreativeAI { .. }
            | InternalAction::GainStorm { .. }
            | InternalAction::GainAfterImage { .. }
            | InternalAction::GainStaticDischarge { .. }
            | InternalAction::GainThorns { .. }
            | InternalAction::IncreaseMaxOrbs { .. }
            | InternalAction::GainMetallicize { .. }
            | InternalAction::GainStrength { .. }
            | InternalAction::GainDexterity { .. }
            | InternalAction::GainTempStrength { .. }
            | InternalAction::GainIntangible { .. }
            | InternalAction::GainRitual { .. }
            | InternalAction::GainArtifact { .. }
            | InternalAction::GainRage { .. }
    )
}

fn apply_play_top_draw_card(
    state: &mut CombatState,
    target: Option<MonsterId>,
    exhaust_played_card: bool,
    random_living_target: bool,
) -> SimResult<Vec<InternalAction>> {
    // Havoc.use (and similar random-target PlayTop callers) draw a living
    // monster from cardRandomRng when constructing PlayTopCardAction — even if
    // the subsequent action no-ops because both draw and discard are empty.
    // Consume that roll up front so later cardRandomRng uses stay aligned.
    let preselected_random_target = random_living_target
        .then(|| random_living_monster_id(state))
        .flatten();

    // Time Warp's onAfterUseCard arms the forced end after card.use() has
    // already queued PlayTopCardAction, so that PlayTop still extracts its
    // top card. If Time Warp was already armed before this card's use() —
    // the 12th card was a parent Havoc whose nested resolve starts with
    // `time_warp_end_turn` set — ResolveTopDrawCard exhausts without use()
    // and never queues a leftover PlayTop (FIDL01271 / FIDL01285).
    // Do not skip extraction here: a 12th-card nested Havoc queues leftover
    // PlayTop during use() before onAfterUseCard, and that card must leave
    // the draw pile (FIDL00021 Wound).
    if random_living_target
        && !state
            .monsters
            .iter()
            .any(|monster| monster.alive || awakened_one_is_half_dead(monster))
    {
        // A lagged Awakened One *second* death can expose a post-lethal PLAY
        // before the room ends. PlayTopCardAction still removes its top card
        // into cardInUse, but a truly ended combat publishes neither a played
        // effect nor a pile destination for that transient card.
        //
        // First-form half-death is not battle-ending (`isBattleEnding` is
        // false). EmptyDeckShuffleAction and the subsequent PlayTop still run
        // (FIDL01451: Havoc on an empty draw while Awakened One is at 0 HP).
        let _ = state.piles.draw_pile.pop();
        return Ok(Vec::new());
    }
    if state.piles.draw_pile.is_empty() {
        if state.piles.discard_pile.is_empty() {
            return Ok(Vec::new());
        }
        player_shuffle_discard_into_draw(state)?;
    }

    let card = state
        .piles
        .draw_pile
        .pop()
        .ok_or(SimError::IllegalAction("draw pile is empty"))?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    // PlayTop may carry a use()-time getRandomMonster roll (Corruption Havoc).
    // Only Enemy tops consume it; non-targeted tops must not keep the monster id
    // or validate_havoc_target rejects "cannot have a target".
    let target = if definition.target == TargetRequirement::Enemy {
        target.or(preselected_random_target)
    } else {
        None
    };

    // The selected card stays in limbo until the parent action queue (including
    // UseCardAction) has drained; ResolveTopDrawCard builds its play queue then.
    let selected_card_id = card.id;
    // PlayTopCardAction moves the selected card into limbo and queues a card
    // play. The action queue (including the parent UseCardAction) drains before
    // that card queue is serviced, so retain the card in limbo until the
    // deterministic ResolveTopDrawCard handoff.
    state.piles.limbo.push(card);
    Ok(vec![InternalAction::ResolveTopDrawCard {
        card_id: selected_card_id,
        target,
        exhaust_played_card,
    }])
}

fn resolve_top_draw_card(
    state: &mut CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
    exhaust_played_card: bool,
) -> SimResult<Vec<InternalAction>> {
    let limbo_index = state
        .piles
        .limbo
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;
    let card = state.piles.limbo.remove(limbo_index);
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    // Havoc's PlayTop can already have parked this card in limbo before Time
    // Warp's onUseCard increment arms the forced end. The leftover resolve must
    // still settle/exhaust without calling use() (FIDL01285 True Grit).
    if state.time_warp_end_turn {
        let mut follow_ups = Vec::new();
        if exhaust_played_card || definition.keywords.exhaust {
            state.piles.exhaust_pile.push(card);
            follow_ups.push(InternalAction::CardExhausted { card_id });
        } else if !card.combat_only {
            state.piles.discard_pile.push(card);
        }
        return Ok(follow_ups);
    }

    // AbstractCard.canUse is checked when the queued card is finally serviced,
    // after the parent UseCardAction. A forced card that cannot be used still
    // leaves limbo and settles through UseCardAction without calling use().
    let clash_is_unplayable = matches!(definition.id, CLASH_ID | CLASH_PLUS_ID)
        && state.piles.hand.iter().any(|card| {
            get_card_definition(card.content_id)
                .is_none_or(|definition| definition.card_type != CardType::Attack)
        });
    let dual_wield_is_unplayable = matches!(definition.id, DUAL_WIELD_ID | DUAL_WIELD_PLUS_ID)
        && !state.piles.hand.iter().any(|hand_card| {
            get_card_definition(hand_card.content_id).is_some_and(|hand_definition| {
                matches!(
                    hand_definition.card_type,
                    CardType::Attack | CardType::Power
                )
            })
        });
    let missing_enemy_target = definition.target == TargetRequirement::Enemy && target.is_none();
    // GameActionManager skips use() when the queued card's monster is
    // isDeadOrEscaped, but autoplay still constructs UseCardAction with
    // dontTriggerOnUseCard. Mayhem PlayTop of Pommel therefore discards
    // without drawing if Fire Breathing already killed its target (FIDL02199).
    let target_is_dead_or_escaped = target.is_some_and(|monster_id| {
        !state.monsters.iter().any(|monster| {
            monster.id == monster_id
                && !monster.escaped
                && (monster.alive || awakened_one_is_half_dead(monster))
        })
    });
    let combat_is_done = state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster));
    let entangled_blocks_attack =
        state.player.powers.entangled > 0 && definition.card_type == CardType::Attack;
    let normality_blocks_play = state
        .piles
        .hand
        .iter()
        .any(|hand_card| hand_card.content_id == NORMALITY_ID)
        && state.relic_counters.cards_played_this_turn >= 3;
    let unplayable_blocked = definition.keywords.unplayable
        && !crate::relic::can_play_unplayable_card_with_relics(
            &state.relics,
            definition.card_type,
            definition.id,
        );
    if unplayable_blocked
        || clash_is_unplayable
        || dual_wield_is_unplayable
        || missing_enemy_target
        || target_is_dead_or_escaped
        || combat_is_done
        || entangled_blocks_attack
        || normality_blocks_play
        || !crate::relic::can_play_card_with_relics(state)
    {
        // GameActionManager skips use() when canUse is false, but autoplay
        // PlayTop still queues UseCardAction with dontTriggerOnUseCard. Havoc
        // already set exhaustOnUseOnce, so the card exhausts and onExhaust
        // (Feel No Pain) still fires (FIDL02388).
        let mut follow_ups = Vec::new();
        if dual_wield_is_unplayable {
            // Dual Wield has no custom canUse. Its action no-ops with no
            // Attack/Power candidate, but the card still triggers on-use hooks.
            apply_enrage_on_card_type(state, definition.card_type)?;
            follow_ups.extend(apply_rage_on_card_type(
                state,
                definition.card_type,
                definition.id,
            )?);
            follow_ups.extend(crate::relic::apply_on_card_play_relics(
                state,
                definition.card_type,
            )?);
            state.last_played_card_type = Some(definition.card_type);
            apply_mummified_hand_on_power_play(state, card_id, definition.card_type)?;
            follow_ups.extend(apply_on_card_play_powers(
                state,
                definition.card_type,
                false,
            )?);
            follow_ups.extend(apply_hand_card_play_triggers(state, card_id));

            // DualWieldAction is ahead of UseCardAction. Drain Hex and other
            // on-use actions now so Strange Spoon rolls only during the later
            // source settlement.
            if !follow_ups.is_empty() {
                let transition = process_internal_queue(state, VecDeque::from(follow_ups))?;
                *state = transition.state;
                follow_ups = Vec::new();
            }
        }
        if exhaust_played_card || definition.keywords.exhaust {
            // PlayTop still constructs UseCardAction with exhaustOnUseOnce.
            // StrangeSpoon.onExhaust can send that unplayable card to discard
            // (FIDL02410 Havoc → Dazed).
            let spoon_saves = state.relics.contains(&Relic::StrangeSpoon)
                && state.rng.card_random_rng.random_bool();
            if spoon_saves {
                state.piles.discard_pile.push(card);
            } else {
                state.piles.exhaust_pile.push(card);
                follow_ups.push(InternalAction::CardExhausted { card_id });
            }
        } else {
            // PlayTop already removed this card from the draw pile. A failed
            // autoplay still settles it into discard; temporary statuses such
            // as Hex Dazed are not dropped merely because they are combat-only.
            state.piles.discard_pile.push(card);
        }
        return Ok(follow_ups);
    }

    let previous_in_use = state.card_in_use;
    let previous_play_top_force_exhaust = state.play_top_force_exhaust_active;
    state.play_top_force_exhaust_active = exhaust_played_card;
    let (mut queued_state, queue) =
        card_effects::play_top_draw_card_queue(state, card, target, exhaust_played_card)?;
    // play_top_draw_card_queue clones CombatState; preserve the force-exhaust
    // marker so nested Dual Wield await can see Havoc's exhaustOnUseOnce.
    queued_state.play_top_force_exhaust_active =
        previous_play_top_force_exhaust || exhaust_played_card;
    *state = queued_state;
    state.card_in_use = Some(card_id);

    let mut immediate = VecDeque::new();
    let mut deferred_power_gains = Vec::new();
    let mut deferred_source_move = None;
    for action in queue {
        if is_play_top_deferred_power_gain(&action) {
            deferred_power_gains.push(action);
        } else if state.defer_mayhem_play_top_draw_inserts && is_play_top_draw_pile_insert(&action)
        {
            state.deferred_mayhem_play_top_draw_inserts.push(action);
        } else if state.defer_mayhem_play_top_settlement
            && matches!(
                action,
                InternalAction::MoveCard {
                    card_id: moved,
                    from: CardPile::Hand,
                    ..
                } if moved == card_id
            )
        {
            // UseCardAction is addToBottom after use(); Evolve residual draws
            // from the base refill are already on that bot queue (FIDL02303).
            deferred_source_move = Some(action);
        } else {
            immediate.push_back(action);
        }
    }
    // Return this card's actions to the outer action manager instead of
    // recursively draining a private queue. Existing ResolveTopDrawCard items
    // remain the card-queue siblings; any nested PlayTop appends its new item
    // behind them through push_follow_up's lane rule.
    state.play_top_resolving_depth = state.play_top_resolving_depth.saturating_add(1);
    immediate.push_back(InternalAction::EndPlayTopCardResolution {
        card_id,
        deferred_destination: deferred_source_move.and_then(|action| match action {
            InternalAction::MoveCard { to, .. } => Some(to),
            _ => None,
        }),
        previous_card_in_use: previous_in_use,
        previous_force_exhaust: previous_play_top_force_exhaust,
    });
    immediate.extend(deferred_power_gains);
    Ok(immediate.into_iter().collect())
}

fn end_play_top_card_resolution(
    state: &mut CombatState,
    card_id: CardId,
    deferred_destination: Option<CardPile>,
    previous_card_in_use: Option<CardId>,
    previous_force_exhaust: bool,
) -> SimResult<Vec<InternalAction>> {
    if let Some(destination) = deferred_destination {
        let index = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == card_id)
            .ok_or(SimError::UnknownCard(card_id))?;
        let card = state.piles.hand.remove(index);
        state
            .deferred_mayhem_play_top_settlements
            .push((card, destination));
    }
    state.play_top_resolving_depth = state.play_top_resolving_depth.saturating_sub(1);
    if state.decision.is_none() {
        state.play_top_force_exhaust_active = previous_force_exhaust;
    }
    state.card_in_use = previous_card_in_use;
    Ok(std::mem::take(&mut state.deferred_play_top_monster_blocks)
        .into_iter()
        .map(|(target, amount)| InternalAction::GainMonsterBlock { target, amount })
        .collect())
}

pub fn choose_hand_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let hand_index = hand_select_ui_to_hand_index(state, ui_index)?;
    let hand_select = state
        .hand_select_mut()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose == HandSelectPurpose::ForethoughtPutAnyOnDraw {
        if let Some(position) = hand_select
            .selected_hand_indices
            .iter()
            .position(|index| *index == hand_index)
        {
            hand_select.selected_hand_indices.remove(position);
        } else {
            hand_select.selected_hand_indices.push(hand_index);
        }
    } else {
        hand_select.selected_hand_index = Some(hand_index);
    }
    Ok(())
}

pub fn hand_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let hand_select = state
        .hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    // Forethought+ (and similar multi put-on-draw) drops selected cards from the
    // CommunicationMod choice_list. CHOOSE indices are therefore among the
    // remaining unselected hand cards only (FIDL00269: three CHOOSE 1 picks).
    let exclude_selected = hand_select.purpose == HandSelectPurpose::ForethoughtPutAnyOnDraw;
    let selectable: Vec<usize> = state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(index, card)| {
            hand_select_allows_card(hand_select, card)
                && !(exclude_selected && hand_select.selected_hand_indices.contains(index))
        })
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("hand select index out of range"))
}

fn hand_select_allows_card(
    hand_select: &crate::combat::HandSelectState,
    card: &CardInstance,
) -> bool {
    if card.id == hand_select.source_card_id {
        return false;
    }

    match hand_select.purpose {
        HandSelectPurpose::WarcryPutOnDraw | HandSelectPurpose::ThinkingAheadPutOnDraw => true,
        HandSelectPurpose::ArmamentsUpgrade => card_instance_is_upgradeable(card),
        HandSelectPurpose::ForethoughtPutOnDraw | HandSelectPurpose::ForethoughtPutAnyOnDraw => {
            true
        }
        HandSelectPurpose::DualWieldCopy => dual_wield_select_allows_card(card),
    }
}

pub fn confirm_hand_select(state: &mut CombatState) -> SimResult<usize> {
    confirm_hand_select_with_time_warp_policy(state, true)
}

/// Like [`confirm_hand_select`], but optionally defers Time Warp forced end-turn.
///
/// CommunicationMod can snapshot the post-CONFIRM pile state before Time Warp's
/// end-turn drains the hand (15ab4cc Warcry as 12th card). Seed-start lag frames
/// use `settle_time_warp = false`; continuous play keeps the default settle.
pub fn confirm_hand_select_with_time_warp_policy(
    state: &mut CombatState,
    settle_time_warp: bool,
) -> SimResult<usize> {
    let (hand_select, pending_actions) = state
        .take_hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    let source_settlement_after_pending = matches!(
        hand_select.purpose,
        HandSelectPurpose::WarcryPutOnDraw
            | HandSelectPurpose::ThinkingAheadPutOnDraw
            | HandSelectPurpose::ForethoughtPutOnDraw
            | HandSelectPurpose::ForethoughtPutAnyOnDraw
    );
    match hand_select.purpose {
        HandSelectPurpose::WarcryPutOnDraw => {
            confirm_warcry_select(
                state,
                hand_select.source_card_id,
                required_hand_select_index(&hand_select)?,
            )?;
        }
        HandSelectPurpose::ThinkingAheadPutOnDraw => {
            confirm_thinking_ahead_select(
                state,
                hand_select.source_card_id,
                required_hand_select_index(&hand_select)?,
            )?;
        }
        HandSelectPurpose::ArmamentsUpgrade => {
            confirm_armaments_select(
                state,
                hand_select.source_card_id,
                required_hand_select_index(&hand_select)?,
            )?;
        }
        HandSelectPurpose::ForethoughtPutOnDraw => {
            confirm_forethought_select(
                state,
                hand_select.source_card_id,
                required_hand_select_index(&hand_select)?,
            )?;
        }
        HandSelectPurpose::ForethoughtPutAnyOnDraw => {
            confirm_forethought_multi_select(
                state,
                hand_select.source_card_id,
                hand_select.selected_hand_indices,
            )?;
        }
        HandSelectPurpose::DualWieldCopy => {
            confirm_dual_wield_select(
                state,
                hand_select.source_card_id,
                required_hand_select_index(&hand_select)?,
                hand_select.dual_wield_restore_on_confirm,
                hand_select.dual_wield_force_exhaust,
            )?;
        }
    };
    // Deferred Time Warp increments on PlayCard but must not force end-turn
    // before UseCardAction settles the source. FIDL01274: Warcry under
    // Corruption/Dark Embrace still needs to exhaust after CONFIRM.
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    if source_settlement_after_pending {
        state.defer_time_warp_end_turn = true;
    }
    // PutOnDeckAction completes, then UseCardAction, then Evolve's addToBot
    // DrawCardAction from the earlier Warcry/Thinking Ahead draw. Hex Dazed
    // inserts are onUseCard and stay ahead of source settlement. Evolve extra
    // draws must run after the source leaves hand, or the limbo card occupies
    // a slot and the chain stops at 10 (FIDL01514: Shrug stays in the draw pile).
    let (pending_before_source, pending_after_source) = if source_settlement_after_pending {
        partition_put_on_deck_source_pending(pending_actions)
    } else {
        (pending_actions, VecDeque::new())
    };
    resume_actions_after_hand_select(state, pending_before_source)?;
    let mut handled_dead_branch_count = 0;
    if source_settlement_after_pending {
        // UseCardAction exhaust is addToBot Feel No Pain, Dead Branch, then
        // Dark Embrace. Immediate Dark Embrace draw would place the run-level
        // Dead Branch card after the put-on-top draw (FIDL01520).
        handled_dead_branch_count = move_delayed_played_source_with_bot_exhaust_queue(
            state,
            hand_select.source_card_id,
            pending_after_source,
            hand_select.dual_wield_force_exhaust,
        )?;
        state.defer_time_warp_end_turn = previous_defer_time_warp;
    } else {
        resume_actions_after_hand_select(state, pending_after_source)?;
    }
    state.activate_next_queued_decision_if_idle();
    if settle_time_warp {
        settle_time_warp_end_turn_if_ready(state)?;
    }
    Ok(handled_dead_branch_count)
}

/// Time Warp 12th-card Warcry CONFIRM can publish after status autoplay and
/// before DiscardAtEndOfTurn (FIDL01274: selected Burn deals 2 and sits in
/// discard; the rest of the hand is still held). PutOnDeck never moved the
/// selected card. Dark Embrace's addToBot draw stays behind the leftover
/// EndTurn and is dropped when that EndTurn sequence replaces the queue.
#[cfg(test)]
fn confirm_hand_select_time_warp_status_lag(state: &mut CombatState) -> SimResult<()> {
    let (hand_select, pending_actions) = state
        .take_hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if !matches!(
        hand_select.purpose,
        HandSelectPurpose::WarcryPutOnDraw
            | HandSelectPurpose::ThinkingAheadPutOnDraw
            | HandSelectPurpose::ForethoughtPutOnDraw
    ) {
        return Err(SimError::IllegalAction(
            "Time Warp status lag requires a put-on-deck hand select",
        ));
    }
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    state.defer_time_warp_end_turn = true;
    resume_actions_after_hand_select(state, pending_actions)?;
    settle_delayed_source_without_bot_exhaust_powers(state, hand_select.source_card_id)?;
    state.defer_time_warp_end_turn = previous_defer_time_warp;
    state.activate_next_queued_decision_if_idle();
    crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(state)?;
    Ok(())
}

/// Time Warp 12th-card Warcry CONFIRM can PutOnDeck the selected card and still
/// autoplay a *remaining* end-turn curse before DiscardAtEndOfTurn (FIDL01425:
/// Pommel on draw, leftover Regret deals 2 and sits in discard, Reaper held).
#[cfg(test)]
fn confirm_hand_select_time_warp_remaining_status_lag(state: &mut CombatState) -> SimResult<usize> {
    let handled_dead_branch_count = confirm_hand_select_with_time_warp_policy(state, false)?;
    crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(state)?;
    // Explicit END after this lagged CONFIRM is a second EndTurnAction, so the
    // monster queue has two items. Strength +2 already published on CONFIRM;
    // both items use live strength (FIDL01425 two Reverberates for 66).
    state.time_warp_duplicate_monster_queue = true;
    Ok(handled_dead_branch_count)
}

#[cfg(test)]
fn settle_delayed_source_without_bot_exhaust_powers(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<()> {
    let source = if let Some(card) = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
    {
        card
    } else if let Some(card) = state
        .piles
        .limbo
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
    {
        card
    } else {
        if state
            .piles
            .discard_pile
            .iter()
            .chain(state.piles.exhaust_pile.iter())
            .any(|card| card.id == source_card_id)
        {
            return Ok(());
        }
        return Err(SimError::IllegalAction(
            "delayed source card is not in a resolved destination",
        ));
    };
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = delayed_source_card_destination(state, definition);
    if !state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        if let Some(index) = state
            .piles
            .limbo
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let card = state.piles.limbo.remove(index);
            state.piles.hand.push(card);
        }
    }
    move_card(state, source_card_id, CardPile::Hand, destination)?;
    if destination == CardPile::ExhaustPile {
        apply_on_exhaust_effects_except_bot_queued_powers(state, source_card_id)?;
    }
    Ok(())
}

/// Public settle for seed-start when a lag CONFIRM deferred Time Warp end-turn.
pub fn settle_time_warp_end_turn_if_ready_public(state: &mut CombatState) -> SimResult<()> {
    settle_time_warp_end_turn_if_ready(state)
}

/// Drain a leftover `EndTurnAction` after CommunicationMod rejects a PLAY.
///
/// Colosseum fight-two opening `END` and Nilry's Codex SKIP can publish a
/// ready hand while the action manager still has EndTurn queued. The next
/// PLAY errors; later STATE polls show the discarded hand, then the next
/// turn (FIDL01505 / FIDL01772).
pub fn settle_queued_end_turn_discard_after_rejected_command(
    state: &mut CombatState,
) -> SimResult<()> {
    if state.opening_end_turn_pending {
        crate::combat::hand::resolve_end_of_turn_hand(state)?;
        crate::combat::hand::discard_end_of_turn_hand(state)?;
        state.opening_end_turn_pending = false;
        state.time_warp_end_turn_pre_discard_settled = true;
        return Ok(());
    }
    if state.resume_end_turn_after_nilrys_codex {
        // FrailPower.atEndOfRound waits for leftover takeTurn (FIDL01807
        // discarded-hand STATE after a rejected PLAY still shows Frail 5).
        let frail_before = state.player.powers.frail;
        crate::combat::turn::apply_pending_nilry_end_powers(state)?;
        state.player.powers.frail = frail_before;
        let block_before = state.player.block;
        crate::combat::hand::resolve_end_of_turn_hand(state)?;
        let gained = state.player.block.saturating_sub(block_before);
        state.player.block = block_before;
        state.pending_end_turn_feel_no_pain_block = state
            .pending_end_turn_feel_no_pain_block
            .saturating_add(gained);
        crate::combat::hand::discard_end_of_turn_hand(state)?;
        state.resume_end_turn_after_nilrys_codex = false;
        state.nilrys_end_powers_pending = false;
        state.time_warp_end_turn_pre_discard_settled = true;
        return Ok(());
    }
    settle_time_warp_pre_discard_if_ready_public(state)
}

/// Resolve the target's observable pre-monster portion of a forced Time Warp
/// END. The game can publish after hand discard while the queued monster turn
/// is still pending; the next explicit END resumes from this marker.
pub fn settle_time_warp_pre_discard_if_ready_public(state: &mut CombatState) -> SimResult<()> {
    if state.time_warp_end_turn
        && !state.time_warp_end_turn_pre_discard_settled
        && state.player.hp > 0
        && state.monsters.iter().any(|monster| monster.alive)
        && state.decision.is_none()
    {
        let hand_nonempty_at_end_click = !state.piles.hand.is_empty();
        crate::combat::hand::discard_end_of_turn_hand(state)?;
        if hand_nonempty_at_end_click && !state.relics.contains(&crate::Relic::RunicPyramid) {
            let pending = std::mem::take(&mut state.pending_hidden_hand_card_until_end_turn);
            state.piles.discard_pile.extend(pending);
            state.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
        }
        state.time_warp_end_turn_pre_discard_settled = true;
    }
    Ok(())
}

/// MetallicizePower.atEndOfTurnPreEndTurnCards can publish on a Time Warp
/// Burning Pact CONFIRM before DiscardAtEndOfTurnAction (FIDL01694).
#[cfg(test)]
fn apply_time_warp_lag_metallicize_keep_hand(state: &mut CombatState) -> SimResult<()> {
    if state.player.powers.metallicize > 0 && !state.time_warp_end_powers_applied {
        apply_player_end_turn_automatic_block_gain(state, state.player.powers.metallicize)?;
    }
    state.time_warp_end_powers_applied = true;
    Ok(())
}

/// Time Warp hits 12 on PlayCard of a card that opens a select. The end-turn is
/// deferred until the select closes (process_internal_queue skips it while a
/// decision is open). Honor it once CONFIRM leaves combat idle (15ab4cc Warcry
/// as 12th card → forced end before the rejected PLAY / next-turn hand).
pub(crate) fn settle_time_warp_end_turn_if_ready(state: &mut CombatState) -> SimResult<()> {
    if state.time_warp_end_turn && state.player.hp > 0 && state.decision.is_none() {
        state.time_warp_end_turn = false;
        if state.monsters.iter().any(|monster| monster.alive) {
            *state = crate::combat::end_player_turn(state)?;
        } else {
            crate::combat::hand::resolve_end_of_turn_playing_cards_for_time_warp_lag(state)?;
        }
    }
    Ok(())
}

/// Confirm force-exhausted Armaments without retrieving the selected card as an
/// upgrade in hand.
///
/// Resolve a CommunicationMod hand-selection boundary after the target action
/// has completed with `HandCardSelectScreen.wereCardsRetrieved == false`.
/// Selected cards remain authoritative but hidden until end-turn settlement;
/// effects that require retrieval (upgrade/copy/put-on-draw) do not occur.
pub fn confirm_hand_select_without_retrieval(state: &mut CombatState) -> SimResult<usize> {
    let purpose = state
        .hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?
        .purpose;
    match purpose {
        HandSelectPurpose::ArmamentsUpgrade => {
            confirm_hand_select_skipped_armaments_retrieval(state)?;
            Ok(0)
        }
        HandSelectPurpose::DualWieldCopy => {
            confirm_dual_wield_select_skipped_retrieval_with_restore(state, false)?;
            Ok(0)
        }
        HandSelectPurpose::WarcryPutOnDraw
        | HandSelectPurpose::ThinkingAheadPutOnDraw
        | HandSelectPurpose::ForethoughtPutOnDraw
        | HandSelectPurpose::ForethoughtPutAnyOnDraw => {
            let (hand_select, pending_actions) = state
                .take_hand_select()
                .ok_or(SimError::IllegalAction("no hand select is open"))?;
            if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
                return Err(SimError::IllegalAction(
                    "pending hidden hand card already occupied",
                ));
            }
            let selected_indices =
                if hand_select.purpose == HandSelectPurpose::ForethoughtPutAnyOnDraw {
                    unique_selected_indices_in_choice_order(hand_select.selected_hand_indices)
                } else {
                    vec![required_hand_select_index(&hand_select)?]
                };
            let mut selected = selected_indices
                .iter()
                .map(|index| {
                    state
                        .piles
                        .hand
                        .get(*index)
                        .copied()
                        .ok_or(SimError::IllegalAction("hand select index out of range"))
                })
                .collect::<SimResult<Vec<_>>>()?;
            let mut removal_order = selected_indices;
            removal_order.sort_unstable();
            removal_order.dedup();
            for index in removal_order.into_iter().rev() {
                state.piles.hand.remove(index);
            }
            state
                .pending_hidden_hand_card_until_end_turn
                .append(&mut selected);

            let (pending_before_source, pending_after_source) =
                partition_put_on_deck_source_pending(pending_actions);
            resume_actions_after_hand_select(state, pending_before_source)?;
            let previous_defer_time_warp = state.defer_time_warp_end_turn;
            state.defer_time_warp_end_turn = true;
            let handled_dead_branch_count = move_delayed_played_source_with_bot_exhaust_queue(
                state,
                hand_select.source_card_id,
                pending_after_source,
                hand_select.dual_wield_force_exhaust,
            )?;
            state.defer_time_warp_end_turn = previous_defer_time_warp;
            state.activate_next_queued_decision_if_idle();
            settle_time_warp_end_turn_if_ready(state)?;
            Ok(handled_dead_branch_count)
        }
    }
}

/// Models `HandCardSelectScreen.wereCardsRetrieved == false`: ArmamentsAction
/// can complete before CONFIRM under CommunicationMod load, so the upgrade never
/// lands. The selected card stays owned by the closed selection screen (absent
/// from every serialized pile) and re-enters via end-turn `DiscardAction`
/// leftover-selectedCards settlement as the unupgraded card
/// (`pending_hidden_hand_card_until_end_turn`). Deferred pending actions (Hex
/// Dazed insert, etc.) still resume after the screen closes.
///
/// Non-upgradeable hand cards (already-upgraded Strikes, statuses, etc.) also
/// leave the serialized hand for the rest of combat under this path — FIDL00400
/// shows `Strike_R` with `upgrades=1` vanishing with the selected Cleave limbo
/// and never reappearing in any combat pile, while only the selected card flushes
/// on a later non-empty-hand END.
///
/// Eligible only when Armaments is already in exhaust/discard (Havoc / Mayhem /
/// Distilled Chaos). Ordinary hand Armaments keeps
/// [`confirm_hand_select`] / [`confirm_armaments_select`] authoritative.
fn confirm_hand_select_skipped_armaments_retrieval(state: &mut CombatState) -> SimResult<()> {
    let (hand_select, pending_actions) = state
        .take_hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose != HandSelectPurpose::ArmamentsUpgrade {
        return Err(SimError::IllegalAction(
            "skipped Armaments retrieval requires Armaments hand select",
        ));
    }
    let index = required_hand_select_index(&hand_select)?;
    let selected = *state
        .piles
        .hand
        .get(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?;
    if selected.id == hand_select.source_card_id {
        return Err(SimError::IllegalAction("cannot select Armaments itself"));
    }
    let source_settled = state
        .piles
        .exhaust_pile
        .iter()
        .chain(state.piles.discard_pile.iter())
        .any(|card| card.id == hand_select.source_card_id);
    if !source_settled && !state.play_top_force_exhaust_active {
        return Err(SimError::IllegalAction(
            "skipped Armaments retrieval requires force-played or settled source",
        ));
    }
    if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
        return Err(SimError::IllegalAction(
            "pending hidden hand card already occupied",
        ));
    }
    if !source_settled {
        force_exhaust_armaments_source(state, hand_select.source_card_id)?;
    }
    let selected_position = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == selected.id)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?;
    state.piles.hand.remove(selected_position);
    state.pending_hidden_hand_card_until_end_turn = vec![selected];
    // Drop non-selectable leftovers so the post-CONFIRM hand matches CM
    // (upgradeable cards that were not chosen stay; upgraded/status cards go).
    state.piles.hand.retain(card_instance_is_upgradeable);
    state.play_top_force_exhaust_active = false;
    resume_actions_after_hand_select(state, pending_actions)?;
    state.activate_next_queued_decision_if_idle();
    settle_time_warp_end_turn_if_ready(state)?;
    Ok(())
}

fn resume_actions_after_hand_select(
    state: &mut CombatState,
    pending_actions: VecDeque<InternalAction>,
) -> SimResult<()> {
    if pending_actions.is_empty() {
        return Ok(());
    }
    let transition = process_internal_queue(state, pending_actions)?;
    *state = transition.state;
    Ok(())
}

fn partition_put_on_deck_source_pending(
    pending_actions: VecDeque<InternalAction>,
) -> (VecDeque<InternalAction>, VecDeque<InternalAction>) {
    let mut before_source = VecDeque::new();
    let mut after_source = VecDeque::new();
    for action in pending_actions {
        if matches!(
            action,
            InternalAction::DrawCards { .. } | InternalAction::FireBreathingDamage { .. }
        ) {
            after_source.push_back(action);
        } else {
            before_source.push_back(action);
        }
    }
    (before_source, after_source)
}

fn required_hand_select_index(hand_select: &crate::combat::HandSelectState) -> SimResult<usize> {
    hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction("hand select choice is required"))
}

pub fn choose_draw_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let draw_index = draw_select_ui_to_draw_index(state, ui_index)?;
    let draw_select = state
        .draw_select_mut()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if draw_select.purpose == DrawSelectPurpose::Scry
        && draw_select.selected_draw_index == Some(draw_index)
    {
        draw_select.selected_draw_index = None;
    } else {
        draw_select.selected_draw_index = Some(draw_index);
    }
    Ok(())
}

pub fn draw_select_ui_to_draw_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let draw_select = state
        .draw_select()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    if !draw_select.selectable_card_ids.is_empty() {
        let card_id = draw_select
            .selectable_card_ids
            .get(ui_index)
            .ok_or(SimError::IllegalAction("draw select index out of range"))?;
        return state
            .piles
            .draw_pile
            .iter()
            .position(|card| card.id == *card_id)
            .ok_or(SimError::IllegalAction(
                "draw select card is no longer in draw pile",
            ));
    }
    let selectable: Vec<usize> = state
        .piles
        .draw_pile
        .iter()
        .enumerate()
        .filter(|(_, card)| draw_select_allows_card(draw_select, card))
        .map(|(index, _)| index)
        .collect();
    selectable
        .get(ui_index)
        .copied()
        .ok_or(SimError::IllegalAction("draw select index out of range"))
}

fn draw_select_allows_card(
    draw_select: &crate::combat::DrawSelectState,
    card: &CardInstance,
) -> bool {
    match draw_select.purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand => get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Skill),
        DrawSelectPurpose::SecretWeaponAttackToHand => get_card_definition(card.content_id)
            .is_some_and(|definition| definition.card_type == CardType::Attack),
        DrawSelectPurpose::Scry => true,
    }
}

pub fn confirm_draw_select(state: &mut CombatState) -> SimResult<usize> {
    let draw_select = state
        .take_draw_select()
        .ok_or(SimError::IllegalAction("no draw select is open"))?;
    let dead_branch_count = match draw_select.purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand
        | DrawSelectPurpose::SecretWeaponAttackToHand => {
            let index = draw_select
                .selected_draw_index
                .ok_or(SimError::IllegalAction("draw select choice is required"))?;
            confirm_secret_draw_select_choice(state, &draw_select, index)?;
            // AttackFromDeckToHandAction completes first. Actions already on the
            // manager (notably Ink Bottle's DrawCardAction) then run before the
            // paused UseCardAction settles and exhausts the source.
            resume_actions_after_hand_select(state, draw_select.pending_actions)?;
            let source_definition =
                draw_select_source_definition(state, draw_select.source_card_id)?;
            move_draw_select_source_card(state, draw_select.source_card_id, source_definition)?
        }
        DrawSelectPurpose::Scry => {
            confirm_scry_select(
                state,
                draw_select.source_card_id,
                draw_select.selected_draw_index,
            )?;
            resume_actions_after_hand_select(state, draw_select.pending_actions)?;
            0
        }
    };
    state.activate_next_queued_decision_if_idle();
    Ok(dead_branch_count)
}

fn confirm_secret_draw_select_choice(
    state: &mut CombatState,
    draw_select: &crate::combat::DrawSelectState,
    index: usize,
) -> SimResult<()> {
    let card = state
        .piles
        .draw_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("draw select index out of range"))?;
    let expected_type = match draw_select.purpose {
        DrawSelectPurpose::SecretTechniqueSkillToHand => CardType::Skill,
        DrawSelectPurpose::SecretWeaponAttackToHand => CardType::Attack,
        DrawSelectPurpose::Scry => {
            return Err(SimError::InvalidState(
                "scry cannot use Secret Technique/Weapon confirmation",
            ));
        }
    };
    if !get_card_definition(card.content_id)
        .is_some_and(|definition| definition.card_type == expected_type)
    {
        return Err(SimError::IllegalAction(
            "Secret Technique/Weapon selection has the wrong card type",
        ));
    }
    move_selected_draw_card_to_hand_or_discard(state, index);
    Ok(())
}

fn confirm_scry_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: Option<usize>,
) -> SimResult<()> {
    let source_definition = draw_select_source_definition(state, source_card_id)?;
    if let Some(index) = index {
        let selected = state
            .piles
            .draw_pile
            .get(index)
            .copied()
            .ok_or(SimError::IllegalAction("scry index out of range"))?;
        state.piles.draw_pile.remove(index);
        state.piles.discard_pile.push(selected);
    }
    // The target's Prismatic Just Lucky is colorless and is discarded even
    // while Corruption is active; Charge Battery remains subject to the
    // ordinary corrupted-skill exhaust path.
    if state.piles.limbo.iter().any(|card| {
        card.id == source_card_id
            && card.content_id == crate::content::cards::JUST_LUCKY_ANY_COLOR_ID
    }) {
        let source = state
            .piles
            .limbo
            .iter()
            .position(|card| card.id == source_card_id)
            .map(|index| state.piles.limbo.remove(index))
            .ok_or(SimError::IllegalAction("scry source card missing"))?;
        state.piles.discard_pile.push(source);
    } else {
        move_draw_select_source_card(state, source_card_id, source_definition)?;
    }
    Ok(())
}

fn draw_select_source_definition(
    state: &CombatState,
    source_card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.limbo.iter())
        .chain(state.piles.exhaust_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .find(|card| card.id == source_card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction("draw select source card missing"))
}

fn move_draw_select_source_card(
    state: &mut CombatState,
    source_card_id: CardId,
    source_definition: &'static crate::card::CardDefinition,
) -> SimResult<usize> {
    // Prefer limbo (await_draw_select parks Secret Technique/Weapon there so the
    // retrieved card can fill the last hand slot — FIDL00413).
    if let Some(index) = state
        .piles
        .limbo
        .iter()
        .position(|card| card.id == source_card_id)
    {
        let card = state.piles.limbo.remove(index);
        let destination = delayed_source_card_destination(state, source_definition);
        match destination {
            CardPile::ExhaustPile => {
                state.piles.exhaust_pile.push(card);
                apply_purity_card_exhausted(state, source_card_id)
            }
            CardPile::DiscardPile => {
                state.piles.discard_pile.push(card);
                Ok(0)
            }
            CardPile::Hand | CardPile::DrawPile => Err(SimError::InvalidState(
                "unexpected Secret Technique/Weapon destination",
            )),
        }
    } else if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
        Ok(0)
    } else {
        Ok(0)
    }
}

fn move_selected_draw_card_to_hand_or_discard(state: &mut CombatState, index: usize) {
    let card = state.piles.draw_pile.remove(index);
    if state.piles.hand.len() >= 10 {
        state.piles.discard_pile.push(card);
    } else {
        state.piles.hand.push(card);
    }
}

fn confirm_warcry_select(
    state: &mut CombatState,
    _source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    // PutOnDeckAction completes before UseCardAction exhausts Warcry. Dark
    // Embrace (and any other on-exhaust draw) must therefore see the selected
    // card already on top of the draw pile.
    let put_back = state.piles.hand[index].id;
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    Ok(())
}

fn finish_warcry_source(state: &mut CombatState, source_card_id: CardId) -> SimResult<()> {
    move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    // Warcry auto-place uses getRandomCard + moveToDeck rather than MoveCard, then
    // delayed source exhaust. PutOnDeck empties the published hand (Warcry is in
    // limbo), and Unceasing Top's later DrawCardAction redraws that top card.
    // Skipping the relic here makes the skip-auto-place candidate look identical
    // to ordinary put-back, so replay keeps the no-RNG path and later Madness
    // samples the wrong remaining card (FIDL01461 Entrench vs Strike).
    if state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    Ok(())
}

fn confirm_thinking_ahead_select(
    state: &mut CombatState,
    _source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    // Same PutOnDeck-before-exhaust ordering as Warcry: the selected card is
    // on top before the source settles into exhaust/discard.
    let put_back = state.piles.hand[index].id;
    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
    state.piles.draw_pile.push(card);
    Ok(())
}

fn apply_deferred_played_card_strange_spoon(
    state: &mut CombatState,
    card_id: CardId,
    to: CardPile,
) -> CardPile {
    let Some(index) = state
        .defer_strange_spoon_until_source_move
        .iter()
        .position(|pending| *pending == card_id)
    else {
        return to;
    };
    state.defer_strange_spoon_until_source_move.remove(index);
    if to != CardPile::ExhaustPile {
        return to;
    }
    delayed_source_exhaust_destination(state)
}

pub(crate) fn move_delayed_played_source_with_strange_spoon(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<()> {
    let force_exhaust = state.play_top_force_exhaust_active;
    move_delayed_played_source_with_exhaust_policy(state, source_card_id, false, force_exhaust)
        .map(|_| ())
}

/// Put-on-deck CONFIRM: pending actions that were already behind UseCardAction
/// remain ahead of the source's exhaust callbacks. The resulting queue is the
/// pending Evolve/Fire Breathing work, then CardExhausted, whose callbacks keep
/// Feel No Pain, Dead Branch, and Dark Embrace order (FIDL01520, FIDL03073).
fn move_delayed_played_source_with_bot_exhaust_queue(
    state: &mut CombatState,
    source_card_id: CardId,
    mut pending_before_exhaust_callbacks: VecDeque<InternalAction>,
    force_exhaust: bool,
) -> SimResult<usize> {
    let generated = state.relics.contains(&Relic::DeadBranch)
        && state.monsters.iter().any(|monster| monster.alive);
    let destination = move_delayed_played_source(state, source_card_id, force_exhaust)?;
    if destination == Some(CardPile::ExhaustPile) {
        pending_before_exhaust_callbacks.push_back(InternalAction::CardExhausted {
            card_id: source_card_id,
        });
    }
    resume_actions_after_hand_select(state, pending_before_exhaust_callbacks)?;
    Ok(usize::from(
        generated && destination == Some(CardPile::ExhaustPile),
    ))
}

fn move_delayed_played_source_with_exhaust_policy(
    state: &mut CombatState,
    source_card_id: CardId,
    queue_bot_exhaust_follow_ups: bool,
    force_exhaust: bool,
) -> SimResult<usize> {
    let Some(destination) = move_delayed_played_source(state, source_card_id, force_exhaust)?
    else {
        return Ok(0);
    };
    if destination != CardPile::ExhaustPile {
        return Ok(0);
    }
    if queue_bot_exhaust_follow_ups {
        let generated = state.relics.contains(&Relic::DeadBranch)
            && state.monsters.iter().any(|monster| monster.alive);
        let transition = process_internal_queue(
            state,
            VecDeque::from([InternalAction::CardExhausted {
                card_id: source_card_id,
            }]),
        )?;
        *state = transition.state;
        return Ok(usize::from(generated));
    }
    apply_on_exhaust_effects(state, source_card_id)?;
    Ok(0)
}

fn move_delayed_played_source(
    state: &mut CombatState,
    source_card_id: CardId,
    force_exhaust: bool,
) -> SimResult<Option<CardPile>> {
    let source = if let Some(card) = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
    {
        card
    } else if let Some(card) = state
        .piles
        .limbo
        .iter()
        .find(|card| card.id == source_card_id)
        .copied()
    {
        card
    } else {
        if state
            .piles
            .discard_pile
            .iter()
            .chain(state.piles.exhaust_pile.iter())
            .any(|card| card.id == source_card_id)
        {
            return Ok(None);
        }
        return Err(SimError::IllegalAction(
            "delayed source card is not in a resolved destination",
        ));
    };
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = if force_exhaust {
        forced_source_card_destination(state, definition)
    } else {
        delayed_source_card_destination(state, definition)
    };
    if !state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        if let Some(index) = state
            .piles
            .limbo
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let card = state.piles.limbo.remove(index);
            state.piles.hand.push(card);
        }
    }
    move_card(state, source_card_id, CardPile::Hand, destination)?;
    if force_exhaust {
        state.play_top_force_exhaust_active = false;
    }
    Ok(Some(destination))
}

pub fn close_discovery_card_reward_source(state: &mut CombatState) -> SimResult<()> {
    let (source, force_exhaust) = {
        let Some(CombatDecisionState::DiscoveryCardReward {
            source_card,
            source_card_force_exhaust,
            ..
        }) = state.decision.as_mut()
        else {
            return Ok(());
        };
        (source_card.take(), *source_card_force_exhaust)
    };
    close_discovery_source_card_with_force_exhaust(state, source, force_exhaust)
}

pub fn close_discovery_source_card(
    state: &mut CombatState,
    source: Option<CardInstance>,
) -> SimResult<()> {
    close_discovery_source_card_with_force_exhaust(state, source, false)
}

pub fn close_discovery_source_card_with_force_exhaust(
    state: &mut CombatState,
    source: Option<CardInstance>,
    force_exhaust: bool,
) -> SimResult<()> {
    let Some(source) = source else {
        return Ok(());
    };
    let source_card_id = source.id;
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let destination = if force_exhaust {
        forced_source_card_destination(state, definition)
    } else {
        delayed_source_card_destination(state, definition)
    };
    match destination {
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(source);
            let transition = process_internal_queue(
                state,
                VecDeque::from([InternalAction::CardExhausted {
                    card_id: source_card_id,
                }]),
            )?;
            *state = transition.state;
        }
        CardPile::DiscardPile => state.piles.discard_pile.push(source),
        CardPile::Hand => state.piles.hand.push(source),
        CardPile::DrawPile => state.piles.draw_pile.push(source),
    }
    Ok(())
}

fn forced_source_card_destination(
    state: &mut CombatState,
    definition: &crate::card::CardDefinition,
) -> CardPile {
    if definition.card_type == CardType::Power {
        return CardPile::DiscardPile;
    }
    if state.relics.contains(&Relic::StrangeSpoon) && state.rng.card_random_rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn delayed_source_card_destination(
    state: &mut CombatState,
    definition: &crate::card::CardDefinition,
) -> CardPile {
    if definition.keywords.exhaust
        || (definition.card_type == CardType::Skill && state.player.powers.corruption > 0)
    {
        delayed_source_exhaust_destination(state)
    } else {
        CardPile::DiscardPile
    }
}

fn delayed_source_exhaust_destination(state: &mut CombatState) -> CardPile {
    if !state.relics.contains(&Relic::StrangeSpoon) {
        return CardPile::ExhaustPile;
    }
    if state.rng.card_random_rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn force_exhaust_armaments_source(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<()> {
    if let Some(position) = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
    {
        let source = state.piles.hand.remove(position);
        let definition = get_card_definition(source.content_id)
            .ok_or(SimError::UnknownContent(source.content_id))?;
        match forced_source_card_destination(state, definition) {
            CardPile::ExhaustPile => {
                state.piles.exhaust_pile.push(source);
                apply_on_exhaust_effects(state, source_card_id)?;
            }
            CardPile::DiscardPile => state.piles.discard_pile.push(source),
            CardPile::Hand => state.piles.hand.push(source),
            CardPile::DrawPile => state.piles.draw_pile.push(source),
        }
        return Ok(());
    }
    if state
        .piles
        .exhaust_pile
        .iter()
        .chain(state.piles.discard_pile.iter())
        .any(|card| card.id == source_card_id)
    {
        return Ok(());
    }
    Err(SimError::UnknownCard(source_card_id))
}

fn confirm_armaments_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let selected = *state
        .piles
        .hand
        .get(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?;
    if selected.id == source_card_id {
        return Err(SimError::IllegalAction("cannot upgrade Armaments"));
    }
    // Ordinary hand Armaments is still delayed in hand. Force-played Armaments
    // (Havoc / Mayhem / Distilled Chaos) now stays there too until CONFIRM so
    // Charon's Ashes waits for the closed screen (FIDL01254). Older early-exhaust
    // leftovers may already sit in exhaust/discard.
    let source_already_settled = state
        .piles
        .exhaust_pile
        .iter()
        .chain(state.piles.discard_pile.iter())
        .any(|card| card.id == source_card_id);
    if !source_already_settled {
        card_content_definition(state, source_card_id)?;
    }
    let upgraded = upgrade_card_instance(selected)?
        .ok_or(SimError::IllegalAction("selected card cannot be upgraded"))?;
    let upgradeable_count = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != source_card_id && card_instance_is_upgradeable(card))
        .count();
    let cannot_upgrade = if upgradeable_count > 1 {
        let card_ids: Vec<CardId> = state
            .piles
            .hand
            .iter()
            .filter(|card| card.id != source_card_id && !card_instance_is_upgradeable(card))
            .map(|card| card.id)
            .collect();
        card_ids
            .into_iter()
            .map(|card_id| remove_card_from_pile(state, card_id, CardPile::Hand))
            .collect::<SimResult<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let selected_card_id = selected.id;
    let _removed = remove_card_from_pile(state, selected_card_id, CardPile::Hand)?;
    // ArmamentsAction returns the upgraded card (and any cards it pulled out of
    // the select projection) before UseCardAction settles the source. Dark
    // Embrace / Dead Branch draws therefore land after Rampage+ and leftover
    // upgraded cards (FIDL01334 Berserk after Havoc+).
    state.piles.hand.push(upgraded);
    state.piles.hand.extend(cannot_upgrade);
    if state.play_top_force_exhaust_active && !source_already_settled {
        force_exhaust_armaments_source(state, source_card_id)?;
    } else if !source_already_settled {
        // Ordinary hand Armaments still settles the delayed source on CONFIRM.
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    }
    state.play_top_force_exhaust_active = false;
    Ok(())
}

fn confirm_forethought_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
) -> SimResult<()> {
    let card_id = state
        .piles
        .hand
        .get(index)
        .ok_or(SimError::IllegalAction("hand select index out of range"))?
        .id;
    if card_id == source_card_id {
        return Err(SimError::IllegalAction("cannot choose Forethought"));
    }
    move_forethought_selected_card_to_draw_bottom(state, card_id)
}

fn confirm_forethought_multi_select(
    state: &mut CombatState,
    source_card_id: CardId,
    indices: Vec<usize>,
) -> SimResult<()> {
    let mut card_ids = Vec::with_capacity(indices.len());
    for index in indices {
        let card_id = state
            .piles
            .hand
            .get(index)
            .ok_or(SimError::IllegalAction("hand select index out of range"))?
            .id;
        if card_id == source_card_id {
            return Err(SimError::IllegalAction("cannot choose Forethought"));
        }
        card_ids.push(card_id);
    }

    for card_id in card_ids {
        move_forethought_selected_card_to_draw_bottom(state, card_id)?;
    }
    Ok(())
}

fn move_forethought_card_to_draw_bottom(
    state: &mut CombatState,
    source_card_id: CardId,
    card_id: CardId,
) -> SimResult<()> {
    let source_definition = forethought_source_definition(state, source_card_id)?;
    move_forethought_selected_card_to_draw_bottom(state, card_id)?;
    move_forethought_source_card(state, source_card_id, source_definition)
}

fn forethought_source_definition(
    state: &CombatState,
    source_card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    card_content_definition(state, source_card_id)
}

fn move_forethought_selected_card_to_draw_bottom(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<()> {
    let mut card = remove_card_from_pile(state, card_id, CardPile::Hand)?;
    crate::combat::cost::set_card_cost_for_turn(&mut card, 0)?;
    card.free_to_play_once = true;
    state.piles.draw_pile.insert(0, card);
    Ok(())
}

fn move_forethought_source_card(
    state: &mut CombatState,
    source_card_id: CardId,
    _source_definition: &'static crate::card::CardDefinition,
) -> SimResult<()> {
    if state.play_top_force_exhaust_active {
        force_exhaust_armaments_source(state, source_card_id)?;
        state.play_top_force_exhaust_active = false;
    } else {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    }
    // Forethought auto-place and confirmed selections settle the source via
    // delayed discard/exhaust rather than MoveCard. If that emptied the hand,
    // Unceasing Top still draws the current top (FIDL01739).
    if state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    Ok(())
}

pub(super) fn confirm_dual_wield_select(
    state: &mut CombatState,
    source_card_id: CardId,
    index: usize,
    restore_on_confirm: Vec<CardInstance>,
    force_exhaust: bool,
) -> SimResult<()> {
    if index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("hand select index out of range"));
    }
    // await_hand_select parks Dual Wield in limbo (cardInUse). Return it to hand
    // so delayed source settlement can exhaust/discard it after copies.
    if let Some(limbo_index) = state
        .piles
        .limbo
        .iter()
        .position(|card| card.id == source_card_id)
    {
        let source = state.piles.limbo.remove(limbo_index);
        state.piles.hand.push(source);
    }
    let source_card_in_hand = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == source_card_id)
        .copied();
    let source_definition = card_content_definition(state, source_card_id)?;
    let copy_count = if source_definition.id == DUAL_WIELD_PLUS_ID {
        2
    } else {
        1
    };
    // Restore cards are held outside piles after take_hand_select; park them in
    // limbo so reserve_card_instance_ids cannot reuse their IDs for copies
    // (trace-session-8 Dual Wield + Defend/Ghostly Armor).
    state.piles.limbo.extend(restore_on_confirm.iter().copied());
    let mut next_id = state.reserve_card_instance_ids(copy_count)?;
    state.piles.limbo.retain(|card| {
        !restore_on_confirm
            .iter()
            .any(|restore| restore.id == card.id)
    });
    let mut selected = None;
    let mut unselected_selectable = Vec::new();
    let mut nonselectable = Vec::new();
    let mut preserved_order = Vec::new();
    for (hand_index, card) in std::mem::take(&mut state.piles.hand)
        .into_iter()
        .enumerate()
    {
        if card.id == source_card_id {
            continue;
        }
        preserved_order.push(card);
        if hand_index == index {
            selected = Some(card);
        } else if dual_wield_select_allows_card(&card) {
            unselected_selectable.push(card);
        } else {
            nonselectable.push(card);
        }
    }
    let selected = selected.ok_or(SimError::IllegalAction("hand select index out of range"))?;
    // Singleton force-play (FIDL01368): no select screen, MakeTempCardInHand
    // appends copies, and the original hand order is unchanged.
    // Multi-select order (9074cf38): remaining attacks/powers, then skills
    // restored from the open-screen drop, then other leftovers, then the
    // selected card and its copies.
    if restore_on_confirm.is_empty() && unselected_selectable.is_empty() {
        state.piles.hand = preserved_order;
    } else {
        state.piles.hand = unselected_selectable;
        state.piles.hand.extend(restore_on_confirm);
        state.piles.hand.extend(nonselectable);
        state.piles.hand.push(selected);
    }
    for _ in 0..copy_count {
        let mut copy = selected;
        copy.id = CardId::new(next_id);
        copy.combat_only = true;
        state.piles.hand.push(copy);
        next_id += 1;
    }
    if let Some(source_card) = source_card_in_hand {
        state.piles.hand.push(source_card);
    }
    if force_exhaust {
        // Havoc/Mayhem/Distilled Chaos: exhaustOnUseOnce, not the card keyword.
        if let Some(position) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let source = state.piles.hand.remove(position);
            state.piles.exhaust_pile.push(source);
            apply_on_exhaust_effects(state, source_card_id)?;
        } else if !state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id)
        {
            return Err(SimError::UnknownCard(source_card_id));
        }
    } else {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    }
    state.play_top_force_exhaust_active = false;
    Ok(())
}

/// DualWieldAction skipped-retrieval: no MakeTempCardInHand copies; selected card
/// stays outside serialized piles until end-turn discard flush; Dual Wield source
/// still exhausts (force-play / exhaust keyword) so Dark Embrace can draw.
#[cfg(test)]
fn confirm_dual_wield_select_skipped_retrieval(state: &mut CombatState) -> SimResult<()> {
    confirm_dual_wield_select_skipped_retrieval_with_restore(state, true)
}

/// Same skipped DualWieldAction path, but dropped non-Attack/Power cards stay
/// off the serialized piles. SuperFastMode can complete `tickDuration` before
/// the select screen returns those cards (FIDL01816 Rage/Wound, FIDL01715
/// Sentinel).
#[cfg(test)]
fn confirm_dual_wield_select_skipped_retrieval_without_restore(
    state: &mut CombatState,
) -> SimResult<()> {
    confirm_dual_wield_select_skipped_retrieval_with_restore(state, false)
}

fn confirm_dual_wield_select_skipped_retrieval_with_restore(
    state: &mut CombatState,
    restore_dropped: bool,
) -> SimResult<()> {
    let (hand_select, pending_actions) = state
        .take_hand_select()
        .ok_or(SimError::IllegalAction("no hand select is open"))?;
    if hand_select.purpose != HandSelectPurpose::DualWieldCopy {
        return Err(SimError::IllegalAction(
            "skipped Dual Wield retrieval requires DualWieldCopy hand select",
        ));
    }
    let index = hand_select
        .selected_hand_index
        .ok_or(SimError::IllegalAction(
            "Dual Wield requires a selected card",
        ))?;
    if index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("hand select index out of range"));
    }
    if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
        return Err(SimError::IllegalAction(
            "pending hidden hand card already occupied",
        ));
    }
    let selected = state.piles.hand.remove(index);
    state.pending_hidden_hand_card_until_end_turn = vec![selected];
    if restore_dropped {
        // Skills dropped for the CM hand projection return on some skipped
        // frames (9074cf38 Defend_R). Other SuperFastMode skips never return
        // them (FIDL01816).
        state
            .piles
            .hand
            .extend(hand_select.dual_wield_restore_on_confirm);
    }

    // Restore limbo-parked source then settle.
    if let Some(limbo_index) = state
        .piles
        .limbo
        .iter()
        .position(|card| card.id == hand_select.source_card_id)
    {
        let source = state.piles.limbo.remove(limbo_index);
        state.piles.hand.push(source);
    }
    if hand_select.dual_wield_force_exhaust {
        if let Some(position) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == hand_select.source_card_id)
        {
            let source = state.piles.hand.remove(position);
            state.piles.exhaust_pile.push(source);
            apply_on_exhaust_effects(state, hand_select.source_card_id)?;
        } else if !state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == hand_select.source_card_id)
        {
            return Err(SimError::UnknownCard(hand_select.source_card_id));
        }
    } else if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == hand_select.source_card_id)
        || state
            .piles
            .exhaust_pile
            .iter()
            .chain(state.piles.discard_pile.iter())
            .any(|card| card.id == hand_select.source_card_id)
    {
        move_delayed_played_source_with_strange_spoon(state, hand_select.source_card_id)?;
    }
    state.play_top_force_exhaust_active = false;
    if !pending_actions.is_empty() {
        let transition = process_internal_queue(state, pending_actions)?;
        *state = transition.state;
    }
    state.activate_next_queued_decision_if_idle();
    settle_time_warp_end_turn_if_ready(state)?;
    Ok(())
}

pub(super) fn dual_wield_select_allows_card(card: &CardInstance) -> bool {
    get_card_definition(card.content_id).is_some_and(|definition| {
        matches!(definition.card_type, CardType::Attack | CardType::Power)
    })
}

/// Skills hidden by force Dual Wield multi-select return on CONFIRM. A
/// deck-owned curse also returns; combat-generated curses/statuses are handled
/// separately by the `combat_only` guard at the call site (Parasite versus
/// Shame, FIDL01438/FIDL00242).
pub(super) fn dual_wield_non_eligible_restores_on_confirm(card: &CardInstance) -> bool {
    get_card_definition(card.content_id).is_some_and(|definition| {
        definition.card_type == CardType::Skill
            || (crate::content::cards::is_curse_content_id(card.content_id) && !card.combat_only)
    })
}

pub fn open_discard_select_with_max_choices(
    state: &mut CombatState,
    max_choices: usize,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Err(SimError::IllegalAction("discard pile is empty"));
    }
    state.decision = Some(CombatDecisionState::DiscardSelect {
        state: crate::combat::DiscardSelectState {
            purpose: DiscardSelectPurpose::LiquidMemoriesReturnToHand,
            source_card_id: None,
            source_card: None,
            source_card_force_exhaust: false,
            selected_discard_indices: Vec::new(),
            max_choices,
            selected_discard_index: None,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(())
}

pub fn choose_discard_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    discard_select_ui_to_discard_index(state, ui_index)?;
    let discard_select = state
        .discard_select_mut()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose == DiscardSelectPurpose::LiquidMemoriesReturnToHand {
        if let Some(position) = discard_select
            .selected_discard_indices
            .iter()
            .position(|index| *index == ui_index)
        {
            discard_select.selected_discard_indices.remove(position);
        } else {
            if discard_select.selected_discard_indices.len() >= discard_select.max_choices {
                return Err(SimError::IllegalAction("too many discard select choices"));
            }
            discard_select.selected_discard_indices.push(ui_index);
        }
        discard_select.selected_discard_index =
            discard_select.selected_discard_indices.first().copied();
    } else {
        discard_select.selected_discard_index = Some(ui_index);
    }
    Ok(())
}

pub fn discard_select_ui_to_discard_index(
    state: &CombatState,
    ui_index: usize,
) -> SimResult<usize> {
    state
        .discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if ui_index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    Ok(ui_index)
}

pub fn confirm_liquid_memories_select(state: &mut CombatState) -> SimResult<()> {
    let discard_select = state
        .take_discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::LiquidMemoriesReturnToHand {
        return Err(SimError::IllegalAction("discard select purpose mismatch"));
    }
    let mut selected = discard_select.selected_discard_indices;
    if selected.is_empty() {
        selected.extend(discard_select.selected_discard_index);
    }
    if selected.is_empty() {
        return Err(SimError::IllegalAction("discard select choice is required"));
    }
    if selected.len() > discard_select.max_choices {
        return Err(SimError::IllegalAction("too many discard select choices"));
    }
    selected.sort_unstable();
    selected.dedup();
    for index in &selected {
        if *index >= state.piles.discard_pile.len() {
            return Err(SimError::IllegalAction("discard select index out of range"));
        }
    }
    let mut cards = Vec::new();
    for index in selected.into_iter().rev() {
        let mut card = state.piles.discard_pile.remove(index);
        crate::combat::cost::set_card_cost_for_turn(&mut card, 0)?;
        cards.push(card);
    }
    cards.reverse();
    state.piles.hand.extend(cards);
    resume_actions_after_discard_select(state, discard_select.pending_actions)?;
    state.activate_next_queued_decision_if_idle();
    Ok(())
}

fn resume_actions_after_discard_select(
    state: &mut CombatState,
    pending_actions: VecDeque<InternalAction>,
) -> SimResult<()> {
    if pending_actions.is_empty() {
        return Ok(());
    }
    let transition = process_internal_queue(state, pending_actions)?;
    *state = transition.state;
    Ok(())
}

pub fn confirm_discard_select(state: &mut CombatState) -> SimResult<usize> {
    let purpose = state
        .discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?
        .purpose;
    match purpose {
        DiscardSelectPurpose::LiquidMemoriesReturnToHand => {
            confirm_liquid_memories_select(state)?;
            Ok(0)
        }
        DiscardSelectPurpose::HeadbuttPutOnDraw => confirm_headbutt_select(state),
        DiscardSelectPurpose::HologramReturnToHand => confirm_hologram_select(state),
    }
}

fn confirm_hologram_select(state: &mut CombatState) -> SimResult<usize> {
    let discard_select = state
        .take_discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::HologramReturnToHand {
        return Err(SimError::IllegalAction("discard select purpose mismatch"));
    }
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    if index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    let selected = state.piles.discard_pile.remove(index);
    if state.piles.hand.len() < crate::combat::draw::MAX_HAND_SIZE {
        state.piles.hand.push(selected);
    } else {
        state.piles.discard_pile.push(selected);
    }
    let source = discard_select.source_card.ok_or(SimError::IllegalAction(
        "Hologram discard select is missing its source card",
    ))?;
    let dead_branch_count = settle_hologram_source_after_discard_select(
        state,
        source,
        discard_select.source_card_force_exhaust,
    )?;
    state.play_top_force_exhaust_active = false;
    resume_actions_after_discard_select(state, discard_select.pending_actions)?;
    state.activate_next_queued_decision_if_idle();
    Ok(dead_branch_count)
}

pub(super) fn settle_hologram_source_after_discard_select(
    state: &mut CombatState,
    source: CardInstance,
    force_exhaust: bool,
) -> SimResult<usize> {
    let definition = get_card_definition(source.content_id)
        .ok_or(SimError::UnknownContent(source.content_id))?;
    let corruption_exhausts =
        definition.card_type == CardType::Skill && state.player.powers.corruption > 0;
    let exhaust = force_exhaust || source.upgrades == 0 || corruption_exhausts;
    settle_headbutt_source_after_discard_select(state, Some(source), exhaust)
}

pub fn confirm_headbutt_select(state: &mut CombatState) -> SimResult<usize> {
    let discard_select = state
        .take_discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return Err(SimError::IllegalAction("discard select purpose mismatch"));
    }
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    if index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    // Headbutt's PutOnDeck always returns the chosen discard card to the top of
    // the draw pile, including when Headbutt itself was already force-exhausted
    // by Havoc / Mayhem / similar PlayTopCard(exhaust=true) paths. The forced
    // source may already sit in exhaust; only the source settlement is skipped
    // below, never the put-on-draw.
    let card = state.piles.discard_pile.remove(index);
    state.piles.draw_pile.push(card);
    let forced_top_draw_source = discard_select.source_card.is_none()
        && discard_select.source_card_id.is_some_and(|source_card_id| {
            state
                .piles
                .exhaust_pile
                .iter()
                .any(|source_card| source_card.id == source_card_id)
        });
    let force_exhaust =
        discard_select.source_card_force_exhaust || state.play_top_force_exhaust_active;
    let mut dead_branch_count = 0;
    if let Some(source_card) = discard_select.source_card {
        dead_branch_count +=
            settle_headbutt_source_after_discard_select(state, Some(source_card), force_exhaust)?;
    } else if let Some(source_card_id) = discard_select.source_card_id {
        if !forced_top_draw_source {
            if force_exhaust {
                if let Some(position) = state
                    .piles
                    .hand
                    .iter()
                    .position(|card| card.id == source_card_id)
                {
                    let source = state.piles.hand.remove(position);
                    dead_branch_count +=
                        settle_headbutt_source_after_discard_select(state, Some(source), true)?;
                }
            } else {
                move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
            }
        }
    }
    state.play_top_force_exhaust_active = false;
    // Pen Nib applies only to the completed top-played attack. Headbutt's
    // source movement is deferred while this grid is open, so closing the grid
    // is the equivalent card-use boundary that clears the scope.
    state.pen_nib_double_active = false;
    if !discard_select.pending_actions.is_empty() {
        let transition = process_internal_queue(state, discard_select.pending_actions)?;
        *state = transition.state;
    }
    if state.decision.is_none() {
        flush_pending_monster_death_relics_if_ready(state)?;
    }
    state.activate_next_queued_decision_if_idle();
    Ok(dead_branch_count)
}

pub(super) fn settle_headbutt_source_after_discard_select(
    state: &mut CombatState,
    source_card: Option<CardInstance>,
    force_exhaust: bool,
) -> SimResult<usize> {
    let Some(source) = source_card else {
        return Ok(0);
    };
    let source_id = source.id;
    if force_exhaust {
        let definition = get_card_definition(source.content_id)
            .ok_or(SimError::UnknownContent(source.content_id))?;
        let dead_branch_count = match forced_source_card_destination(state, definition) {
            CardPile::ExhaustPile => {
                state.piles.exhaust_pile.push(source);
                // Relic onExhaust (Dead Branch) before power onExhaust (Dark
                // Embrace), same as Purity / Secret Technique. Headbutt's
                // UseCardAction is paused while the discard grid is open, so
                // CardExhausted never runs for this source (FIDL01410).
                apply_purity_card_exhausted(state, source_id)?
            }
            CardPile::DiscardPile => {
                state.piles.discard_pile.push(source);
                0
            }
            CardPile::Hand => {
                state.piles.hand.push(source);
                0
            }
            CardPile::DrawPile => {
                state.piles.draw_pile.push(source);
                0
            }
        };
        state.play_top_force_exhaust_active = false;
        return Ok(dead_branch_count);
    }
    state.piles.discard_pile.push(source);
    Ok(0)
}

/// Close Headbutt without putting the chosen discard card on draw.
/// Force-played Headbutt still exhausts so Dark Embrace / Feel No Pain /
/// Charon's Ashes / Dead Branch resolve after the grid closes.
#[cfg(test)]
fn confirm_headbutt_select_skipped_retrieval(state: &mut CombatState) -> SimResult<usize> {
    confirm_headbutt_select_skipped_retrieval_with_time_warp_policy(state, true)
}

#[cfg(test)]
fn confirm_headbutt_select_skipped_retrieval_with_time_warp_policy(
    state: &mut CombatState,
    settle_time_warp: bool,
) -> SimResult<usize> {
    let discard_select = state
        .take_discard_select()
        .ok_or(SimError::IllegalAction("no discard select is open"))?;
    if discard_select.purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return Err(SimError::IllegalAction(
            "skipped Headbutt retrieval requires Headbutt discard select",
        ));
    }
    let index = discard_select
        .selected_discard_index
        .ok_or(SimError::IllegalAction("discard select choice is required"))?;
    if index >= state.piles.discard_pile.len() {
        return Err(SimError::IllegalAction("discard select index out of range"));
    }
    let force_exhaust =
        discard_select.source_card_force_exhaust || state.play_top_force_exhaust_active;
    if discard_select.source_card.is_none() && !force_exhaust {
        return Err(SimError::IllegalAction(
            "skipped Headbutt retrieval requires force-played source",
        ));
    }
    let dead_branch_count = settle_headbutt_source_after_discard_select(
        state,
        discard_select.source_card,
        force_exhaust,
    )?;
    state.play_top_force_exhaust_active = false;
    state.pen_nib_double_active = false;
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    state.defer_time_warp_end_turn = true;
    if !discard_select.pending_actions.is_empty() {
        let transition = process_internal_queue(state, discard_select.pending_actions)?;
        *state = transition.state;
    }
    state.defer_time_warp_end_turn = previous_defer_time_warp;
    if state.decision.is_none() {
        flush_pending_monster_death_relics_if_ready(state)?;
    }
    state.activate_next_queued_decision_if_idle();
    if settle_time_warp {
        settle_time_warp_end_turn_if_ready(state)?;
    }
    Ok(dead_branch_count)
}

pub fn open_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: crate::combat::ExhaustSelectState {
            purpose: crate::combat::ExhaustSelectPurpose::Exhaust,
            source_card_id: None,
            source_card: None,
            source_card_force_exhaust: false,
            selected_hand_indices: Vec::new(),
            interrupted_by_cultist_potion: false,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(())
}

pub fn open_gambling_chip_select(state: &mut CombatState) -> SimResult<()> {
    if state.piles.hand.is_empty() && state.pending_opening_hand_draw == 0 {
        return Ok(());
    }
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: crate::combat::ExhaustSelectState {
            purpose: crate::combat::ExhaustSelectPurpose::GamblingChip,
            source_card_id: None,
            source_card: None,
            source_card_force_exhaust: false,
            selected_hand_indices: Vec::new(),
            interrupted_by_cultist_potion: false,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(())
}

pub fn choose_exhaust_select(state: &mut CombatState, ui_index: usize) -> SimResult<()> {
    let pile_index = exhaust_select_ui_to_hand_index(state, ui_index)?;
    let purity_cap = state
        .exhaust_select()
        .filter(|select| select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3)
        .map(|select| {
            let source_card_id = select
                .source_card_id
                .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
            purity_select_cap(state, source_card_id)
        })
        .transpose()?;
    let exhaust_select = state
        .exhaust_select_mut()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
        exhaust_select.selected_hand_indices.clear();
        exhaust_select.selected_hand_indices.push(pile_index);
        return Ok(());
    }
    if matches!(
        exhaust_select.purpose,
        crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne
            | crate::combat::ExhaustSelectPurpose::RecycleExhaustOne
    ) {
        exhaust_select.selected_hand_indices.clear();
        exhaust_select.selected_hand_indices.push(pile_index);
        return Ok(());
    }
    if let Some(position) = exhaust_select
        .selected_hand_indices
        .iter()
        .position(|index| *index == pile_index)
    {
        exhaust_select.selected_hand_indices.remove(position);
    } else {
        if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3
            && exhaust_select.selected_hand_indices.len() >= purity_cap.unwrap_or(3)
        {
            return Err(purity_too_many_cards_error(purity_cap.unwrap_or(3)));
        }
        exhaust_select.selected_hand_indices.push(pile_index);
    }
    Ok(())
}

pub fn exhaust_select_ui_to_hand_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    let exhaust_select = state
        .exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand {
        return exhumable_ui_to_exhaust_index(state, ui_index);
    }
    if exhaust_select.purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        let source_card_id = exhaust_select
            .source_card_id
            .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
        return exhaust_select_visible_hand_indices(state)
            .into_iter()
            .filter(|index| state.piles.hand[*index].id != source_card_id)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    if matches!(
        exhaust_select.purpose,
        crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne
            | crate::combat::ExhaustSelectPurpose::RecycleExhaustOne
    ) {
        let source_card_id = exhaust_select.source_card_id;
        return state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| Some(card.id) != source_card_id)
            .map(|(index, _)| index)
            .nth(ui_index)
            .ok_or(SimError::IllegalAction("exhaust select index out of range"));
    }
    exhaust_select_visible_hand_indices(state)
        .into_iter()
        .nth(ui_index)
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))
}

fn exhaust_select_visible_hand_indices(state: &CombatState) -> Vec<usize> {
    let Some(exhaust_select) = state.exhaust_select() else {
        return Vec::new();
    };
    state
        .piles
        .hand
        .iter()
        .enumerate()
        .filter(|(index, _)| !exhaust_select.selected_hand_indices.contains(index))
        .map(|(index, _)| index)
        .collect()
}

pub fn confirm_exhaust_select(state: &mut CombatState) -> SimResult<()> {
    confirm_exhaust_select_with_time_warp_policy(state, true)
}

/// Like [`confirm_exhaust_select`], with optional Time Warp end-turn settle.
pub fn confirm_exhaust_select_with_time_warp_policy(
    state: &mut CombatState,
    settle_time_warp: bool,
) -> SimResult<()> {
    confirm_exhaust_select_with_dead_branch_count(state, settle_time_warp).map(|_| ())
}

/// Confirm an exhaust selection and report Dead Branch cards settled by the
/// combat transition itself. Run-level select dispatch uses the count to avoid
/// applying its legacy boundary fallback a second time.
pub(crate) fn confirm_exhaust_select_with_dead_branch_count(
    state: &mut CombatState,
    settle_time_warp: bool,
) -> SimResult<usize> {
    let exhaust_select = state
        .take_exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    if !settle_time_warp {
        // The caller wants the post-CONFIRM pre-END publication. Pending
        // actions can themselves drain through process_internal_queue, so the
        // policy must be active while they run, not only at the final explicit
        // settle helper (FIDL01358).
        state.defer_time_warp_end_turn = true;
    }
    let purpose = exhaust_select.purpose;
    let mut pending_actions = exhaust_select.pending_actions.clone();
    // HandCardSelectScreen keeps the played source as target cardInUse while
    // selected-card callbacks, draws, Hex, and Dead Branch may allocate IDs.
    // Reserve that ID without duplicating the full card into a second pile.
    // Purity has its own explicit limbo transfer inside specialized settlement.
    let previous_card_in_use = state.card_in_use;
    if purpose != crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        if let Some(source) = exhaust_select.source_card {
            state.card_in_use = Some(source.id);
        }
    }
    let mut dead_branch_count = 0;
    let burning_pact_drains_pending = matches!(
        purpose,
        crate::combat::ExhaustSelectPurpose::BurningPactDraw2
            | crate::combat::ExhaustSelectPurpose::BurningPactDraw3
    );
    let mut older_purity_thorns = VecDeque::new();
    if purpose == crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        // BeatOfDeathPower.onAfterUseCard was queued before the selection
        // screen paused the manager. Purity's selected-card ExhaustAction runs
        // first; the older Beat hit then precedes source-card exhaust/FNP.
        pending_actions.retain(|action| {
            if matches!(action, InternalAction::DealThornsDamageToPlayer { .. }) {
                older_purity_thorns.push_back(*action);
                false
            } else {
                true
            }
        });
    }
    match purpose {
        crate::combat::ExhaustSelectPurpose::GamblingChip => {
            confirm_gambling_chip_select(state, exhaust_select.selected_hand_indices)?;
        }
        crate::combat::ExhaustSelectPurpose::ExhumeReturnToHand => {
            confirm_exhume_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 => {
            dead_branch_count = confirm_purity_select(state, exhaust_select, older_purity_thorns)?;
        }
        crate::combat::ExhaustSelectPurpose::BurningPactDraw2 => {
            dead_branch_count = confirm_burning_pact_select(state, exhaust_select, 2)?;
        }
        crate::combat::ExhaustSelectPurpose::BurningPactDraw3 => {
            dead_branch_count = confirm_burning_pact_select(state, exhaust_select, 3)?;
        }
        crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne => {
            confirm_true_grit_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::RecycleExhaustOne => {
            confirm_recycle_select(state, exhaust_select)?;
        }
        crate::combat::ExhaustSelectPurpose::Exhaust => {
            let selected =
                unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
            let mut removal_order = selected.clone();
            removal_order.sort_unstable();
            for index in &removal_order {
                if *index >= state.piles.hand.len() {
                    return Err(SimError::IllegalAction("exhaust select index out of range"));
                }
            }
            let exhausted = selected
                .iter()
                .map(|index| state.piles.hand[*index])
                .collect::<Vec<_>>();
            for index in removal_order.into_iter().rev() {
                state.piles.hand.remove(index);
            }
            // Elixir (and any other ExhaustSelectPurpose::Exhaust source) moves
            // selected cards into the exhaust pile permanently for the rest of
            // combat, matching ExhaustAction.moveToExhaustPile. On-exhaust
            // effects (Feel No Pain, Dark Embrace, etc.) still fire here.
            for card in exhausted {
                let card_id = card.id;
                state.piles.exhaust_pile.push(card);
                apply_on_exhaust_effects(state, card_id)?;
            }
        }
    }
    state.card_in_use = previous_card_in_use;
    // When deferred Hex MakeTempCardInDrawPile lands on an empty draw pile,
    // CardGroup.addToRandomSpot just group.add(c) — no cardRandomRng roll
    // (see desktop-1.0 CardGroup.addToRandomSpot). Do not burn a phantom
    // random_int(0) here: it desyncs later Hex inserts (15ab4cc Battle Trance
    // Dazed index 6 vs 5 → unplayable PLAY).
    // Burning Pact already drained these after DrawCardAction(2/3) and before
    // Evolve follow-ups (FIDL01740).
    if !burning_pact_drains_pending && !pending_actions.is_empty() {
        let transition = process_internal_queue(state, pending_actions)?;
        *state = transition.state;
    }
    // Burning Pact draws apply Fire Breathing immediately. Those pulses can
    // kill the last monster without going through process_internal_queue when
    // no bot-queued on-exhaust follow-ups exist (FIDL01773 Spheric Guardian).
    settle_combat_end_from_current_hp(state)?;
    state.defer_time_warp_end_turn = previous_defer_time_warp;
    state.activate_next_queued_decision_if_idle();
    if settle_time_warp {
        settle_time_warp_end_turn_if_ready(state)?;
    }
    Ok(dead_branch_count)
}

/// Mark Won/Lost after a path that can kill without finishing `process_internal_queue`.
pub(crate) fn settle_combat_end_from_current_hp(state: &mut CombatState) -> SimResult<()> {
    if state.player.hp <= 0 {
        state.player.hp = 0;
        state.player.block = 0;
        state.phase = CombatPhase::Lost;
        state.clear_decisions_on_combat_end();
        return Ok(());
    }
    if state
        .monsters
        .iter()
        .all(|monster| !monster.alive && !awakened_one_is_half_dead(monster))
    {
        // Nested process_internal_queue (e.g. Havoc PlayTop) may already have
        // set Won and applied Burning Blood. Re-applying doubles the heal
        // (13efa069: 8060+6+6 → 8072 vs real 8066).
        let already_won = state.phase == CombatPhase::Won;
        state.phase = CombatPhase::Won;
        state.clear_decisions_on_combat_end();
        if !already_won {
            apply_burning_blood(state)?;
        }
    }
    Ok(())
}

fn confirm_true_grit_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let source_card_id = exhaust_select.source_card_id;
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let target_index = selected.first().copied().ok_or(SimError::IllegalAction(
        "True Grit requires a selected card",
    ))?;
    if target_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    let target_card_id = state.piles.hand[target_index].id;
    if Some(target_card_id) == source_card_id {
        return Err(SimError::IllegalAction("True Grit cannot exhaust itself"));
    }

    let target_position = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == target_card_id)
        .ok_or(SimError::UnknownCard(target_card_id))?;
    let target_card = state.piles.hand.remove(target_position);
    // ExhaustAction always moves the selection to exhaust before resolving
    // on-exhaust hooks.
    state.piles.exhaust_pile.push(target_card);
    apply_on_exhaust_effects(state, target_card_id)?;

    // A prior skipped hand-selection can still own cards in the source
    // HandCardSelectScreen while a forced True Grit selection is being
    // serviced. The source exhausts those stale selectedCards in the same
    // ExhaustAction batch, after the visible choice and before True Grit's own
    // forced settlement (FIDL01272: Heavy Blade, Inflame, True Grit).
    let pending_hidden = std::mem::take(&mut state.pending_hidden_hand_card_until_end_turn);
    state.pending_hidden_hand_card_exhausts_with_fiend_fire = false;
    for card in pending_hidden {
        let card_id = card.id;
        state.piles.exhaust_pile.push(card);
        apply_on_exhaust_effects(state, card_id)?;
    }

    if let Some(source_card) = exhaust_select.source_card {
        let source_id = source_card.id;
        if exhaust_select.source_card_force_exhaust {
            let definition = get_card_definition(source_card.content_id)
                .ok_or(SimError::UnknownContent(source_card.content_id))?;
            match forced_source_card_destination(state, definition) {
                CardPile::ExhaustPile => {
                    state.piles.exhaust_pile.push(source_card);
                    apply_on_exhaust_effects(state, source_id)?;
                }
                CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
                CardPile::Hand => state.piles.hand.push(source_card),
                CardPile::DrawPile => state.piles.draw_pile.push(source_card),
            }
        } else {
            // Ordinary hand-played True Grit+ follows UseCardAction settlement.
            // Corruption (or an intrinsic exhaust keyword) still rewrites that
            // delayed source move to exhaust after the selection closes.
            let definition = get_card_definition(source_card.content_id)
                .ok_or(SimError::UnknownContent(source_card.content_id))?;
            match delayed_source_card_destination(state, definition) {
                CardPile::ExhaustPile => {
                    state.piles.exhaust_pile.push(source_card);
                    apply_on_exhaust_effects(state, source_id)?;
                }
                CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
                CardPile::Hand => state.piles.hand.push(source_card),
                CardPile::DrawPile => state.piles.draw_pile.push(source_card),
            }
        }
    } else if let Some(source_card_id) = source_card_id {
        if let Some(source_position) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let source_card = state.piles.hand.remove(source_position);
            if exhaust_select.source_card_force_exhaust {
                let definition = get_card_definition(source_card.content_id)
                    .ok_or(SimError::UnknownContent(source_card.content_id))?;
                match forced_source_card_destination(state, definition) {
                    CardPile::ExhaustPile => {
                        state.piles.exhaust_pile.push(source_card);
                        apply_on_exhaust_effects(state, source_card_id)?;
                    }
                    CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
                    CardPile::Hand => state.piles.hand.push(source_card),
                    CardPile::DrawPile => state.piles.draw_pile.push(source_card),
                }
            } else {
                let definition = get_card_definition(source_card.content_id)
                    .ok_or(SimError::UnknownContent(source_card.content_id))?;
                match delayed_source_card_destination(state, definition) {
                    CardPile::ExhaustPile => {
                        state.piles.exhaust_pile.push(source_card);
                        apply_on_exhaust_effects(state, source_card_id)?;
                    }
                    CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
                    CardPile::Hand => state.piles.hand.push(source_card),
                    CardPile::DrawPile => state.piles.draw_pile.push(source_card),
                }
            }
        } else if !state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id)
        {
            return Err(SimError::UnknownCard(source_card_id));
        }
    }
    Ok(())
}

fn confirm_recycle_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let target_index = selected
        .first()
        .copied()
        .ok_or(SimError::IllegalAction("Recycle requires a selected card"))?;
    if target_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    let target_card = state.piles.hand.remove(target_index);
    let target_cost = effective_card_cost(&target_card)?;
    state.piles.exhaust_pile.push(target_card);
    apply_on_exhaust_effects(state, target_card.id)?;
    state.player.energy = state
        .player
        .energy
        .checked_add(target_cost)
        .ok_or(SimError::InvalidState("Recycle energy gain overflows i32"))?;
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.discard_pile.push(source_card);
    } else if let Some(source_card_id) = exhaust_select.source_card_id {
        move_card(state, source_card_id, CardPile::Hand, CardPile::DiscardPile)?;
    }
    Ok(())
}

/// True Grit ExhaustAction skipped-retrieval (force-played True Grit+).
///
/// When ExhaustAction completes before CONFIRM, the selected card stays owned by
/// the closed HandCardSelectScreen and only re-enters via end-turn DiscardAction
/// leftover selectedCards. True Grit itself still exhausts (force-play / exhaust
/// path) so Feel No Pain / Dead Branch fire once for the source.
#[cfg(test)]
fn confirm_true_grit_select_skipped_retrieval(state: &mut CombatState) -> SimResult<()> {
    let exhaust_select = state
        .take_exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose != crate::combat::ExhaustSelectPurpose::TrueGritExhaustOne {
        return Err(SimError::IllegalAction(
            "skipped True Grit retrieval requires TrueGritExhaustOne",
        ));
    }
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    let target_index = selected.first().copied().ok_or(SimError::IllegalAction(
        "True Grit requires a selected card",
    ))?;
    if target_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
        return Err(SimError::IllegalAction(
            "pending hidden hand card already occupied",
        ));
    }
    let target_card_id = state.piles.hand[target_index].id;
    if Some(target_card_id) == exhaust_select.source_card_id {
        return Err(SimError::IllegalAction("True Grit cannot exhaust itself"));
    }
    let selected_card = state.piles.hand.remove(target_index);
    state.pending_hidden_hand_card_until_end_turn = vec![selected_card];

    let mut follow_ups = exhaust_select.pending_actions;
    if let Some(source_card) = exhaust_select.source_card {
        let source_id = source_card.id;
        state.piles.exhaust_pile.push(source_card);
        // Use CardExhausted so Feel No Pain / Dark Embrace / Dead Branch all
        // resolve as addToBot follow-ups (FIDL00253 Evolve from Dead Branch).
        follow_ups.push_front(InternalAction::CardExhausted { card_id: source_id });
    } else if let Some(source_card_id) = exhaust_select.source_card_id {
        // Early-exhausted force path already settled the source.
        if !state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id)
            && state
                .piles
                .hand
                .iter()
                .any(|card| card.id == source_card_id)
        {
            move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
        }
    }
    if !follow_ups.is_empty() {
        let transition = process_internal_queue(state, follow_ups)?;
        *state = transition.state;
    }
    state.activate_next_queued_decision_if_idle();
    settle_time_warp_end_turn_if_ready(state)?;
    Ok(())
}

fn confirm_burning_pact_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
    draw_count: usize,
) -> SimResult<usize> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let mut selected = exhaust_select.selected_hand_indices;
    selected.sort_unstable();
    selected.dedup();
    if selected.len() != 1 {
        return Err(SimError::IllegalAction(
            "Burning Pact requires exactly one card",
        ));
    }
    let index = selected[0];
    let card = state
        .piles
        .hand
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
    if card.id == source_card_id {
        return Err(SimError::IllegalAction("Burning Pact cannot select itself"));
    }
    // Normal Burning Pact exhausts the selected card immediately (including
    // Havoc / Mayhem / Distilled Chaos top-draw plays where source_card is
    // already gone). FIDL00221 step 1274 shows Bash+Burning Pact both in
    // exhaust after Havoc→BP confirm; parking the selection in pending_hidden
    // left a cross-combat residual Bash on the next combat's first END
    // (discard_ids[N]: null != "Bash").
    //
    // Rare CommunicationMod skipped-retrieval frames (selected card absent
    // from every pile until end-turn DiscardAction) are rebuilt in the seed-
    // start verifier via seed_start_burning_pact_deferred_selection_state —
    // same pattern as Dual Wield skipped retrieval. Cultist-potion
    // interruption still parks the selection until end-turn.
    let mut deferred_bot_on_exhaust = Vec::new();
    let mut selected_feel_no_pain = Vec::new();
    let mut dead_branch_count = 0;
    if exhaust_select.interrupted_by_cultist_potion {
        state.piles.hand.remove(index);
        state.pending_hidden_hand_card_until_end_turn = vec![card];
    } else {
        state.piles.hand.remove(index);
        state.piles.exhaust_pile.push(card);
        // Dark Embrace uses addToBot after ExhaustAction. Burning Pact still
        // has DrawCardAction on the queue, so DE must not draw until those
        // resolve (9bf0204173fb2a7f step 459). Selected-card FNP must resolve
        // before UseCardAction Beat of Death (FIDL02258).
        apply_on_exhaust_effects_except_bot_queued_powers(state, card.id)?;
        selected_feel_no_pain.extend(feel_no_pain_block_follow_up(state));
        if let Some(dead_branch) = dead_branch_follow_up(state) {
            deferred_bot_on_exhaust.push(dead_branch);
            dead_branch_count += 1;
        }
        // ExhaustAction addToBots onExhaust (DE draw, then Necro MakeTemp).
        // Burning Pact already queued DrawCardAction, so do not apply Necro
        // immediately — that would insert it before BP's draws (FIDL02209).
        deferred_bot_on_exhaust.extend(queued_dark_embrace_then_necronomicurse_follow_ups(
            state, card.id,
        ));
    }
    // DrawCardAction(2/3) is queued in card.use() before HexPower.onUseCard
    // MakeTempCardInDrawPile and before UseCardAction. Evolve/Fire Breathing
    // onCardDraw use addToBot during that draw, so they land after Hex.
    // Inlining Evolve here inserted Hex into the post-Evolve pile
    // (FIDL01740 Dazed vs Battle Trance).
    let evolve_follow_ups = player_draw_cards_with_deferred_evolve(state, draw_count)?;
    if !selected_feel_no_pain.is_empty() {
        let transition = process_internal_queue(state, selected_feel_no_pain.into())?;
        *state = transition.state;
    }
    // TimeWarpPower.onAfterUseCard queues EndTurn after UseCardAction. Pending
    // ApplyDeferredTimeWarpCardPlay must increment here (with Hex) but must not
    // discard the hand before Burning Pact itself is settled (FIDL02206).
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    state.defer_time_warp_end_turn = true;
    if !exhaust_select.pending_actions.is_empty() {
        let transition = process_internal_queue(state, exhaust_select.pending_actions)?;
        *state = transition.state;
    }
    state.defer_time_warp_end_turn = previous_defer_time_warp;
    // UseCardAction settles Burning Pact after its primary DrawCardAction and
    // before Evolve's DrawCardAction follow-ups, which were addToBot'd by that
    // draw. This source membership matters if an Evolve draw reshuffles the
    // discard pile. Under Corruption (or Exhaust), source on-exhaust callbacks
    // are queued behind those already-pending Evolve draws (FIDL00425).
    if let Some(source_card) = exhaust_select.source_card {
        let definition = get_card_definition(source_card.content_id)
            .ok_or(SimError::UnknownContent(source_card.content_id))?;
        let destination = if exhaust_select.source_card_force_exhaust {
            forced_source_card_destination(state, definition)
        } else {
            delayed_source_card_destination(state, definition)
        };
        let source_id = source_card.id;
        match destination {
            CardPile::ExhaustPile => {
                state.piles.exhaust_pile.push(source_card);
                apply_on_exhaust_effects_except_bot_queued_powers(state, source_id)?;
                deferred_bot_on_exhaust.extend(feel_no_pain_block_follow_up(state));
                if let Some(dead_branch) = dead_branch_follow_up(state) {
                    deferred_bot_on_exhaust.push(dead_branch);
                    dead_branch_count += 1;
                }
                deferred_bot_on_exhaust.extend(queued_dark_embrace_then_necronomicurse_follow_ups(
                    state, source_id,
                ));
            }
            CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
            CardPile::Hand => state.piles.hand.push(source_card),
            CardPile::DrawPile => state.piles.draw_pile.push(source_card),
        }
    } else {
        move_delayed_played_source_with_strange_spoon(state, source_card_id)?;
    }
    resolve_deferred_draw_follow_ups(state, evolve_follow_ups)?;
    if !deferred_bot_on_exhaust.is_empty() {
        let transition =
            process_internal_queue(state, deferred_bot_on_exhaust.into_iter().collect())?;
        *state = transition.state;
    }
    Ok(dead_branch_count)
}

/// Confirm a Burning Pact exhaust select without retrieving the selected card.
///
/// `ExhaustAction` can complete while its hand-selection screen is open, so
/// the later `selectedCards -> exhaust` update is skipped. The selected card
/// stays outside every serialized pile, while Burning Pact's own draw and
/// source-card settlement still run. The verifier parks the returned card in
/// the run-level pending-hidden slot until the matching end-turn window.
///
/// This candidate is limited to a held Burning Pact source with no Cultist-
/// potion interruption. The source still settles through the normal delayed
/// destination, including Corruption/keyword-driven exhaust and its queued
/// on-exhaust callbacks; only the selected card's retrieval is skipped.
#[cfg(test)]
fn confirm_burning_pact_select_skipped_retrieval(
    state: &mut CombatState,
) -> SimResult<CardInstance> {
    confirm_burning_pact_select_skipped_retrieval_with_time_warp_policy(state, true)
}

#[cfg(test)]
fn confirm_burning_pact_select_skipped_retrieval_with_time_warp_policy(
    state: &mut CombatState,
    settle_time_warp: bool,
) -> SimResult<CardInstance> {
    let exhaust_select = state
        .take_exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    let draw_count = match exhaust_select.purpose {
        crate::combat::ExhaustSelectPurpose::BurningPactDraw2 => 2,
        crate::combat::ExhaustSelectPurpose::BurningPactDraw3 => 3,
        _ => {
            return Err(SimError::IllegalAction(
                "skipped Burning Pact retrieval requires a Burning Pact exhaust select",
            ));
        }
    };
    if exhaust_select.interrupted_by_cultist_potion {
        return Err(SimError::IllegalAction(
            "skipped Burning Pact retrieval cannot run after a Cultist potion interrupt",
        ));
    }
    if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
        return Err(SimError::IllegalAction(
            "pending hidden hand card already occupied",
        ));
    }
    let previous_defer_time_warp = state.defer_time_warp_end_turn;
    if !settle_time_warp {
        state.defer_time_warp_end_turn = true;
    }
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("Burning Pact source is required"))?;
    let force_exhausted_source = exhaust_select.source_card_force_exhaust
        && exhaust_select.source_card.is_none()
        && state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id);
    let source_card = exhaust_select.source_card;
    if !force_exhausted_source && source_card.is_none() {
        return Err(SimError::IllegalAction("Burning Pact source is not held"));
    }
    let source_definition = source_card
        .as_ref()
        .or_else(|| {
            state
                .piles
                .exhaust_pile
                .iter()
                .find(|card| card.id == source_card_id)
        })
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::IllegalAction(
            "Burning Pact source definition is missing",
        ))?;
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    if selected.len() != 1 {
        return Err(SimError::IllegalAction(
            "Burning Pact requires exactly one selected card",
        ));
    }
    let selected_index = selected[0];
    if selected_index >= state.piles.hand.len() {
        return Err(SimError::IllegalAction("exhaust select index out of range"));
    }
    if state.piles.hand[selected_index].id == source_card_id {
        return Err(SimError::IllegalAction("Burning Pact cannot select itself"));
    }

    let selected_card = state.piles.hand.remove(selected_index);
    // Reserve the hidden card's instance ID while the draw and deferred queue
    // settle, then return it to the verifier rather than exposing it in limbo.
    state.piles.limbo.push(selected_card);
    let evolve_follow_ups = player_draw_cards_with_deferred_evolve(state, draw_count)?;
    if !exhaust_select.pending_actions.is_empty() {
        let transition = process_internal_queue(state, exhaust_select.pending_actions)?;
        *state = transition.state;
    }
    resolve_deferred_draw_follow_ups(state, evolve_follow_ups)?;

    // A held source still settles here, including Havoc force-exhaust that
    // was parked on the decision until CONFIRM (FIDL01636). Older leftover
    // frames that already moved Burning Pact to exhaust skip the re-push
    // (FIDL00221).
    let mut deferred_bot_on_exhaust = Vec::new();
    if !force_exhausted_source {
        if let Some(source_card) = source_card {
            match if exhaust_select.source_card_force_exhaust {
                forced_source_card_destination(state, source_definition)
            } else {
                delayed_source_card_destination(state, source_definition)
            } {
                CardPile::ExhaustPile => {
                    state.piles.exhaust_pile.push(source_card);
                    apply_on_exhaust_effects_except_bot_queued_powers(state, source_card_id)?;
                    deferred_bot_on_exhaust.extend(feel_no_pain_block_follow_up(state));
                    if let Some(dead_branch) = dead_branch_follow_up(state) {
                        deferred_bot_on_exhaust.push(dead_branch);
                    }
                    deferred_bot_on_exhaust.extend(dark_embrace_then_necronomicurse_follow_ups(
                        state,
                        source_card_id,
                    )?);
                }
                CardPile::DiscardPile => state.piles.discard_pile.push(source_card),
                CardPile::Hand => state.piles.hand.push(source_card),
                CardPile::DrawPile => state.piles.draw_pile.push(source_card),
            }
        }
    }

    if !deferred_bot_on_exhaust.is_empty() {
        let transition = process_internal_queue(state, deferred_bot_on_exhaust.into())?;
        *state = transition.state;
    }
    state.defer_time_warp_end_turn = previous_defer_time_warp;
    state.activate_next_queued_decision_if_idle();
    if settle_time_warp {
        settle_time_warp_end_turn_if_ready(state)?;
    }
    state.piles.limbo.pop().ok_or(SimError::InvalidState(
        "skipped Burning Pact limbo card missing after confirmation",
    ))
}

/// Relic `onExhaust` (Dead Branch MakeTempCardInHand) is addToBot before power
/// `onExhaust` (Dark Embrace DrawCardAction). Inlining the Dark Embrace draw
/// first leaves the generated card behind the drawn card (FIDL01373 empty-select
/// Purity: Pommel then Dark Embrace). Do not run these hooks through
/// `process_internal_queue`: cloning mid-CONFIRM can leave the same instance in
/// two piles when several cards exhaust (FIDL01582).
fn apply_purity_card_exhausted(state: &mut CombatState, card_id: CardId) -> SimResult<usize> {
    apply_on_exhaust_effects_inner(state, card_id, false, true, false)?;
    let mut dead_branch_count = 0;
    if let Some(content_id) = reserve_dead_branch_card_content(state) {
        add_generated_card_to_pile(state, content_id, CardPile::Hand, None, false)?;
        dead_branch_count = 1;
    }
    if state.player.powers.dark_embrace > 0 {
        player_draw_cards(state, state.player.powers.dark_embrace as usize)?;
    }
    Ok(dead_branch_count)
}

fn confirm_purity_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
    older_thorns: VecDeque<InternalAction>,
) -> SimResult<usize> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let cap = if let Some(source) = exhaust_select.source_card.as_ref() {
        if source.content_id == PURITY_PLUS_ID {
            5
        } else {
            3
        }
    } else {
        purity_select_cap(state, source_card_id)?
    };
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    if selected.len() > cap {
        return Err(purity_too_many_cards_error(cap));
    }
    let mut removal_order = selected.clone();
    removal_order.sort_unstable();
    for index in &selected {
        let card = state
            .piles
            .hand
            .get(*index)
            .copied()
            .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
        if card.id == source_card_id {
            return Err(SimError::IllegalAction("Purity cannot select itself"));
        }
    }
    let exhausted = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    for index in removal_order.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    // `take_exhaust_select` holds Purity outside every pile. Dead Branch
    // `next_card_instance_id` is max(remaining)+1, so a source that owned the
    // previous max would mint a generated card with Purity's id, then this
    // retrieve would put the same instance in two piles (FIDL01582).
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.limbo.push(source_card);
    }
    let mut dead_branch_count = 0;
    for card in exhausted {
        state.piles.exhaust_pile.push(card);
        dead_branch_count += apply_purity_card_exhausted(state, card.id)?;
    }
    if !older_thorns.is_empty() {
        let transition = process_internal_queue(state, older_thorns)?;
        *state = transition.state;
    }
    if let Some(source_index) = state
        .piles
        .limbo
        .iter()
        .position(|card| card.id == source_card_id)
    {
        let source_card = state.piles.limbo.remove(source_index);
        let source_destination = purity_source_destination(state);
        push_card_to_pile(state, source_card, source_destination);
        if source_destination == CardPile::ExhaustPile {
            dead_branch_count += apply_purity_card_exhausted(state, source_card_id)?;
        }
    } else if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        let source_destination = purity_source_destination(state);
        move_card(state, source_card_id, CardPile::Hand, source_destination)?;
        if source_destination == CardPile::ExhaustPile {
            dead_branch_count += apply_purity_card_exhausted(state, source_card_id)?;
        }
    }
    if state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    Ok(dead_branch_count)
}

/// Purity ExhaustAction skipped-retrieval.
///
/// When ExhaustAction completes before CONFIRM retrieves `selectedCards`, only
/// Purity itself exhausts (Feel No Pain / Dead Branch once). The chosen cards
/// stay owned by the closed HandCardSelectScreen and re-enter discard on the
/// next END (FIDL00405).
#[cfg(test)]
fn confirm_purity_select_skipped_retrieval(state: &mut CombatState) -> SimResult<()> {
    let exhaust_select = state
        .take_exhaust_select()
        .ok_or(SimError::IllegalAction("no exhaust select is open"))?;
    if exhaust_select.purpose != crate::combat::ExhaustSelectPurpose::PurityExhaustUpTo3 {
        return Err(SimError::IllegalAction(
            "skipped Purity retrieval requires PurityExhaustUpTo3",
        ));
    }
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let cap = if let Some(source) = exhaust_select.source_card.as_ref() {
        if source.content_id == PURITY_PLUS_ID {
            5
        } else {
            3
        }
    } else {
        purity_select_cap(state, source_card_id)?
    };
    let selected = unique_selected_indices_in_choice_order(exhaust_select.selected_hand_indices);
    if selected.len() > cap {
        return Err(purity_too_many_cards_error(cap));
    }
    if !state.pending_hidden_hand_card_until_end_turn.is_empty() {
        return Err(SimError::IllegalAction(
            "pending hidden hand card already occupied",
        ));
    }
    for index in &selected {
        let card = state
            .piles
            .hand
            .get(*index)
            .copied()
            .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
        if card.id == source_card_id {
            return Err(SimError::IllegalAction("Purity cannot select itself"));
        }
    }
    let mut removal_order = selected.clone();
    removal_order.sort_unstable();
    // Capture before removing by descending index so the deferred END discard
    // retains the UI's actual selection order rather than pile-index order.
    let hidden = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    for index in removal_order.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    state.pending_hidden_hand_card_until_end_turn = hidden;

    let mut follow_ups = exhaust_select.pending_actions;
    if let Some(source_card) = exhaust_select.source_card {
        let source_destination = purity_source_destination(state);
        let source_id = source_card.id;
        push_card_to_pile(state, source_card, source_destination);
        if source_destination == CardPile::ExhaustPile {
            follow_ups.push_front(InternalAction::CardExhausted { card_id: source_id });
        }
    } else if state
        .piles
        .hand
        .iter()
        .any(|card| card.id == source_card_id)
    {
        let source_destination = purity_source_destination(state);
        move_card(state, source_card_id, CardPile::Hand, source_destination)?;
        if source_destination == CardPile::ExhaustPile {
            follow_ups.push_front(InternalAction::CardExhausted {
                card_id: source_card_id,
            });
        }
    }
    if !follow_ups.is_empty() {
        let transition = process_internal_queue(state, follow_ups)?;
        *state = transition.state;
    }
    if state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    state.activate_next_queued_decision_if_idle();
    settle_time_warp_end_turn_if_ready(state)?;
    Ok(())
}

fn unique_selected_indices_in_choice_order(selected: Vec<usize>) -> Vec<usize> {
    let mut unique = Vec::new();
    for index in selected {
        if !unique.contains(&index) {
            unique.push(index);
        }
    }
    unique
}

fn purity_source_destination(state: &mut CombatState) -> CardPile {
    if !state.relics.contains(&Relic::StrangeSpoon) {
        return CardPile::ExhaustPile;
    }
    if state.rng.card_random_rng.random_bool() {
        CardPile::DiscardPile
    } else {
        CardPile::ExhaustPile
    }
}

fn purity_select_cap(state: &CombatState, source_card_id: CardId) -> SimResult<usize> {
    let source = state
        .piles
        .hand
        .iter()
        .chain(state.piles.exhaust_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .chain(
            state
                .exhaust_select()
                .and_then(|select| select.source_card.as_ref()),
        )
        .find(|card| card.id == source_card_id)
        .ok_or(SimError::IllegalAction(
            "Purity source card is not available",
        ))?;
    Ok(if source.content_id == PURITY_PLUS_ID {
        5
    } else {
        3
    })
}

fn purity_too_many_cards_error(cap: usize) -> SimError {
    if cap == 5 {
        SimError::IllegalAction("Purity can select at most 5 cards")
    } else {
        SimError::IllegalAction("Purity can select at most 3 cards")
    }
}

fn exhumable_ui_to_exhaust_index(state: &CombatState, ui_index: usize) -> SimResult<usize> {
    state
        .piles
        .exhaust_pile
        .iter()
        .enumerate()
        .filter(|(_, card)| card.content_id != EXHUME_ID && card.content_id != EXHUME_PLUS_ID)
        .map(|(index, _)| index)
        .nth(ui_index)
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))
}

fn confirm_exhume_select(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
) -> SimResult<()> {
    let source_card_id = exhaust_select
        .source_card_id
        .ok_or(SimError::IllegalAction("exhaust select source is required"))?;
    let index = exhaust_select
        .selected_hand_indices
        .first()
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select choice is required"))?;
    let card = state
        .piles
        .exhaust_pile
        .get(index)
        .copied()
        .ok_or(SimError::IllegalAction("exhaust select index out of range"))?;
    if card.content_id == EXHUME_ID || card.content_id == EXHUME_PLUS_ID {
        return Err(SimError::IllegalAction("Exhume cannot return Exhume"));
    }
    state.piles.exhaust_pile.remove(index);
    state.piles.hand.push(card);
    settle_exhume_source_after_selection(state, exhaust_select, source_card_id)
}

fn settle_exhume_source_after_selection(
    state: &mut CombatState,
    exhaust_select: crate::combat::ExhaustSelectState,
    source_card_id: CardId,
) -> SimResult<()> {
    if let Some(source_card) = exhaust_select.source_card {
        state.piles.exhaust_pile.push(source_card);
        apply_on_exhaust_effects(state, source_card_id)?;
    } else {
        let source_is_already_exhausted = state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id);
        if !source_is_already_exhausted {
            move_card(state, source_card_id, CardPile::Hand, CardPile::ExhaustPile)?;
            apply_on_exhaust_effects(state, source_card_id)?;
        }
    }
    Ok(())
}

fn confirm_gambling_chip_select(state: &mut CombatState, selected: Vec<usize>) -> SimResult<()> {
    // The target appends discarded cards in the order they were selected in
    // the UI, while removal from the hand must still proceed from the back so
    // earlier indices remain valid.
    let selected = unique_selected_indices_in_choice_order(selected);
    let count = selected.len();
    for index in &selected {
        if *index >= state.piles.hand.len() {
            return Err(SimError::IllegalAction("exhaust select index out of range"));
        }
    }
    let discarded = selected
        .iter()
        .map(|index| state.piles.hand[*index])
        .collect::<Vec<_>>();
    let mut removal_order = selected;
    removal_order.sort_unstable();
    for index in removal_order.into_iter().rev() {
        state.piles.hand.remove(index);
    }
    state.piles.discard_pile.extend(discarded);
    player_draw_cards(state, count)?;
    if state.relic_counters.deferred_warped_tongs {
        state.relic_counters.deferred_warped_tongs = false;
        crate::relic::upgrade_random_non_status_hand_card(state)?;
    }
    Ok(())
}

fn remove_card_from_pile(
    state: &mut CombatState,
    card_id: CardId,
    pile: CardPile,
) -> SimResult<CardInstance> {
    let card = {
        let cards = match pile {
            CardPile::Hand => &mut state.piles.hand,
            CardPile::DrawPile => &mut state.piles.draw_pile,
            CardPile::DiscardPile => &mut state.piles.discard_pile,
            CardPile::ExhaustPile => &mut state.piles.exhaust_pile,
        };
        let index = cards
            .iter()
            .position(|card| card.id == card_id)
            .ok_or(SimError::UnknownCard(card_id))?;
        cards.remove(index)
    };
    Ok(card)
}

fn find_hand_card(state: &CombatState, card_id: CardId) -> SimResult<CardInstance> {
    state
        .piles
        .hand
        .iter()
        .copied()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn card_content_definition(
    state: &CombatState,
    card_id: CardId,
) -> SimResult<&'static crate::card::CardDefinition> {
    state
        .piles
        .hand
        .iter()
        .chain(state.piles.discard_pile.iter())
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.exhaust_pile.iter())
        .find(|card| card.id == card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .ok_or(SimError::UnknownCard(card_id))
}

fn find_hand_card_mut(state: &mut CombatState, card_id: CardId) -> SimResult<&mut CardInstance> {
    state
        .piles
        .hand
        .iter_mut()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn find_combat_card_mut(state: &mut CombatState, card_id: CardId) -> Option<&mut CardInstance> {
    state
        .piles
        .hand
        .iter_mut()
        .chain(state.piles.discard_pile.iter_mut())
        .chain(state.piles.draw_pile.iter_mut())
        .chain(state.piles.exhaust_pile.iter_mut())
        .find(|card| card.id == card_id)
}

fn add_rampage_damage_bonus(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<()> {
    let card = find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))?;
    if card.content_id != crate::content::cards::RAMPAGE_ID
        && card.content_id != crate::content::cards::RAMPAGE_PLUS_ID
    {
        return Err(SimError::InvalidState(
            "Rampage growth source is not Rampage",
        ));
    }
    card.rampage_damage_bonus = card
        .rampage_damage_bonus
        .checked_add(amount)
        .ok_or(SimError::InvalidState("Rampage damage bonus overflows i32"))?;
    Ok(())
}

fn add_ritual_dagger_damage_bonus(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<()> {
    let card = find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))?;
    card.ritual_dagger_damage_bonus = card
        .ritual_dagger_damage_bonus
        .checked_add(amount.max(0))
        .ok_or(SimError::InvalidState(
            "Ritual Dagger damage bonus overflows i32",
        ))?;
    Ok(())
}

fn remove_card_from_hand(state: &mut CombatState, card_id: CardId) -> SimResult<CardInstance> {
    let index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;

    Ok(state.piles.hand.remove(index))
}

fn move_card(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
    to: CardPile,
) -> SimResult<()> {
    let card = match from {
        CardPile::Hand => remove_card_from_hand(state, card_id)?,
        CardPile::DrawPile => {
            let index = state
                .piles
                .draw_pile
                .iter()
                .position(|card| card.id == card_id)
                .ok_or(SimError::UnknownCard(card_id))?;
            state.piles.draw_pile.remove(index)
        }
        CardPile::DiscardPile | CardPile::ExhaustPile => {
            return Err(SimError::IllegalAction(
                "card move source is not implemented",
            ));
        }
    };

    match to {
        CardPile::DiscardPile => {
            // UseCardAction.empower removes the Power that was just played
            // (`card_in_use`). ScrapeFollowUp / other moveToDiscardPile paths
            // still put unplayed Powers into the discard pile (FIDL02294).
            let is_played_power = state.card_in_use == Some(card_id)
                && get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == CardType::Power);
            if !is_played_power {
                state.piles.discard_pile.push(card);
            }
            Ok(())
        }
        CardPile::ExhaustPile => {
            state.piles.exhaust_pile.push(card);
            Ok(())
        }
        CardPile::Hand => {
            if state.piles.hand.len() >= 10 {
                state.piles.discard_pile.push(card);
            } else {
                state.piles.hand.push(card);
            }
            Ok(())
        }
        CardPile::DrawPile => Err(SimError::IllegalAction(
            "card move destination is not implemented",
        )),
    }
}

fn upgrade_combat_cards(state: &mut CombatState) -> SimResult<()> {
    let upgrades = state
        .piles
        .hand
        .iter()
        .chain(state.piles.draw_pile.iter())
        .chain(state.piles.discard_pile.iter())
        .chain(state.piles.exhaust_pile.iter())
        .map(|card| Ok((card.id, upgrade_card_instance(*card)?)))
        .collect::<SimResult<Vec<_>>>()?;
    for (card_id, upgraded) in upgrades {
        if let Some(upgraded) = upgraded {
            *find_combat_card_mut(state, card_id).ok_or(SimError::UnknownCard(card_id))? = upgraded;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::*;
    use crate::content::monsters::{
        mark_awakened_one_half_dead, monster_state, AWAKENED_ONE_A0, BRONZE_ORB_A0, DAGGER_A0,
        DARKLING_A0, FUNGI_BEAST_A0, GUARDIAN_A0, JAW_WORM_A0, REPTOMANCER_A0, SNAKE_PLANT_A0,
        SPIRE_SHIELD_A0, SPIRE_SPEAR_A0,
    };
    use crate::relic::INK_BOTTLE_THRESHOLD;
    use crate::rng::StsRng;
    use crate::run::potion::apply_exhaust_select_choice;
    use crate::{apply_combat_action_on_run, RunState};

    #[test]
    fn increase_max_orbs_rejects_nonpositive_amounts_and_overflow() {
        let state = CombatState::initial_fixture();
        for amount in [0, -1] {
            assert_eq!(
                process_internal_queue(
                    &state,
                    VecDeque::from([InternalAction::IncreaseMaxOrbs { amount }]),
                ),
                Err(SimError::InvalidState(
                    "max orb slot increase must be positive"
                ))
            );
        }

        let mut overflowing = state;
        overflowing.max_orbs = i32::MAX;
        assert_eq!(
            process_internal_queue(
                &overflowing,
                VecDeque::from([InternalAction::IncreaseMaxOrbs { amount: 1 }]),
            ),
            Err(SimError::InvalidState(
                "max orb slot increase overflows i32"
            ))
        );
    }

    #[test]
    fn top_played_hologram_defers_source_settlement_until_discard_choice() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), HOLOGRAM_ANY_COLOR_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
        ];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should open Hologram's discard selection");

        assert!(next.discard_select().is_some());
        choose_discard_select(&mut next, 0).expect("choose a discard card");
        confirm_discard_select(&mut next).expect("confirm Hologram selection");
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(3)));
    }

    #[test]
    fn corruption_exhausts_upgraded_hologram_after_singleton_retrieval() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.corruption = 1;
        let mut hologram = CardInstance::new(CardId::new(1), HOLOGRAM_ANY_COLOR_ID);
        hologram.upgrades = 1;
        state.piles.hand = vec![hologram];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Corruption Hologram+ should auto-return the singleton discard");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].id, CardId::new(2));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
    }

    #[test]
    fn mayhem_hologram_does_not_overfill_a_full_hand_on_singleton_return() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = (1..=10)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), HOLOGRAM_ANY_COLOR_ID)];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(12), DEFEND_R_ID)];
        state.piles.exhaust_pile.clear();

        let next = process_internal_queue(
            &state,
            VecDeque::from([InternalAction::PlayTopDrawCard {
                target: Some(state.monsters[0].id),
                exhaust_played_card: false,
                random_living_target: false,
            }]),
        )
        .expect("Mayhem should settle top-played Hologram")
        .state;

        assert_eq!(next.piles.hand.len(), crate::combat::draw::MAX_HAND_SIZE);
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(12)));
    }

    #[test]
    fn go_for_the_eyes_checks_live_complete_attack_intent_after_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.monsters[0].intent = crate::MonsterIntent::AttackApplyPlayerVulnerable {
            damage: 6,
            vulnerable: 2,
        };
        state.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            GO_FOR_THE_EYES_ANY_COLOR_ID,
        )];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Go for the Eyes should resolve");
        assert_eq!(next.monsters[0].powers.weak, 1);

        let mut shifted = state;
        shifted.monsters[0] = monster_state(&GUARDIAN_A0, shifted.monsters[0].id);
        shifted.monsters[0].hp = 100;
        shifted.monsters[0].max_hp = 100;
        shifted.monsters[0].intent = crate::MonsterIntent::Attack { damage: 9 };
        shifted.monsters[0].mode_shift = 1;
        let shifted = apply_combat_action(
            &shifted,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(shifted.monsters[0].id),
            },
        )
        .expect("Go for the Eyes should trigger Guardian mode shift");
        // Guardian queues Defensive Mode behind ForTheEyesAction, so the live
        // intent check still sees the attack before the queued mode shift runs.
        assert_eq!(shifted.monsters[0].powers.weak, 1);
        assert!(shifted.monsters[0].in_defensive_mode);
    }

    #[test]
    fn pressure_points_queues_mark_loss_after_source_move_and_sadistic_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.sadistic_nature = 1;
        state.relics.push(Relic::GremlinHorn);
        state
            .monsters
            .push(monster_state(&JAW_WORM_A0, crate::MonsterId::new(2)));
        state.monsters[0].hp = 6;
        state.monsters[0].max_hp = 6;
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.hand.push(CardInstance::new(
            CardId::new(10),
            PRESSURE_POINTS_ANY_COLOR_ID,
        ));
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), DEFEND_R_ID)];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Pressure Points should resolve queued Mark loss");

        assert!(!next.monsters[0].alive);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(11)));
    }

    #[test]
    fn deferred_mayhem_spoon_roll_follows_residual_card_random_insert() {
        let card = CardInstance::new(CardId::new(100), SLIMED_ID);
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::StrangeSpoon];
        state.defer_mayhem_play_top_settlement = true;
        state.defer_mayhem_play_top_draw_inserts = true;
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(200), STRIKE_R_ID),
            CardInstance::new(CardId::new(201), DEFEND_R_ID),
        ];
        let counter_before = state.rng.card_random_rng.counter();

        let (mut queued, queue) = card_effects::play_top_draw_card_queue(&state, card, None, false)
            .expect("deferred Mayhem Slimed queue");
        assert_eq!(queued.rng.card_random_rng.counter(), counter_before);
        assert_eq!(queued.defer_strange_spoon_until_source_move, vec![card.id]);
        assert!(queue.iter().any(|action| {
            matches!(
                action,
                InternalAction::MoveCard {
                    card_id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                } if *card_id == card.id
            )
        }));

        let index = queued
            .piles
            .hand
            .iter()
            .position(|candidate| candidate.id == card.id)
            .expect("staged Slimed");
        let parked = queued.piles.hand.remove(index);
        queued
            .deferred_mayhem_play_top_settlements
            .push((parked, CardPile::ExhaustPile));
        queued.deferred_mayhem_play_top_draw_inserts.push(
            InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: WOUND_ID,
            },
        );

        let (result, events) =
            crate::capture_rng_trace(|| flush_deferred_mayhem_play_top_draw_inserts(&mut queued));
        result.expect("deferred insert and Spoon settlement resolve");

        assert_eq!(queued.rng.card_random_rng.counter(), counter_before + 2);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].operation,
            crate::RngTraceOperation::RandomInt { .. }
        ));
        assert!(matches!(
            events[1].operation,
            crate::RngTraceOperation::RandomBool { .. }
        ));
    }

    #[test]
    fn nested_play_top_spoon_settlements_keep_both_card_owners() {
        let outer = CardInstance::new(CardId::new(100), SLIMED_ID);
        let inner = CardInstance::new(CardId::new(101), SEEING_RED_ID);
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::StrangeSpoon];
        state.piles.hand = vec![outer, inner];
        state.piles.exhaust_pile.clear();
        state.piles.discard_pile.clear();
        state.defer_strange_spoon_until_source_move = vec![outer.id, inner.id];
        let counter_before = state.rng.card_random_rng.counter();

        let transition = process_internal_queue(
            &state,
            VecDeque::from([
                InternalAction::MoveCard {
                    card_id: inner.id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
                InternalAction::MoveCard {
                    card_id: outer.id,
                    from: CardPile::Hand,
                    to: CardPile::ExhaustPile,
                },
            ]),
        )
        .expect("nested forced sources settle independently");

        assert!(transition
            .state
            .defer_strange_spoon_until_source_move
            .is_empty());
        assert_eq!(
            transition.state.rng.card_random_rng.counter(),
            counter_before + 2
        );
        assert_eq!(
            transition.state.piles.exhaust_pile.len() + transition.state.piles.discard_pile.len(),
            2
        );
    }

    #[test]
    fn surrounded_back_attack_starts_on_shield_and_clears_after_elite_death() {
        let shield_id = MonsterId::new(1);
        let spear_id = MonsterId::new(2);
        let shield = monster_state(&SPIRE_SHIELD_A0, shield_id);
        let spear = monster_state(&SPIRE_SPEAR_A0, spear_id);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![shield, spear];

        assert!(state.monsters[0].back_attack);
        assert!(!state.monsters[1].back_attack);

        state.monsters[1].hp = 0;
        state.monsters[1].alive = false;
        apply_monster_death_hooks(&mut state, spear_id).expect("Spear death hooks resolve");
        assert!(state.monsters.iter().all(|monster| !monster.back_attack));
    }

    #[test]
    fn captains_wheel_block_triggers_juggernaut() {
        let mut state = CombatState::cultist_fixture();
        state.relics.push(Relic::CaptainsWheel);
        state.player.powers.juggernaut = 5;
        state.relic_counters.player_turns_started = 2;
        state.monsters[0].hp = 30;

        let deferred = crate::relic::apply_start_of_player_turn_relics(&mut state)
            .expect("Captain's Wheel resolves");
        for block in deferred {
            apply_juggernaut_after_direct_block_gain(&mut state, block)
                .expect("Juggernaut after Wheel block");
        }

        assert_eq!(state.player.block, 18);
        assert_eq!(state.monsters[0].hp, 25);
    }

    #[test]
    fn opening_leftover_end_after_flex_keeps_temp_strength() {
        // FIDL01576: play Flex on the fight-two ready frame, then leftover
        // EndTurn discards the rest and redraws without LoseStrengthPower.
        let mut state = CombatState::cultist_fixture();
        state.opening_end_turn_pending = true;
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), FLEX_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: None,
            },
        )
        .expect("Flex then leftover opening EndTurn");

        assert!(!next.opening_end_turn_pending);
        assert_eq!(
            next.player.temp_strength, 2,
            "Flex must survive leftover EndTurn"
        );
        assert!(
            !next
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == FLEX_ID),
            "Flex itself is discarded after play"
        );
        assert_eq!(next.piles.hand.len(), 5);
    }

    #[test]
    fn madness_samples_remaining_hand_then_retries_on_cost() {
        // After useCard.removeCard, MadnessAction getRandomCard samples the
        // remaining hand (Body Slam+ 0, Entrench 2, Strike 1) and retries
        // zero-cost picks.
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.rng.card_random_rng = StsRng::new(0);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(101), MADNESS_ID),
            CardInstance::new(CardId::new(102), BODY_SLAM_PLUS_ID),
            CardInstance::new(CardId::new(103), ENTRENCH_ID),
            CardInstance::new(CardId::new(104), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(101),
                target: None,
            },
        )
        .expect("Madness plays");

        let entrench = next
            .piles
            .hand
            .iter()
            .find(|card| card.id == CardId::new(103))
            .expect("Entrench remains");
        let strike = next
            .piles
            .hand
            .iter()
            .find(|card| card.id == CardId::new(104))
            .expect("Strike remains");
        let slam = next
            .piles
            .hand
            .iter()
            .find(|card| card.id == CardId::new(102))
            .expect("Body Slam+ remains");
        assert_eq!(slam.temp_cost, None, "0-cost Body Slam+ must be retried");
        assert!(
            strike.temp_cost == Some(0) || entrench.temp_cost == Some(0),
            "Madness must zero a positive-cost remaining card"
        );
    }

    #[test]
    fn second_madness_does_not_sample_when_remaining_printed_costs_are_zero() {
        // Java writes cost=0 on the first pick. A later Madness whose remaining
        // hand is already all cost 0 (prior Madness Iron Wave, Clash, Wounds)
        // must not call getRandomCard (FIDL01609 Magnetism after rebirth).
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.rng.card_random_rng =
            StsRng::from_raw_state(8_410_403_961_195_825_053, 10_539_873_050_201_740_548, 96);
        let before = state.rng.card_random_rng.counter();
        let mut iron_wave = CardInstance::new(CardId::new(102), IRON_WAVE_ID);
        iron_wave.temp_cost = Some(0);
        iron_wave.temp_cost_turn_only = false;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(101), MADNESS_PLUS_ID),
            CardInstance::new(CardId::new(103), CLASH_PLUS_ID),
            iron_wave,
            CardInstance::new(CardId::new(104), WOUND_ID),
            CardInstance::new(CardId::new(105), WOUND_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(101),
                target: None,
            },
        )
        .expect("second Madness plays");

        assert_eq!(
            next.rng.card_random_rng.counter(),
            before,
            "second Madness must not sample a remaining hand with no printed cost"
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .find(|card| card.id == CardId::new(102))
                .expect("Iron Wave remains")
                .temp_cost,
            Some(0)
        );
    }

    #[test]
    fn leftover_start_without_opening_flag_expires_temp_strength() {
        // FIDL01597: Nilry leftover EndTurn still expires Flex at the next
        // start. Only the Colosseum opening ready-play sets the preserve flag.
        let mut state = CombatState::cultist_fixture();
        state.player.temp_strength = 2;
        state.piles.hand.clear();
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        crate::combat::turn::start_player_turn(&mut state).expect("start turn");
        assert_eq!(state.player.temp_strength, 0);
    }

    #[test]
    fn ordinary_end_after_flex_expires_temp_strength() {
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 3;
        state.player.temp_strength = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();

        let next = apply_combat_action(&state, CombatAction::EndTurn).expect("ordinary END");
        assert_eq!(next.player.temp_strength, 0);
    }

    #[test]
    fn rejected_play_after_opening_end_discards_the_visible_hand() {
        let mut state = CombatState::cultist_fixture();
        state.opening_end_turn_pending = true;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HEMOKINESIS_ID),
            CardInstance::new(CardId::new(2), BERSERK_ID),
            CardInstance::new(CardId::new(3), POWER_THROUGH_ID),
            CardInstance::new(CardId::new(4), EVOLVE_ID),
            CardInstance::new(CardId::new(5), BERSERK_ID),
        ];
        state.piles.discard_pile.clear();

        settle_queued_end_turn_discard_after_rejected_command(&mut state)
            .expect("leftover EndTurn discards the published opening hand");

        assert!(!state.opening_end_turn_pending);
        assert!(state.time_warp_end_turn_pre_discard_settled);
        assert!(state.piles.hand.is_empty());
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![
                BERSERK_ID,
                EVOLVE_ID,
                POWER_THROUGH_ID,
                BERSERK_ID,
                HEMOKINESIS_ID
            ]
        );
    }

    #[test]
    fn ink_bottle_draws_before_played_card_moves_to_discard() {
        let mut queue = VecDeque::from([
            InternalAction::DealDamageAll {
                source: CardId::new(1),
                amount: 8,
            },
            InternalAction::MoveCard {
                card_id: CardId::new(1),
                from: CardPile::Hand,
                to: CardPile::DiscardPile,
            },
        ]);

        push_follow_up(
            &mut queue,
            InternalAction::DrawCardsFromInkBottle { count: 1 },
            false,
        );

        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DealDamageAll { .. })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DrawCardsFromInkBottle { count: 1 })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::MoveCard { .. })
        ));
    }

    #[test]
    fn ink_bottle_draws_before_transmutation_generation() {
        // TransmutationAction queues MakeTempCardInHand after Ink's onUseCard
        // DrawCardAction, so a full Pyramid hand can still shuffle+draw.
        let mut queue = VecDeque::from([
            InternalAction::AddRandomColorlessCardsToHandWhileSourceInLimbo {
                source_card_id: CardId::new(1),
                count: 4,
                temp_cost: Some(0),
                upgrade: false,
            },
            InternalAction::MoveCard {
                card_id: CardId::new(1),
                from: CardPile::Hand,
                to: CardPile::ExhaustPile,
            },
        ]);

        push_follow_up(
            &mut queue,
            InternalAction::DrawCardsFromInkBottle { count: 1 },
            false,
        );

        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DrawCardsFromInkBottle { count: 1 })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::AddRandomColorlessCardsToHandWhileSourceInLimbo { .. })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::MoveCard { .. })
        ));
    }

    #[test]
    fn ink_bottle_draws_after_havoc_play_top() {
        // Mirrors AbstractPlayer.useCard: Havoc.use queues PlayTop first, then
        // Ink Bottle onUseCard queues its draw, then UseCardAction settles.
        let mut queue = VecDeque::from([
            InternalAction::MoveCard {
                card_id: CardId::new(1),
                from: CardPile::Hand,
                to: CardPile::DiscardPile,
            },
            InternalAction::PlayTopDrawCard {
                target: None,
                exhaust_played_card: true,
                random_living_target: true,
            },
        ]);

        push_follow_up(
            &mut queue,
            InternalAction::DrawCardsFromInkBottle { count: 1 },
            false,
        );

        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::MoveCard { .. })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::PlayTopDrawCard { .. })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DrawCardsFromInkBottle { count: 1 })
        ));
    }

    #[test]
    fn forethought_multi_choose_indexes_unselected_only() {
        // FIDL00269: CM choice_list drops selected cards; repeated CHOOSE 1
        // must pick three different hand cards, not toggle the same slot.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        let ft = CardInstance::new(CardId::new(1), FORETHOUGHT_PLUS_ID);
        let a = CardInstance::new(CardId::new(2), STRIKE_R_ID);
        let b = CardInstance::new(CardId::new(3), DEFEND_R_ID);
        let c = CardInstance::new(CardId::new(4), BASH_ID);
        let d = CardInstance::new(CardId::new(5), STRIKE_R_ID);
        state.piles.hand = vec![ft, a, b, c, d];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(6), WOUND_ID)];
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("play Forethought+");
        assert!(next.decision.is_some());
        let mut next = next;
        choose_hand_select(&mut next, 1).expect("choose 1");
        choose_hand_select(&mut next, 1).expect("choose 1 again");
        choose_hand_select(&mut next, 1).expect("choose 1 third");
        let selected = next
            .hand_select()
            .expect("select open")
            .selected_hand_indices
            .clone();
        assert_eq!(selected.len(), 3, "selected={selected:?}");
        // Forethought is parked in limbo while the source selection is open.
        // CHOOSE 1 thrice therefore maps to internal slots 1,2,3 as each prior
        // selection disappears from the CommunicationMod choice list.
        assert_eq!(selected, vec![1, 2, 3]);
    }

    #[test]
    fn havoc_with_ink_bottle_plays_original_top_then_draws_next() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.relics.push(Relic::InkBottle);
        state.relic_counters.ink_bottle_cards_played = INK_BOTTLE_THRESHOLD - 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        // draw pile is bottom→top; pop takes the last element as top.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), WHIRLWIND_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        let monster_hp_before = state.monsters[0].hp;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ with Ink Bottle should resolve");

        // Whirlwind (original top) was force-played and exhausted; Ink Bottle
        // then drew Strike rather than stealing Whirlwind from the top first.
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == WHIRLWIND_ID),
            "Havoc must play the original top card (Whirlwind)"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID),
            "Ink Bottle must draw the card under the forced top play"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .all(|card| card.content_id != WHIRLWIND_ID),
            "Whirlwind must not remain in hand after a correct PlayTop-first order"
        );
        assert!(
            next.monsters[0].hp < monster_hp_before,
            "forced Whirlwind should deal damage"
        );
        assert_eq!(next.relic_counters.ink_bottle_cards_played, 1); // Havoc + Whirlwind
    }

    #[test]
    fn havoc_hex_after_pommel_plus_inserts_after_remove_before_draws() {
        // Havoc (skill) under Hex force-plays Pommel+ (draw 2). PlayTop removes
        // Pommel first; Hex MakeTempCard (bot after Havoc.use) inserts against
        // the post-remove pile (size 8) before Pommel's draws run (FIDL00381).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.hex = 1;
        // Deterministic: counter 0 → monster roll then Hex insert.
        state.rng.card_random_rng = StsRng::new(0);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        // bottom → top; unique markers under Pommel so draws are identifiable.
        // After Pommel removed: 8 cards. Hex inserts here. Then draw 2 from top.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID), // bottom = index 0
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), BASH_ID),
            CardInstance::new(CardId::new(13), CLEAVE_ID),
            CardInstance::new(CardId::new(14), ANGER_ID),
            CardInstance::new(CardId::new(15), IRON_WAVE_ID),
            CardInstance::new(CardId::new(16), BODY_SLAM_ID), // drawn 2nd
            CardInstance::new(CardId::new(17), CLOTHESLINE_ID), // drawn 1st (top under Pommel)
            CardInstance::new(CardId::new(18), POMMEL_STRIKE_PLUS_ID), // top
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.monsters[0].max_hp = 99;
        state.monsters[0].hp = 99;

        // Expected Hex insert index against post-remove size 8 (bound 7):
        let mut expect_rng = StsRng::new(0);
        let _monster = expect_rng.random_int(0); // single living target
        let expected_index = expect_rng.random_int(7) as usize;
        // Wrong: post-draw size 6 (bound 5):
        let mut wrong_rng = StsRng::new(0);
        let _ = wrong_rng.random_int(0);
        let wrong_index = wrong_rng.random_int(5) as usize;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ under Hex with Pommel+ top");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|c| c.content_id == POMMEL_STRIKE_PLUS_ID),
            "Pommel+ must be force-played and exhausted"
        );
        let hand_ids: Vec<_> = next.piles.hand.iter().map(|c| c.content_id).collect();
        assert!(
            hand_ids.contains(&CLOTHESLINE_ID) && hand_ids.contains(&BODY_SLAM_ID),
            "Pommel+ must draw the two cards under it, got {hand_ids:?}"
        );
        // 8 post-remove + Hex - 2 draws = 7
        assert_eq!(next.piles.draw_pile.len(), 7, "post-remove Hex then draw 2");
        let dazed_pos = next
            .piles
            .draw_pile
            .iter()
            .position(|c| c.content_id == DAZED_ID)
            .expect("Hex Dazed in draw");
        // If Hex inserted at expected_index and the two drawn cards were above
        // that index, dazed_pos stays expected_index; if draws came from below
        // the insert, position shifts. With top-of-pile draws, insert below the
        // top two keeps dazed_pos == expected_index when expected_index < 6.
        let mut expect_final = expected_index;
        if expected_index >= 6 {
            // insert among the two cards that will be drawn — Dazed may leave.
            expect_final = dazed_pos; // accept observed when in draw zone
        }
        if expected_index < 6 {
            assert_eq!(
                dazed_pos, expected_index,
                "Hex must insert post-remove pre-draw (index {expected_index}, wrong post-draw index {wrong_index}); pile={:?}",
                next.piles.draw_pile.iter().map(|c| c.content_id).collect::<Vec<_>>()
            );
        } else {
            let _ = expect_final;
            assert!(
                next.piles
                    .draw_pile
                    .iter()
                    .any(|c| c.content_id == DAZED_ID)
                    || hand_ids.contains(&DAZED_ID),
                "Hex Dazed must exist after insert near top"
            );
        }
        assert_ne!(
            expected_index, wrong_index,
            "test seed must distinguish post-remove vs post-draw insert bounds"
        );
    }

    #[test]
    fn havoc_empty_draw_dual_havoc_chain_exhausts_both() {
        // Empty draw + discard-only-Havoc: settle source into the refill so nested
        // force-play can chain-exhaust both (FIDL00238 step 953 block 9 / dual
        // exhaust). Mixed discards stay PlayTop-first (Sever Soul / step 873).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.block = 3;
        state.player.powers.feel_no_pain = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), HAVOC_ID)];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.monsters[0].max_hp = 99;
        state.monsters[0].hp = 99;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("empty-draw Havoc+ with Havoc in discard");

        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|c| matches!(c.content_id, HAVOC_ID | HAVOC_PLUS_ID))
                .count(),
            2,
            "both Havocs force-exhaust via dual-Havoc settle-first chain"
        );
        assert!(
            next.piles
                .discard_pile
                .iter()
                .all(|c| !matches!(c.content_id, HAVOC_ID | HAVOC_PLUS_ID)),
            "no Havoc remains in discard"
        );
        assert_eq!(
            next.player.block, 9,
            "FNP twice: discard Havoc + source Havoc+"
        );
    }

    #[test]
    fn corruption_havoc_empty_draw_force_plays_enemy_top_with_use_time_target_roll() {
        // Corruption Havoc.use always rolls getRandomMonster for PlayTop even when
        // the draw pile is empty (top unknown until shuffle). Dropping that roll
        // when attach-at-build fails rejects Clothesline after refill (FIDL00428).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.corruption = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), CLOTHESLINE_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.monsters[0].max_hp = 99;
        state.monsters[0].hp = 99;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Corruption empty-draw Havoc force-plays enemy top");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|c| c.content_id == HAVOC_ID),
            "Corruption exhausts Havoc source"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|c| matches!(c.content_id, CLOTHESLINE_ID | STRIKE_R_ID)),
            "force-exhausted top card from refill"
        );
    }

    #[test]
    fn corruption_empty_draw_havoc_dead_branch_generates_source_card_before_top() {
        // FIDL01410: empty-draw Corruption must settle/exhaust Havoc (and its
        // Dead Branch) before PlayTop shuffles and force-exhausts the refill.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.relics.push(Relic::DeadBranch);
        state.player.powers.corruption = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.rng.card_random_rng = StsRng::new(7);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Corruption empty-draw Havoc still force-plays after exhaust");

        let exhaust: Vec<_> = next
            .piles
            .exhaust_pile
            .iter()
            .map(|card| card.content_id)
            .collect();
        assert_eq!(
            exhaust,
            vec![HAVOC_ID, DEFEND_R_ID],
            "Havoc must exhaust before the forced refill card"
        );
        assert_eq!(
            next.piles.hand.len(),
            2,
            "Dead Branch should add one card per exhaust"
        );
    }

    #[test]
    fn havoc_dual_wield_singleton_keeps_original_hand_order() {
        // FIDL01368: Havoc force-plays Dual Wield with one Power (Feel No Pain)
        // and one Skill (Seeing Red). No select opens; the FNP copy is appended.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FEEL_NO_PAIN_ID),
            CardInstance::new(CardId::new(2), SEEING_RED_ID),
            CardInstance::new(CardId::new(3), HAVOC_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), DUAL_WIELD_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(3),
                target: None,
            },
        )
        .expect("Havoc Dual Wield singleton auto-confirms");

        assert!(
            next.decision.is_none(),
            "force-played Dual Wield with one eligible card must not open a select"
        );
        let hand: Vec<_> = next.piles.hand.iter().map(|card| card.content_id).collect();
        assert_eq!(
            hand,
            vec![FEEL_NO_PAIN_ID, SEEING_RED_ID, FEEL_NO_PAIN_ID],
            "original order stays; copy is appended"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == DUAL_WIELD_ID),
            "force-played Dual Wield exhausts"
        );
    }

    #[test]
    fn havoc_empty_draw_shuffles_while_awakened_one_is_half_dead() {
        // FIDL01451: Havoc on an empty draw after form-one death. First-form
        // half-death is not isBattleEnding, so EmptyDeckShuffleAction still
        // refills and PlayTop still force-plays the new top card.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), FLEX_ID),
            CardInstance::new(CardId::new(4), INFLAME_ID),
        ];
        state.piles.exhaust_pile.clear();
        let mut awakened = monster_state(&AWAKENED_ONE_A0, MonsterId::new(1));
        assert!(mark_awakened_one_half_dead(&mut awakened));
        state.monsters = vec![awakened];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc PlayTop still shuffles during Awakened One half-death");

        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "non-Corruption Havoc still settles to discard"
        );
        assert!(
            next.piles.draw_pile.len() + next.piles.exhaust_pile.len()
                == state.piles.discard_pile.len(),
            "shuffle must refill from discard and PlayTop must take one card; draw={} exhaust={:?}",
            next.piles.draw_pile.len(),
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            next.piles
                .draw_pile
                .iter()
                .all(|card| card.content_id != HAVOC_ID)
                && next
                    .piles
                    .exhaust_pile
                    .iter()
                    .all(|card| card.content_id != HAVOC_ID),
            "Havoc source must not be the force-played refill card"
        );
    }

    #[test]
    fn havoc_empty_draw_mixed_discard_play_top_first_keeps_source_out_of_reshuffle() {
        // Mixed discard must not settle-first (FIDL00238 step 873 / Sever Soul).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.block = 0;
        state.player.powers.feel_no_pain = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(3), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.monsters[0].max_hp = 99;
        state.monsters[0].hp = 99;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("empty-draw Havoc with mixed discard");

        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|c| c.content_id == HAVOC_ID),
            "source Havoc settles to discard under mixed empty-draw PlayTop-first"
        );
    }

    #[test]
    fn nested_havoc_empty_draw_dark_embrace_playtops_parent_not_defend() {
        // FIDL01677 step 217: hand Havoc, draw Havoc, discard Defend, Dark Embrace.
        // Observed: both Havocs exhaust, Defend is drawn, block stays 5.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 2;
        state.player.block = 5;
        state.player.powers.dark_embrace = 1;
        state.player.powers.metallicize = 3;
        state.relics = vec![
            Relic::BurningBlood,
            Relic::NeowsLament,
            Relic::BagOfPreparation,
            Relic::StrikeDummy,
        ];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(14), HAVOC_ID),
            CardInstance::new(CardId::new(22), FLAME_BARRIER_PLUS_ID),
            CardInstance::new(CardId::new(10), BASH_ID),
            CardInstance::new(CardId::new(15), ARMAMENTS_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(29), HAVOC_ID)];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(9), DEFEND_R_ID)];
        state.piles.exhaust_pile.clear();
        state.monsters = vec![monster_state(&GUARDIAN_A0, MonsterId::new(1))];
        state.monsters[0].hp = 111;
        state.monsters[0].max_hp = 240;
        state.rng.shuffle_rng =
            StsRng::from_raw_state(14238308195503378694, 13957919737925544295, 15);
        state.rng.card_random_rng =
            StsRng::from_raw_state(3406355565463945630, 14238308195503378694, 14);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(14),
                target: None,
            },
        )
        .expect("play hand Havoc into nested Havoc");

        let hand: Vec<_> = next
            .piles
            .hand
            .iter()
            .map(|c| (c.id, c.content_id))
            .collect();
        let draw: Vec<_> = next
            .piles
            .draw_pile
            .iter()
            .map(|c| (c.id, c.content_id))
            .collect();
        let discard: Vec<_> = next
            .piles
            .discard_pile
            .iter()
            .map(|c| (c.id, c.content_id))
            .collect();
        let exhaust: Vec<_> = next
            .piles
            .exhaust_pile
            .iter()
            .map(|c| (c.id, c.content_id))
            .collect();
        assert_eq!(next.player.block, 5, "Defend must be drawn, not PlayTop'd");
        assert!(
            hand.iter()
                .any(|(id, content)| *id == CardId::new(9) && *content == DEFEND_R_ID),
            "Dark Embrace draws leftover Defend before nested parent Havoc resolves: {hand:?}"
        );
        assert!(draw.is_empty(), "draw should be empty, got {draw:?}");
        assert!(
            discard.is_empty(),
            "discard should be empty, got {discard:?}"
        );
        assert!(
            exhaust.contains(&(CardId::new(14), HAVOC_ID))
                && exhaust.contains(&(CardId::new(29), HAVOC_ID)),
            "both Havocs exhaust: {exhaust:?}"
        );
        assert!(
            !exhaust.iter().any(|(id, _)| *id == CardId::new(9)),
            "Defend must not be force-played: {exhaust:?}"
        );
    }

    #[test]
    fn panache_skips_a_target_killed_by_the_triggering_attack() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.panache = 10;
        state.player.powers.panache_cards_played = 4;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![
            monster_state(&JAW_WORM_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 6;
        state.monsters[0].max_hp = 6;
        state.monsters[1].hp = 30;
        state.monsters[1].max_hp = 30;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("Strike plus Panache should resolve after a lethal hit");

        assert!(!next.monsters[0].alive);
        assert_eq!(next.monsters[1].hp, 20);
    }

    #[test]
    fn reptomancer_death_ends_combat_with_daggers_alive() {
        let reptomancer_id = MonsterId::new(2);
        let mut left_dagger = monster_state(&DAGGER_A0, MonsterId::new(1));
        left_dagger.powers.minion = 1;
        let mut right_dagger = monster_state(&DAGGER_A0, MonsterId::new(3));
        right_dagger.powers.minion = 1;
        let mut reptomancer = monster_state(&REPTOMANCER_A0, reptomancer_id);
        reptomancer.hp = 5;
        reptomancer.max_hp = 5;

        let mut state = CombatState::initial_fixture();
        state.monsters = vec![left_dagger, reptomancer, right_dagger];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), PERFECTED_STRIKE_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(reptomancer_id),
            },
        )
        .expect("Perfected Strike should resolve");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(next.monsters.iter().all(|monster| !monster.alive));
    }

    #[test]
    fn all_enemy_damage_skips_daggers_that_flee_after_reptomancer_dies() {
        let reptomancer_id = MonsterId::new(2);
        let mut left_dagger = monster_state(&DAGGER_A0, MonsterId::new(1));
        left_dagger.powers.minion = 1;
        let mut right_dagger = monster_state(&DAGGER_A0, MonsterId::new(3));
        right_dagger.powers.minion = 1;
        let mut reptomancer = monster_state(&REPTOMANCER_A0, reptomancer_id);
        reptomancer.hp = 5;
        reptomancer.max_hp = 5;

        let mut state = CombatState::initial_fixture();
        state.player.energy = 2;
        state.monsters = vec![left_dagger, reptomancer, right_dagger];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), IMMOLATE_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Immolate+ should skip Daggers that flee after Reptomancer dies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(next.monsters.iter().all(|monster| !monster.alive));
    }

    #[test]
    fn gremlin_horn_triggers_on_awakened_one_half_death() {
        // FIDL00378: killing Awakened One form 1 still grants Horn energy+draw
        // because combat continues into form 2.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::AWAKENED_ONE_A0,
            target,
        )];
        state.monsters[0].hp = 5;
        state.monsters[0].max_hp = 300;
        state.monsters[0].mode_shift = 0;
        state.player.energy = 3;
        state.relics.push(Relic::GremlinHorn);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Strike kills form 1");
        assert!(!next.monsters[0].alive);
        assert!(crate::content::monsters::awakened_one_is_half_dead(
            &next.monsters[0]
        ));
        assert_eq!(
            next.player.energy,
            3 - 1 + 1,
            "pay Strike, gain Horn energy"
        );
        assert!(
            next.piles.hand.iter().any(|c| c.content_id == DEFEND_R_ID),
            "Horn draws a card"
        );
    }

    #[test]
    fn gremlin_horn_shuffle_includes_a_lethal_aoe_card() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.relics.push(Relic::GremlinHorn);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), CLEAVE_PLUS_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters = vec![
            monster_state(&JAW_WORM_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Cleave+ should trigger Gremlin Horn after entering discard");

        assert!(!next.monsters[0].alive);
        assert!(next.monsters[1].alive);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, CLEAVE_PLUS_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.player.energy, 1);
    }

    #[test]
    fn hex_draw_pile_mutation_precedes_ink_bottle_draw() {
        let mut queue = VecDeque::from([
            InternalAction::DrawCardsWhilePlayedCardIsInLimbo {
                card_id: CardId::new(1),
                count: 1,
            },
            InternalAction::DrawCardsFromInkBottle { count: 1 },
        ]);

        push_follow_up(
            &mut queue,
            InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: DAZED_ID,
            },
            false,
        );

        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DrawCardsWhilePlayedCardIsInLimbo { .. })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                content_id: DAZED_ID
            })
        ));
        assert!(matches!(
            queue.pop_front(),
            Some(InternalAction::DrawCardsFromInkBottle { count: 1 })
        ));
    }

    #[test]
    fn gambling_chip_discards_in_selection_order() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), CLASH_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(99), ANGER_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), DEFEND_R_ID),
            CardInstance::new(CardId::new(7), BASH_ID),
        ];

        open_gambling_chip_select(&mut state).expect("Gambler's Brew opens selection");
        choose_exhaust_select(&mut state, 0).expect("select first card");
        choose_exhaust_select(&mut state, 1).expect("select second card in visible order");
        choose_exhaust_select(&mut state, 0).expect("select third card in visible order");
        confirm_exhaust_select(&mut state).expect("Gambler's Brew confirms selection");

        let discarded = state
            .piles
            .discard_pile
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        assert_eq!(
            discarded,
            vec![
                CardId::new(99),
                CardId::new(1),
                CardId::new(3),
                CardId::new(2)
            ]
        );
    }

    #[test]
    fn rampage_growth_overflow_and_wrong_source_fail_closed() {
        let mut state = CombatState::initial_fixture();
        let mut rampage = CardInstance::new(CardId::new(1), RAMPAGE_ID);
        rampage.rampage_damage_bonus = i32::MAX;
        state.piles.hand = vec![rampage];
        let before = state.clone();

        assert_eq!(
            add_rampage_damage_bonus(&mut state, CardId::new(1), 1),
            Err(SimError::InvalidState("Rampage damage bonus overflows i32"))
        );
        assert_eq!(state, before);

        state.piles.hand[0] = CardInstance::new(CardId::new(1), STRIKE_R_ID);
        let wrong_source = state.clone();
        assert_eq!(
            add_rampage_damage_bonus(&mut state, CardId::new(1), 5),
            Err(SimError::InvalidState(
                "Rampage growth source is not Rampage"
            ))
        );
        assert_eq!(state, wrong_source);
    }

    #[test]
    fn ritual_dagger_growth_overflow_and_missing_source_fail_closed() {
        let mut state = CombatState::initial_fixture();
        let mut ritual_dagger = CardInstance::new(CardId::new(1), RITUAL_DAGGER_ID);
        ritual_dagger.ritual_dagger_damage_bonus = i32::MAX;
        state.piles.hand = vec![ritual_dagger];
        let before = state.clone();

        assert_eq!(
            add_ritual_dagger_damage_bonus(&mut state, CardId::new(1), 1),
            Err(SimError::InvalidState(
                "Ritual Dagger damage bonus overflows i32"
            ))
        );
        assert_eq!(state, before);
        assert_eq!(
            add_ritual_dagger_damage_bonus(&mut state, CardId::new(99), 1),
            Err(SimError::UnknownCard(CardId::new(99)))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn power_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.strength = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn red_skull_healing_failure_reaches_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::RedSkull);
        state.player.hp = state.player.max_hp / 2;
        state.player.powers.strength = i32::MIN;
        state.relic_counters.red_skull_active = true;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BANDAGE_UP_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "Red Skull Strength removal underflows i32"
            ))
        );
    }

    #[test]
    fn monster_vulnerable_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.vulnerable = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BASH_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "monster Vulnerable application overflows i32"
            ))
        );
    }

    #[test]
    fn bash_damage_uses_strength_before_pain_rupture_from_same_play() {
        // Pain LoseHP + Rupture must not boost Strength until after Bash hits.
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 3;
        state.player.powers.strength = 2;
        state.player.powers.rupture = 1;
        state.monsters[0].hp = 30;
        state.monsters[0].powers.vulnerable = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BASH_ID),
            CardInstance::new(CardId::new(2), PAIN_ID),
        ];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Bash resolves");

        // 8 base + 2 str = 10; vuln x1.5 → 15. Cultist 30 → 15.
        assert_eq!(
            next.monsters[0].hp, 15,
            "Bash must not use post-Pain Strength"
        );
        assert_eq!(
            next.player.powers.strength, 3,
            "PainPower trigger must apply Rupture after card damage"
        );
    }

    #[test]
    fn rupture_power_does_not_react_to_pain_from_its_own_play() {
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 3;
        state.player.hp = 30;
        state.player.powers.strength = 2;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), RUPTURE_ID),
            CardInstance::new(CardId::new(2), PAIN_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Rupture resolves");

        assert_eq!(next.player.hp, 29);
        assert_eq!(next.player.powers.rupture, 1);
        assert_eq!(next.player.powers.strength, 2);
    }

    #[test]
    fn pain_loss_settles_before_warcry_selection_opens() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 50;
        state.player.max_hp = 50;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), PAIN_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), CLEAVE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry with Pain resolves to hand selection");

        assert_eq!(next.player.hp, 49, "Pain must resolve before the select");
        assert!(next.hand_select().is_some());
        assert_eq!(next.pending_hand_select_action_count(), 0);
    }

    #[test]
    fn pain_loss_triggers_centennial_draw_before_burning_pact_selection() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 50;
        state.player.max_hp = 50;
        state.relics.push(Relic::CentennialPuzzle);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), PAIN_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), CLEAVE_ID),
            CardInstance::new(CardId::new(7), IRON_WAVE_ID),
            CardInstance::new(CardId::new(8), CLOTHESLINE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact with Pain resolves to exhaust selection");

        assert_eq!(next.player.hp, 49, "Pain must resolve before the select");
        assert!(next.exhaust_select().is_some());
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
        assert!(next
            .exhaust_select()
            .expect("Burning Pact select")
            .pending_actions
            .is_empty());
    }

    #[test]
    fn battle_trance_pain_puzzle_draws_before_no_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.relics.push(Relic::CentennialPuzzle);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BATTLE_TRANCE_ID),
            CardInstance::new(CardId::new(2), PAIN_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), CLEAVE_ID),
            CardInstance::new(CardId::new(7), INFLAME_ID),
        ];
        state.piles.draw_pile = (10..20)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Battle Trance with Pain and Puzzle resolves");

        assert_eq!(next.player.hp, state.player.hp - 1);
        assert!(next.player.cannot_draw);
        assert_eq!(next.piles.hand.len(), 10);
        assert_eq!(next.relic_counters.centennial_puzzle_triggers, 1);
    }

    #[test]
    fn battle_trance_no_draw_is_blocked_by_artifact() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.player.powers.artifact = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BATTLE_TRANCE_ID)];
        state.piles.draw_pile = (10..13)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Battle Trance with Artifact");

        assert_eq!(next.player.powers.artifact, 0);
        assert!(!next.player.cannot_draw);
        assert_eq!(next.piles.hand.len(), 3);
    }

    #[test]
    fn transmutation_x_cost_fills_hand_to_ten_with_source_in_limbo() {
        // FIDL00413: with 4 other cards, X>=6 must yield a full 10-card hand
        // after Transmutation exhausts (source not counted during generation).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 8;
        // FIDL00394 floor 44: cardRandom seed = runSeed + floor.
        let seed = 34_961_238_619_619i64.wrapping_add(44);
        state.rng.card_random_rng = crate::rng::StsRng::new(seed);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), DEFEND_R_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
            CardInstance::new(CardId::new(14), TRANSMUTATION_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.piles.limbo.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(14),
                target: None,
            },
        )
        .expect("Transmutation resolves");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.piles.hand.len(), 10, "hand must fill to max");
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRANSMUTATION_ID));
        assert_eq!(
            next.piles.hand.len() + next.piles.discard_pile.len(),
            4 + 8,
            "4 kept + 8 generated colorless"
        );
    }

    #[test]
    fn champion_belt_weak_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::ChampionBelt);
        state.monsters[0].powers.weak = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BASH_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "monster Weak application overflows i32"
            ))
        );
        assert_eq!(state.monsters[0].powers.vulnerable, 0);
        assert_eq!(state.monsters[0].powers.weak, i32::MAX);
    }

    #[test]
    fn card_play_trigger_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.slow = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn copied_hand_card_rejects_id_exhaustion_without_mutating_combat() {
        let mut state = CombatState::initial_fixture();
        let card_id = CardId::new(crate::ids::MAX_SUPPORTED_CARD_INSTANCE_ID);
        state.piles.hand = vec![CardInstance::new(card_id, STRIKE_R_ID)];
        let before = state.clone();

        assert_eq!(
            apply_internal_action(&mut state, InternalAction::CopyHandCardToHand { card_id },),
            Err(SimError::InvalidState(
                "card instance ID allocation exceeds the supported domain"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn rage_block_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.block = i32::MAX;
        state.player.temp_rage_block = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn sentinel_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(2), SENTINEL_ID),
        ];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat player block or energy is negative"
            ))
        );
    }

    #[test]
    fn relic_counter_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.relic_counters.cards_played_this_turn = i32::MAX as u32;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFLAME_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat relic counter exceeds the target signed range"
            ))
        );
    }

    #[test]
    fn nunchaku_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::Nunchaku);
        state.relic_counters.nunchaku_attacks_played = crate::relic::NUNCHAKU_THRESHOLD - 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    fn shuffle_trigger_state(relic: Relic) -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.relics.push(relic);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        state
    }

    #[test]
    fn abacus_block_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::TheAbacus);
        state.player.block = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn lethal_damage_skips_queued_random_spot_card_without_rng() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.block = 0;
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        let card_random_counter = state.rng.card_random_rng.counter();

        let next = process_internal_queue(
            &state,
            VecDeque::from([
                InternalAction::LoseHp {
                    amount: 1,
                    source: HpLossSource::Other,
                },
                InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                    content_id: DAZED_ID,
                },
            ]),
        )
        .expect("lethal queue should resolve");

        assert_eq!(next.state.phase, CombatPhase::Lost);
        assert_eq!(next.state.piles.draw_pile.len(), 1);
        assert_eq!(
            next.state.rng.card_random_rng.counter(),
            card_random_counter
        );
    }

    #[test]
    fn lethal_pommel_draw_skips_empty_deck_shuffle_and_sundial() {
        // Target: Pommel damage kills the last enemy, then DrawCardAction runs.
        // EmptyDeckShuffleAction / Sundial do not fire once the battle is ending
        // (permanent a56e1754 energy 6!=4 was an extra post-lethal shuffle).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::Sundial);
        state.relic_counters.sundial_shuffles = 2;
        state.player.energy = 4;
        state.player.max_energy = 4;
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("lethal Pommel resolves");

        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(
            next.relic_counters.sundial_shuffles, 2,
            "post-lethal empty-deck shuffle must not advance Sundial"
        );
        assert!(
            next.piles.draw_pile.is_empty(),
            "discard stays unshuffled when the battle is ending"
        );
        assert_eq!(next.piles.discard_pile.len(), 3); // 2 prior + played Pommel? or Pommel may exhaust-no
    }

    #[test]
    fn sundial_counter_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::Sundial);
        state.relic_counters.sundial_shuffles = i32::MAX as u32;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat relic counter is outside its stable range"
            ))
        );
    }

    #[test]
    fn sundial_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = shuffle_trigger_state(Relic::Sundial);
        state.player.energy = i32::MAX;
        state.relic_counters.sundial_shuffles = crate::relic::SUNDIAL_THRESHOLD - 1;

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: None,
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn gremlin_horn_energy_overflow_fails_closed_at_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::GremlinHorn);
        state.monsters[0].hp = 1;
        let mut survivor = state.monsters[0].clone();
        survivor.id = MonsterId::new(2);
        survivor.hp = survivor.max_hp;
        state.monsters.push(survivor);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(state.monsters[0].id),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn juggernaut_death_hook_failure_propagates_to_the_combat_action_boundary() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.player.temp_rage_block = 1;
        state.player.powers.juggernaut = 1;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(1)),
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(2)),
        ];
        for monster in &mut state.monsters {
            monster.hp = 1;
        }
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        assert_eq!(
            apply_combat_action(
                &state,
                CombatAction::PlayCard {
                    card_id: CardId::new(1),
                    target: Some(MonsterId::new(1)),
                },
            ),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
    }

    #[test]
    fn immediate_death_hook_failure_rolls_back_spore_cloud_and_relics() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].hp = 0;
        let before = state.clone();

        assert_eq!(
            apply_monster_death_hooks(&mut state, fungi_id),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn spore_cloud_vulnerable_overflow_rolls_back_death_hooks() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.powers.vulnerable = i32::MAX;
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].hp = 0;
        let before = state.clone();

        assert_eq!(
            apply_monster_death_hooks(&mut state, fungi_id),
            Err(SimError::InvalidState(
                "player Vulnerable application overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn draw_failure_rolls_back_card_rng_fire_breathing_and_death_hooks() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = i32::MAX;
        state.player.powers.fire_breathing = 1;
        state.relics.push(Relic::GremlinHorn);
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), BURN_ID)];
        let before = state.clone();

        assert_eq!(
            player_draw_cards(&mut state, 1),
            Err(SimError::InvalidState(
                "combat integer addition overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn queued_monster_death_queues_gremlin_horn_follow_up() {
        let mut state = CombatState::initial_fixture();
        let dead_id = state.monsters[0].id;
        let mut survivor = state.monsters[0].clone();
        survivor.id = MonsterId::new(2);
        state.monsters[0].alive = false;
        state.monsters.push(survivor);
        state.relics.push(Relic::GremlinHorn);

        let follow_ups = queue_monster_death_hooks(&mut state, dead_id).expect("queue Horn");
        assert_eq!(follow_ups, vec![InternalAction::ApplyGremlinHornOnDeath]);
        assert_eq!(state.pending_monster_death_relic_triggers, 0);
    }

    #[test]
    fn later_gremlin_horn_actions_precede_first_draws_evolve_follow_up() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.player.powers.evolve = 1;
        state.relics = vec![Relic::GremlinHorn];
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), WOUND_ID),
        ];
        state.piles.discard_pile.clear();

        let transition = process_internal_queue(
            &state,
            VecDeque::from([
                InternalAction::ApplyGremlinHornOnDeath,
                InternalAction::ApplyGremlinHornOnDeath,
            ]),
        )
        .expect("two queued Horn deaths");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::ApplyGremlinHornOnDeath,
                InternalAction::GainEnergy { amount: 1 },
                InternalAction::DrawCards { count: 1 },
                InternalAction::ApplyGremlinHornOnDeath,
                InternalAction::GainEnergy { amount: 1 },
                InternalAction::DrawCards { count: 1 },
                InternalAction::DrawCards { count: 1 },
            ],
            "Evolve from Horn draw one is addToBot behind Horn death two"
        );
        assert_eq!(transition.state.player.energy, 2);
        assert_eq!(transition.state.piles.hand.len(), 3);
    }

    #[test]
    fn hex_dazed_waits_for_armaments_hand_select_to_close() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.rng.card_random_rng = StsRng::new(7_141_693_325_691_831_207);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), ARMAMENTS_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), RAMPAGE_ID),
        ];

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Armaments should open its hand-select screen");

        assert!(next.hand_select().is_some());
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            0
        );
        assert_eq!(next.pending_hand_select_action_count(), 1);

        choose_hand_select(&mut next, 0).expect("Strike is selectable");
        confirm_hand_select(&mut next).expect("Armaments selection should resolve");

        assert_eq!(next.pending_hand_select_action_count(), 0);
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            1
        );
    }

    #[test]
    fn armaments_dark_embrace_draw_lands_after_upgraded_card_and_leftovers() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), ARMAMENTS_ID),
            CardInstance::new(CardId::new(2), HAVOC_ID),
            CardInstance::new(CardId::new(3), ARMAMENTS_PLUS_ID),
            CardInstance::new(CardId::new(4), RAMPAGE_ID),
            CardInstance::new(CardId::new(5), HAVOC_PLUS_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(6), BERSERK_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Armaments opens its hand-select screen");
        next.play_top_force_exhaust_active = true;
        choose_hand_select(&mut next, 1).expect("Rampage is selectable among upgradeables");
        confirm_hand_select(&mut next).expect("Armaments confirm");

        let hand: Vec<ContentId> = next.piles.hand.iter().map(|card| card.content_id).collect();
        assert_eq!(
            hand,
            vec![
                HAVOC_ID,
                RAMPAGE_PLUS_ID,
                ARMAMENTS_PLUS_ID,
                HAVOC_PLUS_ID,
                BERSERK_ID,
            ],
            "Dark Embrace draw must follow ArmamentsAction's returned cards"
        );
    }

    #[test]
    fn hex_uses_warcry_selected_card_before_deferred_dazed() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
            CardInstance::new(CardId::new(6), DEFEND_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry should open its hand-select screen");
        choose_hand_select(&mut next, 0).expect("select Strike");

        let mut expected_rng = next.rng.card_random_rng.clone();
        let mut expected_draw = next
            .piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        expected_draw.push(STRIKE_R_ID);
        let generated_index = expected_rng.random_int((expected_draw.len() - 1) as i32) as usize;
        expected_draw.insert(generated_index, DAZED_ID);

        confirm_hand_select(&mut next).expect("confirm Warcry selection");

        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            expected_draw
        );
    }

    #[test]
    fn havoc_hex_force_played_burning_pact_parks_second_dazed_until_confirm() {
        // FIDL01694: Havoc under Hex force-plays Burning Pact. Havoc's Dazed
        // lands after PlayTop removes Burning Pact; Burning Pact's own Hex
        // stays behind ExhaustSelect so the PLAY frame has only one new Dazed.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.hex = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), HAVOC_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), DAZED_ID),
            CardInstance::new(CardId::new(5), SEARING_BLOW_ID),
            CardInstance::new(CardId::new(6), PERFECTED_STRIKE_ID),
            CardInstance::new(CardId::new(7), BURNING_PACT_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(3),
                target: None,
            },
        )
        .expect("Havoc force-plays Burning Pact");

        assert!(
            next.exhaust_select().is_some(),
            "force-played Burning Pact must still open exhaust select"
        );
        let dazed = next
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id == DAZED_ID)
            .count();
        assert_eq!(
            dazed, 2,
            "one pre-existing Dazed plus Havoc Hex; Burning Pact Hex is parked"
        );
        assert_eq!(next.piles.draw_pile.len(), 4);
    }

    #[test]
    fn hex_dazed_waits_for_burning_pact_exhaust_select_to_close() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.rng.card_random_rng = StsRng::new(7_141_693_325_691_831_207);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), RAMPAGE_ID),
        ];

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact should open its exhaust-select screen");

        assert!(next.exhaust_select().is_some());
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            0
        );
        assert_eq!(
            next.exhaust_select()
                .expect("exhaust select")
                .pending_actions
                .len(),
            1
        );

        choose_exhaust_select(&mut next, 0).expect("Strike is selectable");
        confirm_exhaust_select(&mut next).expect("Burning Pact selection should resolve");

        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            1
        );
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DAZED_ID, SHRUG_IT_OFF_ID]
        );
    }

    #[test]
    fn burning_pact_under_corruption_exhausts_source_not_discard() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.corruption = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_exhaust_select(&mut next).expect("confirm Burning Pact");

        assert!(
            next.piles
                .discard_pile
                .iter()
                .all(|card| card.content_id != BURNING_PACT_ID),
            "Corruption must not leave Burning Pact in discard"
        );
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == BURNING_PACT_ID)
                .count(),
            1
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == STRIKE_R_ID));
    }

    #[test]
    fn hex_on_empty_draw_after_burning_pact_does_not_burn_card_random() {
        // CardGroup.addToRandomSpot on an empty pile is group.add only — no
        // cardRandomRng call. A phantom random_int(0) after BP emptied the
        // pile desynced later Hex inserts (15ab4cc).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.hex = 1;
        state.rng.card_random_rng = StsRng::new(99);
        let counter_before = state.rng.card_random_rng.counter();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        // Exactly two cards: BP draws both, leaving empty before deferred Hex.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
        ];
        state.piles.discard_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_exhaust_select(&mut next).expect("BP resolves");

        assert_eq!(
            next.rng.card_random_rng.counter(),
            counter_before,
            "empty-pile Hex must not consume cardRandomRng"
        );
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            1,
            "Hex still adds one Dazed when the pile was emptied by BP draw"
        );

        // Next public Hex roll uses the stream that was not poisoned.
        next.player.energy = 1;
        next.piles.hand = vec![CardInstance::new(CardId::new(10), DEFEND_R_ID)];
        next.piles.draw_pile = vec![
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), BASH_ID),
            CardInstance::new(CardId::new(13), CLEAVE_ID),
        ];
        let mut expected_rng = next.rng.card_random_rng.clone();
        let mut expected_draw = next
            .piles
            .draw_pile
            .iter()
            .map(|c| c.content_id)
            .collect::<Vec<_>>();
        let idx = expected_rng.random_int((expected_draw.len() - 1) as i32) as usize;
        expected_draw.insert(idx, DAZED_ID);

        let after = apply_combat_action(
            &next,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Defend under Hex");
        assert_eq!(
            after
                .piles
                .draw_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            expected_draw
        );
    }

    #[test]
    fn burning_pact_hex_inserts_before_evolve_extra_draw() {
        // Hex onUseCard is queued after Burning Pact's DrawCardAction(2) and
        // before Evolve's addToBot extra draw. Inserting after Evolve samples
        // the wrong pile size (FIDL01740 Battle Trance vs Dazed).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.hex = 1;
        state.player.powers.evolve = 1;
        state.rng.card_random_rng = StsRng::new(7_141_693_325_691_831_207);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), RECKLESS_CHARGE_ID),
            CardInstance::new(CardId::new(3), THUNDERCLAP_ID),
        ];
        // Last entry is the top. Draw(2) takes Strike then Dazed; Evolve should
        // then take Exhume after Hex inserts into this 4-card remainder.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), CLASH_ID),
            CardInstance::new(CardId::new(5), BATTLE_TRANCE_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), EXHUME_ID),
            CardInstance::new(CardId::new(8), DAZED_ID),
            CardInstance::new(CardId::new(9), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut expected_rng = state.rng.card_random_rng.clone();
        let mut expected_draw = vec![CLASH_ID, BATTLE_TRANCE_ID, STRIKE_R_ID, EXHUME_ID];
        let insert_index = expected_rng.random_int((expected_draw.len() - 1) as i32) as usize;
        expected_draw.insert(insert_index, DAZED_ID);
        let evolved = expected_draw.pop().expect("Evolve draws the post-Hex top");

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        choose_exhaust_select(&mut next, 1).expect("select Thunderclap");
        confirm_exhaust_select(&mut next).expect("BP resolves");

        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            expected_draw,
            "Hex must insert against the post-Draw(2) pile, then Evolve draws"
        );
        assert_eq!(evolved, EXHUME_ID);
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == EXHUME_ID),
            "Evolve extra draw should be Exhume, got {:?}",
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn juggernaut_random_target_selection_waits_for_hex_draw_insertion() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.hex = 1;
        state.player.powers.juggernaut = 5;
        state.rng.card_random_rng = StsRng::new(1_234);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), DEFEND_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), BERSERK_ID),
            CardInstance::new(CardId::new(3), ARMAMENTS_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), COMBUST_ID),
        ];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Defend should resolve with Juggernaut and Hex");

        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![BERSERK_ID, DAZED_ID, ARMAMENTS_ID, DEFEND_R_ID, COMBUST_ID]
        );
        assert_eq!(next.rng.card_random_rng.counter(), 2);
    }

    #[test]
    fn hex_dazed_inserts_before_dark_embrace_draw_under_corruption() {
        // HexPower.onUseCard queues MakeTempCardInDrawPileAction before
        // UseCardAction. Under Corruption the skill exhausts and Dark Embrace
        // draws afterward, so the random-spot roll uses the pre-draw pile size.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.player.powers.hex = 1;
        state.player.powers.corruption = 1;
        state.player.powers.dark_embrace = 1;
        state.rng.card_random_rng = StsRng::new(7_141_693_325_691_831_207);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), DEFEND_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), PERFECTED_STRIKE_ID),
            CardInstance::new(CardId::new(7), DEFEND_R_PLUS_ID),
            CardInstance::new(CardId::new(8), STRIKE_R_ID),
            CardInstance::new(CardId::new(9), HEAVY_BLADE_ID),
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut expected_rng = state.rng.card_random_rng.clone();
        let mut expected_draw = state
            .piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        let insert_index = expected_rng.random_int((expected_draw.len() - 1) as i32) as usize;
        expected_draw.insert(insert_index, DAZED_ID);
        let drawn = expected_draw
            .pop()
            .expect("Dark Embrace draws the top card after Hex insert");

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Defend under Hex/Corruption/Dark Embrace should resolve");

        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            expected_draw,
            "Hex must insert against the full pre-exhaust draw pile"
        );
        assert_eq!(
            next.piles.hand.last().map(|card| card.content_id),
            Some(drawn)
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
        assert_eq!(next.rng.card_random_rng.counter(), 1);
    }

    #[test]
    fn violence_uses_card_group_add_to_random_spot_bounds() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), HEADBUTT_ID),
            CardInstance::new(CardId::new(5), RAMPAGE_ID),
            CardInstance::new(CardId::new(6), ANGER_ID),
        ];
        state.rng.card_random_rng = StsRng::new(1_234);
        state.rng.shuffle_rng = StsRng::new(5_678);

        draw_random_attacks_from_draw_pile(&mut state, 3);

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(5), CardId::new(4), CardId::new(6)]
        );
        assert_eq!(
            state
                .piles
                .draw_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(1), CardId::new(2), CardId::new(3)]
        );
    }

    #[test]
    fn violence_strange_spoon_rolls_after_attack_tmp_group() {
        // UseCardAction's Spoon roll is after ViolenceAction. An early roll
        // would consume cardRandomRng before addToRandomSpot and pick a
        // different three-attack set (FIDL01427).
        let mut expected = CombatState::initial_fixture();
        expected.piles.hand.clear();
        expected.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), HEADBUTT_ID),
            CardInstance::new(CardId::new(5), RAMPAGE_ID),
            CardInstance::new(CardId::new(6), ANGER_ID),
        ];
        expected.rng.card_random_rng = StsRng::new(1_234);
        expected.rng.shuffle_rng = StsRng::new(5_678);
        draw_random_attacks_from_draw_pile(&mut expected, 3);
        let expected_hand: Vec<_> = expected.piles.hand.iter().map(|card| card.id).collect();

        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.relics.push(Relic::StrangeSpoon);
        state.piles.hand = vec![CardInstance::new(CardId::new(20), VIOLENCE_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), HEADBUTT_ID),
            CardInstance::new(CardId::new(5), RAMPAGE_ID),
            CardInstance::new(CardId::new(6), ANGER_ID),
        ];
        state.rng.card_random_rng = StsRng::new(1_234);
        state.rng.shuffle_rng = StsRng::new(5_678);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(20),
                target: None,
            },
        )
        .expect("Violence with Strange Spoon resolves");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            expected_hand
        );
        assert_eq!(
            next.rng.card_random_rng.counter(),
            expected.rng.card_random_rng.counter() + 1
        );
    }

    #[test]
    fn violence_does_not_count_source_toward_hand_capacity() {
        // FIDL01255: 7 other cards + Violence in hand, three attacks in draw.
        // cardInUse is out of the hand, so all three attacks fit.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), VIOLENCE_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), FLEX_ID),
            CardInstance::new(CardId::new(5), HAVOC_ID),
            CardInstance::new(CardId::new(6), MAGNETISM_ID),
            CardInstance::new(CardId::new(7), STRIKE_R_ID),
            CardInstance::new(CardId::new(8), SWIFT_STRIKE_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), PERFECTED_STRIKE_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), HEADBUTT_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
        ];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Violence resolves");

        assert_eq!(next.piles.hand.len(), 10);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.id == CardId::new(12)));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.id != CardId::new(12)));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == CardId::new(1)));
    }

    #[test]
    fn feed_kill_applies_magic_flower_to_the_hp_gain() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.hp = 44;
        state.player.max_hp = 109;
        state.player.energy = 3;
        state.relics.push(Relic::MagicFlower);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed should kill the target");

        assert_eq!(next.player.max_hp, 112);
        assert_eq!(next.player.hp, 49);
    }

    #[test]
    fn feed_kill_with_mark_of_bloom_raises_max_hp_without_healing() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.hp = 80;
        state.player.max_hp = 109;
        state.player.energy = 3;
        state.mark_of_bloom = true;
        state.relics.push(Relic::MarkOfBloom);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed+ should kill the target");

        assert_eq!(next.player.max_hp, 113);
        assert_eq!(next.player.hp, 80);
    }

    #[test]
    fn feed_does_not_gain_max_hp_from_a_half_dead_darkling() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        // Keep a living sibling so Life Link does not permanently kill the pack
        // (source Darkling.damage allDead requires every Darkling half-dead).
        state.monsters = vec![
            monster_state(&DARKLING_A0, target),
            monster_state(&DARKLING_A0, MonsterId::new(2)),
        ];
        for monster in &mut state.monsters {
            monster.rolled_attack_damage = Some(8);
            monster.intent = crate::MonsterIntent::Attack { damage: 8 };
        }
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.hp = 81;
        state.player.max_hp = 114;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed should put the Darkling into its half-dead state");

        assert_eq!(next.player.max_hp, 114);
        assert_eq!(next.player.hp, 81);
        assert!(!next.monsters[0].alive);
        assert!(next.monsters[0].escaped);
        assert!(next.monsters[1].alive);
    }

    #[test]
    fn rage_triggered_block_ignores_dexterity_before_guardian_sharp_hide() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&GUARDIAN_A0, target)];
        state.monsters[0].powers.spikes = 3;
        state.monsters[0].powers.strength = -2;
        state.player.block = 7;
        state.player.powers.dexterity = 2;
        state.player.temp_rage_block = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Anger should resolve through Rage and Sharp Hide");

        assert_eq!(next.player.block, 7);
    }

    #[test]
    fn whirlwind_rage_block_absorbs_spikes_before_deferred_hits() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 80;
        state.player.energy = 2;
        state.player.temp_rage_block = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), WHIRLWIND_ID)];
        state.monsters[0].powers.spikes = 15;
        state.monsters[0].hp = 40;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Whirlwind should resolve Rage before deferred hits");

        assert_eq!(next.player.block, 0);
        assert_eq!(next.player.hp, 53);
    }

    #[test]
    fn sever_soul_sharp_hide_lands_before_feel_no_pain_block() {
        // SharpHidePower.onUseCard queues its damage before ExhaustAll runs;
        // FeelNoPainPower.onExhaust addToBot's block behind it (FIDL02470).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&GUARDIAN_A0, target)];
        state.monsters[0].powers.spikes = 3;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 240;
        state.monsters[0].block = 0;
        state.player.hp = 80;
        state.player.block = 0;
        state.player.energy = 2;
        state.player.powers.feel_no_pain = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Sever Soul should resolve Sharp Hide before exhaust FNP");

        assert_eq!(next.player.hp, 77);
        assert_eq!(next.player.block, 3);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
    }

    #[test]
    fn sever_soul_dark_embrace_draws_after_played_card_discard_reshuffle() {
        // DarkEmbracePower.onExhaust addToBot's DrawCardAction, so DE draws run
        // after UseCardAction discards Sever Soul. With an empty draw pile the
        // reshuffle must include Sever Soul; immediate-on-exhaust draws leave it
        // stranded in discard and desync the post-play hand.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].block = 0;
        state.player.energy = 2;
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), ARMAMENTS_ID),
            CardInstance::new(CardId::new(7), FLEX_ID),
            CardInstance::new(CardId::new(8), MADNESS_ID),
        ];
        state.piles.exhaust_pile.clear();
        state.rng.shuffle_rng = StsRng::new(42);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Sever Soul with Dark Embrace should resolve");

        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, DEFEND_R_ID]
        );
        assert!(
            next.piles.discard_pile.is_empty(),
            "reshuffle for DE draws must consume discard after Sever Soul settles"
        );
        assert!(
            next.piles
                .draw_pile
                .iter()
                .any(|card| card.content_id == SEVER_SOUL_ID)
                || next
                    .piles
                    .hand
                    .iter()
                    .any(|card| card.content_id == SEVER_SOUL_ID),
            "Sever Soul must be in the reshuffled draw or hand, not stranded in discard"
        );
        // Iron Wave-equivalent: kept attack + two DE draws from the 5-card
        // reshuffle (4 prior discard + Sever Soul).
        assert_eq!(next.piles.hand.len(), 3);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert_eq!(next.piles.draw_pile.len(), 3);
    }

    #[test]
    fn spore_cloud_player_turn_kill_does_not_set_vulnerable_just_applied() {
        // Player-turn Fungi kill (even with Flame Barrier up) must allow the
        // upcoming monster-turn cleanup to tick Vulnerable.
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.phase = CombatPhase::WaitingForPlayer;
        state.player.temp_thorns = 4;
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].powers.spore_cloud = 2;

        apply_monster_death_hooks(&mut state, fungi_id).expect("death hooks resolve");

        assert_eq!(state.player.powers.vulnerable, 2);
        assert!(
            !state.player.vulnerable_just_applied,
            "player-turn Spore Cloud must not set just_applied"
        );
    }

    #[test]
    fn spore_cloud_monster_turn_without_temp_thorns_does_not_set_just_applied() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.phase = CombatPhase::MonsterTurn;
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].powers.spore_cloud = 2;

        apply_monster_death_hooks(&mut state, fungi_id).expect("death hooks resolve");

        assert_eq!(state.player.powers.vulnerable, 2);
        assert!(!state.player.vulnerable_just_applied);
    }

    #[test]
    fn spore_cloud_monster_turn_flame_barrier_sets_vulnerable_just_applied() {
        // Mid-monster-turn Flame Barrier kill must survive the same cleanup tick.
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.phase = CombatPhase::MonsterTurn;
        state.player.temp_thorns = 4;
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;
        state.monsters[0].powers.spore_cloud = 2;

        apply_monster_death_hooks(&mut state, fungi_id).expect("death hooks resolve");

        assert_eq!(state.player.powers.vulnerable, 2);
        assert!(
            state.player.vulnerable_just_applied,
            "monster-turn FB Spore Cloud must set just_applied"
        );
    }

    #[test]
    fn spore_cloud_releases_only_when_battle_is_not_ending() {
        let fungi_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&FUNGI_BEAST_A0, fungi_id),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].alive = false;

        apply_monster_death_hooks(&mut state, fungi_id).expect("death hooks resolve");

        assert_eq!(state.player.powers.vulnerable, 2);

        let last_fungi_id = MonsterId::new(3);
        let mut ending_state = CombatState::initial_fixture();
        ending_state.monsters = vec![monster_state(&FUNGI_BEAST_A0, last_fungi_id)];
        ending_state.monsters[0].alive = false;

        apply_monster_death_hooks(&mut ending_state, last_fungi_id)
            .expect("ending death hooks resolve");

        assert_eq!(ending_state.player.powers.vulnerable, 0);
    }

    #[test]
    fn copied_single_target_card_fizzles_when_original_kills_target() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.player.energy = 3;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Pommel Strike+ should play");

        assert_eq!(next.piles.hand.len(), 2);
        assert_eq!(next.piles.draw_pile.len(), 2);
    }

    #[test]
    fn stasis_release_is_queued_after_a_lethal_card_draw() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.monsters = vec![
            monster_state(&BRONZE_ORB_A0, target),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.monsters[0].stasis_card = Some(CardInstance::new(CardId::new(2), IMMOLATE_ID));
        state.piles.hand = vec![CardInstance::new(CardId::new(1), POMMEL_STRIKE_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), DEFEND_R_ID)];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Pommel Strike should resolve the lethal Stasis hit");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID, IMMOLATE_ID]
        );
    }

    #[test]
    fn fire_breathing_stasis_release_waits_until_draw_batch_finishes() {
        // FireBreathingPower.onCardDraw addToBot's DamageAllEnemiesAction, so a
        // mid-batch curse/status draw must not release Bronze Orb Stasis into
        // the hand before the remaining cards of the same DrawCardAction.
        // Draw order is top-last: Anger, Strike, Regret (FB kills orb), Exhume,
        // Metallicize — then Disarm returns from Stasis.
        let mut state = CombatState::initial_fixture();
        state.player.powers.fire_breathing = 6;
        state.monsters = vec![
            monster_state(&BRONZE_ORB_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 5;
        state.monsters[0].max_hp = 5;
        state.monsters[0].stasis_card = Some(CardInstance::new(CardId::new(99), DISARM_ID));
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        // Bottom → top (drawn last-first via pop).
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), METALLICIZE_ID),
            CardInstance::new(CardId::new(2), EXHUME_ID),
            CardInstance::new(CardId::new(3), REGRET_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), ANGER_ID),
        ];

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 5)
            .expect("draw batch with Fire Breathing");

        assert!(!state.monsters[0].alive);
        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![
                ANGER_ID,
                STRIKE_R_ID,
                REGRET_ID,
                EXHUME_ID,
                METALLICIZE_ID,
                DISARM_ID,
            ]
        );
    }

    #[test]
    fn evolve_and_fire_breathing_callbacks_preserve_source_fifo_and_stasis_order() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&BRONZE_ORB_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.monsters[0].stasis_card = Some(CardInstance::new(CardId::new(99), BLUDGEON_ID));
        state.piles.hand.clear();
        // Bottom → top: two Evolve follow-up cards, then the five-card base
        // draw whose top card is Wound. Evolve is applied before Fire Breathing
        // so the source callback FIFO is unambiguous.
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), EXHUME_ID),
            CardInstance::new(CardId::new(2), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(3), CLASH_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DEFEND_R_ID),
            CardInstance::new(CardId::new(6), BASH_ID),
            CardInstance::new(CardId::new(7), WOUND_ID),
        ];
        apply_internal_action(&mut state, InternalAction::GainEvolve { amount: 2 })
            .expect("Evolve gain");
        apply_internal_action(&mut state, InternalAction::GainFireBreathing { amount: 6 })
            .expect("Fire Breathing gain");

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 5)
            .expect("ordered draw callbacks");

        assert!(!state.monsters[0].alive);
        let hand = state
            .piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        assert_eq!(
            &hand[hand.len() - 3..],
            &[HAVOC_PLUS_ID, EXHUME_ID, BLUDGEON_ID]
        );
        assert_eq!(hand.len(), 8);
        assert!(state.piles.draw_pile.is_empty());
        assert!(state.piles.discard_pile.is_empty());
    }

    #[test]
    fn nested_draw_callbacks_append_after_already_queued_fire_breathing() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&BRONZE_ORB_A0, MonsterId::new(1)),
            monster_state(&JAW_WORM_A0, MonsterId::new(2)),
        ];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 1;
        state.monsters[0].stasis_card = Some(CardInstance::new(CardId::new(99), BLUDGEON_ID));
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), EXHUME_ID),
            CardInstance::new(CardId::new(2), WOUND_ID),
            CardInstance::new(CardId::new(3), WOUND_ID),
        ];
        apply_internal_action(&mut state, InternalAction::GainEvolve { amount: 1 })
            .expect("Evolve gain");
        apply_internal_action(&mut state, InternalAction::GainFireBreathing { amount: 6 })
            .expect("Fire Breathing gain");

        crate::combat::draw::draw_cards_with_combat_rng(&mut state, 1)
            .expect("nested ordered draw callbacks");

        let hand = state
            .piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        assert_eq!(hand, vec![WOUND_ID, WOUND_ID, BLUDGEON_ID, EXHUME_ID]);
        assert!(!state.monsters[0].alive);
        assert!(state.piles.draw_pile.is_empty());
    }

    #[test]
    fn shrug_it_off_draws_after_freeing_its_slot_from_a_full_hand() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance::new(
                CardId::new(10),
                SHRUG_IT_OFF_ID,
            )))
            .collect();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), DEFEND_R_ID)];
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Shrug It Off should play from a full hand");

        assert_eq!(next.piles.hand.len(), 10);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn shrug_it_off_is_not_shuffled_into_its_own_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Shrug It Off should draw through a reshuffle");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DEFEND_R_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, SHRUG_IT_OFF_ID);
    }

    /// Reckless Charge leaves a generated Dazed on top of an empty draw pile.
    /// Shrug It Off draws that Dazed while the source is still in limbo; Evolve
    /// must wait until after UseCardAction discards Shrug so the status-triggered
    /// reshuffle includes the played card (real STS addToBot ordering).
    #[test]
    fn shrug_it_off_evolve_reshuffle_includes_discarded_source_after_dazed() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.evolve = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DAZED_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
        ];
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Shrug It Off draws Dazed then Evolve-draws through reshuffle");

        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == DAZED_ID),
            "Dazed from Reckless Charge / draw pile should be in hand"
        );
        assert_eq!(
            next.piles.hand.len(),
            2,
            "Evolve should draw one more card after Dazed"
        );
        assert!(
            next.piles.discard_pile.is_empty(),
            "reshuffle after discard should empty discard; got {:?}",
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
        assert!(
            next.piles
                .draw_pile
                .iter()
                .any(|card| card.content_id == SHRUG_IT_OFF_ID),
            "played Shrug must be in the post-Evolve draw pile, not left alone in discard"
        );
        assert_eq!(next.piles.draw_pile.len(), 3);
    }

    #[test]
    fn battle_trance_draws_after_freeing_its_full_hand_slot() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = (1..=9)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance::new(
                CardId::new(10),
                BATTLE_TRANCE_PLUS_ID,
            )))
            .collect();
        state.piles.draw_pile = (11..=14)
            .map(|id| CardInstance::new(CardId::new(id), DEFEND_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Battle Trance+ should play from a full hand");

        assert_eq!(next.piles.hand.len(), 10);
        assert_eq!(next.piles.draw_pile.len(), 3);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, BATTLE_TRANCE_PLUS_ID);
    }

    #[test]
    fn copied_attack_resolves_malleable_block_before_second_hit() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 71;
        state.monsters[0].block = 3;
        state.monsters[0].powers.malleable = 4;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 2;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HEAVY_BLADE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Heavy Blade should play");

        assert_eq!(next.monsters[0].hp, 50);
        assert_eq!(next.monsters[0].block, 5);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn double_tap_and_necronomicon_each_copy_the_original_once() {
        // Each extra play is an independent purgeOnUse copy of the original
        // card (FIDL00036 Heavy Blade vs intangible Nemesis: 3 hits of 1).
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::Necronomicon);
        state.double_tap_pending = 1;
        state.player.energy = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HEAVY_BLADE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.monsters[0].hp = 168;
        state.monsters[0].max_hp = 185;
        state.monsters[0].powers.intangible = 1;
        let target = state.monsters[0].id;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Heavy Blade");

        assert_eq!(
            next.monsters[0].hp, 165,
            "original + Necronomicon + Double Tap, not nested copies"
        );
        assert!(next.relic_counters.necronomicon_used_this_turn);
        assert_eq!(next.double_tap_pending, 0);
    }

    #[test]
    fn iron_wave_juggernaut_kills_before_malleable_block() {
        // Permanent tip 1ac7db2c9f4a3da9 step 670: Iron Wave with Juggernaut 5 vs
        // Snake Plant at 7 HP / Malleable 5. Block first queues Juggernaut
        // (addToBot), then Iron Wave damage (3 after Weak) leaves 4 HP and
        // queues Malleable (addToBot after Juggernaut). Juggernaut's 5 thorns
        // must kill before Malleable grants block — otherwise combat continues.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 7;
        state.monsters[0].max_hp = 75;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 5;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 1;
        state.player.block = 9;
        state.player.powers.juggernaut = 5;
        state.player.powers.dexterity = 2;
        state.player.powers.frail = 2;
        state.player.powers.weak = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), IRON_WAVE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Iron Wave should play");

        assert!(
            !next.monsters[0].alive,
            "Juggernaut must kill before Malleable block"
        );
        assert_eq!(next.monsters[0].hp, 0);
        assert_eq!(next.player.block, 14);
        assert_eq!(next.phase, CombatPhase::Won);
    }

    #[test]
    fn twin_strike_resolves_both_hits_before_curl_up_block() {
        let target = MonsterId::new(1);
        let mut state = CombatState::red_louse_fixture();
        state.monsters[0].hp = 15;
        state.monsters[0].max_hp = 15;
        state.monsters[0].powers.curl_up = 3;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), TWIN_STRIKE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Twin Strike should play");

        assert_eq!(next.monsters[0].hp, 5);
        assert_eq!(next.monsters[0].block, 3);
        assert_eq!(next.monsters[0].powers.curl_up, 0);
    }

    #[test]
    fn fiend_fire_resolves_all_hits_before_malleable_block() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FIEND_FIRE_PLUS_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), BASH_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Fiend Fire+ should play");

        assert_eq!(next.monsters[0].hp, 70);
        assert_eq!(next.monsters[0].block, 12);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn fiend_fire_with_dead_branch_spends_energy_and_refills_hand() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 80;
        state.monsters[0].max_hp = 80;
        state.player.energy = 3;
        state.relics = vec![Relic::DeadBranch];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), PERFECTED_STRIKE_ID),
            CardInstance::new(CardId::new(2), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(3), WARCRY_ID),
            CardInstance::new(CardId::new(4), DUAL_WIELD_PLUS_ID),
            CardInstance::new(CardId::new(5), SENTINEL_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: Some(target),
            },
        )
        .expect("Fiend Fire + Dead Branch");

        // Cost 2 then Sentinel on-exhaust +2 energy → net 3 (FIDL584b41).
        assert_eq!(next.player.energy, 3, "FF spend 2, Sentinel exhaust +2");
        // 4 other-card exhausts + Fiend Fire itself on MoveCard each proc Dead Branch.
        assert_eq!(
            next.piles.hand.len(),
            5,
            "Dead Branch per exhaust including FF"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|c| c.id == CardId::new(2)),
            "played Fiend Fire instance exhausted"
        );
        assert!(
            next.piles.hand.iter().all(|c| c.id != CardId::new(2)),
            "played Fiend Fire instance left hand"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|c| c.content_id == SENTINEL_ID),
            "Sentinel must be exhausted for energy refund"
        );
    }

    #[test]
    fn fiend_fire_dead_branch_still_generates_after_awakened_one_half_death() {
        // Dead Branch MakeTempCardInHand is addToBot from UseCardAction after
        // Fiend Fire's hits. Awakened One first-form death is not isBattleEnding,
        // so the source exhaust still generates (FIDL01750 Anger).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&AWAKENED_ONE_A0, target)];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 240;
        state.player.energy = 2;
        state.player.powers.strength = 0;
        state.relics = vec![Relic::DeadBranch];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), FIEND_FIRE_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: Some(target),
            },
        )
        .expect("Fiend Fire kills first-form Awakened One");

        assert!(
            awakened_one_is_half_dead(&next.monsters[0]),
            "hits must half-kill Awakened One so the source exhaust sees that window"
        );
        assert_eq!(
            next.piles.hand.len(),
            2,
            "Dead Branch for the snapshot Strike and for Fiend Fire itself, hand={:?}",
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn double_tap_reckless_charge_replays_damage_and_dazed() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].max_hp = 50;
        state.player.energy = 1;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), RECKLESS_CHARGE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Reckless Charge should play");
        // 7 + 7 = 14; each RC adds 1 Dazed → 2 with Double Tap.
        assert_eq!(
            next.monsters[0].hp, 36,
            "double tap should deal damage twice"
        );
        assert_eq!(next.double_tap_pending, 0);
        let dazed_all = next
            .piles
            .draw_pile
            .iter()
            .chain(next.piles.discard_pile.iter())
            .filter(|c| c.content_id == DAZED_ID)
            .count();
        assert_eq!(dazed_all, 2, "each RC adds 1 dazed; DT replays it");
    }

    #[test]
    fn double_tap_body_slam_copy_reads_rage_block() {
        // Rage onUseCard is addToBot after Body Slam.use() (0 block) and before
        // the Double Tap copy, so the copy deals the Rage block (FIDL01618).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].max_hp = 50;
        state.player.energy = 1;
        state.player.block = 0;
        state.player.temp_rage_block = 5;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BODY_SLAM_ID)];
        state.piles.draw_pile.clear();
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Double Tap Body Slam");
        assert_eq!(next.monsters[0].hp, 45, "copy deals Rage block, original 0");
        assert_eq!(
            next.player.block, 10,
            "Rage grants block on original and copy"
        );
        assert_eq!(next.double_tap_pending, 0);
    }

    #[test]
    fn double_tap_anger_hand_drill_applies_vulnerable_before_copy() {
        // The original 7-damage Anger consumes 3 block and deals 4 HP. Hand
        // Drill then applies Vulnerable before the card queue uses the copy,
        // so its 7 damage rounds to 10: 142 - 4 - 10 = 128 (FIDL01945).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters[0].id = target;
        state.monsters[0].hp = 142;
        state.monsters[0].max_hp = 142;
        state.monsters[0].block = 3;
        state.player.energy = 1;
        state.player.powers.strength = 1;
        state.relics.push(Relic::HandDrill);
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Double Tap Anger resolves through Hand Drill");

        assert_eq!(next.monsters[0].hp, 128);
        assert_eq!(next.monsters[0].block, 0);
        assert_eq!(next.monsters[0].powers.vulnerable, 2);
    }

    #[test]
    fn double_tap_anger_still_applies_rage_after_killing_the_target() {
        // Lethal original Anger skips the Double Tap copy, but Rage/Juggernaut
        // belong to the original onUseCard and must still resolve (FIDL01768).
        let target = MonsterId::new(1);
        let other = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&JAW_WORM_A0, target),
            monster_state(&JAW_WORM_A0, other),
        ];
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 40;
        state.monsters[1].hp = 40;
        state.monsters[1].max_hp = 40;
        state.monsters[1].block = 12;
        state.player.energy = 0;
        state.player.block = 0;
        state.player.powers.strength = 20;
        state.player.powers.juggernaut = 5;
        state.player.temp_rage_block = 3;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), ANGER_ID)];
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Anger");
        assert!(!next.monsters[0].alive);
        assert_eq!(next.player.block, 3);
        assert_eq!(next.monsters[1].block, 7);
    }

    #[test]
    fn time_warp_twelfth_card_skips_double_tap_copy() {
        // Time Warp's onAfterUseCard endTurnEarly clears the card queue, so a
        // Double Tap copy queued by the 12th card never uses (FIDL01433 Strike+).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.double_tap_pending = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Strike as Time Warp 12th card");

        assert_eq!(
            next.monsters[0].hp, 194,
            "Time Warp must skip the Double Tap copy hit"
        );
        assert_eq!(next.monsters[0].powers.time_warp, 0);
        assert_eq!(next.double_tap_pending, 0);
    }

    #[test]
    fn pen_nib_doubles_tenth_single_attack() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].max_hp = 50;
        state.player.energy = 1;
        state.relics.push(Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 9;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Strike");
        // base 6 doubled via pen nib formula with str 0 → 12
        assert_eq!(next.monsters[0].hp, 38);
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
        assert!(!next.pen_nib_double_active);
    }

    #[test]
    fn double_tap_pen_nib_on_second_hit_when_counter_starts_at_eight() {
        // attacks_played=8: first hit is 9th (no nib), second is 10th (nib).
        // FIDL00421: 7 + 14 = 21, counter ends at 0.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].max_hp = 50;
        state.player.energy = 1;
        state.double_tap_pending = 1;
        state.relics.push(Relic::PenNib);
        state.relic_counters.pen_nib_attacks_played = 8;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), RECKLESS_CHARGE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Reckless Charge should play");
        assert_eq!(next.monsters[0].hp, 29, "7 + pen-nib 14 = 21");
        assert_eq!(next.relic_counters.pen_nib_attacks_played, 0);
        assert_eq!(next.double_tap_pending, 0);
        assert!(!next.pen_nib_double_active);
    }

    #[test]
    fn dead_target_necronomicon_fiend_fire_copy_skips_card_play_triggers() {
        let target = MonsterId::new(1);
        let mut first = monster_state(&JAW_WORM_A0, target);
        first.hp = 1;
        first.max_hp = 1;
        let second = monster_state(&JAW_WORM_A0, MonsterId::new(2));
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![first, second];
        state.player.energy = 3;
        state.relics = vec![Relic::Necronomicon, Relic::ArtOfWar];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("dead-target Fiend Fire copy is discarded without use");

        assert!(!next.monsters[0].alive);
        assert!(next.monsters[1].alive);
        assert_eq!(next.relic_counters.cards_played_this_turn, 1);
        assert_eq!(next.relic_counters.attacks_played_this_turn, 1);
    }

    #[test]
    fn double_tap_fiend_fire_second_copy_hits_zero_on_empty_hand() {
        // FIDL00237: Double Tap replays Fiend Fire after the hand is already
        // exhausted; the copy must not queue phantom hits.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 43;
        state.monsters[0].max_hp = 43;
        state.monsters[0].block = 0;
        state.player.energy = 5;
        state.player.powers.strength = 4;
        state.double_tap_pending = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FIEND_FIRE_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), BASH_ID),
        ];
        state.piles.draw_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Double Tap Fiend Fire resolves");

        // Base 7 + 4 str = 11 × 3 exhausts = 33 → 10 HP remaining; no 2nd-copy hits.
        assert_eq!(next.monsters[0].hp, 10);
        assert!(next.monsters[0].alive);
        assert_eq!(next.double_tap_pending, 0);
    }

    #[test]
    fn havoc_played_fiend_fire_exhausts_hand_without_unknown_card() {
        // FIDL00369 step 343: Havoc top-draws Fiend Fire with a multi-card hand.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        // Mirror FIDL00369 pre-PLAY hand: two Havocs + Headbutt + Defend + Rampage,
        // Fiend Fire on top of draw (CardId 15 is the unknown id in the fail).
        state.piles.hand = vec![
            CardInstance::new(CardId::new(10), HAVOC_ID),
            CardInstance::new(CardId::new(11), HAVOC_ID),
            CardInstance::new(CardId::new(12), HEADBUTT_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
            CardInstance::new(CardId::new(14), RAMPAGE_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(15), FIEND_FIRE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Havoc → Fiend Fire should resolve without UnknownCard");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == FIEND_FIRE_ID),
            "Fiend Fire force-exhausted by Havoc"
        );
        assert!(
            next.piles.hand.is_empty(),
            "Fiend Fire exhausts the rest of the hand: {:?}",
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn havoc_played_forethought_defers_source_until_confirm() {
        // FIDL01437 / FIDL01593: Havoc top-plays Forethought. Exhausting the
        // source when the select opens drops the instance and CONFIRM fails
        // with UnknownCard.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(10), HAVOC_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), DEFEND_R_ID),
            CardInstance::new(CardId::new(13), BLOODLETTING_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(36), FORETHOUGHT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Havoc → Forethought should open a hand select");
        assert!(
            opened
                .hand_select()
                .is_some_and(|select| select.purpose == HandSelectPurpose::ForethoughtPutOnDraw),
            "expected Forethought hand select, got {:?}",
            opened.decision
        );
        assert!(
            opened
                .piles
                .hand
                .iter()
                .chain(opened.piles.limbo.iter())
                .any(|card| card.id == CardId::new(36)),
            "Forethought must remain staged until CONFIRM"
        );
        assert!(
            opened
                .piles
                .exhaust_pile
                .iter()
                .all(|card| card.id != CardId::new(36)),
            "Forethought must not exhaust when the select opens"
        );

        let mut chosen = opened;
        choose_hand_select(&mut chosen, 2).expect("choose Bloodletting");
        confirm_hand_select(&mut chosen).expect("CONFIRM must find the staged Forethought");
        let confirmed = chosen;
        assert!(
            confirmed
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == FORETHOUGHT_ID),
            "Havoc force-exhausts Forethought on CONFIRM"
        );
        assert_eq!(
            confirmed
                .piles
                .draw_pile
                .first()
                .map(|card| card.content_id),
            Some(BLOODLETTING_ID),
            "selected card goes to the bottom of draw"
        );
        assert!(confirmed
            .piles
            .hand
            .iter()
            .all(|card| card.content_id != BLOODLETTING_ID));
    }

    #[test]
    fn havoc_played_warcry_defers_source_until_confirm() {
        // FIDL01410: Havoc top-plays Warcry. The source must stay staged so
        // CONFIRM can put the selected card on draw and then exhaust Warcry
        // (Dead Branch may then generate a replacement).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(10), HAVOC_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), DEFEND_R_ID),
            CardInstance::new(CardId::new(13), BASH_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(36), WARCRY_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Havoc → Warcry should open a hand select");
        assert!(
            opened
                .hand_select()
                .is_some_and(|select| select.purpose == HandSelectPurpose::WarcryPutOnDraw),
            "expected Warcry hand select, got {:?}",
            opened.decision
        );
        assert!(
            opened
                .piles
                .hand
                .iter()
                .chain(opened.piles.limbo.iter())
                .any(|card| card.id == CardId::new(36)),
            "Warcry must remain staged until CONFIRM"
        );

        let mut chosen = opened;
        choose_hand_select(&mut chosen, 0).expect("choose first selectable");
        confirm_hand_select(&mut chosen).expect("CONFIRM must find the staged Warcry");
        assert!(
            chosen
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == WARCRY_ID),
            "Havoc force-exhausts Warcry on CONFIRM"
        );
    }

    #[test]
    fn havoc_played_burning_pact_skipped_retrieval_reenters_discard_on_end() {
        // FIDL01509: Havoc force-plays Burning Pact+. ExhaustAction can skip
        // retrieving the selected card; it stays out of every pile until the
        // next non-empty END flushes leftover selectedCards into discard.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(10), HAVOC_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), BASH_ID),
            CardInstance::new(CardId::new(3), BURNING_PACT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Havoc → Burning Pact should open exhaust select");
        assert!(
            opened.exhaust_select().is_some_and(|select| {
                matches!(
                    select.purpose,
                    crate::combat::ExhaustSelectPurpose::BurningPactDraw2
                        | crate::combat::ExhaustSelectPurpose::BurningPactDraw3
                ) && select.source_card_force_exhaust
                    && select
                        .source_card
                        .as_ref()
                        .is_some_and(|card| card.content_id == BURNING_PACT_ID)
            }),
            "expected parked force-exhaust Burning Pact select, got {:?}",
            opened.decision
        );
        assert!(!opened
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == BURNING_PACT_ID));

        let mut chosen = opened;
        choose_exhaust_select(&mut chosen, 0).expect("select Defend");
        let selected_id = chosen.piles.hand[0].id;
        let hidden = confirm_burning_pact_select_skipped_retrieval(&mut chosen)
            .expect("skipped Burning Pact retrieval");
        assert_eq!(hidden.id, selected_id);
        assert!(chosen
            .piles
            .exhaust_pile
            .iter()
            .all(|card| card.id != selected_id));
        assert!(chosen
            .piles
            .hand
            .iter()
            .chain(chosen.piles.discard_pile.iter())
            .all(|card| card.id != selected_id));
        chosen.pending_hidden_hand_card_until_end_turn = vec![hidden];
        assert!(
            !chosen.piles.hand.is_empty(),
            "hand must stay non-empty so END can flush leftover select: {:?}",
            chosen
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );

        let ended = apply_combat_action(&chosen, CombatAction::EndTurn).expect("END");
        let visible = ended
            .piles
            .hand
            .iter()
            .chain(ended.piles.draw_pile.iter())
            .chain(ended.piles.discard_pile.iter())
            .any(|card| card.id == selected_id);
        assert!(
            visible && ended.pending_hidden_hand_card_until_end_turn.is_empty(),
            "skipped Burning Pact selection must re-enter piles on END; hand={:?} draw={:?} discard={:?}",
            ended
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            ended
                .piles
                .draw_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            ended
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn sword_boomerang_resolves_all_random_hits_before_malleable_block() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 8;
        state.monsters[0].max_hp = 8;
        state.monsters[0].block = 3;
        state.monsters[0].powers.malleable = 4;
        state.monsters[0].powers.malleable_base = 3;
        state.monsters[0].powers.vulnerable = 2;
        state.player.energy = 1;
        state.player.powers.weak = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SWORD_BOOMERANG_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Sword Boomerang should play");

        assert_eq!(next.monsters[0].hp, 2);
        assert_eq!(next.monsters[0].block, 9);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn whirlwind_blocked_hits_only_trigger_malleable_after_hp_loss() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 56;
        state.monsters[0].block = 15;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 4;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), WHIRLWIND_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Whirlwind should play");

        assert_eq!(next.monsters[0].hp, 51);
        assert_eq!(next.monsters[0].block, 3);
        assert_eq!(next.monsters[0].powers.malleable, 4);
    }

    #[test]
    fn whirlwind_resolves_all_hits_before_malleable_block() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 56;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), WHIRLWIND_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Whirlwind should play");

        assert_eq!(next.monsters[0].hp, 41);
        assert_eq!(next.monsters[0].block, 12);
        assert_eq!(next.monsters[0].powers.malleable, 6);
    }

    #[test]
    fn top_draw_double_tap_plus_grants_two_pending_attack_replays() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), DOUBLE_TAP_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, None).expect("top Double Tap+ plays");

        assert_eq!(state.double_tap_pending, 2);
        assert!(state.piles.draw_pile.is_empty());
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].content_id, DOUBLE_TAP_PLUS_ID);
        assert!(state.piles.exhaust_pile.is_empty());
    }

    #[test]
    fn top_draw_perfected_strike_does_not_count_the_limbo_card() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), TWIN_STRIKE_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), PERFECTED_STRIKE_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, Some(target))
            .expect("top Perfected Strike plays");

        assert_eq!(state.monsters[0].hp, 86);
    }

    #[test]
    fn havoc_played_double_tap_plus_replays_the_following_attack() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), BASH_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), DOUBLE_TAP_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let after_havoc = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ plays Double Tap+");
        assert_eq!(after_havoc.double_tap_pending, 2);

        let after_bash = apply_combat_action(
            &after_havoc,
            CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: Some(target),
            },
        )
        .expect("Double Tap+ replays Bash");

        assert_eq!(after_bash.double_tap_pending, 1);
        assert_eq!(after_bash.monsters[0].hp, 80);
        assert_eq!(after_bash.monsters[0].powers.vulnerable, 4);
    }

    #[test]
    fn havoc_force_pain_with_blue_candle_loses_hp() {
        // FIDL00419: Havoc force-plays Pain; Blue Candle exhausts curse for 1 HP.
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.player.hp = 50;
        state.player.max_hp = 50;
        state.player.powers.rupture = 1;
        state.relics.push(Relic::BlueCandle);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), PAIN_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ → Pain resolves");

        assert_eq!(next.player.hp, 49, "Blue Candle HP loss on forced Pain");
        assert_eq!(
            next.player.powers.strength, 1,
            "Blue Candle's card-caused HP loss triggers Rupture"
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == PAIN_ID));
    }

    #[test]
    fn corruption_havoc_dead_branch_before_forced_card_rng() {
        // FIDL00441: Corruption Havoc → Sword Boomerang. Target RNG burns in
        // Havoc.use, Havoc self-exhaust DB runs before SB hit RNG, then SB DB.
        use crate::content::cards::{
            ANGER_ID, BODY_SLAM_ID, DUAL_WIELD_ID, EXHUME_ID, HAVOC_ID, POWER_THROUGH_ID,
            SWORD_BOOMERANG_ID, TRUE_GRIT_ID, TRUE_GRIT_PLUS_ID,
        };
        use crate::content::monsters::monster_state_for_ascension;
        use crate::relic::Relic;
        let seed = 34_961_238_620_666i64.wrapping_add(41);
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::DeadBranch];
        state.player.powers.corruption = 1;
        state.player.energy = 3;
        let mut ms = Vec::new();
        for i in 0..3u64 {
            let mut m = monster_state_for_ascension(
                &crate::content::monsters::JAW_WORM_A0,
                MonsterId::new(i + 1),
                0,
            );
            if i != 1 {
                m.hp = 0;
                m.alive = false;
            } else {
                m.hp = 37;
            }
            ms.push(m);
        }
        state.monsters = ms;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), TRUE_GRIT_PLUS_ID),
            CardInstance::new(CardId::new(2), EXHUME_ID),
            CardInstance::new(CardId::new(3), HAVOC_ID),
            CardInstance::new(CardId::new(4), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(5), TRUE_GRIT_ID),
            CardInstance::new(CardId::new(6), HAVOC_ID),
            CardInstance::new(CardId::new(7), ANGER_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), BODY_SLAM_ID),
            CardInstance::new(CardId::new(11), SWORD_BOOMERANG_ID),
        ];
        state.rng.card_random_rng = crate::rng::StsRng::with_counter(seed, 0);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(3),
                target: None,
            },
        )
        .expect("corruption havoc");

        let exhaust: Vec<_> = next
            .piles
            .exhaust_pile
            .iter()
            .map(|c| c.content_id)
            .collect();
        assert_eq!(
            exhaust,
            vec![HAVOC_ID, SWORD_BOOMERANG_ID],
            "Havoc self-exhaust before SB force-exhaust"
        );
        // Two Dead Branch cards after the remaining 6 hand cards.
        assert_eq!(next.piles.hand.len(), 8);
        assert_eq!(next.rng.card_random_rng.counter(), 6); // T + 3 SB hits + 2 DB
        let _ = (DUAL_WIELD_ID, POWER_THROUGH_ID);
    }

    #[test]
    fn dual_havoc_doubt_dead_branch_probe() {
        use crate::content::cards::{
            DOUBT_ID, EVOLVE_PLUS_ID, FIEND_FIRE_ID, HAVOC_PLUS_ID, IRON_WAVE_PLUS_ID, STRIKE_R_ID,
            SWORD_BOOMERANG_ID, SWORD_BOOMERANG_PLUS_ID,
        };
        use crate::content::monsters::monster_state_for_ascension;
        use crate::relic::Relic;
        // FIDL00394 floor 44: dual Havoc top-play → nested Havoc exhausts after its
        // PlayTop resolves Doubt; Dead Branch order is T,T,DB,DB → Fiend Fire, SB.
        let seed = 34_961_238_619_619i64.wrapping_add(44);
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::DeadBranch, Relic::LetterOpener];
        state.monsters = (0..4)
            .map(|i| {
                let mut m = monster_state_for_ascension(
                    &crate::content::monsters::SPIKER_A0,
                    MonsterId::new(i + 1),
                    0,
                );
                m.hp = 40;
                m
            })
            .collect();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), EVOLVE_PLUS_ID),
            CardInstance::new(CardId::new(2), IRON_WAVE_PLUS_ID),
            CardInstance::new(CardId::new(3), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(4), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(5), SWORD_BOOMERANG_PLUS_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DOUBT_ID),
            CardInstance::new(CardId::new(12), HAVOC_PLUS_ID),
        ];
        state.rng.card_random_rng = crate::rng::StsRng::with_counter(seed, 0);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(3),
                target: None,
            },
        )
        .expect("play havoc");

        let exhaust: Vec<_> = next
            .piles
            .exhaust_pile
            .iter()
            .map(|c| c.content_id)
            .collect();
        // The parent UseCardAction settles each forced Havoc before its queued
        // top card is serviced (source checkpoint: Havoc, then Doubt).
        assert_eq!(exhaust, vec![HAVOC_PLUS_ID, DOUBT_ID], "exhaust order");
        let db: Vec<_> = next
            .piles
            .hand
            .iter()
            .skip(4)
            .map(|c| c.content_id)
            .collect();
        assert_eq!(
            db,
            vec![FIEND_FIRE_ID, SWORD_BOOMERANG_ID],
            "dead branch picks"
        );
        assert_eq!(next.rng.card_random_rng.counter(), 4);
    }

    #[test]
    fn nested_havoc_strike_dead_branch_lands_before_inner_card_branch() {
        // FIDL01582 PLAY 1645: Havoc+ PlayTops Havoc, which PlayTops Strike.
        // Both force-exhaust. Dead Branch on the action queue must add the
        // nested Havoc card before Strike's. This seed's first two branch
        // rolls are True Grit then Fiend Fire.
        use crate::content::cards::{FIEND_FIRE_ID, HAVOC_PLUS_ID, STRIKE_R_ID, TRUE_GRIT_ID};
        use crate::relic::Relic;

        let mut state = CombatState::cultist_fixture();
        state.relics = vec![Relic::DeadBranch];
        state.player.energy = 1;
        state.monsters[0].hp = 50;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), HAVOC_PLUS_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.rng.card_random_rng = crate::rng::StsRng::with_counter(34_961_238_664_907, 0);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("nested Havoc PlayTop");

        let exhaust: Vec<_> = next
            .piles
            .exhaust_pile
            .iter()
            .map(|card| card.content_id)
            .collect();
        assert_eq!(exhaust, vec![HAVOC_PLUS_ID, STRIKE_R_ID], "exhaust order");
        let db: Vec<_> = next.piles.hand.iter().map(|card| card.content_id).collect();
        assert_eq!(
            db,
            vec![TRUE_GRIT_ID, FIEND_FIRE_ID],
            "Dead Branch from nested Havoc then Strike"
        );
    }

    #[test]
    fn havoc_force_sever_soul_does_not_exhaust_havoc_source() {
        // FIDL00418: empty-draw Havoc PlayTop-first; Sever Soul must not exhaust
        // Havoc still sitting in hand (STS already moved Havoc to cardInUse).
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.monsters[0].hp = 50;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), FLEX_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(4), SEVER_SOUL_ID)];
        state.piles.exhaust_pile.clear();
        state.piles.limbo.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc → Sever Soul resolves");

        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "Havoc settles to discard"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == FLEX_ID),
            "Sever Soul still exhausts other skills"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID),
            "attacks remain in hand"
        );
        assert!(next.piles.limbo.is_empty());
    }

    #[test]
    fn secret_technique_retrieves_skill_into_full_hand_when_source_in_limbo() {
        // FIDL00413: with 9 other hand cards, Secret Technique must leave hand
        // (limbo) so the chosen skill can fill the 10th slot instead of discard.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.rng.card_random_rng = crate::rng::StsRng::new(0);
        state.piles.hand = (0..9)
            .map(|i| CardInstance::new(CardId::new(100 + i), STRIKE_R_ID))
            .chain(std::iter::once(CardInstance::new(
                CardId::new(200),
                SECRET_TECHNIQUE_ID,
            )))
            .collect();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(201), DAZED_ID),
            CardInstance::new(CardId::new(202), DEFEND_R_ID),
            CardInstance::new(CardId::new(203), STRIKE_R_ID),
            CardInstance::new(CardId::new(204), TRUE_GRIT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.piles.limbo.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(200),
                target: None,
            },
        )
        .expect("Secret Technique opens select");
        assert!(next.draw_select().is_some());
        assert_eq!(next.piles.hand.len(), 9);
        assert!(next
            .piles
            .limbo
            .iter()
            .any(|card| card.content_id == SECRET_TECHNIQUE_ID));

        choose_draw_select(&mut next, 0).expect("choose Defend");
        confirm_draw_select(&mut next).expect("confirm Secret Technique");

        assert_eq!(next.piles.hand.len(), 10);
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| { card.content_id == DEFEND_R_ID || card.content_id == TRUE_GRIT_ID }),
            "chosen skill must land in hand"
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == SECRET_TECHNIQUE_ID));
        assert!(next.piles.limbo.is_empty());
    }

    #[test]
    fn havoc_exhausts_top_card_without_effect_when_normality_blocks_fourth_play() {
        // GameActionManager autoplay still settles UseCardAction after Normality
        // rejects canUse, so Havoc's exhaustOnUseOnce card is exhausted.
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.player.powers.strength = 2;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;
        state.relic_counters.cards_played_this_turn = 2;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), NORMALITY_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), STRIKE_R_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves");

        assert_eq!(
            next.monsters[0].hp, 200,
            "Normality must suppress forced Strike"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID),
            "Normality-rejected autoplay still exhausts"
        );
        assert!(!next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == STRIKE_R_ID));
        assert_eq!(next.relic_counters.cards_played_this_turn, 3);
    }

    #[test]
    fn havoc_exhausts_unplayable_top_card_without_fabricating_an_effect() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DOUBT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves an unplayable top card");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DOUBT_ID);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn havoc_exhausts_clash_when_the_hand_contains_a_skill() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters[0].id = target;
        state.monsters[0].hp = 100;
        state.monsters[0].max_hp = 100;
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), CLASH_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should resolve an unplayable Clash");

        assert_eq!(next.monsters[0].hp, 100);
        assert_eq!(next.piles.exhaust_pile[0].content_id, CLASH_ID);
        assert_eq!(next.piles.discard_pile[0].content_id, HAVOC_PLUS_ID);
    }

    #[test]
    fn havoc_exhausts_attack_without_effect_while_entangled() {
        // Entangled makes AbstractCard.canUse reject Attacks. Havoc still
        // force-exhausts the top attack (exhaustOnUseOnce) without resolving
        // damage or apply-power effects — see GameActionManager autoplay fail
        // path + UseCardAction(dontTriggerOnUseCard).
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters[0].id = target;
        state.monsters[0].hp = 40;
        state.monsters[0].max_hp = 40;
        state.monsters[0].powers.vulnerable = 0;
        state.player.energy = 1;
        state.player.powers.entangled = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), BASH_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc resolves while Entangled");

        assert_eq!(
            next.monsters[0].hp, 40,
            "Entangled blocks Bash damage through Havoc"
        );
        assert_eq!(
            next.monsters[0].powers.vulnerable, 0,
            "Entangled blocks Bash Vulnerable through Havoc"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "Bash is still force-exhausted by Havoc"
        );
        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "Havoc itself still settles to discard"
        );
    }

    #[test]
    fn havoc_exhausts_slimed_after_its_explicit_no_effect_play() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), SLIMED_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves Slimed's explicit no-effect play");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SLIMED_ID);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn top_draw_bash_applies_vulnerable_before_followup_hand_strike() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(4), STRIKE_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), BASH_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Strike");
        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Strike");
        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("top Bash");

        assert_eq!(state.monsters[0].hp, 20);
        assert_eq!(state.monsters[0].powers.vulnerable, 2);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(4),
                target: Some(target),
            },
        )
        .expect("hand Strike should play into Vulnerable");

        assert_eq!(next.monsters[0].hp, 11);
    }

    #[test]
    fn top_draw_anger_copy_is_not_purged_when_mayhem_plays_it_again() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 100;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), ANGER_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        apply_play_top_draw_card_to_state(&mut state, Some(target)).expect("play original Anger");
        assert_eq!(state.piles.discard_pile.len(), 2);
        assert!(state
            .piles
            .discard_pile
            .iter()
            .all(|card| !card.combat_only));

        let generated_copy = state
            .piles
            .discard_pile
            .pop()
            .expect("generated Anger copy");
        state.piles.discard_pile.clear();
        state.piles.draw_pile.push(generated_copy);

        apply_play_top_draw_card_to_state(&mut state, Some(target))
            .expect("Mayhem-style play of generated Anger copy");

        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![ANGER_ID, ANGER_ID]
        );
    }

    #[test]
    fn feed_kill_completing_darkling_pack_awards_max_hp() {
        let target = MonsterId::new(3);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&DARKLING_A0, MonsterId::new(1)),
            monster_state(&DARKLING_A0, MonsterId::new(2)),
            monster_state(&DARKLING_A0, target),
        ];
        for monster in &mut state.monsters {
            monster.rolled_attack_damage = Some(8);
            monster.intent = crate::MonsterIntent::Attack { damage: 8 };
        }
        for monster in &mut state.monsters[..2] {
            monster.alive = false;
            monster.escaped = true;
            monster.hp = 0;
        }
        state.monsters[2].hp = 1;
        state.monsters[2].max_hp = 1;
        state.player.hp = 81;
        state.player.max_hp = 114;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEED_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Feed should finish the Darkling pack");

        assert_eq!(next.player.max_hp, 117);
        assert_eq!(next.player.hp, 84);
    }

    #[test]
    fn havoc_played_feed_deals_damage_before_exhausting_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].block = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), FEED_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Feed");

        assert_eq!(next.monsters[0].hp, 20);
        assert_eq!(next.monsters[0].block, 30);
        assert_eq!(next.player.energy, 3);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, FEED_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, HAVOC_PLUS_ID);
    }

    #[test]
    fn hand_of_greed_awards_gold_when_it_finishes_darkling_pack() {
        let target = MonsterId::new(3);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state(&DARKLING_A0, MonsterId::new(1)),
            monster_state(&DARKLING_A0, MonsterId::new(2)),
            monster_state(&DARKLING_A0, target),
        ];
        for monster in &mut state.monsters[..2] {
            monster.alive = false;
            monster.escaped = true;
            monster.hp = 0;
        }
        state.monsters[2].hp = 1;
        state.monsters[2].max_hp = 1;
        for monster in &mut state.monsters {
            monster.rolled_attack_damage = Some(8);
            monster.intent = crate::MonsterIntent::Attack { damage: 8 };
        }
        state.player.energy = 2;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAND_OF_GREED_PLUS_ID)];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Hand of Greed+ should finish the Darkling pack");

        assert_eq!(next.combat_gold_gained, 25);
    }

    #[test]
    fn havoc_played_rampage_deals_damage_and_scales_before_exhausting_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), RAMPAGE_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        let starting_hp = state.monsters[0].hp;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Rampage");

        assert_eq!(next.monsters[0].hp, starting_hp - 8);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, RAMPAGE_ID);
        assert_eq!(next.piles.exhaust_pile[0].rampage_damage_bonus, 5);
    }

    #[test]
    fn havoc_played_burning_pact_resolves_selection_after_source_exhaust() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), BURNING_PACT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Burning Pact queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Burning Pact opens exhaust selection")
            .state;
        assert!(next.exhaust_select().is_some());
        assert!(next
            .exhaust_select()
            .and_then(|select| select.source_card.as_ref())
            .is_some_and(|card| card.content_id == BURNING_PACT_ID));
        assert!(next.piles.exhaust_pile.is_empty());

        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_exhaust_select(&mut next).expect("resolve Burning Pact selection");

        assert!(next.exhaust_select().is_none());
        assert_eq!(next.piles.hand.len(), 3);
        // Top-draw BP exhausts the selection immediately (FIDL00221). Skipped-
        // retrieval limbo is a verifier rebuild, not the core default.
        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == BURNING_PACT_ID)
                .count(),
            1
        );
    }

    #[test]
    fn havoc_played_burning_pact_defers_charons_ashes_until_confirm() {
        use crate::relic::{Relic, CHARONS_ASHES_DAMAGE};

        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 160;
        state.monsters[0].max_hp = 160;
        state.player.energy = 1;
        state.relics = vec![Relic::CharonsAshes];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), BURNING_PACT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc force-plays Burning Pact");
        assert!(opened.exhaust_select().is_some());
        assert_eq!(
            opened.monsters[0].hp, 160,
            "Charon's Ashes must wait while Burning Pact is still in play"
        );

        let mut next = opened;
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_exhaust_select(&mut next).expect("confirm Burning Pact");
        assert_eq!(
            next.monsters[0].hp,
            160 - CHARONS_ASHES_DAMAGE * 2,
            "selected card and force-exhausted Burning Pact each proc Charon's Ashes"
        );
    }

    #[test]
    fn mayhem_played_burning_pact_keeps_source_until_selection_confirm() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), BURNING_PACT_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, false, false)
            .expect("ordinary top-draw Burning Pact queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Burning Pact opens exhaust selection")
            .state;
        assert!(next.exhaust_select().is_some());
        assert!(next
            .exhaust_select()
            .and_then(|select| select.source_card.as_ref())
            .is_some_and(|card| card.content_id == BURNING_PACT_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == BURNING_PACT_ID));

        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_exhaust_select(&mut next).expect("resolve Burning Pact selection");

        assert!(next.exhaust_select().is_none());
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == BURNING_PACT_ID));
    }

    #[test]
    fn havoc_played_exhume_defers_dark_embrace_until_after_selection() {
        // Havoc force-plays Exhume while Dark Embrace is active. Exhume must
        // not sit in exhaust (and must not DE-draw) until the exhaust select
        // returns a card — matching CM frames where hand gains only the
        // exhumed card on CHOOSE.
        let mut state = CombatState::initial_fixture();
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), CLEAVE_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), FLEX_ID),
            CardInstance::new(CardId::new(4), EXHUME_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile = vec![
            CardInstance::new(CardId::new(10), BASH_ID),
            CardInstance::new(CardId::new(11), WOUND_ID),
            CardInstance::new(CardId::new(12), ARMAMENTS_ID),
        ];

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Exhume queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Exhume opens exhaust selection")
            .state;
        assert!(next.exhaust_select().is_some());
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .all(|card| card.content_id != EXHUME_ID),
            "Exhume must not exhaust before selection closes"
        );
        assert_eq!(
            next.piles.hand.len(),
            3,
            "Dark Embrace must not draw before Exhume selection: {:?}",
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>()
        );

        // UI index 0 among non-Exhume exhaust cards → BASH.
        choose_exhaust_select(&mut next, 0).expect("select Bash from exhaust");
        confirm_exhaust_select(&mut next).expect("resolve Exhume selection");

        assert!(next.exhaust_select().is_none());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == EXHUME_ID));
        // Hand: original 3 + exhumed Bash + Dark Embrace draw.
        assert_eq!(next.piles.hand.len(), 5);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == BASH_ID));
    }

    #[test]
    fn havoc_played_armaments_upgrades_selected_card_into_hand() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), ARMAMENTS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Armaments queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Armaments opens hand selection")
            .state;
        assert!(next.hand_select().is_some());
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == ARMAMENTS_ID),
            "force-played Armaments stays in cardInUse until CONFIRM"
        );
        assert!(
            !next
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == ARMAMENTS_ID),
            "Charon's Ashes must not fire while the upgrade screen is open"
        );

        choose_hand_select(&mut next, 0).expect("select Strike");
        confirm_hand_select(&mut next).expect("resolve top-draw Armaments selection");

        assert!(next.hand_select().is_none());
        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
        // Selected Strike is upgraded and returned; Defend remains.
        assert_eq!(next.piles.hand.len(), 2);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == STRIKE_R_PLUS_ID));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn charons_ashes_breaking_block_applies_hand_drill_vulnerable() {
        // DamageAllEnemiesAction THORNS hits block; brokeBlock notifies Hand Drill.
        let mut state = CombatState::cultist_fixture();
        state.relics = vec![Relic::CharonsAshes, Relic::HandDrill];
        state.monsters[0].block = 3;
        let exhausted = CardInstance::new(CardId::new(50), STRIKE_R_ID);
        state.piles.exhaust_pile.push(exhausted);
        apply_on_exhaust_effects(&mut state, CardId::new(50)).expect("Ashes");
        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(
            state.monsters[0].powers.vulnerable,
            crate::relic::HAND_DRILL_VULNERABLE
        );
    }

    #[test]
    fn sever_soul_exhausts_necronomicurse_replacement_for_feel_no_pain() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        // Necronomicon's second use() snapshots the Soulbound replacement.
        // One ExhaustAllNonAttackAction snapshot would leave Feel No Pain at 6.
        state.relics = vec![Relic::Necronomicon];
        state.player.energy = 2;
        state.player.powers.feel_no_pain = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), SEVER_SOUL_PLUS_ID),
            CardInstance::new(CardId::new(2), WOUND_ID),
            CardInstance::new(CardId::new(3), BASH_PLUS_ID),
            CardInstance::new(CardId::new(4), NECRONOMICURSE_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].hp = 80;
        state.monsters[0].max_hp = 80;
        state.monsters[0].block = 0;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Sever Soul+ resolves");

        assert_eq!(next.player.block, 9);
        assert_eq!(
            next.piles
                .exhaust_pile
                .iter()
                .filter(|card| card.content_id == NECRONOMICURSE_ID)
                .count(),
            2
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == NECRONOMICURSE_ID)
                .count(),
            1
        );
    }

    #[test]
    fn havoc_lethal_strike_applies_burning_blood_once() {
        // Nested PlayTop process_internal_queue must not double BB heal when the
        // outer queue also settles combat Won (13efa069: 6 not 12).
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::BurningBlood];
        state.player.hp = 50;
        state.player.max_hp = 100;
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].hp = 6; // Strike base 6, no str
        state.monsters[0].max_hp = 50;
        state.monsters[0].block = 0;
        state.monsters[0].alive = true;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc plays Strike and kills");
        assert_eq!(next.phase, CombatPhase::Won);
        assert!(next.monsters.iter().all(|m| !m.alive));
        assert_eq!(
            next.player.hp,
            50 + crate::content::character::BURNING_BLOOD_HEAL_AMOUNT,
            "Burning Blood must apply once, not once per nested process_internal_queue"
        );
    }

    #[test]
    fn burning_pact_fire_breathing_kill_wins_combat_on_exhaust_confirm() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.fire_breathing = 6;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), WOUND_ID),
        ];
        state.monsters[0].hp = 2;
        state.monsters[0].max_hp = 20;
        state.monsters[0].block = 0;

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        let mut chosen = opened;
        choose_exhaust_select(&mut chosen, 0).expect("select Strike");
        confirm_exhaust_select(&mut chosen).expect("CONFIRM draws Wound and Fire Breathing kills");

        assert_eq!(chosen.phase, CombatPhase::Won);
        assert!(chosen.monsters.iter().all(|monster| !monster.alive));
    }

    #[test]
    fn scrape_discards_nonzero_cost_power_instead_of_empowering_it() {
        // ScrapeFollowUp.moveToDiscardPile is not UseCardAction.empower.
        // A drawn Power that costs more than 0 must remain in discard.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SCRAPE_ANY_COLOR_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), EVOLVE_ID)];
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Scrape draws and discards Evolve");

        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == EVOLVE_ID),
            "unplayed Power must stay in discard, discard={:?} hand={:?}",
            next.piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
        );
        assert!(
            !next
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == EVOLVE_ID),
            "nonzero-cost Evolve must not remain in hand",
        );
    }

    #[test]
    fn bane_plus_deals_ten_into_block() {
        // Bane.baseDamage is 7; upgradeDamage(3). A + card into 12 block must
        // leave 2, not 3 (FIDL02294 step 855).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        let mut bane = CardInstance::new(CardId::new(1), BANE_ANY_COLOR_ID);
        bane.upgrades = 1;
        state.piles.hand = vec![bane];
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state.monsters[0].block = 12;
        state.monsters[0].hp = 26;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Bane+ hits block");

        assert_eq!(
            next.monsters[0].block, 2,
            "Bane+ is 10 damage; leftover block={}",
            next.monsters[0].block
        );
        assert_eq!(next.monsters[0].hp, 26);
    }

    #[test]
    fn judgement_instant_kill_puts_darkling_in_count_pose() {
        // InstantKillAction goes through AbstractMonster.damage, so a Darkling
        // first death is COUNT / Life Link, not a full die() (FIDL02294).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), JUDGEMENT_ANY_COLOR_ID)];
        state.piles.discard_pile.clear();
        state.monsters = vec![
            monster_state(&DARKLING_A0, MonsterId::new(1)),
            monster_state(&DARKLING_A0, MonsterId::new(2)),
            monster_state(&DARKLING_A0, MonsterId::new(3)),
        ];
        for monster in &mut state.monsters {
            monster.rolled_attack_damage = Some(9);
            monster.intent = crate::MonsterIntent::Attack { damage: 9 };
        }
        state.monsters[1].hp = 26;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(2)),
            },
        )
        .expect("Judgement instant-kills the mid Darkling");

        assert!(!next.monsters[1].alive);
        assert!(
            next.monsters[1].escaped,
            "first death is Life Link half-dead"
        );
        assert_eq!(next.monsters[1].intent, crate::MonsterIntent::DarklingCount);
        assert_eq!(
            crate::content::monsters::target_move_byte_for_monster(&next.monsters[1]),
            Some(4)
        );
    }

    #[test]
    fn conclude_rerolls_writhing_mass_before_forced_end_turn() {
        // ReactivePower.onAttacked addToBots RollMoveAction during DamageAll.
        // PressEndTurnButton only flags the end; that roll must drain before
        // the enemy phase (FIDL02294 Conclude smash block).
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::WRITHING_MASS_A0,
            MonsterId::new(1),
        )];
        state.monsters[0].hp = 70;
        state.monsters[0].max_hp = 70;
        state.monsters[0].block = 0;
        state.monsters[0].intent = crate::MonsterIntent::AttackAndBlock {
            damage: 15,
            block: 20,
        };
        state.monsters[0].move_history = vec![2];
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), CONCLUDE_ANY_COLOR_ID)];
        state.piles.draw_pile = (10..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();

        let transition = apply_combat_action_with_events(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Conclude hits then ends the turn");

        let reroll = transition
            .event_log
            .iter()
            .position(|action| {
                matches!(action, InternalAction::RerollWrithingMassAfterAttack { .. })
            })
            .expect("Reactive reroll");
        let settle = transition
            .event_log
            .iter()
            .position(|action| matches!(action, InternalAction::SettleForcedEndTurn))
            .expect("forced end-turn settlement");
        assert!(
            reroll < settle,
            "RollMoveAction must drain before the enemy phase"
        );
        assert_ne!(
            transition.state.monsters[0].block, 20,
            "pre-hit smash block must not apply after Reactive reroll"
        );
    }

    #[test]
    fn conclude_horn_draw_settles_before_forced_end_turn() {
        // GremlinHorn addToBots Draw+Energy during DamageAll. Those must run
        // before the flagged end turn so the extra card is discarded and
        // energy is overwritten by the refill (FIDL02294).
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::GremlinHorn);
        state.player.energy = 1;
        state.player.max_energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), CONCLUDE_ANY_COLOR_ID)];
        state.piles.draw_pile = (10..=20)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        let mut minion = state.monsters[0].clone();
        minion.id = MonsterId::new(2);
        minion.hp = 1;
        minion.max_hp = 20;
        minion.powers.minion = 1;
        state.monsters[0].hp = 40;
        state.monsters[0].max_hp = 40;
        state.monsters.push(minion);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Conclude kills the minion and ends the turn");

        assert_eq!(
            next.player.energy, 3,
            "Horn energy must not survive the end-turn refill"
        );
        assert_eq!(
            next.piles.hand.len(),
            5,
            "Horn draw must be discarded with the rest of the old hand"
        );
    }

    #[test]
    fn fear_no_evil_returns_flurry_of_blows_from_discard_on_calm() {
        // ChangeStanceAction iterates discard and FlurryOfBlows queues
        // DiscardToHandAction (FIDL02294).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), FEAR_NO_EVIL_ANY_COLOR_ID)];
        state.piles.discard_pile = vec![CardInstance::new(
            CardId::new(2),
            FLURRY_OF_BLOWS_ANY_COLOR_ID,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackAndBlock {
            damage: 11,
            block: 5,
        };
        state.monsters[0].hp = 40;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Fear No Evil enters Calm");

        assert_eq!(next.player.powers.calm, 1);
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == FLURRY_OF_BLOWS_ANY_COLOR_ID),
            "Flurry of Blows must return from discard"
        );
        assert!(next
            .piles
            .discard_pile
            .iter()
            .all(|card| card.content_id != FLURRY_OF_BLOWS_ANY_COLOR_ID));
    }

    #[test]
    fn fear_no_evil_returns_flurry_when_source_still_fills_the_hand() {
        // useCard removes the source before ChangeStance / DiscardToHand, so a
        // 10-card hand still has a free slot for Flurry.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.draw_pile.clear();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), FEAR_NO_EVIL_ANY_COLOR_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), STRIKE_R_ID),
            CardInstance::new(CardId::new(8), DEFEND_R_ID),
            CardInstance::new(CardId::new(9), DEFEND_R_ID),
            CardInstance::new(CardId::new(10), DEFEND_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(
            CardId::new(2),
            FLURRY_OF_BLOWS_ANY_COLOR_ID,
        )];
        state.monsters[0].intent = crate::MonsterIntent::AttackAndBlock {
            damage: 11,
            block: 5,
        };
        state.monsters[0].hp = 40;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("Fear No Evil enters Calm from a full hand");

        assert_eq!(next.piles.hand.len(), 10);
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == FLURRY_OF_BLOWS_ANY_COLOR_ID),
            "source occupancy must not block Flurry"
        );
    }

    #[test]
    fn empty_body_from_neutral_does_not_return_flurry() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), EMPTY_BODY_ANY_COLOR_ID)];
        state.piles.discard_pile = vec![CardInstance::new(
            CardId::new(2),
            FLURRY_OF_BLOWS_ANY_COLOR_ID,
        )];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Empty Body from Neutral");

        assert_eq!(next.player.powers.calm, 0);
        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == FLURRY_OF_BLOWS_ANY_COLOR_ID),
            "same-stance Neutral must not retrieve Flurry"
        );
    }

    #[test]
    fn empty_body_from_calm_returns_flurry_and_grants_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.calm = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), EMPTY_BODY_ANY_COLOR_ID)];
        state.piles.discard_pile = vec![CardInstance::new(
            CardId::new(2),
            FLURRY_OF_BLOWS_ANY_COLOR_ID,
        )];

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Empty Body leaves Calm");

        assert_eq!(next.player.powers.calm, 0);
        assert_eq!(next.player.energy, 2);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == FLURRY_OF_BLOWS_ANY_COLOR_ID));
    }

    #[test]
    fn time_warp_burning_pact_lag_applies_metallicize_without_discard() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.metallicize = 4;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
        ];
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_exhaust_select_with_time_warp_policy(&mut next, false)
            .expect("CONFIRM without settling Time Warp end");
        apply_time_warp_lag_metallicize_keep_hand(&mut next).expect("Metallicize lag");

        assert_eq!(next.player.block, 4);
        assert!(
            !next.piles.hand.is_empty(),
            "Time Warp lag keeps the post-CONFIRM hand"
        );
        assert!(next.time_warp_end_powers_applied);
        assert_eq!(next.monsters[0].powers.time_warp, 0);
    }

    #[test]
    fn time_warp_burning_pact_settles_source_before_end_turn_discard() {
        // UseCardAction is queued before Time Warp's EndTurn. CONFIRM must put
        // Burning Pact in discard ahead of the leftover hand (FIDL02206).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = (4..=14)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.draw_pile[0] = CardInstance::new(CardId::new(4), UPPERCUT_ID);
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(20), WOUND_ID)];
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_exhaust_select(&mut next).expect("CONFIRM settles Time Warp");

        let discard_ids: Vec<_> = next
            .piles
            .discard_pile
            .iter()
            .map(|card| card.content_id)
            .collect();
        let pact_at = discard_ids
            .iter()
            .position(|id| *id == BURNING_PACT_ID)
            .unwrap_or_else(|| {
                panic!(
                    "Burning Pact discarded; discard={discard_ids:?} hand={:?} exhaust={:?}",
                    next.piles
                        .hand
                        .iter()
                        .map(|c| c.content_id)
                        .collect::<Vec<_>>(),
                    next.piles
                        .exhaust_pile
                        .iter()
                        .map(|c| c.content_id)
                        .collect::<Vec<_>>()
                )
            });
        let wound_at = discard_ids
            .iter()
            .position(|id| *id == WOUND_ID)
            .expect("pre-existing Wound remains");
        assert!(pact_at > wound_at);
        assert!(
            discard_ids
                .iter()
                .skip(pact_at + 1)
                .any(|id| *id == DEFEND_R_ID || *id == UPPERCUT_ID),
            "Time Warp must discard the leftover hand after Burning Pact, discard={discard_ids:?}"
        );
    }

    #[test]
    fn havoc_unplayable_playtop_still_rolls_strange_spoon() {
        // Havoc PlayTop of Dazed still constructs UseCardAction with
        // exhaustOnUseOnce; Spoon can send that Dazed to discard (FIDL02410).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.relics = vec![Relic::StrangeSpoon];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DAZED_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.rng.card_random_rng = crate::rng::StsRng::new(2);

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc force-plays Dazed");

        let dazed_discarded = next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == DAZED_ID);
        let dazed_exhausted = next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DAZED_ID);
        assert!(
            dazed_discarded != dazed_exhausted,
            "Spoon must decide Dazed destination, discard={:?} exhaust={:?}",
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            next.piles
                .exhaust_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn forced_empty_dual_wield_runs_hex_before_strange_spoon() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.hex = 1;
        state.relics = vec![Relic::StrangeSpoon];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), DEFEND_R_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), RUPTURE_ID),
            CardInstance::new(CardId::new(5), DUAL_WIELD_PLUS_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.rng.card_random_rng = crate::rng::StsRng::new(1);
        let transition = process_internal_queue(
            &state,
            VecDeque::from([InternalAction::PlayTopDrawCard {
                target: None,
                exhaust_played_card: true,
                random_living_target: false,
            }]),
        )
        .expect("force-play empty Dual Wield");
        let next = transition.state;

        assert_eq!(next.decision, None);
        assert_eq!(next.rng.card_random_rng.counter(), 2);
        assert!(next
            .piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == DAZED_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DUAL_WIELD_PLUS_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == DUAL_WIELD_PLUS_ID));
    }

    #[test]
    fn strange_spoon_waits_behind_hex_dazed_insert() {
        // Hex onUseCard addToBots MakeTempCardInDrawPile before UseCardAction
        // rolls Strange Spoon (FIDL02399).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.hex = 1;
        state.relics = vec![Relic::StrangeSpoon];
        state.piles.hand = vec![CardInstance::new(CardId::new(1), LIMIT_BREAK_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), RUPTURE_ID),
            CardInstance::new(CardId::new(3), THUNDERCLAP_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), BASH_ID),
        ];
        state.piles.discard_pile.clear();
        state.rng.card_random_rng = crate::rng::StsRng::new(1);

        let expected_index = {
            let mut rng = crate::rng::StsRng::new(1);
            rng.random_int(3) as usize
        };

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Limit Break under Hex");

        assert_eq!(next.piles.draw_pile[expected_index].content_id, DAZED_ID);
    }

    #[test]
    fn time_warp_ends_turn_after_warcry_select_confirm() {
        // Time Warp increments on Warcry PlayCard; end-turn is deferred while
        // hand select is open and must fire on CONFIRM (15ab4cc step 1625–1631).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), DEFEND_R_ID),
            CardInstance::new(CardId::new(8), BASH_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry opens select");
        assert!(next.hand_select().is_some());
        assert!(
            !next.time_warp_end_turn,
            "Time Warp card count is deferred while the select is open"
        );
        choose_hand_select(&mut next, 0).expect("select Strike");
        confirm_hand_select(&mut next).expect("confirm Warcry");

        assert!(next.hand_select().is_none());
        assert!(
            !next.time_warp_end_turn,
            "CONFIRM must consume deferred Time Warp end-turn"
        );
        // After forced end + monster + start player, hand should be refilled.
        assert!(
            next.piles.hand.len() >= 3,
            "forced end-turn must reach next player hand, got {:?}",
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(next.player.energy, 3);
    }

    #[test]
    fn time_warp_warcry_status_lag_autoplays_selected_burn_and_keeps_hand() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.hp = 40;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), BURN_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), CLEAVE_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry opens select");
        choose_hand_select(&mut next, 0).expect("select Burn");
        confirm_hand_select_time_warp_status_lag(&mut next).expect("status-lag CONFIRM");

        assert!(next.hand_select().is_none());
        assert_eq!(next.player.energy, 1, "Lantern / refill must wait");
        assert_eq!(next.player.hp, 38, "selected Burn autoplays for 2");
        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == BURN_ID),
            "Burn must land in discard after autoplay"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID),
            "non-status cards stay in hand"
        );
        assert_eq!(next.monsters[0].block, 0, "monster turn has not run");
        assert_eq!(next.monsters[0].powers.strength, 2);
        assert!(next.time_warp_end_turn);
    }

    #[test]
    fn time_warp_warcry_remaining_status_lag_put_on_deck_then_autoplays_leftover_regret() {
        // FIDL01425: Warcry as 12th card selects Pommel onto draw; leftover
        // Regret autoplays for remaining-hand size 2; Reaper stays held.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 2;
        state.player.hp = 40;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
            CardInstance::new(CardId::new(3), POMMEL_STRIKE_PLUS_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), BASH_ID),
            CardInstance::new(CardId::new(6), CLEAVE_ID),
            CardInstance::new(CardId::new(4), REAPER_ID),
        ];
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry opens select");
        choose_hand_select(&mut next, 1).expect("select Pommel");
        confirm_hand_select_time_warp_remaining_status_lag(&mut next)
            .expect("remaining-status-lag CONFIRM");

        assert!(next.hand_select().is_none());
        assert_eq!(next.player.energy, 2, "energy refill must wait");
        assert_eq!(
            next.player.hp, 38,
            "leftover Regret deals remaining hand size 2"
        );
        assert_eq!(
            next.piles.draw_pile.last().map(|card| card.content_id),
            Some(POMMEL_STRIKE_PLUS_ID),
            "selected Pommel is put on draw"
        );
        assert!(
            next.piles
                .discard_pile
                .iter()
                .any(|card| card.content_id == REGRET_ID),
            "leftover Regret lands in discard after autoplay"
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![REAPER_ID],
            "non-status leftover stays in hand"
        );
        assert_eq!(next.monsters[0].block, 0, "monster turn has not run");
        assert!(next.time_warp_end_turn);
        assert!(
            next.time_warp_duplicate_monster_queue,
            "explicit END after lagged CONFIRM must run two MonsterQueueItems"
        );
    }

    #[test]
    fn explicit_end_after_deferred_time_warp_runs_duplicate_monster_queue() {
        // FIDL01601 END 1773: Burning Pact CONFIRM parks Time Warp, then END
        // is a second EndTurnAction. Reverberate 7+2 through Vulnerable hits
        // twice (39+39) instead of once.
        let mut state = CombatState::initial_fixture();
        state.time_warp_end_turn = true;
        state.player.hp = 4341;
        state.player.max_hp = 4341;
        state.player.powers.vulnerable = 1;
        state.piles.hand.clear();
        state.piles.draw_pile = (1..=10)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].intent = crate::MonsterIntent::AttackMultiple { damage: 7, hits: 3 };
        state.monsters[0].powers.strength = 2;
        state.monsters[0].hp = 400;
        state.monsters[0].max_hp = 400;

        let next = apply_combat_action(&state, CombatAction::EndTurn).expect("duplicate END");

        assert_eq!(next.player.hp, 4263);
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn time_warp_lethal_twelfth_card_still_autoplays_burn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 70;
        state.player.max_hp = 70;
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), BURN_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 1;
        state.monsters[0].max_hp = 456;
        state.monsters[0].block = 0;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(state.monsters[0].id),
            },
        )
        .expect("lethal Strike as 12th card");

        assert!(!next.monsters[0].alive);
        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 68);
        assert_eq!(next.monsters[0].powers.time_warp, 0);
    }

    #[test]
    fn time_warp_cancels_havoc_play_top_after_source_settles() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), HAVOC_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(3), WOUND_ID),
            CardInstance::new(CardId::new(4), WOUND_ID),
            CardInstance::new(CardId::new(5), PARASITE_ID),
            CardInstance::new(CardId::new(6), HAVOC_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(7), STRIKE_R_ID)];
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect(
            "Havoc+ as the 12th card force-plays top Havoc, then Time Warp cancels nested PlayTop",
        );

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "the first PlayTop still force-exhausts top Havoc"
        );
        assert!(
            !next
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == WOUND_ID || card.content_id == PARASITE_ID),
            "Time Warp must cancel the nested leftover PlayTop"
        );
        assert!(next.hand_select().is_none());
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn time_warp_twelfth_play_top_havoc_still_extracts_leftover_top() {
        // Hand Havoc is the 11th card. Its PlayTop force-plays draw-pile Havoc
        // as the 12th. Nested Havoc.use() queues leftover PlayTop before
        // onAfterUseCard arms Time Warp, so that leftover card is extracted
        // and force-exhausted without use() (FIDL00021 Wound / Evolve).
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), WOUND_ID),
            CardInstance::new(CardId::new(4), HAVOC_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 10;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("11th-card Havoc PlayTops Havoc as the 12th card");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "the first PlayTop still force-exhausts top Havoc"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == WOUND_ID),
            "leftover PlayTop queued by the 12th-card Havoc still extracts Wound"
        );
        assert!(next.hand_select().is_none());
        assert!(!next.time_warp_end_turn);
        assert_eq!(next.monsters[0].powers.time_warp, 0);
    }

    #[test]
    fn time_warp_cancels_havoc_true_grit_use_after_limbo_handoff() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc as the 12th card parks True Grit, then Time Warp cancels use");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == TRUE_GRIT_ID),
            "leftover True Grit still force-exhausts"
        );
        assert!(next.exhaust_select().is_none());
        assert!(!next.time_warp_end_turn);
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == STRIKE_R_ID));
    }

    #[test]
    fn time_warp_warcry_under_corruption_exhausts_source_before_forced_end() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.corruption = 1;
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), WARCRY_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), CLEAVE_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), DEFEND_R_ID),
            CardInstance::new(CardId::new(8), BASH_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 11;
        state.monsters[0].hp = 200;
        state.monsters[0].max_hp = 200;

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Warcry opens select");
        choose_hand_select(&mut next, 0).expect("select Strike");
        confirm_hand_select(&mut next).expect("confirm Warcry under Corruption");

        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == WARCRY_ID),
            "UseCardAction must exhaust Warcry before Time Warp ends the turn"
        );
        assert!(next.hand_select().is_none());
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn havoc_played_armaments_skipped_retrieval_parks_unupgraded_and_flushes_hex() {
        // wereCardsRetrieved=false: selected card stays off every pile until
        // end-turn discard; deferred Hex still inserts after the select closes.
        let mut state = CombatState::initial_fixture();
        state.rng.card_random_rng = StsRng::new(7);
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), ARMAMENTS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Armaments queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Armaments opens hand selection")
            .state;
        assert!(next.hand_select().is_some());
        // Deferred Hex behind the select (same interleaving as enemy Hex on the
        // Havoc-played Armaments card under CommunicationMod).
        match next.decision.as_mut() {
            Some(crate::combat::CombatDecisionState::HandSelect {
                pending_actions, ..
            }) => {
                pending_actions.push_back(InternalAction::AddGeneratedCardToDrawPileRandomSpot {
                    content_id: DAZED_ID,
                });
            }
            _ => panic!("expected hand select decision"),
        }
        choose_hand_select(&mut next, 0).expect("select Strike");
        confirm_hand_select_skipped_armaments_retrieval(&mut next)
            .expect("skipped Armaments retrieval");

        assert!(next.hand_select().is_none());
        assert_eq!(
            next.pending_hidden_hand_card_until_end_turn
                .first()
                .map(|card| card.content_id),
            Some(STRIKE_R_ID),
            "selected card parks unupgraded"
        );
        assert!(
            !next
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == STRIKE_R_ID || card.content_id == STRIKE_R_PLUS_ID),
            "selected card must not return upgraded to hand"
        );
        assert_eq!(
            next.piles
                .draw_pile
                .iter()
                .filter(|card| card.content_id == DAZED_ID)
                .count(),
            1,
            "deferred Hex still inserts Dazed after skipped retrieval"
        );
        // Unselected upgradeable Defend remains in hand.
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
    }

    #[test]
    fn havoc_played_armaments_skipped_retrieval_drops_non_upgradeable_hand_cards() {
        // FIDL00400: already-upgraded Strikes leave combat with skipped
        // retrieval; only the selected unupgraded card parks for END flush.
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), CLEAVE_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_PLUS_ID),
            CardInstance::new(CardId::new(3), THUNDERCLAP_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_PLUS_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(5), ARMAMENTS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Armaments queue");
        let mut next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Armaments opens hand selection")
            .state;
        // Select Cleave (first upgradeable UI slot).
        choose_hand_select(&mut next, 0).expect("select Cleave");
        confirm_hand_select_skipped_armaments_retrieval(&mut next)
            .expect("skipped Armaments retrieval");

        assert_eq!(
            next.pending_hidden_hand_card_until_end_turn
                .first()
                .map(|card| card.content_id),
            Some(CLEAVE_ID)
        );
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![THUNDERCLAP_ID],
            "non-upgradeable Strikes drop; unselected Thunderclap remains"
        );
        assert!(
            !next
                .piles
                .hand
                .iter()
                .chain(next.piles.discard_pile.iter())
                .chain(next.piles.exhaust_pile.iter())
                .chain(next.piles.draw_pile.iter())
                .any(|card| card.content_id == STRIKE_R_PLUS_ID),
            "upgraded Strikes must not re-enter any combat pile"
        );
    }

    #[test]
    fn burning_pact_crossing_deck_boundary_exhausts_selected_card_before_draw() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), STRIKE_R_ID)];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(4), DEFEND_R_ID)];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact should open its exhaust-select screen");

        choose_exhaust_select(&mut next, 0).expect("Shrug It Off is selectable");
        confirm_exhaust_select(&mut next).expect("Burning Pact selection should resolve");

        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == SHRUG_IT_OFF_ID));
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == BURNING_PACT_ID)
                .count(),
            1
        );
    }

    #[test]
    fn burning_pact_small_hand_exhausts_selected_card() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), BASH_PLUS_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DEFEND_R_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
        ];
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(7), STRIKE_R_ID)];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact should open its exhaust-select screen");

        choose_exhaust_select(&mut next, 0).expect("Bash should be selectable");
        confirm_exhaust_select(&mut next).expect("Burning Pact selection should resolve");

        assert!(next.pending_hidden_hand_card_until_end_turn.is_empty());
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == BASH_PLUS_ID));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == BURNING_PACT_ID));
    }

    #[test]
    fn burning_pact_dark_embrace_draws_after_source_discard_reshuffle() {
        // DE onExhaust is addToBot after ExhaustAction. With empty draw after
        // Burning Pact's own draws, DE must reshuffle a discard that already
        // contains Burning Pact — so BP can re-enter hand (9bf020 step 459).
        let mut state = CombatState::initial_fixture();
        state.player.powers.dark_embrace = 2;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BURNING_PACT_ID),
            CardInstance::new(CardId::new(2), PERFECTED_STRIKE_ID),
            CardInstance::new(CardId::new(3), BURN_ID),
            CardInstance::new(CardId::new(4), BURN_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
        ];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(20), DEFEND_R_ID),
            CardInstance::new(CardId::new(21), FLEX_ID),
            CardInstance::new(CardId::new(22), CLASH_ID),
        ];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Burning Pact opens exhaust select");

        // UI index 1 among non-source hand cards: Perfected Strike(0), Burn(1), Burn(2).
        choose_exhaust_select(&mut next, 1).expect("select Burn");
        confirm_exhaust_select(&mut next).expect("resolve Burning Pact");

        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == BURN_ID));
        // BP drew the two Strikes first; DE then reshuffles discard (incl. BP)
        // and draws two more. Burning Pact must be able to sit in hand again.
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == BURNING_PACT_ID)
                || next
                    .piles
                    .draw_pile
                    .iter()
                    .any(|card| card.content_id == BURNING_PACT_ID),
            "Burning Pact must be discarded before Dark Embrace reshuffles: hand={:?} draw={:?} discard={:?}",
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            next.piles
                .draw_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            next.piles
                .discard_pile
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
        );
        // Immediate-DE-first would leave BP stuck only in discard with empty draw
        // after DE stole the two Strikes — assert we drew 4 cards into hand total
        // beyond the leftover Burn and Perfected Strike (2 BP + 2 DE).
        assert!(
            next.piles.hand.len() >= 5,
            "BP draw 2 + DE draw 2 should fill hand: {:?}",
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn havoc_played_dual_wield_resolves_without_duplicating_source() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DUAL_WIELD_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut staged = state;
        let queue = apply_play_top_draw_card(&mut staged, None, true, false)
            .expect("top-draw Dual Wield queue");
        let next = process_internal_queue(&staged, queue.into())
            .expect("top-draw Dual Wield with a single eligible attack auto-confirms")
            .state;

        assert!(
            next.hand_select().is_none(),
            "single eligible Dual Wield target is auto-selected"
        );
        assert_eq!(next.piles.hand.len(), 2);
        assert!(next
            .piles
            .hand
            .iter()
            .all(|card| card.content_id == STRIKE_R_ID));
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DUAL_WIELD_ID);
    }

    #[test]
    fn havoc_exhausts_dual_wield_without_targets_without_rejecting_play() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(3), DUAL_WIELD_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 6;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ resolves Dual Wield+ with no attack/power targets");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, DUAL_WIELD_PLUS_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.decision, None);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
        assert_eq!(next.monsters[0].powers.time_warp, 8);
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn havoc_play_top_defend_increments_time_warp_twice() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), DEFEND_R_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 9;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc PlayTops Defend");

        assert_eq!(
            next.monsters[0].powers.time_warp, 11,
            "Havoc and the forced Defend each increment Time Warp"
        );
        assert_eq!(next.piles.exhaust_pile[0].content_id, DEFEND_R_ID);
        assert!(!next.time_warp_end_turn);
    }

    #[test]
    fn havoc_play_top_defend_with_dark_embrace_increments_time_warp_twice() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HEAVY_BLADE_PLUS_ID),
            CardInstance::new(CardId::new(2), SEVER_SOUL_ID),
            CardInstance::new(CardId::new(3), THUNDERCLAP_ID),
            CardInstance::new(CardId::new(4), SWORD_BOOMERANG_ID),
            CardInstance::new(CardId::new(5), HAVOC_ID),
            CardInstance::new(CardId::new(6), ANGER_ID),
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), SEARING_BLOW_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
        state.monsters[0].powers.time_warp = 9;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(5),
                target: None,
            },
        )
        .expect("Havoc PlayTops Defend with Dark Embrace");

        assert_eq!(
            next.monsters[0].powers.time_warp, 11,
            "Havoc and forced Defend still increment Time Warp under Dark Embrace"
        );
    }

    #[test]
    fn havoc_play_top_exhume_increments_time_warp_on_choose() {
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 1;
            combat.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
            combat.piles.draw_pile = vec![CardInstance::new(CardId::new(2), EXHUME_ID)];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile = vec![
                CardInstance::new(CardId::new(10), STRIKE_R_ID),
                CardInstance::new(CardId::new(11), HEAVY_BLADE_PLUS_ID),
            ];
            combat.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
            combat.monsters[0].powers.time_warp = 9;
        }

        let after_havoc = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc PlayTops Exhume");
        assert_eq!(
            after_havoc.combat.as_ref().unwrap().monsters[0]
                .powers
                .time_warp,
            10,
            "Havoc increments Time Warp before Exhume select"
        );
        assert!(after_havoc
            .combat
            .as_ref()
            .unwrap()
            .exhaust_select()
            .is_some());

        let after_choose =
            apply_exhaust_select_choice(&after_havoc, 1).expect("Exhume chooses Heavy Blade+");
        assert_eq!(
            after_choose.combat.as_ref().unwrap().monsters[0]
                .powers
                .time_warp,
            11,
            "Exhume UseCardAction increments Time Warp when the select closes"
        );
    }

    #[test]
    fn skipped_exhume_retrieval_still_increments_time_warp() {
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat");
            combat.player.energy = 1;
            combat.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
            combat.piles.draw_pile = vec![CardInstance::new(CardId::new(2), EXHUME_ID)];
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile = vec![
                CardInstance::new(CardId::new(10), STRIKE_R_ID),
                CardInstance::new(CardId::new(11), HEAVY_BLADE_PLUS_ID),
            ];
            combat.monsters[0].content_id = crate::content::monsters::TIME_EATER_ID;
            combat.monsters[0].powers.time_warp = 9;
        }

        let after_havoc = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc PlayTops Exhume");
        let after_skip = apply_exhaust_select_choice(&after_havoc, 1).expect("Exhume return");

        let combat = after_skip.combat.as_ref().expect("combat");
        assert_eq!(
            combat.monsters[0].powers.time_warp, 11,
            "UseCardAction still increments Time Warp"
        );
        assert!(
            combat
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == HEAVY_BLADE_PLUS_ID),
            "selected exhaust card returns to hand"
        );
        assert!(combat
            .piles
            .exhaust_pile
            .iter()
            .all(|card| card.content_id != HEAVY_BLADE_PLUS_ID));
    }

    #[test]
    fn havoc_played_shrug_it_off_plus_draws_after_gaining_block() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), SHRUG_IT_OFF_PLUS_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Shrug It Off+");

        assert_eq!(next.player.block, 11);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SHRUG_IT_OFF_PLUS_ID);
    }

    #[test]
    fn havoc_played_headbutt_opens_discard_choice_without_moving_it_from_exhaust() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), HEADBUTT_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(3), POWER_THROUGH_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should play top-deck Headbutt");

        assert_eq!(next.monsters[0].hp, 31);
        assert_eq!(next.player.energy, 2);
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .all(|card| card.content_id != HEADBUTT_ID),
            "force-played Headbutt stays out of exhaust while discard select is open"
        );
        assert_eq!(
            next.discard_select().unwrap().source_card_id,
            Some(CardId::new(2))
        );
        assert_eq!(
            next.discard_select()
                .unwrap()
                .source_card
                .map(|card| card.content_id),
            Some(HEADBUTT_ID)
        );

        choose_discard_select(&mut next, 0).expect("select Power Through");
        confirm_headbutt_select(&mut next).expect("confirm forced Headbutt selection");

        // Havoc exhausts Headbutt, but Headbutt's PutOnDeck still returns the
        // chosen discard card to the top of the draw pile.
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, POWER_THROUGH_ID);
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == POWER_THROUGH_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
    }

    #[test]
    fn havoc_played_headbutt_skipped_retrieval_leaves_discard_and_exhausts_source() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), HEADBUTT_ID)];
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(3), POWER_THROUGH_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
        ];
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should play top-deck Headbutt");

        assert!(next.discard_select().is_some());
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(next.piles.discard_pile.len(), 3);

        choose_discard_select(&mut next, 1).expect("select Strike");
        confirm_headbutt_select_skipped_retrieval(&mut next).expect("skipped Headbutt retrieval");

        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
        assert!(next.piles.draw_pile.is_empty());
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![POWER_THROUGH_ID, STRIKE_R_ID, HAVOC_ID]
        );
        assert_eq!(next.decision, None);
    }

    #[test]
    fn leftover_force_exhaust_marker_does_not_exhaust_later_hand_headbutt() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.play_top_force_exhaust_active = true;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HEADBUTT_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("hand Headbutt should ignore a leftover force-exhaust marker");

        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == HEADBUTT_ID));
        assert!(!next.play_top_force_exhaust_active);
        assert_eq!(next.piles.draw_pile[0].content_id, STRIKE_R_ID);
    }

    #[test]
    fn havoc_places_source_in_discard_before_headbutt_can_return_it_to_draw() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), HEADBUTT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc should expose itself to top-deck Headbutt");

        assert_eq!(next.piles.discard_pile.len(), 0);
        assert_eq!(next.piles.draw_pile.len(), 1);
        assert_eq!(next.piles.draw_pile[0].content_id, HAVOC_ID);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, HEADBUTT_ID);
    }

    #[test]
    fn havoc_under_corruption_plays_top_card_before_dark_embrace_from_source_exhaust() {
        // Corruption exhausts Havoc after its effect. Dark Embrace must not
        // draw the forced top card (Bash) before that play resolves.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.monsters[0].hp = 40;
        state.player.energy = 3;
        state.player.powers.corruption = 1;
        state.player.powers.dark_embrace = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(2), BATTLE_TRANCE_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ under Corruption plays top Bash");

        assert_eq!(
            next.monsters[0].powers.vulnerable, 2,
            "Bash must resolve as the forced top card"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "forced Bash is exhausted by Havoc"
        );
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == HAVOC_PLUS_ID),
            "Corruption exhausts Havoc after the forced play"
        );
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == BATTLE_TRANCE_ID),
            "Dark Embrace draws the card under Bash after Bash is exhausted, not Bash itself"
        );
        assert!(
            !next
                .piles
                .hand
                .iter()
                .any(|card| card.content_id == BASH_ID),
            "Bash must not be drawn by premature Dark Embrace"
        );
    }

    #[test]
    fn havoc_dual_wield_multi_select_defers_exhaust_until_confirm_with_dark_embrace() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        state.player.powers.dark_embrace = 1;
        let mut generated_wound = CardInstance::new(CardId::new(6), WOUND_ID);
        generated_wound.combat_only = true;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), HEMOKINESIS_ID),
            CardInstance::new(CardId::new(3), BASH_ID),
            CardInstance::new(CardId::new(4), JUGGERNAUT_PLUS_ID),
            CardInstance::new(CardId::new(5), SHAME_ID),
            generated_wound,
        ];
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), BITE_ID),
            CardInstance::new(CardId::new(11), DUAL_WIELD_ID),
        ];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc opens Dual Wield select");

        assert!(next.hand_select().is_some());
        assert_eq!(
            next.hand_select().map(|s| s.dual_wield_force_exhaust),
            Some(true),
            "Havoc PlayTop marks Dual Wield force-exhaust"
        );
        assert!(
            !next
                .piles
                .exhaust_pile
                .iter()
                .any(|c| c.content_id == DUAL_WIELD_ID),
            "Dual Wield must not exhaust before CONFIRM"
        );
        assert!(
            !next.piles.hand.iter().any(|c| c.content_id == SHAME_ID),
            "non-eligible Shame leaves the hand when Dual Wield select opens"
        );
        assert_eq!(next.piles.hand.len(), 3);
        assert!(
            !next.piles.hand.iter().any(|c| c.content_id == WOUND_ID),
            "generated Wound is held outside the filtered hand until CONFIRM"
        );

        choose_hand_select(&mut next, 1).expect("select Bash");
        confirm_dual_wield_select_skipped_retrieval(&mut next)
            .expect("skipped Dual Wield retrieval");

        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|c| c.content_id == DUAL_WIELD_ID));
        assert!(
            next.piles.hand.iter().any(|c| c.content_id == BITE_ID),
            "DE draws"
        );
        assert!(
            !next.piles.hand.iter().any(|c| c.content_id == BASH_ID),
            "selected Bash stays hidden"
        );
        assert!(
            next.piles.hand.iter().any(|c| c.content_id == WOUND_ID),
            "generated combat-only Wound is restored after CONFIRM"
        );
        assert_eq!(next.pending_hidden_hand_card_until_end_turn.len(), 1);
    }

    #[test]
    fn havoc_dual_wield_skipped_retrieval_can_leave_dropped_cards_off_piles() {
        // FIDL01816: SuperFastMode skip never returns Rage (skill) or Wound.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        let mut generated_wound = CardInstance::new(CardId::new(6), WOUND_ID);
        generated_wound.combat_only = true;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_ID),
            CardInstance::new(CardId::new(2), SEARING_BLOW_ID),
            CardInstance::new(CardId::new(3), CLASH_ID),
            CardInstance::new(CardId::new(4), RAGE_ID),
            generated_wound,
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(11), DUAL_WIELD_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc opens Dual Wield select");
        choose_hand_select(&mut next, 0).expect("select Searing Blow");
        confirm_dual_wield_select_skipped_retrieval_without_restore(&mut next)
            .expect("skipped Dual Wield retrieval without restore");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![CLASH_ID],
            "unselected attack remains; dropped skill/status stay off piles"
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DUAL_WIELD_ID));
        assert_eq!(next.pending_hidden_hand_card_until_end_turn.len(), 1);
        assert_eq!(
            next.pending_hidden_hand_card_until_end_turn[0].content_id,
            SEARING_BLOW_ID
        );
    }

    #[test]
    fn dual_wield_plus_restores_combat_only_dazed_before_skills() {
        // FIDL01246: Deca Dazed stays combat-only after shuffle, so Dual Wield
        // restores it with Shockwave/Shrug instead of dropping it.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;
        let mut dazed = CardInstance::new(CardId::new(3), DAZED_ID);
        dazed.combat_only = true;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DUAL_WIELD_PLUS_ID),
            CardInstance::new(CardId::new(2), HEAVY_BLADE_PLUS_ID),
            dazed,
            CardInstance::new(CardId::new(4), WILD_STRIKE_PLUS_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), SHOCKWAVE_ID),
            CardInstance::new(CardId::new(7), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(8), SHRUG_IT_OFF_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Dual Wield+ opens select");
        choose_hand_select(&mut next, 2).expect("select Strike");
        confirm_hand_select(&mut next).expect("confirm Dual Wield+");

        assert_eq!(
            next.piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![
                HEAVY_BLADE_PLUS_ID,
                WILD_STRIKE_PLUS_ID,
                DAZED_ID,
                SHOCKWAVE_ID,
                SHRUG_IT_OFF_ID,
                SHRUG_IT_OFF_ID,
                STRIKE_R_ID,
                STRIKE_R_ID,
                STRIKE_R_ID,
            ]
        );
        assert!(
            next.piles
                .hand
                .iter()
                .any(|card| card.content_id == DAZED_ID && card.combat_only),
            "restored Dazed stays combat-only"
        );
    }

    #[test]
    fn havoc_letter_opener_resolves_before_play_top_malleable_block() {
        // FIDL00428: Havoc → Bash under Letter Opener. Bash damage, then LO 5,
        // then Malleable block (3) so HP 33 block 3 — not LO after Malleable.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::WRITHING_MASS_A0,
            target,
        )];
        state.monsters[0].hp = 46;
        state.monsters[0].max_hp = 46;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 1;
        state.relics.push(Relic::LetterOpener);
        state.relic_counters.letter_opener_skills_this_turn = 2; // next skill trips
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), BASH_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc plays Bash");
        assert_eq!(next.monsters[0].hp, 33, "Bash 8 + LO 5");
        assert_eq!(next.monsters[0].block, 3, "Malleable after LO");
        assert_eq!(next.monsters[0].powers.malleable, 4);
    }

    #[test]
    fn writhing_mass_compulsive_reroll_waits_for_headbutt_discard_grid() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::WRITHING_MASS_A0,
            target,
        )];
        state.monsters[0].hp = 70;
        state.monsters[0].max_hp = 70;
        state.monsters[0].intent = crate::MonsterIntent::AttackAndBlock {
            damage: 15,
            block: 15,
        };
        state.monsters[0].move_history = vec![2];
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HEADBUTT_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
        ];

        let opened = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Headbutt opens discard select");
        assert!(opened.discard_select().is_some());
        assert_eq!(
            opened.monsters[0].intent,
            crate::MonsterIntent::AttackAndBlock {
                damage: 15,
                block: 15,
            },
            "RollMoveAction stays behind PutOnDeck's GRID"
        );

        let mut chosen = opened;
        crate::combat::transition::choose_discard_select(&mut chosen, 0)
            .expect("choose discard card");
        crate::combat::transition::confirm_headbutt_select(&mut chosen)
            .expect("close Headbutt GRID");
        assert_ne!(
            chosen.monsters[0].intent,
            crate::MonsterIntent::AttackAndBlock {
                damage: 15,
                block: 15,
            },
            "Compulsive rerolls after the GRID closes"
        );
    }

    #[test]
    fn bash_malleable_block_applies_before_sadistic_nature_damage() {
        // FIDL00242: Bash damage queues Malleable addToBot block before Sadistic
        // Nature's addToBot damage from ApplyVulnerable, so Sadistic eats block.
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::SNAKE_PLANT_A0,
            target,
        )];
        state.monsters[0].hp = 27;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.player.energy = 2;
        state.player.powers.sadistic_nature = 5;
        state.player.powers.weak = 0;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BASH_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(target),
            },
        )
        .expect("Bash resolves");

        assert_eq!(next.monsters[0].powers.vulnerable, 2);
        assert_eq!(
            next.monsters[0].block, 0,
            "Sadistic must consume Malleable block"
        );
        // Bash 8 then Malleable 3 block then Sadistic 5 => 2 hp after block => 17.
        assert_eq!(next.monsters[0].hp, 17);
    }

    #[test]
    fn havoc_play_top_feel_no_pain_applies_after_source_exhaust_under_corruption() {
        // FIDL00253: PlayTop Feel No Pain+ queues ApplyPower addToBot behind the
        // outer Havoc UseCardAction, so Corruption exhausts Havoc before FNP is
        // active (no block from that exhaust). Dead Branch still rolls.
        let mut state = CombatState::cultist_fixture();
        state.player.energy = 1;
        state.player.powers.corruption = 1;
        state.relics.push(crate::relic::Relic::DeadBranch);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), FEEL_NO_PAIN_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        // Deterministic Dead Branch: reserve by running once is RNG-based; just
        // assert FNP timing and exhaust destination.

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc → Feel No Pain+ under Corruption");

        assert_eq!(
            next.player.block, 0,
            "FNP must not block for the Havoc exhaust that played it"
        );
        assert_eq!(next.player.powers.feel_no_pain, 4, "FNP+ still applies");
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == HAVOC_ID),
            "Corruption exhausts Havoc"
        );
        assert!(
            !next.piles.hand.is_empty(),
            "Dead Branch adds a generated card after Havoc exhausts"
        );
    }

    #[test]
    fn havoc_played_true_grit_exhausts_a_random_hand_card() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck True Grit");

        assert_eq!(next.player.block, 7);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn havoc_played_true_grit_plus_selects_without_moving_it_from_exhaust() {
        // FIDL00253: force-played True Grit+ stays out of exhaust until CONFIRM,
        // then exhausts the selection and the source together.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck True Grit+");

        assert_eq!(next.player.block, 9);
        assert!(next.exhaust_select().is_some());
        assert!(
            !next
                .piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == TRUE_GRIT_PLUS_ID),
            "True Grit+ must not exhaust before CONFIRM"
        );

        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_exhaust_select(&mut next).expect("confirm forced True Grit+ selection");

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(
            next.piles
                .exhaust_pile
                .iter()
                .any(|card| card.content_id == DEFEND_R_ID),
            "force-play True Grit+ selection exhausts on CONFIRM"
        );
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_PLUS_ID));
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_PLUS_ID));
    }

    #[test]
    fn havoc_played_true_grit_plus_skipped_retrieval_hides_selection() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.feel_no_pain = 4;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), HAVOC_PLUS_ID),
            CardInstance::new(CardId::new(2), DEFEND_R_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
        ];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(4), TRUE_GRIT_PLUS_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ plays True Grit+");
        assert_eq!(next.player.block, 9);
        choose_exhaust_select(&mut next, 0).expect("select Defend");
        confirm_true_grit_select_skipped_retrieval(&mut next).expect("skipped retrieval");

        assert_eq!(next.player.block, 13, "only True Grit exhaust procs FNP");
        assert!(next
            .pending_hidden_hand_card_until_end_turn
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == TRUE_GRIT_PLUS_ID));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn purity_skipped_retrieval_exhausts_only_source_and_hides_selection() {
        // FIDL00405: ExhaustAction can finish before CONFIRM retrieves
        // selectedCards. Only Purity exhausts; selection re-enters on END.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.player.powers.feel_no_pain = 3;
        state.player.block = 8;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), PURITY_ID),
            CardInstance::new(CardId::new(2), BASH_ID),
            CardInstance::new(CardId::new(3), STRIKE_R_ID),
            CardInstance::new(CardId::new(4), DEFEND_R_ID),
        ];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let mut next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Purity opens exhaust select");
        choose_exhaust_select(&mut next, 0).expect("select Bash");
        choose_exhaust_select(&mut next, 0).expect("select Strike");
        confirm_purity_select_skipped_retrieval(&mut next).expect("skipped retrieval");

        assert_eq!(next.player.block, 11, "only Purity exhaust procs FNP");
        assert_eq!(next.pending_hidden_hand_card_until_end_turn.len(), 2);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == PURITY_ID));
        assert!(!next
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.content_id == BASH_ID || card.content_id == STRIKE_R_ID));
        assert!(next
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn havoc_played_whirlwind_uses_current_energy_for_x_without_spending_it() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SNAKE_PLANT_A0, target)];
        state.monsters[0].hp = 50;
        state.monsters[0].block = 0;
        state.monsters[0].powers.malleable = 3;
        state.monsters[0].powers.malleable_base = 3;
        state.player.energy = 4;
        state.player.powers.weak = 2;
        state.relics = vec![Relic::DeadBranch];
        state.rng.card_random_rng = StsRng::with_counter(22_079_335_132, 1);
        state.piles.hand = vec![CardInstance::new(CardId::new(1), HAVOC_PLUS_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), WHIRLWIND_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Havoc+ should play top-deck Whirlwind");

        assert_eq!(next.player.energy, 4);
        assert_eq!(next.monsters[0].hp, 38);
        assert_eq!(next.monsters[0].block, 18);
        assert_eq!(next.monsters[0].powers.malleable, 7);
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, WHIRLWIND_ID);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].content_id, HAVOC_PLUS_ID);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DUAL_WIELD_ID);
    }

    #[test]
    fn infernal_blade_exhausts_source_after_adding_generated_attack() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), INFERNAL_BLADE_ID)];
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Infernal Blade should play");

        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, INFERNAL_BLADE_ID);
        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.player.energy, 2);
    }

    #[test]
    fn corruption_exhausts_delayed_limbo_skills() {
        let target = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, target)];
        state.player.energy = 3;
        state.player.powers.corruption = 1;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), SHRUG_IT_OFF_ID)];
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(2), STRIKE_R_ID)];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("Corruption should exhaust Shrug It Off after its draw");

        assert!(next.piles.discard_pile.is_empty());
        assert_eq!(next.piles.exhaust_pile.len(), 1);
        assert_eq!(next.piles.exhaust_pile[0].content_id, SHRUG_IT_OFF_ID);
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
    }

    #[test]
    fn generated_blood_for_blood_copies_current_combat_damage_events() {
        let mut state = CombatState::initial_fixture();
        state.player.damage_events_this_combat = 3;
        state.piles.hand.clear();

        add_generated_card_to_pile(
            &mut state,
            BLOOD_FOR_BLOOD_ID,
            CardPile::Hand,
            Some(0),
            true,
        )
        .expect("generated Blood for Blood should enter hand");

        assert_eq!(state.piles.hand.len(), 1);
        assert_eq!(
            state.piles.hand[0].blood_for_blood_cost_reduction,
            state.player.damage_events_this_combat
        );
    }

    #[test]
    fn infernal_blade_uses_source_groups_in_rarity_order() {
        let pool = card_effects::infernal_blade_modeled_attack_pool();

        assert_eq!(pool[17], SEARING_BLOW_ID);
        assert_eq!(pool[21], PUMMEL_ID);
        assert_eq!(pool[25], BLUDGEON_ID);
        assert_eq!(pool[26], FIEND_FIRE_ID);
        assert_eq!(pool[27], IMMOLATE_ID);
    }

    #[test]
    fn exhaust_multi_select_indexes_skip_hidden_selected_cards() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), STRIKE_R_ID),
            CardInstance::new(CardId::new(2), WOUND_ID),
            CardInstance::new(CardId::new(3), BLOOD_FOR_BLOOD_ID),
            CardInstance::new(CardId::new(4), STRIKE_R_ID),
            CardInstance::new(CardId::new(5), DAZED_ID),
            CardInstance::new(CardId::new(6), DAZED_ID),
            CardInstance::new(CardId::new(7), ANGER_PLUS_ID),
            CardInstance::new(CardId::new(8), WOUND_ID),
        ];

        open_exhaust_select(&mut state).expect("open exhaust select");
        choose_exhaust_select(&mut state, 0).expect("select first visible card");
        choose_exhaust_select(&mut state, 4).expect("select second visible Dazed");
        choose_exhaust_select(&mut state, 3).expect("select remaining visible Dazed");

        assert_eq!(
            state
                .exhaust_select()
                .expect("exhaust select")
                .selected_hand_indices,
            vec![0, 5, 4]
        );

        confirm_exhaust_select(&mut state).expect("confirm exhaust select");

        assert!(state.pending_elixir_exhaust_card_ids.is_empty());
        assert_eq!(state.pending_elixir_exhaust_turns_remaining, 0);
        assert_eq!(
            state
                .piles
                .exhaust_pile
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![CardId::new(1), CardId::new(6), CardId::new(5)]
        );

        let hand_ids = state
            .piles
            .hand
            .iter()
            .map(|card| card.content_id)
            .collect::<Vec<_>>();
        assert_eq!(
            hand_ids,
            vec![
                WOUND_ID,
                BLOOD_FOR_BLOOD_ID,
                STRIKE_R_ID,
                ANGER_PLUS_ID,
                WOUND_ID
            ]
        );
    }

    #[test]
    fn blasphemy_spends_energy_enters_divinity_and_marks_death() {
        use crate::content::cards::BLASPHEMY_ID;
        let mut state = CombatState::initial_fixture();
        state.player.energy = 3;
        state.piles.hand = vec![CardInstance::new(CardId::new(1), BLASPHEMY_ID)];
        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: None,
            },
        )
        .expect("play blasphemy");
        assert_eq!(
            next.player.energy, 5,
            "Blasphemy spends 1 then Divinity grants 3"
        );
        assert_eq!(next.player.powers.divinity, 1);
        assert_eq!(next.player.powers.end_turn_death, 1);
        assert!(next
            .piles
            .exhaust_pile
            .iter()
            .any(|c| c.content_id == BLASPHEMY_ID));
    }

    #[test]
    fn calm_to_divinity_queues_exit_entry_then_flurry() {
        let flurry_id = CardId::new(7);
        let mut state = CombatState::initial_fixture();
        state.player.powers.calm = 1;
        state.piles.discard_pile = vec![CardInstance::new(flurry_id, FLURRY_OF_BLOWS_ANY_COLOR_ID)];

        let follow_ups = player_actions::enter_divinity(&mut state).expect("enter Divinity");

        assert_eq!(state.player.powers.calm, 0);
        assert_eq!(state.player.powers.divinity, 1);
        assert_eq!(
            follow_ups,
            vec![
                InternalAction::GainEnergy { amount: 2 },
                InternalAction::GainEnergy { amount: 3 },
                InternalAction::DiscardToHand { card_id: flurry_id },
            ]
        );
        assert!(player_actions::enter_divinity(&mut state)
            .expect("same-stance Divinity is a no-op")
            .is_empty());
    }

    #[test]
    fn dark_passive_growth_applies_focus_but_initial_evoke_does_not() {
        for (focus, expected_growth) in [(-7, 0), (0, 6), (3, 9)] {
            let mut impulse = CombatState::initial_fixture();
            impulse.player.powers.focus = focus;
            impulse.orbs = vec![crate::combat::CombatOrb::Dark { evoke: 10 }];
            player_actions::dark_impulse(&mut impulse).expect("Darkness+ impulse");
            assert_eq!(
                impulse.orbs,
                vec![crate::combat::CombatOrb::Dark {
                    evoke: 10 + expected_growth
                }]
            );

            let mut end_turn = CombatState::initial_fixture();
            end_turn.player.powers.focus = focus;
            end_turn.orbs = vec![crate::combat::CombatOrb::Dark { evoke: 10 }];
            player_actions::apply_orb_end_of_turn_passives(&mut end_turn)
                .expect("Dark end-of-turn passive");
            assert_eq!(
                end_turn.orbs,
                vec![crate::combat::CombatOrb::Dark {
                    evoke: 10 + expected_growth
                }]
            );
        }

        let mut channel = CombatState::initial_fixture();
        channel.max_orbs = 1;
        channel.player.powers.focus = 3;
        player_actions::channel_dark(&mut channel).expect("channel Dark with Focus");
        assert_eq!(
            channel.orbs,
            vec![crate::combat::CombatOrb::Dark { evoke: 6 }]
        );
    }

    #[test]
    fn void_drawn_at_zero_energy_floors_energy_at_zero() {
        // FIDL01581: Burning Pact+ confirm drew Void while energy was already 0.
        // i32::saturating_sub only clamps at i32::MIN, so energy became -1 and
        // combat validate rejected the transition.
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(CardId::new(1), VOID_ID)];
        state.piles.discard_pile.clear();
        player_draw_cards(&mut state, 1).expect("draw void at zero energy");
        assert_eq!(state.player.energy, 0);
        assert_eq!(state.piles.hand[0].content_id, VOID_ID);
        state
            .validate()
            .expect("zero energy after void remains valid");
    }

    #[test]
    fn fidl1581_burning_pact_confirm_with_void_draw_stays_valid() {
        let raw = include_str!("testdata/fidl1581_bp_confirm.json");
        let mut state: CombatState =
            serde_json::from_str(raw).expect("deserialize combat checkpoint");
        assert!(state.exhaust_select().is_some());
        confirm_exhaust_select(&mut state).expect("Burning Pact+ confirm");
        assert!(state.player.energy >= 0, "energy={}", state.player.energy);
        assert!(state.player.block >= 0, "block={}", state.player.block);
        state.validate().expect("post-confirm combat remains valid");
    }
}
