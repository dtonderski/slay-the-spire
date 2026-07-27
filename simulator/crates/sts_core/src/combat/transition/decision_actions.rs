use super::{finish_warcry_source, remove_card_from_pile};
use crate::{
    action::CardPile,
    combat::{
        CombatDecisionState, CombatState, DiscardSelectPurpose, DrawSelectPurpose,
        ExhaustSelectPurpose, HandSelectPurpose,
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
    if purpose == HandSelectPurpose::WarcryPutOnDraw
        && !state
            .piles
            .hand
            .iter()
            .any(|card| card.id != source_card_id)
    {
        finish_warcry_source(state, source_card_id)?;
        return Ok(Vec::new());
    }
    state.decision = Some(CombatDecisionState::HandSelect {
        state: crate::combat::HandSelectState {
            purpose,
            source_card_id,
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
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
    let mut selectable_card_ids = Vec::new();
    for card in &state.piles.draw_pile {
        let selectable = match purpose {
            DrawSelectPurpose::SecretTechniqueSkillToHand => {
                crate::content::cards::get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == crate::card::CardType::Skill)
            }
            DrawSelectPurpose::SecretWeaponAttackToHand => {
                crate::content::cards::get_card_definition(card.content_id)
                    .is_some_and(|definition| definition.card_type == crate::card::CardType::Attack)
            }
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
        },
    });
    Ok(Vec::new())
}

pub(super) fn await_discard_select(
    state: &mut CombatState,
    source_card_id: CardId,
    purpose: DiscardSelectPurpose,
) -> SimResult<Vec<crate::action::InternalAction>> {
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

        if state.monsters.iter().all(|monster| !monster.alive) {
            if let Some(source_card) = source_card {
                state.piles.discard_pile.push(source_card);
            }
            return Ok(Vec::new());
        }
        if state.piles.discard_pile.is_empty() {
            if let Some(source_card) = source_card {
                state.piles.discard_pile.push(source_card);
            }
            return Ok(Vec::new());
        }
        if state.piles.discard_pile.len() == 1 {
            let selected = state.piles.discard_pile.remove(0);
            state.piles.draw_pile.push(selected);
            if let Some(source_card) = source_card {
                state.piles.discard_pile.push(source_card);
            }
            return Ok(Vec::new());
        }
        state.decision = Some(CombatDecisionState::DiscardSelect {
            state: crate::combat::DiscardSelectState {
                purpose,
                source_card_id: Some(source_card_id),
                source_card,
                selected_discard_indices: Vec::new(),
                max_choices: 1,
                selected_discard_index: None,
            },
        });
        return Ok(Vec::new());
    }
    state.decision = Some(CombatDecisionState::DiscardSelect {
        state: crate::combat::DiscardSelectState {
            purpose,
            source_card_id: Some(source_card_id),
            source_card: None,
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
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
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
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
    state.decision = Some(CombatDecisionState::ExhaustSelect {
        state: crate::combat::ExhaustSelectState {
            purpose,
            source_card_id: Some(source_card_id),
            source_card,
            selected_hand_indices: Vec::new(),
            interrupted_by_cultist_potion: false,
            pending_actions: VecDeque::new(),
        },
    });
    Ok(Vec::new())
}

pub(super) fn open_discovery_card_reward(
    state: &mut CombatState,
    source_card_id: CardId,
) -> SimResult<Vec<crate::action::InternalAction>> {
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
    Ok(Vec::new())
}
