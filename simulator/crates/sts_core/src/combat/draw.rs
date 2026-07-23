use crate::{
    card::CardType,
    combat::{damage::deal_unmodified_damage_to_monster, CombatState},
    content::{
        cards::{get_card_definition, is_curse_content_id},
        monsters::{check_slime_boss_split, guardian_on_hp_damage, wake_lagavulin_on_damage},
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

fn drawable_count(state: &CombatState, count: usize) -> usize {
    count.min(MAX_HAND_SIZE.saturating_sub(state.piles.hand.len()))
}

pub fn draw_cards_with_sts_rng(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<()> {
    let mut next = state.clone();
    let mut next_rng = rng.clone();
    draw_cards_with_sts_rng_inner(&mut next, count, &mut next_rng)?;
    *state = next;
    *rng = next_rng;
    Ok(())
}

fn draw_cards_with_sts_rng_inner(
    state: &mut CombatState,
    count: usize,
    rng: &mut StsRng,
) -> SimResult<()> {
    for _ in 0..drawable_count(state, count) {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw_sts(state, rng)?;
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id)?;
            draw_cards_with_sts_rng_inner(state, evolve_extra_draw_count(state, content_id), rng)?;
        }
    }
    Ok(())
}

pub(crate) fn draw_cards_with_combat_rng(state: &mut CombatState, count: usize) -> SimResult<()> {
    let mut next = state.clone();
    draw_cards_with_combat_rng_inner(&mut next, count)?;
    *state = next;
    Ok(())
}

fn draw_cards_with_combat_rng_inner(state: &mut CombatState, count: usize) -> SimResult<()> {
    for _ in 0..drawable_count(state, count) {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw_with_combat_rng(state)?;
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = draw_card_from_pile_top(state) {
            let content_id = card.content_id;
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id)?;
            draw_cards_with_combat_rng_inner(state, evolve_extra_draw_count(state, content_id))?;
        }
    }
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
            guardian_on_hp_damage(monster, hp_damage);
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
    if state.player.powers.evolve <= 0 {
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
    if get_card_definition(card.content_id).is_none_or(|definition| definition.keywords.unplayable)
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
