use crate::{
    combat::{
        transition::{
            apply_on_exhaust_effects_without_dark_embrace, dead_branch_card_for_end_turn,
            player_draw_cards,
        },
        CombatState,
    },
    content::cards::{
        get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DECAY_ID, DOUBT_ID, REGRET_ID, SHAME_ID,
    },
    ids::CardId,
    CardInstance, SimError, SimResult,
};

pub(crate) struct EndOfTurnHandResolution {
    pub(crate) deferred_dark_embrace_draws: usize,
    pub(crate) dead_branch_cards: Vec<CardInstance>,
}

pub fn resolve_end_of_turn_hand(state: &mut CombatState) -> SimResult<()> {
    resolve_end_of_turn_hand_with_deferred_dark_embrace_draws(state).map(|_| ())
}

pub(crate) fn resolve_end_of_turn_hand_with_deferred_dark_embrace_draws(
    state: &mut CombatState,
) -> SimResult<EndOfTurnHandResolution> {
    let mut next = state.clone();
    let resolution = resolve_end_of_turn_hand_inner(&mut next)?;
    *state = next;
    Ok(resolution)
}

pub(crate) fn resolve_deferred_dark_embrace_draws(
    state: &mut CombatState,
    count: usize,
) -> SimResult<()> {
    for _ in 0..count {
        player_draw_cards(state, 1)?;
    }
    Ok(())
}

fn resolve_end_of_turn_hand_inner(state: &mut CombatState) -> SimResult<EndOfTurnHandResolution> {
    let hand_size_for_regret = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size_for_regret)?;
    exhaust_unplayed_ethereal_cards(state)
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) {
    discard_non_retain_hand(state);
}

fn apply_end_of_turn_for_playing_cards_in_hand_order(
    state: &mut CombatState,
    hand_size: i32,
) -> SimResult<()> {
    let hand = std::mem::take(&mut state.piles.hand);

    let mut hand = hand;
    let mut remaining_indices = Vec::with_capacity(hand.len());
    for index in 0..hand.len() {
        let card = hand[index];
        match card.content_id {
            BURN_ID => {
                let burn_damage = if card.upgrades > 0 {
                    BURN_END_TURN_DAMAGE * 2
                } else {
                    BURN_END_TURN_DAMAGE
                };
                let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, burn_damage);
                crate::combat::hp_loss::apply_player_card_hp_loss_hooks_with_pending_hand(
                    state, hp_loss, &mut hand,
                )?;
                state.piles.discard_pile.push(hand[index]);
            }
            DECAY_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, 2);
                crate::combat::hp_loss::apply_player_card_hp_loss_hooks_with_pending_hand(
                    state, hp_loss, &mut hand,
                )?;
                state.piles.discard_pile.push(hand[index]);
            }
            REGRET_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_hp(state, hand_size);
                crate::combat::hp_loss::apply_player_card_hp_loss_hooks_with_pending_hand(
                    state, hp_loss, &mut hand,
                )?;
                state.piles.discard_pile.push(hand[index]);
            }
            DOUBT_ID => {
                if state.relics.contains(&crate::Relic::RunicPyramid) {
                    crate::relic::apply_player_weak_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    )?;
                    remaining_indices.push(index);
                } else {
                    crate::relic::apply_player_weak_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    )?;
                    state.piles.discard_pile.push(card);
                }
            }
            SHAME_ID => {
                crate::relic::apply_player_frail_with_relics(
                    &mut state.player.powers,
                    &state.relics,
                    1,
                )?;
                state.piles.discard_pile.push(card);
            }
            _ => remaining_indices.push(index),
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

fn exhaust_unplayed_ethereal_cards(state: &mut CombatState) -> SimResult<EndOfTurnHandResolution> {
    let ethereal_ids: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect();

    let first_dead_branch_id = (!ethereal_ids.is_empty())
        .then(|| state.reserve_card_instance_ids(ethereal_ids.len()))
        .transpose()?;
    let mut deferred_dark_embrace_draws: usize = 0;
    let mut dead_branch_cards = Vec::new();
    let mut dead_branch_count = 0_u64;
    for card_id in ethereal_ids {
        if let Some(index) = state.piles.hand.iter().position(|card| card.id == card_id) {
            let card = state.piles.hand.remove(index);
            state.piles.exhaust_pile.push(card);
            apply_on_exhaust_effects_without_dark_embrace(state, card_id)?;
            let generated_id = CardId::new(
                first_dead_branch_id
                    .expect("ethereal cards reserve a Dead Branch ID range")
                    + dead_branch_count,
            );
            if let Some(card) = dead_branch_card_for_end_turn(state, generated_id)? {
                dead_branch_cards.push(card);
                dead_branch_count += 1;
            }
            deferred_dark_embrace_draws = deferred_dark_embrace_draws
                .checked_add(state.player.powers.dark_embrace.max(0) as usize)
                .ok_or(SimError::InvalidState(
                    "Dark Embrace deferred draw count overflows usize",
                ))?;
        }
    }
    Ok(EndOfTurnHandResolution {
        deferred_dark_embrace_draws,
        dead_branch_cards,
    })
}

fn discard_non_retain_hand(state: &mut CombatState) {
    if state.relics.contains(&crate::Relic::RunicPyramid) {
        return;
    }

    let mut retained = Vec::new();
    let mut discarded = Vec::new();

    for card in state.piles.hand.drain(..) {
        if get_card_definition(card.content_id).is_some_and(|definition| definition.keywords.retain)
        {
            retained.push(card);
        } else {
            discarded.push(card);
        }
    }

    state.piles.hand = retained;
    discarded.reverse();
    state.piles.discard_pile.extend(discarded);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{BLOOD_FOR_BLOOD_ID, DEFEND_R_ID},
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
