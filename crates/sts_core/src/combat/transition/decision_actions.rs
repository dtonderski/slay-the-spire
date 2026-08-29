use super::{finish_warcry_source, remove_card_from_pile};
use crate::{
    action::CardPile,
    combat::{
        draw::MAX_HAND_SIZE, CombatDecisionState, CombatState, DiscardSelectPurpose,
        DrawSelectPurpose, ExhaustSelectPurpose, HandSelectPurpose,
    },
    ids::CardId,
    SimError, SimResult,
};
use std::collections::VecDeque;

pub(super) fn await_hand_select(
    state: &mut CombatState,
    source_card_id: CardId,
    purpose: HandSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
    // Warcry queues PutOnDeckAction(amount=1, isRandom=false). Vanilla opens
    // HandCardSelectScreen only when hand.size() > amount. The played Warcry is
    // in limbo, so the decisive hand is the non-source cards the sim still holds
    // as a limbo stand-in. When size <= amount, every remaining non-source card
    // is moved via getRandomCard(cardRandomRng) without a player decision — so a
    // lone drawn card is auto-placed and END is legal immediately (no CHOOSE/
    // CONFIRM). Empty hand (failed draw) still only settles the source.
    if matches!(
        purpose,
        HandSelectPurpose::WarcryPutOnDraw | HandSelectPurpose::ThinkingAheadPutOnDraw
    ) {
        let selectable: Vec<usize> = state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| card.id != source_card_id)
            .map(|(index, _)| index)
            .collect();
        // PutOnDeckAction amount is 1 for Warcry / Warcry+.
        const PUT_ON_DECK_AMOUNT: usize = 1;
        if selectable.len() <= PUT_ON_DECK_AMOUNT {
            if !selectable.is_empty() {
                // CardGroup.getRandomCard(cardRandomRng) always draws
                // random(size-1), including the singleton size==1 case.
                let mut remaining = selectable.len();
                while remaining > 0 {
                    let candidates: Vec<usize> = state
                        .piles
                        .hand
                        .iter()
                        .enumerate()
                        .filter(|(_, card)| card.id != source_card_id)
                        .map(|(index, _)| index)
                        .collect();
                    let pick = state
                        .rng
                        .card_random_rng
                        .random_int((candidates.len() - 1) as i32)
                        as usize;
                    let index = candidates[pick];
                    let put_back = state.piles.hand[index].id;
                    let card = remove_card_from_pile(state, put_back, CardPile::Hand)?;
                    state.piles.draw_pile.push(card);
                    remaining -= 1;
                }
            }
            finish_warcry_source(state, source_card_id)?;
            return Ok(Vec::new());
        }
    }
    // Dual Wield with a single eligible attack/power: CM does not surface a
    // HAND_SELECT frame for force-played Dual Wield (source already out of hand).
    // Hand-played Dual Wield still opens the select even with one eligible card.
    let mut dual_wield_restore_on_confirm = Vec::new();
    // This field also preserves the generic PlayTop force-exhaust marker while
    // any source-delaying hand selection is open.
    let mut dual_wield_force_exhaust = state.play_top_force_exhaust_active;
    if purpose == HandSelectPurpose::DualWieldCopy {
        let source_started_in_hand = state
            .piles
            .hand
            .iter()
            .any(|card| card.id == source_card_id);
        // PlayTop stages Dual Wield into hand before await, so hand membership
        // is not a reliable force-play signal. Havoc sets play_top_force_exhaust.
        dual_wield_force_exhaust = state.play_top_force_exhaust_active || !source_started_in_hand;
        // Park the source in limbo while the select is open so combat hand
        // projection matches CommunicationMod (source already in cardInUse).
        if let Some(index) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let source = state.piles.hand.remove(index);
            state.piles.limbo.push(source);
        }
        // CommunicationMod Dual Wield hand only exposes Attack/Power candidates.
        // Force-play multi-select: skills re-enter hand on CONFIRM (9074cf38
        // Defend_R uuids), while statuses/curses leave every combat pile for the
        // rest of the fight (FIDL00242 Shame).
        let eligible_count = state
            .piles
            .hand
            .iter()
            .filter(|card| super::dual_wield_select_allows_card(card))
            .count();
        // Multi-select Dual Wield (force or hand-play after source is parked in
        // limbo) drops non-Attack/Power cards from the serialized hand. Skills
        // restore on CONFIRM (9074cf38 Defend_R); statuses stay gone (FIDL00242
        // Shame). Note: PlayTop stages Dual Wield into hand before await, so
        // source_started_in_hand is not a reliable force-play signal here.
        if eligible_count > 1 {
            let mut eligible = Vec::new();
            for card in state.piles.hand.drain(..) {
                if super::dual_wield_select_allows_card(&card) {
                    eligible.push(card);
                } else if super::dual_wield_non_eligible_restores_on_confirm(&card)
                    || card.combat_only
                {
                    // Generated combat-only statuses (for example Wild
                    // Strike's Wound) are transient hand cards, not the
                    // deck-owned Status/Curses that Dual Wield intentionally
                    // drops. Restore them with skills when CONFIRM rebuilds
                    // the filtered hand.
                    dual_wield_restore_on_confirm.push(card);
                }
                // else: deck-owned status/curse dropped from combat piles for this fight
            }
            state.piles.hand = eligible;
        }
        let eligible: Vec<usize> = state
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(_, card)| super::dual_wield_select_allows_card(card))
            .map(|(index, _)| index)
            .collect();
        if eligible.len() == 1 && dual_wield_force_exhaust {
            // Force-played Dual Wield (Havoc/Mayhem) does not open a select
            // when only one Attack/Power remains. PlayTop stages Dual Wield
            // into hand, so source_started_in_hand is not a force-play signal.
            super::confirm_dual_wield_select(
                state,
                source_card_id,
                eligible[0],
                dual_wield_restore_on_confirm,
                dual_wield_force_exhaust,
            )?;
            return Ok(Vec::new());
        }
    }
    if matches!(
        purpose,
        HandSelectPurpose::WarcryPutOnDraw
            | HandSelectPurpose::ThinkingAheadPutOnDraw
            | HandSelectPurpose::ForethoughtPutOnDraw
            | HandSelectPurpose::ForethoughtPutAnyOnDraw,
    ) {
        // UseCardAction has already moved the source to limbo / cardInUse.
        // The internal stand-in must leave hand while the screen is open so
        // queued Runic Cube/Evolve draws see the target hand capacity.
        if let Some(index) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            let source = state.piles.hand.remove(index);
            state.piles.limbo.push(source);
        }
    }
    state.decision = Some(CombatDecisionState::HandSelect {
        state: crate::combat::HandSelectState {
            purpose,
            source_card_id,
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
            dual_wield_restore_on_confirm,
            dual_wield_force_exhaust,
        },
        pending_actions: VecDeque::new(),
    });
    Ok(Vec::new())
}

pub(super) fn await_draw_select(
    state: &mut CombatState,
    source_card_id: CardId,
    purpose: DrawSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
    // STS removes Secret Technique/Weapon from hand (cardInUse) before the
    // filtered grid opens. Keeping the source in hand makes a full 10-card hand
    // overflow the retrieved skill into discard (FIDL00413).
    if let Some(source_index) = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
    {
        let source = state.piles.hand.remove(source_index);
        state.piles.limbo.push(source);
    }
    let mut selectable_card_ids = Vec::new();
    if purpose == DrawSelectPurpose::Scry {
        if let Some(card) = state.piles.draw_pile.last() {
            selectable_card_ids.push(card.id);
        }
    }
    for card in &state.piles.draw_pile {
        if purpose == DrawSelectPurpose::Scry {
            continue;
        }
        let selectable = match purpose {
            DrawSelectPurpose::SecretTechniqueSkillToHand => {
                crate::content::cards::get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == crate::card::CardType::Skill)
            }
            DrawSelectPurpose::SecretWeaponAttackToHand => {
                crate::content::cards::get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == crate::card::CardType::Attack)
            }
            DrawSelectPurpose::Scry => false,
        };
        if !selectable {
            continue;
        }
        if selectable_card_ids.is_empty() {
            selectable_card_ids.push(card.id);
        } else {
            // CardGroup::addToRandomSpot chooses an inclusive insertion slot
            // from the already-built temporary group.
            let index = state
                .rng
                .card_random_rng
                .random_int((selectable_card_ids.len() - 1) as i32)
                as usize;
            selectable_card_ids.insert(index, card.id);
        }
    }
    state.decision = Some(CombatDecisionState::DrawSelect {
        state: crate::combat::DrawSelectState {
            purpose,
            source_card_id,
            selectable_card_ids,
            selected_draw_index: None,
            pending_actions: Default::default(),
        },
    });
    Ok(Vec::new())
}

pub(super) fn await_discard_select(
    state: &mut CombatState,
    source_card_id: CardId,
    purpose: DiscardSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
    if purpose == DiscardSelectPurpose::HologramReturnToHand {
        let source_card = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
            .map(|index| state.piles.hand.remove(index))
            .ok_or(SimError::IllegalAction(
                "Hologram source card is not in hand",
            ))?;
        let force_exhaust = state.play_top_force_exhaust_active;
        if state.piles.discard_pile.is_empty() {
            super::settle_hologram_source_after_discard_select(state, source_card, force_exhaust)?;
            state.play_top_force_exhaust_active = false;
            return Ok(Vec::new());
        }
        if state.piles.discard_pile.len() == 1 {
            if state.piles.hand.len() < crate::combat::draw::MAX_HAND_SIZE {
                let selected = state.piles.discard_pile.remove(0);
                state.piles.hand.push(selected);
            }
            super::settle_hologram_source_after_discard_select(state, source_card, force_exhaust)?;
            state.play_top_force_exhaust_active = false;
            return Ok(Vec::new());
        }
        state.decision = Some(CombatDecisionState::DiscardSelect {
            state: crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card: Some(source_card),
                source_card_force_exhaust: force_exhaust,
                selected_discard_indices: Vec::new(),
                max_choices: 1,
                selected_discard_index: None,
                pending_actions: VecDeque::new(),
            },
        });
        state.play_top_force_exhaust_active = false;
        return Ok(Vec::new());
    }
    if purpose == DiscardSelectPurpose::HeadbuttPutOnDraw {
        let source_card = if let Some(index) = state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
        {
            Some(state.piles.hand.remove(index))
        } else if let Some(index) = state
            .piles
            .discard_pile
            .iter()
            .position(|card| card.id == source_card_id)
        {
            Some(state.piles.discard_pile.remove(index))
        } else if state
            .piles
            .exhaust_pile
            .iter()
            .any(|card| card.id == source_card_id)
        {
            // A Headbutt played by Havoc is already in exhaust when its
            // discard selection opens. Its identity is retained separately
            // below so confirmation can preserve the forced-play settlement.
            None
        } else {
            return Err(SimError::IllegalAction(
                "Headbutt source card is not in a playable destination",
            ));
        };

        let force_exhaust = state.play_top_force_exhaust_active;
        if state.piles.discard_pile.is_empty() {
            super::settle_headbutt_source_after_discard_select(state, source_card, force_exhaust)?;
            state.play_top_force_exhaust_active = false;
            return Ok(Vec::new());
        }
        // Headbutt's singleton auto-placement is part of the card queue even
        // when the damage just killed the final monster. Only a multi-card
        // discard suppresses the player-facing select on a lethal action.
        if state.piles.discard_pile.len() == 1 {
            let selected = state.piles.discard_pile.remove(0);
            state.piles.draw_pile.push(selected);
            super::settle_headbutt_source_after_discard_select(state, source_card, force_exhaust)?;
            state.play_top_force_exhaust_active = false;
            return Ok(Vec::new());
        }
        if state.monsters.iter().all(|monster| !monster.alive) {
            super::settle_headbutt_source_after_discard_select(state, source_card, force_exhaust)?;
            state.play_top_force_exhaust_active = false;
            return Ok(Vec::new());
        }
        state.decision = Some(CombatDecisionState::DiscardSelect {
            state: crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card,
                source_card_force_exhaust: force_exhaust,
                selected_discard_indices: Vec::new(),
                max_choices: 1,
                selected_discard_index: None,
                pending_actions: VecDeque::new(),
            },
        });
        state.play_top_force_exhaust_active = false;
        return Ok(Vec::new());
    }
    state.decision = Some(CombatDecisionState::DiscardSelect {
        state: crate::combat::DiscardSelectState {
            purpose,
            source_card_id: Some(source_card_id),
            source_card: None,
            source_card_force_exhaust: false,
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(Vec::new())
}

pub(super) fn await_copied_discard_select(
    state: &mut CombatState,
    purpose: DiscardSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
    if purpose != DiscardSelectPurpose::HeadbuttPutOnDraw {
        return Err(SimError::IllegalAction(
            "copied discard select purpose is unsupported",
        ));
    }
    if state.monsters.iter().all(|monster| !monster.alive) || state.piles.discard_pile.is_empty() {
        return Ok(Vec::new());
    }
    if state.piles.discard_pile.len() == 1 {
        let card = state.piles.discard_pile.remove(0);
        state.piles.draw_pile.push(card);
        return Ok(Vec::new());
    }
    state.decision = Some(CombatDecisionState::DiscardSelect {
        state: crate::combat::DiscardSelectState {
            purpose,
            source_card_id: None,
            source_card: None,
            source_card_force_exhaust: false,
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(Vec::new())
}

pub(super) fn await_exhaust_select(
    state: &mut CombatState,
    source_card_id: CardId,
    purpose: ExhaustSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
    let source_card = if matches!(
        purpose,
        ExhaustSelectPurpose::BurningPactDraw2
            | ExhaustSelectPurpose::BurningPactDraw3
            | ExhaustSelectPurpose::ExhumeReturnToHand
            | ExhaustSelectPurpose::TrueGritExhaustOne
            | ExhaustSelectPurpose::RecycleExhaustOne,
    ) {
        if state
            .piles
            .hand
            .iter()
            .any(|card| card.id == source_card_id)
        {
            Some(remove_card_from_pile(
                state,
                source_card_id,
                CardPile::Hand,
            )?)
        } else if state
            .piles
            .discard_pile
            .iter()
            .chain(state.piles.exhaust_pile.iter())
            .any(|card| card.id == source_card_id)
        {
            None
        } else {
            return Err(SimError::UnknownCard(source_card_id));
        }
    } else if purpose == ExhaustSelectPurpose::PurityExhaustUpTo3 {
        state
            .piles
            .hand
            .iter()
            .position(|card| card.id == source_card_id)
            .map(|index| state.piles.hand.remove(index))
    } else {
        None
    };
    let exhaust_select = crate::combat::ExhaustSelectState {
        purpose,
        source_card_id: Some(source_card_id),
        source_card,
        source_card_force_exhaust: state.play_top_force_exhaust_active,
        selected_hand_indices: Vec::new(),
        interrupted_by_cultist_potion: false,
        pending_actions: VecDeque::new(),
    };
    // ExhumeAction checks the live hand size when the queued action runs, not
    // when Exhume.use() is queued. Start-of-turn draws (Offering plus Dark
    // Embrace behind Mayhem, for example) can fill the hand in between. A full
    // hand makes ExhumeAction finish without opening GRID; UseCardAction still
    // exhausts the source and runs its on-exhaust callbacks.
    if purpose == ExhaustSelectPurpose::ExhumeReturnToHand
        && state.piles.hand.len() >= MAX_HAND_SIZE
    {
        super::settle_exhume_source_after_selection(state, exhaust_select, source_card_id)?;
        return Ok(Vec::new());
    }
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: exhaust_select,
    });
    Ok(Vec::new())
}

pub(super) fn open_discovery_card_reward(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<Vec<crate::action::InternalAction>> {
    if !matches!(
        state.decision,
        Some(CombatDecisionState::DiscoveryCardReward { .. })
    ) {
        crate::combat::card_effects::open_discovery_card_reward_for_play(state, source_card_id)?;
    }
    let source_card = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
        .map(|index| state.piles.hand.remove(index));
    let Some(CombatDecisionState::DiscoveryCardReward {
        source_card: decision_source,
        ..
    }) = state.decision.as_mut()
    else {
        return Err(SimError::InvalidState(
            "Discovery source opened without its card reward",
        ));
    };
    *decision_source = source_card;
    if let Some(CombatDecisionState::DiscoveryCardReward {
        source_card_force_exhaust,
        source_card_play_top,
        ..
    }) = state.decision.as_mut()
    {
        *source_card_force_exhaust = state.play_top_force_exhaust_active;
        *source_card_play_top = *source_card_play_top || state.play_top_resolving_depth > 0;
    }
    Ok(Vec::new())
}
