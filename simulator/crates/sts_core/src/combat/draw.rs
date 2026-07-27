use crate::{
    card::CardType,
    combat::{damage::deal_unmodified_damage_to_monster, CombatState},
    content::{
        cards::{get_card_definition, is_curse_content_id, VOID_ID},
        monsters::{check_slime_boss_split, wake_lagavulin_on_damage},
    },
    ids::{ContentId, MonsterId},
    rng::{JavaRng, StsRng},
    CardInstance, Relic, SimResult,
};

/// CommunicationMod lists draw piles bottom-first; the game draws from the top (last entry).
fn draw_card_from_pile_top(state: &mut CombatState) -> Option<CardInstance> {
    state.piles.draw_pile.pop()
}

pub(crate) const MAX_HAND_SIZE: usize = 10;

pub fn draw_cards_with_sts_rng(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<()> {
    let mut next = state.clone();
    let mut next_rng = rng.clone();
    let mut pending = std::collections::VecDeque::from(draw_cards_with_sts_rng_batch_deferred(
        &mut next,
        count,
        &mut next_rng,
    )?);
    while let Some(extra) = pending.pop_front() {
        pending.extend(draw_cards_with_sts_rng_batch_deferred(
            &mut next,
            extra,
            &mut next_rng,
        )?);
    }
    *state = next;
    *rng = next_rng;
    Ok(())
}

fn draw_cards_with_sts_rng_batch_deferred(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<Vec<usize>> {
    let mut deferred_evolve_draws = Vec::new();
    for _ in 0..count {
        if state.piles.hand.len() >= MAX_HAND_SIZE {
            break;
        }
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw_sts(state, rng)?;
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            let extra_draws = evolve_extra_draw_count(state, content_id);
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            if content_id == VOID_ID {
                state.player.energy = state.player.energy.saturating_sub(1);
            }
            apply_fire_breathing_on_draw(state, content_id)?;
            if extra_draws > 0 {
                deferred_evolve_draws.push(extra_draws);
            }
        }
    }
    Ok(deferred_evolve_draws)
}

/// Draw `count` cards, matching one target-game `DrawCardAction`.
///
/// `EvolvePower.onCardDraw` uses `addToBot(new DrawCardAction(...))`, so extra
/// draws from statuses are queued after the current draw action finishes—not
/// interleaved between the remaining cards of this batch. Nested status draws
/// from those follow-up actions are processed FIFO in the same way.
pub fn draw_cards_with_combat_rng(state: &mut CombatState, count: usize) -> SimResult<()> {
    let mut next = state.clone();
    let mut pending = std::collections::VecDeque::from(draw_cards_batch_deferred_evolve_in_place(
        &mut next, count,
    )?);
    while let Some(extra) = pending.pop_front() {
        pending.extend(draw_cards_batch_deferred_evolve_in_place(&mut next, extra)?);
    }
    *state = next;
    Ok(())
}

/// Draw `count` cards without resolving Evolve follow-ups; return each
/// Evolve-triggered follow-up draw count (one entry per status drawn) so the
/// action queue can append them after other already-queued actions.
pub(crate) fn draw_cards_with_combat_rng_deferred_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<usize>> {
    let mut next = state.clone();
    let deferred_evolve_draws = draw_cards_batch_deferred_evolve_in_place(&mut next, count)?;
    *state = next;
    Ok(deferred_evolve_draws)
}

pub(crate) fn draw_cards_with_combat_rng_without_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<()> {
    let mut next = state.clone();
    draw_cards_batch_in_place(&mut next, count, false)?;
    *state = next;
    Ok(())
}

fn draw_cards_batch_deferred_evolve_in_place(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<usize>> {
    draw_cards_batch_in_place(state, count, true)
}

fn draw_cards_batch_in_place(
    state: &mut CombatState,
    count: usize,
    trigger_evolve: bool,
) -> SimResult<Vec<usize>> {
    let mut deferred_evolve_draws = Vec::new();
    let had_cards_at_start =
        !state.piles.draw_pile.is_empty() || !state.piles.discard_pile.is_empty();
    for _ in 0..count {
        if state.piles.hand.len() >= MAX_HAND_SIZE {
            break;
        }
        if state.piles.draw_pile.is_empty() {
            if state.piles.discard_pile.is_empty() {
                if had_cards_at_start {
                    consume_empty_deck_shuffle_with_combat_rng(state)?;
                }
            } else {
                shuffle_discard_into_draw_with_combat_rng(state)?;
            }
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            let extra_draws = if trigger_evolve {
                evolve_extra_draw_count(state, content_id)
            } else {
                0
            };
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            if content_id == VOID_ID {
                state.player.energy = state.player.energy.saturating_sub(1);
            }
            apply_fire_breathing_on_draw(state, content_id)?;
            if extra_draws > 0 {
                deferred_evolve_draws.push(extra_draws);
            }
        }
    }
    Ok(deferred_evolve_draws)
}

pub(crate) fn consume_empty_deck_shuffle_with_combat_rng(state: &mut CombatState) -> SimResult<()> {
    let _ = state.rng.shuffle_rng.random_long();
    Ok(())
}

pub(crate) fn apply_fire_breathing_on_draw(
    state: &mut CombatState,
    content_id: crate::ContentId,
) -> SimResult<()> {
    let amount = state.player.powers.fire_breathing;
    if amount <= 0 || !is_status_or_curse(content_id) {
        return Ok(());
    }

    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<MonsterId>>();

    for target in targets {
        let still_alive = {
            let Some(monster) = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == target && monster.alive)
            else {
                continue;
            };
            let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
            wake_lagavulin_on_damage(monster, hp_damage);
            monster.alive
        };
        check_slime_boss_split(state, target);
        if !still_alive {
            crate::combat::transition::apply_monster_death_hooks(state, target)?;
        }
    }
    Ok(())
}

pub(crate) fn evolve_extra_draw_count(state: &CombatState, content_id: ContentId) -> usize {
    // EvolvePower.onCardDraw skips when the player has No Draw.
    if state.player.powers.evolve <= 0 || state.player.cannot_draw {
        return 0;
    }
    if get_card_definition(content_id).is_some_and(|definition| {
        definition.card_type == CardType::Status && !is_curse_content_id(content_id)
    }) {
        state.player.powers.evolve as usize
    } else {
        0
    }
}

fn is_status_or_curse(content_id: crate::ContentId) -> bool {
    is_curse_content_id(content_id)
        || get_card_definition(content_id)
            .is_some_and(|definition| definition.card_type == CardType::Status)
}

pub(crate) fn apply_confusion_cost_randomization(state: &mut CombatState, card: &mut CardInstance) {
    if !state.relics.contains(&Relic::SneckoEye) && state.player.powers.confusion <= 0 {
        return;
    }
    if get_card_definition(card.content_id)
        .is_none_or(|definition| definition.keywords.unplayable || definition.cost < 0)
    {
        return;
    }
    let rng = &mut state.rng.card_random_rng;
    card.temp_cost = Some(rng.random_int(3) as u8);
}

pub(crate) fn shuffle_discard_into_draw_sts(
    state: &mut CombatState,
    rng: &mut StsRng,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let shuffle_seed = rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}

pub(crate) fn shuffle_discard_into_draw_with_combat_rng(state: &mut CombatState) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}

pub(crate) fn deep_breath_shuffle_discard_into_draw_with_combat_rng(
    state: &mut CombatState,
) -> SimResult<()> {
    if state.piles.discard_pile.is_empty() {
        return Ok(());
    }

    let discard_shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(discard_shuffle_seed).collections_shuffle(&mut state.piles.discard_pile);
    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    let draw_shuffle_seed = state.rng.shuffle_rng.random_long();
    JavaRng::new(draw_shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
    crate::relic::apply_shuffle_relics(state)
}
