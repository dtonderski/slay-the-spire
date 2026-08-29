use crate::{
    action::InternalAction,
    combat::{
        transition::{apply_on_exhaust_effects_for_end_turn, dead_branch_card_for_end_turn},
        CombatState,
    },
    content::cards::{
        get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DECAY_ID, DOUBT_ID, REGRET_ID, SHAME_ID,
    },
    ids::CardId,
    CardInstance, SimError, SimResult,
};

#[derive(Clone)]
pub(crate) enum EtherealEndTurnFollowUp {
    DeadBranch(CardInstance),
    DarkEmbraceDraw,
}

pub(crate) struct EndOfTurnHandResolution {
    /// The visible hand existed at END click but was completely consumed by
    /// auto-play curses before DiscardAtEndOfTurnAction.
    pub(crate) auto_play_emptied_hand: bool,
    /// Per-ethereal addToBot order: Dead Branch then Dark Embrace (FIDL02353).
    pub(crate) ethereal_follow_ups: Vec<EtherealEndTurnFollowUp>,
    pub(crate) deferred_juggernaut_damage: Vec<i32>,
}

pub fn resolve_end_of_turn_hand(state: &mut CombatState) -> SimResult<()> {
    resolve_end_of_turn_hand_with_deferred_dark_embrace_draws(state).map(|_| ())
}

pub(crate) fn resolve_end_of_turn_hand_with_deferred_dark_embrace_draws(
    state: &mut CombatState,
) -> SimResult<EndOfTurnHandResolution> {
    resolve_end_of_turn_hand_with_queued_autoplay(state, None)
}

/// `queued_autoplay` is the card-instance set `callEndOfTurnActions` queued
/// from the hand at END click. Burns drawn later (Combust / Runic Cube) stay
/// in hand and are discarded without playing (FIDL01762 step 1393).
pub(crate) fn resolve_end_of_turn_hand_with_queued_autoplay(
    state: &mut CombatState,
    queued_autoplay: Option<&std::collections::HashSet<CardId>>,
) -> SimResult<EndOfTurnHandResolution> {
    let mut next = state.clone();
    let resolution = resolve_end_of_turn_hand_inner(&mut next, queued_autoplay)?;
    *state = next;
    Ok(resolution)
}

pub(crate) fn draw_dark_embrace_with_follow_ups_deferred(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    use crate::combat::transition::player_draw_cards_with_deferred_evolve;
    let mut follow_ups = Vec::new();
    for _ in 0..count {
        follow_ups.extend(player_draw_cards_with_deferred_evolve(state, 1)?);
    }
    Ok(follow_ups)
}

fn resolve_end_of_turn_hand_inner(
    state: &mut CombatState,
    queued_autoplay: Option<&std::collections::HashSet<CardId>>,
) -> SimResult<EndOfTurnHandResolution> {
    let auto_play_emptied_hand = resolve_end_of_turn_autoplay_in_place(state, queued_autoplay)?;
    let mut resolution = exhaust_unplayed_ethereal_cards(state)?;
    resolution.auto_play_emptied_hand = auto_play_emptied_hand;
    Ok(resolution)
}

/// Play Burn/Decay/Regret/Doubt/Shame queued by `callEndOfTurnActions`.
/// Constricted and ethereal exhaust belong to later `AbstractRoom.endTurn`
/// actions and must not run here.
pub(crate) fn resolve_end_of_turn_autoplay_with_queued(
    state: &mut CombatState,
    queued_autoplay: Option<&std::collections::HashSet<CardId>>,
) -> SimResult<bool> {
    let mut next = state.clone();
    let auto_play_emptied_hand = resolve_end_of_turn_autoplay_in_place(&mut next, queued_autoplay)?;
    *state = next;
    Ok(auto_play_emptied_hand)
}

fn resolve_end_of_turn_autoplay_in_place(
    state: &mut CombatState,
    queued_autoplay: Option<&std::collections::HashSet<CardId>>,
) -> SimResult<bool> {
    let hand_was_nonempty = !state.piles.hand.is_empty();
    let hand_size_for_regret = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(
        state,
        hand_size_for_regret,
        queued_autoplay,
        false,
    )?;
    Ok(hand_was_nonempty && state.piles.hand.is_empty())
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) -> SimResult<()> {
    discard_non_retain_hand(state)
}

/// Resolve only the end-turn cards that auto-play before the bulk hand discard.
/// CommunicationMod can observe this queue boundary when Time Warp ends a turn
/// while a hand-selection action is closing.
pub fn resolve_end_of_turn_playing_cards_for_time_warp_lag(
    state: &mut CombatState,
) -> SimResult<()> {
    let hand_size = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size, None, false)
}

fn apply_end_of_turn_for_playing_cards_in_hand_order(
    state: &mut CombatState,
    hand_size: i32,
    queued_autoplay: Option<&std::collections::HashSet<CardId>>,
    skip_curses: bool,
) -> SimResult<()> {
    let hand = std::mem::take(&mut state.piles.hand);

    let mut hand = hand;
    let mut remaining_indices = Vec::with_capacity(hand.len());
    for index in 0..hand.len() {
        let card = hand[index];
        let previous_card_in_use = state.card_in_use;
        let queued = queued_autoplay.is_none_or(|ids| ids.contains(&card.id));
        let is_end_turn_curse = matches!(card.content_id, REGRET_ID | DOUBT_ID | SHAME_ID);
        let auto_played = queued
            && matches!(
                card.content_id,
                BURN_ID | DECAY_ID | REGRET_ID | DOUBT_ID | SHAME_ID
            )
            && !(skip_curses && is_end_turn_curse);
        if auto_played {
            // End-turn curses/statuses are queued as cards. AbstractPlayer moves
            // the queued card out of hand into cardInUse before its card action;
            // UseCardAction returns it to discard only after that action settles.
            state.card_in_use = Some(card.id);
        }

        let mut settled_before_draw_follow_ups = false;
        let moves_to_discard = if !auto_played {
            remaining_indices.push(index);
            false
        } else {
            match card.content_id {
                BURN_ID => {
                    let burn_damage = if card.upgrades > 0 {
                        BURN_END_TURN_DAMAGE * 2
                    } else {
                        BURN_END_TURN_DAMAGE
                    };
                    let hp_loss =
                        crate::combat::hp_loss::lose_player_blockable_hp(state, burn_damage);
                    settled_before_draw_follow_ups =
                        apply_end_turn_card_hp_loss_hooks_with_live_hand(
                            state,
                            hp_loss,
                            card,
                            &mut hand,
                            &remaining_indices,
                            index,
                        )?;
                    true
                }
                DECAY_ID => {
                    let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, 2);
                    settled_before_draw_follow_ups =
                        apply_end_turn_card_hp_loss_hooks_with_live_hand(
                            state,
                            hp_loss,
                            card,
                            &mut hand,
                            &remaining_indices,
                            index,
                        )?;
                    true
                }
                REGRET_ID => {
                    let hp_loss = crate::combat::hp_loss::lose_player_hp(state, hand_size);
                    settled_before_draw_follow_ups =
                        apply_end_turn_card_hp_loss_hooks_with_live_hand(
                            state,
                            hp_loss,
                            card,
                            &mut hand,
                            &remaining_indices,
                            index,
                        )?;
                    true
                }
                // Doubt is auto-played via CardQueueItem at end of turn (see
                // Doubt.triggerOnEndOfTurnForPlayingCard). That removes it from hand
                // before DiscardAtEndOfTurnAction. Runic Pyramid only skips the bulk
                // hand discard — it does not keep auto-played curses (FIDL00288).
                DOUBT_ID => {
                    let had_no_weak = state.player.powers.weak == 0;
                    crate::relic::apply_player_weak_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    )?;
                    if had_no_weak && state.player.powers.weak > 0 {
                        state.player.weak_just_applied = true;
                    }
                    true
                }
                SHAME_ID => {
                    let had_no_frail = state.player.powers.frail == 0;
                    crate::relic::apply_player_frail_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    )?;
                    if had_no_frail && state.player.powers.frail > 0 {
                        state.player.frail_just_applied = true;
                    }
                    true
                }
                _ => {
                    remaining_indices.push(index);
                    false
                }
            }
        };

        if !moves_to_discard {
            continue;
        }

        if state.player.hp <= 0 {
            // LoseHPAction is ahead of the queued UseCardAction. A lethal
            // end-turn card therefore remains cardInUse while the queued
            // DiscardAtEndOfTurnAction is cancelled, leaving later hand cards
            // untouched just as the source action queue does.
            let mut remaining = remaining_indices
                .iter()
                .map(|remaining_index| hand[*remaining_index])
                .collect::<Vec<_>>();
            remaining.extend(hand.iter().skip(index + 1).copied());
            remaining.extend(std::mem::take(&mut state.piles.hand));
            state.piles.hand = remaining;
            return Ok(());
        }

        state.card_in_use = previous_card_in_use;
        if !settled_before_draw_follow_ups {
            state.piles.discard_pile.push(card);
        }
    }
    // End-turn card actions can trigger relic draws before the hand cleanup
    // action finishes. Those cards are already in the authoritative hand and
    // must be discarded by the following DiscardAtEndOfTurnAction; do not lose
    // them when replacing the temporary list of cards being processed.
    let drawn_during_cleanup = std::mem::take(&mut state.piles.hand);
    let remaining = remaining_indices
        .into_iter()
        .map(|index| hand[index])
        .collect::<Vec<_>>();
    state.piles.hand = remaining;
    state.piles.hand.extend(drawn_during_cleanup);
    Ok(())
}

fn apply_end_turn_card_hp_loss_hooks_with_live_hand(
    state: &mut CombatState,
    hp_loss: i32,
    settled_card: CardInstance,
    pending_hand: &mut [CardInstance],
    retained_indices: &[usize],
    current_index: usize,
) -> SimResult<bool> {
    // CardQueue removes only the current Burn/Decay/Regret. Every other card
    // remains in the real hand while Runic Cube/Centennial Puzzle draws, so the
    // ten-card cap must include both earlier retained cards and later queued
    // cards. The simulator parks that hand slice while iterating; restore it
    // around the HP-loss hook and keep only newly drawn cards in state storage.
    let mut live_indices = retained_indices.to_vec();
    live_indices.extend((current_index + 1)..pending_hand.len());
    let mut index_by_id = std::collections::HashMap::new();
    let drawn_before = std::mem::take(&mut state.piles.hand);
    for index in &live_indices {
        let card = pending_hand[*index];
        index_by_id.insert(card.id, *index);
        state.piles.hand.push(card);
    }
    state.piles.hand.extend(drawn_before);

    let prior_follow_up_count = state.pending_hp_loss_draw_follow_ups.len();
    crate::combat::hp_loss::apply_player_card_hp_loss_hooks_queued_follow_ups(state, hp_loss)?;
    let settled = state.player.hp > 0;
    if settled {
        state.piles.discard_pile.push(settled_card);
        let follow_ups = state
            .pending_hp_loss_draw_follow_ups
            .split_off(prior_follow_up_count);
        crate::combat::transition::resolve_deferred_draw_follow_ups(
            state,
            follow_ups.into_iter().collect(),
        )?;
    } else {
        // The death screen freezes actions queued by the lethal HP-loss event.
        state
            .pending_hp_loss_draw_follow_ups
            .truncate(prior_follow_up_count);
    }

    let combined = std::mem::take(&mut state.piles.hand);
    for card in combined {
        if let Some(index) = index_by_id.get(&card.id).copied() {
            pending_hand[index] = card;
        } else {
            state.piles.hand.push(card);
        }
    }
    Ok(settled)
}

/// The Bomb can end combat while all status/curse CardQueueItems from
/// `callEndOfTurnActions` still belong to the same turn queue.
pub(crate) fn apply_end_of_turn_autoplay_for_bomb_victory(
    state: &mut CombatState,
) -> SimResult<()> {
    let hand_size = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size, None, false)
}

/// Stone Calendar's lethal relic action clears queued Burn status plays, but
/// already-triggered end-turn curses still resolve before victory publication
/// (FIDL03064 Burns canceled; FIDL03098 Decay then Burning Blood).
pub(crate) fn apply_end_of_turn_curses_for_calendar_victory(
    state: &mut CombatState,
) -> SimResult<()> {
    let queued_curses = state
        .piles
        .hand
        .iter()
        .filter(|card| matches!(card.content_id, DECAY_ID | REGRET_ID | DOUBT_ID | SHAME_ID))
        .map(|card| card.id)
        .collect::<std::collections::HashSet<_>>();
    let hand_size = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size, Some(&queued_curses), false)
}

pub(crate) fn exhaust_unplayed_ethereal_cards(
    state: &mut CombatState,
) -> SimResult<EndOfTurnHandResolution> {
    let mut ethereal_ids: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect();
    // AbstractCard.triggerOnEndOfTurnForPlayingCard addToTop's
    // ExhaustSpecificCardAction per ethereal in hand order, so the last
    // ethereal exhausts first. Dead Branch onExhaust addToBot follows that
    // exhaust order (FIDL02353 two Voids).
    ethereal_ids.reverse();

    let first_dead_branch_id = (!ethereal_ids.is_empty())
        .then(|| state.reserve_card_instance_ids(ethereal_ids.len()))
        .transpose()?;
    let mut deferred_juggernaut_damage = Vec::new();
    let mut ethereal_follow_ups = Vec::new();
    let mut dead_branch_count = 0_u64;
    for card_id in ethereal_ids {
        if let Some(index) = state.piles.hand.iter().position(|card| card.id == card_id) {
            let card = state.piles.hand.remove(index);
            state.piles.exhaust_pile.push(card);
            if let Some(amount) = apply_on_exhaust_effects_for_end_turn(state, card_id)? {
                deferred_juggernaut_damage.push(amount);
            }
            let generated_id = CardId::new(
                first_dead_branch_id.expect("ethereal cards reserve a Dead Branch ID range")
                    + dead_branch_count,
            );
            if let Some(card) = dead_branch_card_for_end_turn(state, generated_id)? {
                ethereal_follow_ups.push(EtherealEndTurnFollowUp::DeadBranch(card));
                dead_branch_count += 1;
            }
            let embrace = state.player.powers.dark_embrace.max(0) as usize;
            for _ in 0..embrace {
                ethereal_follow_ups.push(EtherealEndTurnFollowUp::DarkEmbraceDraw);
            }
        }
    }
    Ok(EndOfTurnHandResolution {
        auto_play_emptied_hand: false,
        ethereal_follow_ups,
        deferred_juggernaut_damage,
    })
}

fn discard_non_retain_hand(state: &mut CombatState) -> SimResult<()> {
    if state.relics.contains(&crate::Relic::RunicPyramid) {
        apply_on_retained_card_effects(state)?;
        apply_sands_of_time_end_of_turn_cost(state)?;
        return Ok(());
    }
    let retain_hand = std::mem::take(&mut state.player.retain_hand_next_turn);

    let mut retained = Vec::new();
    let mut discarded = Vec::new();

    for card in state.piles.hand.drain(..) {
        if retain_hand
            || get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.retain)
        {
            retained.push(card);
        } else {
            discarded.push(card);
        }
    }

    state.piles.hand = retained;
    discarded.reverse();
    state.piles.discard_pile.extend(discarded);
    apply_on_retained_card_effects(state)?;
    apply_sands_of_time_end_of_turn_cost(state)?;
    Ok(())
}

fn apply_on_retained_card_effects(state: &mut CombatState) -> SimResult<()> {
    use crate::content::cards::WINDMILL_STRIKE_ANY_COLOR_ID;
    for card in &mut state.piles.hand {
        if card.content_id != WINDMILL_STRIKE_ANY_COLOR_ID {
            continue;
        }
        // WindmillStrike.onRetained: upgradeDamage(magicNumber).
        // magicNumber is 4, or 5 after upgradeMagicNumber(1).
        let bonus = if card.upgrades > 0 { 5 } else { 4 };
        card.windmill_retain_damage =
            card.windmill_retain_damage
                .checked_add(bonus)
                .ok_or(SimError::InvalidState(
                    "Windmill Strike retain damage overflows i32",
                ))?;
    }
    Ok(())
}

fn apply_sands_of_time_end_of_turn_cost(state: &mut CombatState) -> SimResult<()> {
    use crate::content::cards::{SANDS_OF_TIME_ID, SANDS_OF_TIME_PLUS_ID};
    for card in &mut state.piles.hand {
        if !matches!(card.content_id, SANDS_OF_TIME_ID | SANDS_OF_TIME_PLUS_ID) {
            continue;
        }
        if card.temp_cost_turn_only {
            crate::combat::cost::reduce_card_cost_for_combat(card, 1)?;
        } else {
            let current = card.temp_cost.map_or_else(
                || {
                    get_card_definition(card.content_id)
                        .map(|definition| i32::from(definition.cost))
                        .unwrap_or(4)
                },
                i32::from,
            );
            card.temp_cost = Some(current.saturating_sub(1).clamp(0, 255) as u8);
            card.combat_cost_under_turn_override = None;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{
            BLOOD_FOR_BLOOD_ID, DEFEND_R_ID, STRIKE_R_ID, WINDMILL_STRIKE_ANY_COLOR_ID, WOUND_ID,
        },
        ids::CardId,
        CardInstance,
    };

    #[test]
    fn end_turn_trigger_cards_discard_in_hand_order() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
            CardInstance::new(CardId::new(3), BURN_ID),
            CardInstance::new(CardId::new(4), DOUBT_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![REGRET_ID, BURN_ID, DOUBT_ID]
        );
        assert_eq!(state.player.powers.weak, 1);
    }

    #[test]
    fn end_turn_doubt_leaves_hand_even_with_runic_pyramid() {
        // Doubt is card-queue auto-played at EOT; Pyramid only skips bulk discard.
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::RunicPyramid];
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(2), DOUBT_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        assert_eq!(state.player.powers.weak, 1);
        assert!(state
            .piles
            .hand
            .iter()
            .all(|card| card.content_id != DOUBT_ID));
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DOUBT_ID]
        );
        assert!(state
            .piles
            .hand
            .iter()
            .any(|card| card.content_id == DEFEND_R_ID));
    }

    #[test]
    fn end_turn_doubt_and_shame_preserve_hand_order_in_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), SHAME_ID),
            CardInstance::new(CardId::new(2), DOUBT_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![SHAME_ID, DOUBT_ID]
        );
        assert_eq!(state.player.powers.weak, 1);
        assert_eq!(state.player.powers.frail, 1);
    }

    #[test]
    fn lethal_end_turn_card_stays_in_use_before_discard_cleanup() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 1;
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
        ];
        state.piles.discard_pile.clear();

        let next = crate::combat::end_player_turn(&state).expect("end turn resolves");

        assert_eq!(next.phase, crate::combat::CombatPhase::Lost);
        assert_eq!(next.card_in_use, Some(CardId::new(2)));
        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, DEFEND_R_ID);
        assert!(next.piles.discard_pile.is_empty());
    }

    #[test]
    fn end_turn_card_damage_triggers_rupture() {
        for content_id in [BURN_ID, DECAY_ID, REGRET_ID] {
            let mut state = CombatState::initial_fixture();
            state.player.powers.rupture = 2;
            state.piles.hand = vec![CardInstance::new(CardId::new(1), content_id)];
            state.piles.discard_pile.clear();

            resolve_end_of_turn_hand(&mut state).expect("end-turn damage card resolves");

            assert_eq!(state.player.powers.strength, 2, "{content_id:?}");
        }
    }

    #[test]
    fn end_turn_burn_cube_draw_counts_the_other_nine_hand_cards() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::RunicCube];
        state.player.powers.evolve = 2;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), STRIKE_R_ID),
            CardInstance::new(CardId::new(21), WOUND_ID),
        ];
        state.piles.discard_pile.clear();
        let burn = CardInstance::new(CardId::new(1), BURN_ID);
        let mut pending_hand = vec![burn];
        pending_hand.extend((2..=10).map(|id| CardInstance::new(CardId::new(id), DEFEND_R_ID)));

        let hp_loss =
            crate::combat::hp_loss::lose_player_blockable_hp(&mut state, BURN_END_TURN_DAMAGE);
        let settled = apply_end_turn_card_hp_loss_hooks_with_live_hand(
            &mut state,
            hp_loss,
            burn,
            &mut pending_hand,
            &[],
            0,
        )
        .expect("Burn HP-loss queue settles");

        assert!(settled);
        assert_eq!(state.piles.hand.len(), 1);
        assert_eq!(state.piles.hand[0].content_id, WOUND_ID);
        assert_eq!(
            state.piles.draw_pile.len(),
            1,
            "Evolve draw sees a full live hand"
        );
        assert_eq!(state.piles.discard_pile, vec![burn]);
        assert!(state.pending_hp_loss_draw_follow_ups.is_empty());
    }

    #[test]
    fn windmill_retain_damage_overflow_is_rejected() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            WINDMILL_STRIKE_ANY_COLOR_ID,
        )];
        state.piles.hand[0].windmill_retain_damage = i32::MAX;

        assert_eq!(
            apply_on_retained_card_effects(&mut state),
            Err(SimError::InvalidState(
                "Windmill Strike retain damage overflows i32"
            ))
        );
    }

    #[test]
    fn end_turn_regret_updates_blood_for_blood_still_in_hand() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), BLOOD_FOR_BLOOD_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        let blood_for_blood = state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == BLOOD_FOR_BLOOD_ID)
            .expect("Blood for Blood remains in hand until discard cleanup");
        assert_eq!(blood_for_blood.blood_for_blood_cost_reduction, 1);
        assert_eq!(state.player.damage_events_this_combat, 1);
    }
}
